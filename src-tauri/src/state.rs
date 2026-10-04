//! `AppState`: the composition of repositories, adapters and the check runner
//! shared by every command through Tauri's managed state.

use std::sync::Arc;

use academic_core::pipeline::{CheckRunner, Repositories};
use academic_core::ports::{
    AccountRepository, CatalogRepository, Clock, EmailProviderFactory, ModelProvider,
    ResultRepository, RunObserver, RunRepository, SecretStore, SettingsRepository,
    SyncStateRepository,
};
use academic_infra::db::Database;
use chrono::NaiveDate;
use tracing_appender::non_blocking::WorkerGuard;

/// Adapters and services `AppState` is composed from. `Database` is passed
/// separately because every repository port is implemented by it.
pub struct Collaborators {
    pub secrets: Arc<dyn SecretStore>,
    pub factory: Arc<dyn EmailProviderFactory>,
    pub models: Arc<dyn ModelProvider>,
    pub http: reqwest::Client,
    pub observer: Arc<dyn RunObserver>,
    pub clock: Arc<dyn Clock>,
}

pub struct AppState {
    pub accounts: Arc<dyn AccountRepository>,
    pub catalog: Arc<dyn CatalogRepository>,
    pub sync: Arc<dyn SyncStateRepository>,
    pub results: Arc<dyn ResultRepository>,
    pub runs: Arc<dyn RunRepository>,
    pub settings: Arc<dyn SettingsRepository>,
    pub secrets: Arc<dyn SecretStore>,
    pub factory: Arc<dyn EmailProviderFactory>,
    pub models: Arc<dyn ModelProvider>,
    pub http: reqwest::Client,
    pub clock: Arc<dyn Clock>,
    pub observer: Arc<dyn RunObserver>,
    pub runner: Arc<CheckRunner>,
    /// Keeps the non-blocking log writer flushing for the lifetime of the app.
    _log_guard: Option<WorkerGuard>,
}

impl AppState {
    /// Wires the single `db` into every repository port and builds the
    /// [`CheckRunner`] over the same adapters.
    pub fn new(db: Arc<Database>, collaborators: Collaborators) -> Self {
        let Collaborators {
            secrets,
            factory,
            models,
            http,
            observer,
            clock,
        } = collaborators;
        let runner = Arc::new(CheckRunner::new(
            Repositories {
                accounts: db.clone(),
                catalog: db.clone(),
                sync: db.clone(),
                processed: db.clone(),
                results: db.clone(),
                runs: db.clone(),
                settings: db.clone(),
            },
            factory.clone(),
            models.clone(),
            observer.clone(),
            clock.clone(),
        ));
        Self {
            accounts: db.clone(),
            catalog: db.clone(),
            sync: db.clone(),
            results: db.clone(),
            runs: db.clone(),
            settings: db,
            secrets,
            factory,
            models,
            http,
            clock,
            observer,
            runner,
            _log_guard: None,
        }
    }

    pub fn with_log_guard(mut self, guard: WorkerGuard) -> Self {
        self._log_guard = Some(guard);
        self
    }

    /// The current date in the user's local time zone.
    pub fn today(&self) -> NaiveDate {
        self.clock
            .now()
            .with_timezone(&self.clock.local_offset())
            .date_naive()
    }
}
