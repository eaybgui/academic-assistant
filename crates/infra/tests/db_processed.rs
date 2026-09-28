//! `ProcessedMessageRepository` behavior: dedup/audit trail.

#[path = "support/mod.rs"]
mod support;

use academic_core::domain::MessageOutcome;
use academic_core::ports::{ProcessedMessageRepository, RepoError};
use academic_infra::db::Database;
use chrono::Utc;
use support::{analyzed_message, insert_account, insert_run};

#[test]
fn exists_is_false_before_insert_and_true_after() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let message = analyzed_message(account.id, run_id, "uid-1", Utc::now());

    assert!(!db.exists(account.id, "INBOX", "uid-1").unwrap());

    db.insert(&message).unwrap();

    assert!(db.exists(account.id, "INBOX", "uid-1").unwrap());
    assert!(!db.exists(account.id, "Archive", "uid-1").unwrap());
    assert!(!db.exists(account.id, "INBOX", "uid-2").unwrap());
}

#[test]
fn inserting_the_same_provider_message_id_twice_is_conflict() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let message = analyzed_message(account.id, run_id, "uid-1", Utc::now());

    db.insert(&message).unwrap();
    let err = db.insert(&message).unwrap_err();
    assert!(matches!(err, RepoError::Conflict(_)));
}

#[test]
fn is_duplicate_matches_by_message_id_header() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let mut message = analyzed_message(account.id, run_id, "uid-1", Utc::now());
    message.message_id_header = Some("<same@example.test>".to_string());
    message.content_hash = "hash-a".to_string();
    db.insert(&message).unwrap();

    // Same header, different hash: still a duplicate.
    let is_dup = db
        .is_duplicate(account.id, Some("<same@example.test>"), "hash-b")
        .unwrap();
    assert!(is_dup);

    // Different header and different hash: not a duplicate.
    let not_dup = db
        .is_duplicate(account.id, Some("<other@example.test>"), "hash-c")
        .unwrap();
    assert!(!not_dup);
}

#[test]
fn is_duplicate_matches_by_content_hash() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let mut message = analyzed_message(account.id, run_id, "uid-1", Utc::now());
    message.message_id_header = None;
    message.content_hash = "same-hash".to_string();
    db.insert(&message).unwrap();

    // No header on the new message either: falls back to the hash, which matches.
    let is_dup = db.is_duplicate(account.id, None, "same-hash").unwrap();
    assert!(is_dup);

    let not_dup = db.is_duplicate(account.id, None, "different-hash").unwrap();
    assert!(!not_dup);
}

#[test]
fn is_duplicate_is_scoped_to_the_account() {
    let db = Database::open_in_memory().unwrap();
    let account_a = insert_account(&db, "a");
    let account_b = insert_account(&db, "b");
    let run_id = insert_run(&db);
    let mut message = analyzed_message(account_a.id, run_id, "uid-1", Utc::now());
    message.content_hash = "shared-hash".to_string();
    db.insert(&message).unwrap();

    let dup_for_owner = db.is_duplicate(account_a.id, None, "shared-hash").unwrap();
    let dup_for_other = db.is_duplicate(account_b.id, None, "shared-hash").unwrap();
    assert!(dup_for_owner);
    assert!(!dup_for_other);
}

#[test]
fn insert_stores_non_analyzed_outcomes_too() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let mut message = analyzed_message(account.id, run_id, "uid-1", Utc::now());
    message.outcome = MessageOutcome::Filtered;
    message.reason = Some("blocked_sender".to_string());

    let id = db.insert(&message).unwrap();
    assert!(id > 0);
    assert!(db.exists(account.id, "INBOX", "uid-1").unwrap());
}
