//! `CheckRunner`: orchestrates the sync + filter + analyze pipeline across every
//! enabled account (Task T12, spec §5). See `docs/core/pipeline.md` for the full
//! behavior.
//!
//! Split by responsibility:
//! - `mod.rs` (this file): `CheckRunner`/`PreparedRun`/`Repositories`, the public
//!   API, and the run-level orchestration (`prepare`, `execute`).
//! - [`account`]: per-account processing — connect, fetch each folder, run every
//!   message, report the account's outcome.
//! - [`message`]: the per-message decision table (spec §5.1): dedup, filters,
//!   date extraction, AI analysis, persistence.
//! - [`lock`]: the single-run lock (`prepare`/`execute` take and release it via
//!   RAII, so it is always released — including on every internal error path).
//!
//! `pipeline` is pure orchestration: no `tokio::spawn`, no logging, no I/O beyond
//! the ports it is handed. `execute` is a plain `async fn` that the Tauri layer
//! spawns; nothing here assumes a particular async runtime.

mod account;
mod lock;
mod message;

use std::sync::Arc;

use crate::domain::{
    Account, Catalog, Counters, ErrorKind, RunAccountResult, RunAccountStatus, RunError, RunId,
    RunStatus, RunTrigger, Settings,
};
use crate::ports::{
    AccountRepository, CatalogRepository, Clock, EmailProviderFactory, ModelProvider,
    ProcessedMessageRepository, RepoError, ResultRepository, RunObserver, RunRepository,
    SettingsRepository, StructuredModel, SyncStateRepository,
};

use lock::{RunLock, RunLockGuard};

/// Every repository port the pipeline depends on, bundled for [`CheckRunner::new`].
pub struct Repositories {
    pub accounts: Arc<dyn AccountRepository>,
    pub catalog: Arc<dyn CatalogRepository>,
    pub sync: Arc<dyn SyncStateRepository>,
    pub processed: Arc<dyn ProcessedMessageRepository>,
    pub results: Arc<dyn ResultRepository>,
    pub runs: Arc<dyn RunRepository>,
    pub settings: Arc<dyn SettingsRepository>,
}

/// Orchestrates check runs: validates preconditions and creates the run row
/// (`prepare`), then executes every enabled account to completion (`execute`).
/// Holds the single-run lock so at most one run is ever in progress.
pub struct CheckRunner {
    repos: Repositories,
    providers: Arc<dyn EmailProviderFactory>,
    models: Arc<dyn ModelProvider>,
    observer: Arc<dyn RunObserver>,
    clock: Arc<dyn Clock>,
    lock: RunLock,
}

