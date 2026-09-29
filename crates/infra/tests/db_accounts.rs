//! `AccountRepository` behavior: CRUD and provider config JSON round-tripping.

#[path = "support/mod.rs"]
mod support;

use academic_core::domain::{
    Category, Priority, RunAccountResult, RunAccountStatus, RunTrigger, SyncCursor,
};
use academic_core::ports::{
    AccountRepository, ProcessedMessageRepository, RepoError, ResultRepository, RunRepository,
    SyncStateRepository,
};
use academic_infra::db::Database;
use chrono::Utc;
use support::{analyzed_message, graph_draft, imap_draft, new_result};

#[test]
fn insert_and_get_round_trip_imap_config() {
    let db = Database::open_in_memory().unwrap();
    let now = Utc::now();
    let draft = imap_draft("uv-imap");

    let created = AccountRepository::insert(&db, &draft, now).unwrap();
    assert_eq!(created.provider, draft.provider);
    assert_eq!(created.name, draft.name);
    assert_eq!(created.created_at, now);
    assert_eq!(created.updated_at, now);

    let fetched = AccountRepository::get(&db, created.id).unwrap().unwrap();
    assert_eq!(fetched, created);
}

#[test]
fn insert_and_get_round_trip_graph_config() {
    let db = Database::open_in_memory().unwrap();
    let draft = graph_draft("uv-graph");

    let created = AccountRepository::insert(&db, &draft, Utc::now()).unwrap();
    assert_eq!(created.provider, draft.provider);

    let fetched = AccountRepository::get(&db, created.id).unwrap().unwrap();
    assert_eq!(fetched, created);
}

#[test]
fn list_returns_every_account_ordered_by_id() {
    let db = Database::open_in_memory().unwrap();
    let a = AccountRepository::insert(&db, &imap_draft("a"), Utc::now()).unwrap();
    let b = AccountRepository::insert(&db, &graph_draft("b"), Utc::now()).unwrap();

    let accounts = db.list().unwrap();
    assert_eq!(
        accounts.iter().map(|acc| acc.id).collect::<Vec<_>>(),
        vec![a.id, b.id]
    );
}

#[test]
fn get_missing_account_returns_none() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(AccountRepository::get(&db, 999).unwrap(), None);
}

#[test]
fn update_replaces_fields_and_preserves_created_at() {
    let db = Database::open_in_memory().unwrap();
    let created_at = Utc::now();
    let created = AccountRepository::insert(&db, &imap_draft("orig"), created_at).unwrap();

    let mut draft = imap_draft("renamed");
    draft.enabled = false;
    let updated_at = created_at + chrono::Duration::seconds(5);
    let updated = db.update(created.id, &draft, updated_at).unwrap();

    assert_eq!(updated.name, "renamed");
    assert!(!updated.enabled);
    assert_eq!(updated.created_at, created.created_at);
    assert_eq!(updated.updated_at, updated_at);

    let fetched = AccountRepository::get(&db, created.id).unwrap().unwrap();
    assert_eq!(fetched, updated);
}

#[test]
fn update_can_switch_provider_kind() {
    let db = Database::open_in_memory().unwrap();
    let created = AccountRepository::insert(&db, &imap_draft("switches"), Utc::now()).unwrap();

    let draft = graph_draft("switches");
    let updated = db.update(created.id, &draft, Utc::now()).unwrap();
    assert_eq!(updated.provider, draft.provider);
}

#[test]
fn update_missing_account_returns_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db.update(999, &imap_draft("x"), Utc::now()).unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn delete_removes_the_account() {
    let db = Database::open_in_memory().unwrap();
    let created = AccountRepository::insert(&db, &imap_draft("gone"), Utc::now()).unwrap();

    db.delete(created.id).unwrap();

    assert_eq!(AccountRepository::get(&db, created.id).unwrap(), None);
}

#[test]
fn account_ids_are_never_reused_after_deleting_the_highest_one() {
    let db = Database::open_in_memory().unwrap();
    AccountRepository::insert(&db, &imap_draft("first"), Utc::now()).unwrap();
    let highest = AccountRepository::insert(&db, &imap_draft("second"), Utc::now()).unwrap();

    db.delete(highest.id).unwrap();
    let next = AccountRepository::insert(&db, &imap_draft("third"), Utc::now()).unwrap();

    assert!(next.id > highest.id);
}

#[test]
fn delete_missing_account_returns_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db.delete(999).unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn delete_cascades_results_processed_messages_sync_states_and_run_accounts() {
    let db = Database::open_in_memory().unwrap();
    let account = AccountRepository::insert(&db, &imap_draft("doomed"), Utc::now()).unwrap();

    let run_id = db.create(RunTrigger::Manual, Utc::now()).unwrap();
    db.record_account(
        run_id,
        &RunAccountResult {
            account_id: account.id,
            status: RunAccountStatus::Success,
            counters: Default::default(),
            error_kind: None,
            error_message: None,
        },
    )
    .unwrap();

    db.record_success(
        account.id,
        "INBOX",
        &SyncCursor::Imap {
            uidvalidity: 1,
            last_uid: 1,
        },
        Utc::now(),
    )
    .unwrap();

    let received_at = Utc::now();
    let message = analyzed_message(account.id, run_id, "uid-1", received_at);
    let result = new_result(
        account.id,
        run_id,
        "Subject",
        received_at,
        Category::Notice,
        Priority::Low,
        vec![],
    );
    let result_id = db.save_analyzed(&message, &result).unwrap();

    db.delete(account.id).unwrap();

    assert_eq!(AccountRepository::get(&db, account.id).unwrap(), None);
    assert!(!db.exists(account.id, "INBOX", "uid-1").unwrap());
    assert_eq!(db.list_for_account(account.id).unwrap(), vec![]);
    assert_eq!(ResultRepository::get(&db, result_id).unwrap(), None);

    let run = RunRepository::get(&db, run_id).unwrap().unwrap();
    assert!(run.accounts.is_empty());
}
