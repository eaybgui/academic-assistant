//! `SyncStateRepository` behavior: per-account, per-folder cursor bookkeeping.

#[path = "support/mod.rs"]
mod support;

use academic_core::domain::SyncCursor;
use academic_core::ports::SyncStateRepository;
use academic_infra::db::Database;
use chrono::{Duration, Utc};
use support::insert_account;

#[test]
fn get_missing_state_returns_none() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    assert_eq!(db.get(account.id, "INBOX").unwrap(), None);
}

#[test]
fn record_success_then_get_round_trips_cursor_and_status() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let cursor = SyncCursor::Imap {
        uidvalidity: 7,
        last_uid: 100,
    };
    let at = Utc::now();

    db.record_success(account.id, "INBOX", &cursor, at).unwrap();

    let state = db.get(account.id, "INBOX").unwrap().unwrap();
    assert_eq!(state.cursor, Some(cursor));
    assert_eq!(state.status.as_str(), "ok");
    assert_eq!(state.last_success_at, Some(at));
    assert_eq!(state.last_run_at, Some(at));
    assert_eq!(state.last_error, None);
}

#[test]
fn record_failure_sets_error_and_preserves_prior_cursor() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let cursor = SyncCursor::Imap {
        uidvalidity: 7,
        last_uid: 100,
    };
    let success_at = Utc::now();
    db.record_success(account.id, "INBOX", &cursor, success_at)
        .unwrap();

    let failure_at = success_at + Duration::minutes(5);
    db.record_failure(account.id, "INBOX", "connection refused", failure_at)
        .unwrap();

    let state = db.get(account.id, "INBOX").unwrap().unwrap();
    assert_eq!(state.status.as_str(), "error");
    assert_eq!(state.last_error.as_deref(), Some("connection refused"));
    assert_eq!(state.last_run_at, Some(failure_at));
    // A failure must not discard the cursor or timestamp from the last success.
    assert_eq!(state.cursor, Some(cursor));
    assert_eq!(state.last_success_at, Some(success_at));
}

#[test]
fn record_failure_without_prior_success_has_no_cursor() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let at = Utc::now();

    db.record_failure(account.id, "INBOX", "auth failed", at)
        .unwrap();

    let state = db.get(account.id, "INBOX").unwrap().unwrap();
    assert_eq!(state.cursor, None);
    assert_eq!(state.last_success_at, None);
    assert_eq!(state.status.as_str(), "error");
}

#[test]
fn list_for_account_returns_every_folder_ordered_by_name() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let cursor = SyncCursor::Graph {
        link: "https://graph/delta".to_string(),
    };
    db.record_success(account.id, "Archive", &cursor, Utc::now())
        .unwrap();
    db.record_success(account.id, "INBOX", &cursor, Utc::now())
        .unwrap();

    let states = db.list_for_account(account.id).unwrap();
    assert_eq!(
        states.iter().map(|s| s.folder.clone()).collect::<Vec<_>>(),
        vec!["Archive".to_string(), "INBOX".to_string()]
    );
}