impl CheckRunner {
    pub fn new(
        repos: Repositories,
        providers: Arc<dyn EmailProviderFactory>,
        models: Arc<dyn ModelProvider>,
        observer: Arc<dyn RunObserver>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repos,
            providers,
            models,
            observer,
            clock,
            lock: RunLock::new(),
        }
    }

    /// Checks preconditions, takes the single-run lock, and creates the run row.
    /// Does no I/O on mail (no provider or model call beyond `models.current`,
    /// which only builds the client, per spec §5 step 1).
    ///
    /// Precondition order: lock busy -> [`RunError::AlreadyRunning`]; then
    /// `settings.load` and `accounts.list` (repo errors -> [`RunError::Storage`]);
    /// no enabled accounts -> [`RunError::NoEnabledAccounts`]; `models.current`
    /// error -> [`RunError::AiNotConfigured`]. The run row is only created once
    /// every precondition has passed (spec §5 step 1: "no run created" on
    /// failure), and the lock is released if any step after acquiring it fails.
    pub fn prepare(&self, trigger: RunTrigger) -> Result<PreparedRun, RunError> {
        let guard = self.lock.try_acquire().ok_or(RunError::AlreadyRunning)?;

        let settings = self.repos.settings.load().map_err(storage_error)?;

        let accounts = self.repos.accounts.list().map_err(storage_error)?;
        let enabled_accounts: Vec<Account> = accounts.into_iter().filter(|a| a.enabled).collect();
        if enabled_accounts.is_empty() {
            return Err(RunError::NoEnabledAccounts);
        }

        let model = self
            .models
            .current(&settings)
            .map_err(RunError::AiNotConfigured)?;

        let run_id = self
            .repos
            .runs
            .create(trigger, self.clock.now())
            .map_err(storage_error)?;
        guard.set_run_id(run_id);

        Ok(PreparedRun {
            run_id,
            model,
            settings,
            accounts: enabled_accounts,
            _lock: guard,
        })
    }

    /// Executes a prepared run to completion. Never panics on a provider or model
    /// error: every account is processed independently (spec §5 step 3), and the
    /// run's final status/counters/summary are always recorded before returning.
    /// Releases the single-run lock when it returns, on every path (the lock
    /// guard lives inside `run` and is dropped when this function's local
    /// bindings go out of scope).
    pub async fn execute(&self, run: PreparedRun) -> RunStatus {
        let PreparedRun {
            run_id,
            model,
            settings,
            accounts,
            _lock,
        } = run;

        let catalog = match self.repos.catalog.catalog() {
            Ok(catalog) => catalog,
            Err(error) => {
                return self.fail_before_processing(run_id, &accounts, &error.to_string())
            }
        };

        let ctx = RunContext {
            repos: &self.repos,
            providers: self.providers.as_ref(),
            model: model.as_ref(),
            observer: self.observer.as_ref(),
            clock: self.clock.as_ref(),
            run_id,
            settings: &settings,
            catalog: &catalog,
        };

        let mut total_counters = Counters::default();
        let mut failed_accounts = 0u32;
        let accounts_processed = accounts.len() as u32;

        for account in &accounts {
            let result = account::process_account(&ctx, account).await;
            total_counters.add(&result.counters);
            if result.status == RunAccountStatus::Failed {
                failed_accounts += 1;
            }
            let _ = self.repos.runs.record_account(run_id, &result);
        }

        let status = overall_status(failed_accounts, accounts_processed);
        let _ = self.repos.runs.finish(
            run_id,
            status,
            self.clock.now(),
            accounts_processed,
            &total_counters,
            error_summary(status, failed_accounts, accounts_processed).as_deref(),
        );

        self.observer.finished(run_id, status);
        status
    }

    /// The id of the run currently in progress, if any.
    pub fn running_run(&self) -> Option<RunId> {
        self.lock.running_run()
    }

    /// Handles the one failure `execute` can hit before any account is
    /// processed: the catalog (loaded once per run, per the task contract)
    /// failing to load. Every enabled account is recorded `Failed`/`Internal` so
    /// the run's history stays consistent with the "one row per attempted
    /// account" contract, and the run finishes `Failed`.
    fn fail_before_processing(
        &self,
        run_id: RunId,
        accounts: &[Account],
        message: &str,
    ) -> RunStatus {
        let accounts_processed = accounts.len() as u32;
        for account in accounts {
            let result = RunAccountResult {
                account_id: account.id,
                status: RunAccountStatus::Failed,
                counters: Counters::default(),
                error_kind: Some(ErrorKind::Internal),
                error_message: Some(message.to_string()),
            };
            let _ = self.repos.runs.record_account(run_id, &result);
        }
        let summary = format!("{accounts_processed} of {accounts_processed} accounts failed");
        let _ = self.repos.runs.finish(
            run_id,
            RunStatus::Failed,
            self.clock.now(),
            accounts_processed,
            &Counters::default(),
            Some(&summary),
        );
        self.observer.finished(run_id, RunStatus::Failed);
        RunStatus::Failed
    }
}

/// A validated, locked-in run ready for [`CheckRunner::execute`]. Only `run_id`
/// is public; the selected model, the settings snapshot and the enabled-account
/// list are captured at `prepare` time so `execute` runs against a consistent
/// view even if settings change while the run is in flight.
pub struct PreparedRun {
    pub run_id: RunId,
    model: Arc<dyn StructuredModel>,
    settings: Settings,
    accounts: Vec<Account>,
    _lock: RunLockGuard,
}

/// Everything a single account/message's processing needs, borrowed for the
/// duration of one [`CheckRunner::execute`] call.
struct RunContext<'a> {
    repos: &'a Repositories,
    providers: &'a dyn EmailProviderFactory,
    model: &'a dyn StructuredModel,
    observer: &'a dyn RunObserver,
    clock: &'a dyn Clock,
    run_id: RunId,
    settings: &'a Settings,
    catalog: &'a Catalog,
}

fn storage_error(error: RepoError) -> RunError {
    RunError::Storage(error.to_string())
}

/// `Success` when no account failed, `Failed` when every attempted account
/// failed, `Partial` otherwise (spec §5 step 5).
fn overall_status(failed_accounts: u32, accounts_processed: u32) -> RunStatus {
    if failed_accounts == 0 {
        RunStatus::Success
    } else if failed_accounts == accounts_processed {
        RunStatus::Failed
    } else {
        RunStatus::Partial
    }
}

fn error_summary(
    status: RunStatus,
    failed_accounts: u32,
    accounts_processed: u32,
) -> Option<String> {
    if status == RunStatus::Success {
        None
    } else {
        Some(format!(
            "{failed_accounts} of {accounts_processed} accounts failed"
        ))
    }
}
