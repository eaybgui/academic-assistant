//! Commands: `get_results`, `set_result_seen`, `mark_all_seen`.

use tauri::State;

use super::logged;
use crate::dto::{PageDto, ResultDto, ResultQueryInput};
use crate::error::CommandError;
use crate::state::AppState;
use crate::validation;

#[tauri::command]
pub async fn get_results(
    state: State<'_, AppState>,
    query: ResultQueryInput,
) -> Result<PageDto<ResultDto>, CommandError> {
    logged("get_results", results(&state, query))
}

#[tauri::command]
pub async fn set_result_seen(
    state: State<'_, AppState>,
    id: i64,
    seen: bool,
) -> Result<ResultDto, CommandError> {
    logged("set_result_seen", set_seen(&state, id, seen))
}

#[tauri::command]
pub async fn mark_all_seen(state: State<'_, AppState>) -> Result<u64, CommandError> {
    logged("mark_all_seen", mark_all(&state))
}

fn results(state: &AppState, query: ResultQueryInput) -> Result<PageDto<ResultDto>, CommandError> {
    let page = state
        .results
        .query(&validation::result_query(query), state.today())?;
    Ok(PageDto {
        items: page.items.iter().map(ResultDto::from).collect(),
        total: page.total,
    })
}

fn set_seen(state: &AppState, id: i64, seen: bool) -> Result<ResultDto, CommandError> {
    let result = state.results.set_seen(id, seen, state.clock.now())?;
    Ok(ResultDto::from(&result))
}

fn mark_all(state: &AppState) -> Result<u64, CommandError> {
    Ok(state.results.mark_all_seen(state.clock.now())?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use crate::test_support::{harness, imap_draft, seed_result};
    use academic_core::domain::ResultSort;
    use chrono::Utc;

    fn query(search: Option<&str>, seen: Option<bool>) -> ResultQueryInput {
        ResultQueryInput {
            search: search.map(str::to_string),
            seen,
            category: None,
            account_id: None,
            course_id: None,
            sort: ResultSort::Received,
            limit: 25,
            offset: 0,
        }
    }

    #[test]
    fn results_are_paged_and_filtered() {
        let h = harness();
        let account = h
            .state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        seed_result(&h.state, account.id, "Algebra exam");
        let read = seed_result(&h.state, account.id, "Chemistry lab");
        set_seen(&h.state, read, true).unwrap();

        assert_eq!(results(&h.state, query(None, None)).unwrap().total, 2);
        let unseen = results(&h.state, query(None, Some(false))).unwrap();
        assert_eq!(unseen.total, 1);
        assert_eq!(unseen.items[0].subject, "Algebra exam");
        let searched = results(&h.state, query(Some("  chem  "), None)).unwrap();
        assert_eq!(searched.total, 1);
        assert_eq!(searched.items[0].subject, "Chemistry lab");
    }

    #[test]
    fn set_seen_toggles_both_ways() {
        let h = harness();
        let account = h
            .state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        let id = seed_result(&h.state, account.id, "Exam");

        let seen = set_seen(&h.state, id, true).unwrap();
        assert!(seen.seen);
        assert!(seen.seen_at.is_some());
        let unseen = set_seen(&h.state, id, false).unwrap();
        assert!(!unseen.seen);
        assert_eq!(unseen.seen_at, None);
    }

    #[test]
    fn set_seen_on_a_missing_result_is_not_found() {
        let h = harness();
        let error = set_seen(&h.state, 999, true).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[test]
    fn mark_all_returns_the_rows_updated() {
        let h = harness();
        let account = h
            .state
            .accounts
            .insert(&imap_draft("IMAP"), Utc::now())
            .unwrap();
        seed_result(&h.state, account.id, "one");
        seed_result(&h.state, account.id, "two");
        assert_eq!(mark_all(&h.state).unwrap(), 2);
        assert_eq!(mark_all(&h.state).unwrap(), 0);
    }
}
