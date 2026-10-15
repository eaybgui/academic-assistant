//! Commands: `check_now`, `get_dashboard`, `get_run_history`.

use std::sync::Arc;

use academic_core::domain::{Counters, ResultQuery, ResultSort, RunId, RunStatus, RunTrigger};
use academic_core::ports::{Clock, RunObserver, RunRepository};
use tauri::async_runtime::JoinHandle;
use tauri::State;

use super::logged;
use crate::account_status;
use crate::dto::{DashboardCountsDto, DashboardDto, ResultDto, RunDto};
use crate::error::CommandError;
use crate::state::AppState;
use crate::validation;

/// Maximum number of unseen results shown on the dashboard.
const DASHBOARD_RESULT_LIMIT: u32 = 100;
/// Error summary of a run whose execution task ended abnormally (same text the
/// startup recovery uses for runs interrupted by a crash).
const INTERRUPTED_SUMMARY: &str = "Interrupted";
/// Email analysis is secondary: it stays disabled until a course is tracked.
const NO_TRACKED_COURSE: &str = "Track at least one course before checking email";

#[tauri::command]
pub async fn check_now(state: State<'_, AppState>) -> Result<RunId, CommandError> {
    logged("check_now", start_check(&state))
}

#[tauri::command]
pub async fn get_dashboard(
    state: State<'_, AppState>,
    sort: ResultSort,
) -> Result<DashboardDto, CommandError> {
    logged("get_dashboard", dashboard(&state, sort))
}

#[tauri::command]
pub async fn get_run_history(
    state: State<'_, AppState>,
    limit: i64,
) -> Result<Vec<RunDto>, CommandError> {
    logged("get_run_history", run_history(&state, limit))
}

/// Validates preconditions (a tracked course first, then the runner's) and creates
/// the run row, then executes the run in the background. Progress and completion are reported through the run observer; a
/// supervisor task recovers the run if the execution task panics.
fn start_check(state: &AppState) -> Result<RunId, CommandError> {
    if state.catalog.catalog()?.courses.is_empty() {
        return Err(CommandError::validation(NO_TRACKED_COURSE));
    }
    let prepared = state.runner.prepare(RunTrigger::Manual)?;
    let run_id = prepared.run_id;
    tracing::info!(run_id, "check started");
    let runner = state.runner.clone();
    let execution = tauri::async_runtime::spawn(async move { runner.execute(prepared).await });
    tauri::async_runtime::spawn(supervise(
        execution,
        run_id,
        state.runs.clone(),
        state.observer.clone(),
        state.clock.clone(),
    ));
    Ok(run_id)
}

/// Waits for the run's execution task. If the task panicked or was cancelled the
/// run would stay `running` forever, so it is marked failed and the observer is
/// told, letting the UI recover.
async fn supervise(
    execution: JoinHandle<RunStatus>,
    run_id: RunId,
    runs: Arc<dyn RunRepository>,
    observer: Arc<dyn RunObserver>,
    clock: Arc<dyn Clock>,
) {
    let Err(error) = execution.await else {
        return;
    };
    tracing::error!(run_id, %error, "check run ended abnormally");
    match runs.get(run_id) {
        Ok(Some(run)) if run.status == RunStatus::Running => {}
        Ok(_) => return,
        Err(error) => {
            tracing::error!(run_id, %error, "could not read the interrupted run");
            return;
        }
    }
    if let Err(error) = runs.finish(
        run_id,
        RunStatus::Failed,
        clock.now(),
        0,
        &Counters::default(),
        Some(INTERRUPTED_SUMMARY),
    ) {
        tracing::error!(run_id, %error, "could not mark the interrupted run as failed");
    }
    observer.finished(run_id, RunStatus::Failed);
}

fn dashboard(state: &AppState, sort: ResultSort) -> Result<DashboardDto, CommandError> {
    let query = ResultQuery {
        search: None,
        seen: Some(false),
        category: None,
        account_id: None,
        course_id: None,
        sort,
        limit: DASHBOARD_RESULT_LIMIT,
        offset: 0,
    };
    let new_results = state.results.query(&query, state.today())?;
    Ok(DashboardDto {
        last_run: state.runs.latest()?.as_ref().map(RunDto::from),
        running_run_id: state.runner.running_run(),
        attention: account_status::attention(state)?,
        counts: DashboardCountsDto::from(&state.results.unseen_counts()?),
        new_results: new_results.items.iter().map(ResultDto::from).collect(),
    })
}

