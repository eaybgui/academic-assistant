//! `RunRepository` behavior: run lifecycle and history.

#[path = "support/mod.rs"]
mod support;

use academic_core::domain::{
    Counters, ErrorKind, RunAccountResult, RunAccountStatus, RunStatus, RunTrigger,
};
use academic_core::ports::{RepoError, RunRepository};
use academic_infra::db::Database;
use chrono::{Duration, Utc};
use support::insert_account;

fn sample_counters() -> Counters {
    Counters {
        found: 10,
        filtered: 2,
        analyzed: 6,
        failed: 2,
        results_created: 5,
    }
}

#[test]
fn create_starts_a_running_run_with_zero_counters() {
    let db = Database::open_in_memory().unwrap();
    let started_at = Utc::now();
    let run_id = db.create(RunTrigger::Manual, started_at).unwrap();

    let run = db.get(run_id).unwrap().unwrap();
    assert_eq!(run.id, run_id);
    assert_eq!(run.trigger, RunTrigger::Manual);
    assert_eq!(run.status, RunStatus::Running);
    assert_eq!(run.started_at, started_at);
    assert_eq!(run.finished_at, None);
    assert_eq!(run.counters, Counters::default());
    assert!(run.accounts.is_empty());
}

#[test]
fn record_account_then_get_includes_account_name() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = db.create(RunTrigger::Manual, Utc::now()).unwrap();

    let result = RunAccountResult {
        account_id: account.id,
        status: RunAccountStatus::Success,
        counters: sample_counters(),
        error_kind: None,
        error_message: None,
    };
    db.record_account(run_id, &result).unwrap();

    let run = db.get(run_id).unwrap().unwrap();
    assert_eq!(run.accounts.len(), 1);
    assert_eq!(run.accounts[0].account_name, account.name);
    assert_eq!(run.accounts[0].result, result);
}

#[test]
fn record_account_stores_failure_details() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = db.create(RunTrigger::Manual, Utc::now()).unwrap();

    let result = RunAccountResult {
        account_id: account.id,
        status: RunAccountStatus::Failed,
        counters: Counters::default(),
        error_kind: Some(ErrorKind::Auth),
        error_message: Some("bad credentials".to_string()),
    };
    db.record_account(run_id, &result).unwrap();

    let run = db.get(run_id).unwrap().unwrap();
    assert_eq!(run.accounts[0].result.error_kind, Some(ErrorKind::Auth));
    assert_eq!(
        run.accounts[0].result.error_message.as_deref(),
        Some("bad credentials")
    );
}

#[test]
fn finish_updates_status_counters_and_error_summary() {
    let db = Database::open_in_memory().unwrap();
    let started_at = Utc::now();
    let run_id = db.create(RunTrigger::Manual, started_at).unwrap();
    let finished_at = started_at + Duration::minutes(2);

    db.finish(
        run_id,
        RunStatus::Partial,
        finished_at,
        1,
        &sample_counters(),
        Some("one account failed"),
    )
    .unwrap();

    let run = db.get(run_id).unwrap().unwrap();
    assert_eq!(run.status, RunStatus::Partial);
    assert_eq!(run.finished_at, Some(finished_at));
    assert_eq!(run.accounts_processed, 1);
    assert_eq!(run.counters, sample_counters());
    assert_eq!(run.error_summary.as_deref(), Some("one account failed"));
}

#[test]
fn finish_missing_run_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db
        .finish(
            999,
            RunStatus::Success,
            Utc::now(),
            0,
            &Counters::default(),
            None,
        )
        .unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn get_missing_run_returns_none() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.get(999).unwrap(), None);
}

#[test]
fn latest_returns_the_most_recently_started_run() {
    let db = Database::open_in_memory().unwrap();
    let first = db.create(RunTrigger::Manual, Utc::now()).unwrap();
    let second = db
        .create(RunTrigger::Manual, Utc::now() + Duration::seconds(1))
        .unwrap();

    let latest = db.latest().unwrap().unwrap();
    assert_eq!(latest.id, second);
    assert_ne!(latest.id, first);
}

#[test]
fn latest_is_none_when_no_runs_exist() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.latest().unwrap(), None);
}

#[test]
fn history_returns_newest_first_and_respects_limit() {
    let db = Database::open_in_memory().unwrap();
    let base = Utc::now();
    let first = db.create(RunTrigger::Manual, base).unwrap();
    let second = db
        .create(RunTrigger::Manual, base + Duration::seconds(1))
        .unwrap();
    let third = db
        .create(RunTrigger::Manual, base + Duration::seconds(2))
        .unwrap();

    let history = db.history(2).unwrap();
    assert_eq!(
        history.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![third, second]
    );
    assert!(!history.iter().any(|r| r.id == first));
}

#[test]
fn mark_interrupted_fails_every_running_run() {
    let db = Database::open_in_memory().unwrap();
    let running = db.create(RunTrigger::Manual, Utc::now()).unwrap();
    let already_done = db.create(RunTrigger::Manual, Utc::now()).unwrap();
    db.finish(
        already_done,
        RunStatus::Success,
        Utc::now(),
        0,
        &Counters::default(),
        None,
    )
    .unwrap();

    let at = Utc::now();
    let changed = db.mark_interrupted(at).unwrap();
    assert_eq!(changed, 1);

    let interrupted = db.get(running).unwrap().unwrap();
    assert_eq!(interrupted.status, RunStatus::Failed);
    assert_eq!(interrupted.error_summary.as_deref(), Some("Interrupted"));
    assert_eq!(interrupted.finished_at, Some(at));

    // The already-finished run is left untouched.
    let untouched = db.get(already_done).unwrap().unwrap();
    assert_eq!(untouched.status, RunStatus::Success);
    assert_eq!(untouched.error_summary, None);
}
