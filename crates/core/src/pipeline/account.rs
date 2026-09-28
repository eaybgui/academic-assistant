//! Per-account processing: connect, fetch each folder, run every message through
//! [`super::message::process_message`], and report the account's overall outcome
//! (spec §5, steps 3-4).

use chrono::Duration;

use crate::domain::{
    Account, Counters, ErrorKind, RunAccountResult, RunAccountStatus, RunProgress, RunStage,
};

use super::message::{self, MessageError, MessageOutcome};
use super::RunContext;

/// Runs the full check for one account: every configured folder, sequentially,
/// stopping the account early only on a transient AI error or a repository error
/// (spec §5.1's last row; both are treated as an internal/AI failure that leaves
/// the folder's cursor un-advanced).
pub(super) async fn process_account(ctx: &RunContext<'_>, account: &Account) -> RunAccountResult {
    emit(ctx, account, RunStage::Connecting, 0, 0);

    let provider = match ctx.providers.create(account) {
        Ok(provider) => provider,
        Err(error) => {
            emit(ctx, account, RunStage::Failed, 0, 0);
            return RunAccountResult {
                account_id: account.id,
                status: RunAccountStatus::Failed,
                counters: Counters::default(),
                error_kind: Some(error.kind()),
                error_message: Some(error.to_string()),
            };
        }
    };

    let mut counters = Counters::default();
    let mut account_error: Option<(ErrorKind, String)> = None;

    for folder in &account.folders {
        emit(ctx, account, RunStage::Fetching, 0, 0);

        let sync_state = match ctx.repos.sync.get(account.id, folder) {
            Ok(state) => state,
            Err(error) => {
                account_error.get_or_insert((ErrorKind::Internal, error.to_string()));
                continue;
            }
        };
        let cursor = sync_state.and_then(|state| state.cursor);
        let since = ctx.clock.now() - Duration::days(i64::from(account.initial_sync_days));

        let batch = match provider.fetch_new(folder, cursor.as_ref(), since).await {
            Ok(batch) => batch,
            Err(error) => {
                let _ = ctx.repos.sync.record_failure(
                    account.id,
                    folder,
                    &error.to_string(),
                    ctx.clock.now(),
                );
                account_error.get_or_insert((error.kind(), error.to_string()));
                continue;
            }
        };

        let total = batch.emails.len() as u32;
        let mut processed = 0u32;
        let mut stop_account = false;

        for email in &batch.emails {
            let result = message::process_message(ctx, account, email).await;
            processed += 1;

            match result {
                Ok(MessageOutcome::Skipped) => {}
                Ok(MessageOutcome::Filtered) => {
                    counters.found += 1;
                    counters.filtered += 1;
                }
                Ok(MessageOutcome::Irrelevant) => {
                    counters.found += 1;
                    counters.analyzed += 1;
                }
                Ok(MessageOutcome::Relevant) => {
                    counters.found += 1;
                    counters.analyzed += 1;
                    counters.results_created += 1;
                }
                Ok(MessageOutcome::InvalidResponse) => {
                    counters.found += 1;
                    counters.analyzed += 1;
                    counters.failed += 1;
                }
                Err(MessageError::Model(model_error)) => {
                    counters.found += 1;
                    let _ = ctx.repos.sync.record_failure(
                        account.id,
                        folder,
                        &model_error.to_string(),
                        ctx.clock.now(),
                    );
                    account_error = Some((ErrorKind::Ai, model_error.to_string()));
                    stop_account = true;
                }
                Err(MessageError::Repo(repo_error)) => {
                    let _ = ctx.repos.sync.record_failure(
                        account.id,
                        folder,
                        &repo_error.to_string(),
                        ctx.clock.now(),
                    );
                    account_error = Some((ErrorKind::Internal, repo_error.to_string()));
                    stop_account = true;
                }
            }

            if stop_account || processed.is_multiple_of(10) || processed == total {
                emit(ctx, account, RunStage::Analyzing, processed, total);
            }

            if stop_account {
                break;
            }
        }

        if stop_account {
            break;
        }

        let _ =
            ctx.repos
                .sync
                .record_success(account.id, folder, &batch.next_cursor, ctx.clock.now());
    }

    let status = if account_error.is_some() {
        RunAccountStatus::Failed
    } else {
        RunAccountStatus::Success
    };
    emit(
        ctx,
        account,
        if account_error.is_some() {
            RunStage::Failed
        } else {
            RunStage::Done
        },
        0,
        0,
    );

    let (error_kind, error_message) = match account_error {
        Some((kind, message)) => (Some(kind), Some(message)),
        None => (None, None),
    };

    RunAccountResult {
        account_id: account.id,
        status,
        counters,
        error_kind,
        error_message,
    }
}

fn emit(ctx: &RunContext<'_>, account: &Account, stage: RunStage, processed: u32, total: u32) {
    ctx.observer.progress(&RunProgress {
        run_id: ctx.run_id,
        account_id: account.id,
        account_name: account.name.clone(),
        stage,
        processed,
        total,
    });
}