fn run_history(state: &AppState, limit: i64) -> Result<Vec<RunDto>, CommandError> {
    let runs = state.runs.history(validation::clamp_limit(limit))?;
    Ok(runs.iter().map(RunDto::from).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use crate::test_support::{
        harness, harness_with, imap_draft, seed_result, track_one_course, FakeProviderBehavior,
        Harness,
    };
    use academic_core::domain::RunProgress;
    use chrono::Utc;
    use std::sync::Mutex;

    #[test]
    fn check_now_without_a_tracked_course_is_a_validation_error() {
        let h = harness();
        h.state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        let error = start_check(&h.state).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(
            error.message,
            "Track at least one course before checking email"
        );
        assert_eq!(h.state.runs.latest().unwrap(), None);
    }

    #[test]
    fn check_now_without_accounts_is_a_validation_error() {
        let h = harness();
        track_one_course(&h.state);
        let error = start_check(&h.state).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(error.message, "There are no enabled accounts");
    }

    #[test]
    fn check_now_reports_a_missing_ai_configuration() {
        let h = harness_with(FakeProviderBehavior::default(), Err("Add a key".into()));
        track_one_course(&h.state);
        h.state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        let error = start_check(&h.state).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert!(error.message.contains("Add a key"));
    }

    #[test]
    fn check_now_creates_the_run_and_returns_its_id() {
        let h = harness();
        track_one_course(&h.state);
        h.state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        let run_id = start_check(&h.state).unwrap();
        assert!(h.state.runs.get(run_id).unwrap().is_some());
    }

    #[derive(Default)]
    struct RecordingObserver(Mutex<Vec<(RunId, RunStatus)>>);

    impl RunObserver for RecordingObserver {
        fn progress(&self, _progress: &RunProgress) {}

        fn finished(&self, run_id: RunId, status: RunStatus) {
            self.0.lock().unwrap().push((run_id, status));
        }
    }

    async fn panicking_run() -> RunStatus {
        panic!("simulated panic in a run")
    }

    async fn supervise_panicking_run(
        h: &Harness,
        run_id: RunId,
        observer: &Arc<RecordingObserver>,
    ) {
        supervise(
            tauri::async_runtime::spawn(panicking_run()),
            run_id,
            h.state.runs.clone(),
            observer.clone(),
            h.state.clock.clone(),
        )
        .await;
    }

    #[tokio::test]
    async fn a_panicking_run_is_marked_failed_and_reported() {
        let h = harness();
        let run_id = h.state.runs.create(RunTrigger::Manual, Utc::now()).unwrap();
        let observer = Arc::new(RecordingObserver::default());

        supervise_panicking_run(&h, run_id, &observer).await;

        let run = h.state.runs.get(run_id).unwrap().unwrap();
        assert_eq!(run.status, RunStatus::Failed);
        assert_eq!(run.error_summary.as_deref(), Some("Interrupted"));
        assert_eq!(
            *observer.0.lock().unwrap(),
            vec![(run_id, RunStatus::Failed)]
        );
    }

    #[tokio::test]
    async fn a_panic_after_the_run_finished_keeps_its_recorded_status() {
        let h = harness();
        let run_id = h.state.runs.create(RunTrigger::Manual, Utc::now()).unwrap();
        h.state
            .runs
            .finish(
                run_id,
                RunStatus::Success,
                Utc::now(),
                1,
                &Counters::default(),
                None,
            )
            .unwrap();
        let observer = Arc::new(RecordingObserver::default());

        supervise_panicking_run(&h, run_id, &observer).await;

        let run = h.state.runs.get(run_id).unwrap().unwrap();
        assert_eq!(run.status, RunStatus::Success);
        assert!(observer.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_run_that_completes_normally_is_left_alone() {
        let h = harness();
        let run_id = h.state.runs.create(RunTrigger::Manual, Utc::now()).unwrap();
        let observer = Arc::new(RecordingObserver::default());

        supervise(
            tauri::async_runtime::spawn(async { RunStatus::Success }),
            run_id,
            h.state.runs.clone(),
            observer.clone(),
            h.state.clock.clone(),
        )
        .await;

        let run = h.state.runs.get(run_id).unwrap().unwrap();
        assert_eq!(run.status, RunStatus::Running);
        assert!(observer.0.lock().unwrap().is_empty());
    }

    #[test]
    fn dashboard_of_an_empty_database() {
        let h = harness();
        let dto = dashboard(&h.state, ResultSort::Upcoming).unwrap();
        assert_eq!(dto.last_run, None);
        assert_eq!(dto.running_run_id, None);
        assert!(dto.attention.is_empty());
        assert_eq!(dto.counts.new_total, 0);
        assert!(dto.new_results.is_empty());
    }

    #[test]
    fn dashboard_lists_unseen_results_and_counts() {
        let h = harness();
        let account = h
            .state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        let seen = seed_result(&h.state, account.id, "already read");
        seed_result(&h.state, account.id, "new one");
        h.state.results.set_seen(seen, true, Utc::now()).unwrap();

        let dto = dashboard(&h.state, ResultSort::Received).unwrap();
        assert_eq!(dto.counts.new_total, 1);
        assert_eq!(dto.counts.tasks, 1);
        assert_eq!(dto.new_results.len(), 1);
        assert_eq!(dto.new_results[0].subject, "new one");
        assert_eq!(dto.last_run.map(|run| run.status), Some(RunStatus::Running));
    }

    #[test]
    fn run_history_clamps_the_limit() {
        let h = harness();
        let account = h
            .state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        seed_result(&h.state, account.id, "one");
        seed_result(&h.state, account.id, "two");
        assert_eq!(run_history(&h.state, 0).unwrap().len(), 1);
        assert_eq!(run_history(&h.state, 50).unwrap().len(), 2);
    }
}
