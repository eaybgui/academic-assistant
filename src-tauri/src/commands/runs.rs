//! Commands: `check_now`, `get_dashboard`, `get_run_history`.

use academic_core::domain::{ResultQuery, ResultSort, RunId, RunTrigger};
use tauri::State;

use super::logged;
use crate::account_status;
use crate::dto::{DashboardCountsDto, DashboardDto, ResultDto, RunDto};
use crate::error::CommandError;
use crate::state::AppState;
use crate::validation;

/// Maximum number of unseen results shown on the dashboard.
const DASHBOARD_RESULT_LIMIT: u32 = 100;

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

/// Validates preconditions and creates the run row, then executes the run in the
/// background. Progress and completion are reported through the run observer.
fn start_check(state: &AppState) -> Result<RunId, CommandError> {
    let prepared = state.runner.prepare(RunTrigger::Manual)?;
    let run_id = prepared.run_id;
    let runner = state.runner.clone();
    tauri::async_runtime::spawn(async move {
        runner.execute(prepared).await;
    });
    Ok(run_id)
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
        harness, harness_with, imap_draft, seed_result, FakeProviderBehavior,
    };
    use academic_core::domain::RunStatus;
    use chrono::Utc;

    #[test]
    fn check_now_without_accounts_is_a_validation_error() {
        let h = harness();
        let error = start_check(&h.state).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(error.message, "There are no enabled accounts");
    }

    #[test]
    fn check_now_reports_a_missing_ai_configuration() {
        let h = harness_with(FakeProviderBehavior::default(), Err("Add a key".into()));
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
        h.state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        let run_id = start_check(&h.state).unwrap();
        assert!(h.state.runs.get(run_id).unwrap().is_some());
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
