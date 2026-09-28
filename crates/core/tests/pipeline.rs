//! Integration tests for `core::pipeline::CheckRunner`, exercising the full
//! sync -> dedup -> filter -> analyze -> persist flow against in-memory fakes of
//! every port (see `tests/support/`).

mod support;

use std::sync::Arc;

use academic_core::domain::{
    Catalog, ErrorKind, MessageOutcome, ProcessedMessage, RunStatus, RunTrigger, Settings,
    SyncCursor, DEFAULT_IMAP_FOLDER,
};
use academic_core::pipeline::{CheckRunner, Repositories};
use academic_core::ports::{Answer, EmailProvider, EmailProviderFactory, ProviderError};
use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, NaiveDate, TimeZone, Utc};
use tokio::sync::Notify;

use support::{
    account, account_with_inbox, base_time, irrelevant_answers, relevant_answers, simple_email,
    FakeClock, FakeEmailProviderFactory, FakeModelProvider, FakeObserver, FakeProviderError,
    FakeStructuredModel, TestStore,
};

fn empty_catalog() -> Catalog {
    Catalog {
        universities: vec![],
        courses: vec![],
    }
}

fn utc_offset() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap()
}

fn repos(store: &Arc<TestStore>) -> Repositories {
    Repositories {
        accounts: store.clone(),
        catalog: store.clone(),
        sync: store.clone(),
        processed: store.clone(),
        results: store.clone(),
        runs: store.clone(),
        settings: store.clone(),
    }
}

fn imap_cursor(last_uid: u32) -> SyncCursor {
    SyncCursor::Imap {
        uidvalidity: 1,
        last_uid,
    }
}

fn seeded_message(
    account_id: i64,
    folder: &str,
    provider_message_id: &str,
    outcome: MessageOutcome,
) -> ProcessedMessage {
    ProcessedMessage {
        account_id,
        folder: folder.to_string(),
        provider_message_id: provider_message_id.to_string(),
        message_id_header: Some(format!("<{provider_message_id}@example.edu>")),
        content_hash: format!("hash-{provider_message_id}"),
        received_at: base_time(),
        outcome,
        reason: None,
        run_id: 0,
        processed_at: base_time(),
    }
}

// ---------------------------------------------------------------------------
// Happy path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn happy_path_creates_results_and_advances_cursor() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "Exam reminder",
                "The exam is next week.",
            )],
            next_cursor: imap_cursor(100),
        })],
    );

    let model = Arc::new(FakeStructuredModel::new(vec![Ok(relevant_answers())]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));

    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer.clone(),
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    let status = runner.execute(prepared).await;

    assert_eq!(status, RunStatus::Success);
    let results = store.results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].subject, "Exam reminder");

    let successes = store.sync_successes();
    assert_eq!(successes.len(), 1);
    assert_eq!(
        successes[0],
        (1, DEFAULT_IMAP_FOLDER.to_string(), imap_cursor(100))
    );

    let runs = store.runs();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].counters.found, 1);
    assert_eq!(runs[0].counters.analyzed, 1);
    assert_eq!(runs[0].counters.results_created, 1);
    assert_eq!(runs[0].counters.filtered, 0);
    assert_eq!(runs[0].counters.failed, 0);
    assert_eq!(runs[0].accounts_processed, 1);
    assert_eq!(runs[0].error_summary, None);

    assert_eq!(
        observer.finished_calls(),
        vec![(runs[0].id, RunStatus::Success)]
    );
}

// ---------------------------------------------------------------------------
// Dedup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn already_exists_messages_are_skipped_and_not_counted() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));
    store.seed_processed_message(seeded_message(
        1,
        DEFAULT_IMAP_FOLDER,
        "1",
        MessageOutcome::Irrelevant,
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "Already seen",
                "body",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    let model = Arc::new(FakeStructuredModel::new(vec![]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model.clone())),
        observer,
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    let status = runner.execute(prepared).await;

    assert_eq!(status, RunStatus::Success);
    assert_eq!(
        model.call_count(),
        0,
        "the model must never be asked about a skipped message"
    );
    let runs = store.runs();
    assert_eq!(runs[0].counters.found, 0);
    assert_eq!(runs[0].counters.filtered, 0);
    assert_eq!(runs[0].counters.analyzed, 0);
}

#[tokio::test]
async fn duplicate_content_hash_is_filtered() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let first = simple_email(
        1,
        DEFAULT_IMAP_FOLDER,
        "1",
        "prof@example.edu",
        "Notice",
        "same body",
    );
    let mut second = simple_email(
        1,
        DEFAULT_IMAP_FOLDER,
        "2",
        "prof@example.edu",
        "Notice",
        "same body",
    );
    second.message_id_header = Some("<different@example.edu>".to_string());

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![first, second],
            next_cursor: imap_cursor(2),
        })],
    );

    let model = Arc::new(FakeStructuredModel::new(vec![Ok(irrelevant_answers())]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model.clone())),
        observer,
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    let status = runner.execute(prepared).await;

    assert_eq!(status, RunStatus::Success);
    assert_eq!(
        model.call_count(),
        1,
        "only the first message should reach the model"
    );

    let messages = store.processed_messages();
    let filtered = messages
        .iter()
        .find(|m| m.provider_message_id == "2")
        .expect("second message recorded");
    assert_eq!(filtered.outcome, MessageOutcome::Filtered);
    assert_eq!(filtered.reason.as_deref(), Some("duplicate"));

    let runs = store.runs();
    assert_eq!(runs[0].counters.found, 2);
    assert_eq!(runs[0].counters.filtered, 1);
    assert_eq!(runs[0].counters.analyzed, 1);
}

// ---------------------------------------------------------------------------
// Deterministic filters
// ---------------------------------------------------------------------------

#[tokio::test]
async fn blocked_sender_is_filtered_with_reason() {
    let account = account_with_inbox(1, "student@example.edu");
    let settings = Settings {
        blocked_senders: vec!["spam@example.com".to_string()],
        ..Settings::default()
    };
    let store = Arc::new(TestStore::new(vec![account], empty_catalog(), settings));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "spam@example.com",
                "Offer",
                "buy now",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    let status = run_single_account(&store, providers, vec![]).await;

    assert_eq!(status, RunStatus::Success);
    let messages = store.processed_messages();
    assert_eq!(messages[0].outcome, MessageOutcome::Filtered);
    assert_eq!(messages[0].reason.as_deref(), Some("blocked_sender"));
}

#[tokio::test]
async fn blocked_domain_is_filtered_with_reason() {
    let account = account_with_inbox(1, "student@example.edu");
    let settings = Settings {
        blocked_senders: vec!["@marketing.com".to_string()],
        ..Settings::default()
    };
    let store = Arc::new(TestStore::new(vec![account], empty_catalog(), settings));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "noreply@marketing.com",
                "Newsletter",
                "body",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    let status = run_single_account(&store, providers, vec![]).await;

    assert_eq!(status, RunStatus::Success);
    let messages = store.processed_messages();
    assert_eq!(messages[0].reason.as_deref(), Some("blocked_domain"));
}

#[tokio::test]
async fn own_message_is_filtered_with_reason() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "student@example.edu",
                "Note to self",
                "body",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    let status = run_single_account(&store, providers, vec![]).await;

    assert_eq!(status, RunStatus::Success);
    let messages = store.processed_messages();
    assert_eq!(messages[0].reason.as_deref(), Some("own_message"));
}

#[tokio::test]
async fn empty_subject_and_body_is_filtered_with_reason() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "   ",
                "   ",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    let status = run_single_account(&store, providers, vec![]).await;

    assert_eq!(status, RunStatus::Success);
    let messages = store.processed_messages();
    assert_eq!(messages[0].reason.as_deref(), Some("empty"));
}

// ---------------------------------------------------------------------------
// Analysis outcomes
// ---------------------------------------------------------------------------

#[tokio::test]
async fn irrelevant_email_is_stored_as_irrelevant_without_a_result() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "FYI",
                "Not academic content.",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    let status = run_single_account(&store, providers, vec![Ok(irrelevant_answers())]).await;

    assert_eq!(status, RunStatus::Success);
    assert!(store.results().is_empty());
    let messages = store.processed_messages();
    assert_eq!(messages[0].outcome, MessageOutcome::Irrelevant);
    assert_eq!(messages[0].reason, None);

    let runs = store.runs();
    assert_eq!(runs[0].counters.analyzed, 1);
    assert_eq!(runs[0].counters.results_created, 0);
}

#[tokio::test]
async fn invalid_response_is_recorded_failed_and_the_folder_continues() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![
                simple_email(
                    1,
                    DEFAULT_IMAP_FOLDER,
                    "1",
                    "prof@example.edu",
                    "First",
                    "body one",
                ),
                simple_email(
                    1,
                    DEFAULT_IMAP_FOLDER,
                    "2",
                    "prof@example.edu",
                    "Second",
                    "body two",
                ),
            ],
            next_cursor: imap_cursor(2),
        })],
    );

    let status = run_single_account(
        &store,
        providers,
        vec![
            Err(support::FakeModelError::InvalidResponse(
                "missing answer for question 'priority'".to_string(),
            )),
            Ok(relevant_answers()),
        ],
    )
    .await;

    assert_eq!(
        status,
        RunStatus::Success,
        "InvalidResponse must not fail the account"
    );

    let messages = store.processed_messages();
    let failed = messages
        .iter()
        .find(|m| m.provider_message_id == "1")
        .unwrap();
    assert_eq!(failed.outcome, MessageOutcome::Failed);
    assert_eq!(
        failed.reason.as_deref(),
        Some("missing answer for question 'priority'")
    );

    assert_eq!(
        store.results().len(),
        1,
        "the second message is still analyzed"
    );

    let runs = store.runs();
    assert_eq!(runs[0].counters.failed, 1);
    assert_eq!(runs[0].counters.analyzed, 2);

    // The folder completed (it was not stopped), so the cursor advances.
    assert_eq!(store.sync_successes().len(), 1);
}

#[tokio::test]
async fn transient_ai_error_stops_the_account_but_other_accounts_still_run() {
    let account1 = account_with_inbox(1, "student1@example.edu");
    let account2 = account_with_inbox(2, "student2@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account1, account2],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "Exam",
                "body",
            )],
            next_cursor: imap_cursor(1),
        })],
    );
    providers.queue_responses(
        2,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                2,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "Exam",
                "body",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    // Processing order is account 1 then account 2: the model queue is consumed
    // in that order.
    let model = Arc::new(FakeStructuredModel::new(vec![
        Err(support::FakeModelError::Auth),
        Ok(relevant_answers()),
    ]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer,
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    let status = runner.execute(prepared).await;

    assert_eq!(status, RunStatus::Partial);

    let runs = store.runs();
    assert_eq!(
        runs[0].error_summary.as_deref(),
        Some("1 of 2 accounts failed")
    );

    let account_results = store.run_account_calls();
    let account1_result = account_results
        .iter()
        .find(|(_, r)| r.account_id == 1)
        .unwrap()
        .1
        .clone();
    assert_eq!(
        account1_result.status,
        academic_core::domain::RunAccountStatus::Failed
    );
    assert_eq!(account1_result.error_kind, Some(ErrorKind::Ai));

    let account2_result = account_results
        .iter()
        .find(|(_, r)| r.account_id == 2)
        .unwrap()
        .1
        .clone();
    assert_eq!(
        account2_result.status,
        academic_core::domain::RunAccountStatus::Success
    );

    // Account 1's cursor was not advanced (the folder was stopped).
    assert!(store
        .sync_state(1, DEFAULT_IMAP_FOLDER)
        .unwrap()
        .cursor
        .is_none());
    assert_eq!(store.sync_failures().len(), 1);
    assert_eq!(store.sync_failures()[0].0, 1);

    // Account 2 completed normally.
    assert!(store
        .sync_state(2, DEFAULT_IMAP_FOLDER)
        .unwrap()
        .cursor
        .is_some());
}

// ---------------------------------------------------------------------------
// Provider / account failures
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provider_creation_error_fails_the_account_with_its_error_kind() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.fail_creation_for(1, FakeProviderError::Auth("bad credentials".to_string()));

    let model = Arc::new(FakeStructuredModel::new(vec![]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer,
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    let status = runner.execute(prepared).await;

    assert_eq!(status, RunStatus::Failed);
    let account_results = store.run_account_calls();
    assert_eq!(account_results.len(), 1);
    assert_eq!(account_results[0].1.error_kind, Some(ErrorKind::Auth));
}

#[tokio::test]
async fn provider_creation_error_maps_protocol_errors_too() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.fail_creation_for(
        1,
        FakeProviderError::Protocol("unexpected greeting".to_string()),
    );

    let status = run_single_account(&store, providers, vec![]).await;

    assert_eq!(status, RunStatus::Failed);
    assert_eq!(
        store.run_account_calls()[0].1.error_kind,
        Some(ErrorKind::Protocol)
    );
}

#[tokio::test]
async fn fetch_new_error_fails_the_folder_but_the_next_folder_still_runs() {
    let account = account(1, "student@example.edu", &["INBOX", "Archive"]);
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![
            Err(FakeProviderError::Connection("timeout".to_string())),
            Ok(academic_core::domain::FetchBatch {
                emails: vec![],
                next_cursor: imap_cursor(1),
            }),
        ],
    );

    let status = run_single_account(&store, providers, vec![]).await;

    assert_eq!(
        status,
        RunStatus::Failed,
        "the account had one failing folder"
    );
    assert_eq!(
        store.run_account_calls()[0].1.error_kind,
        Some(ErrorKind::Connection)
    );

    let failures = store.sync_failures();
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].1, "INBOX");

    // The second folder was still attempted and completed successfully.
    let successes = store.sync_successes();
    assert_eq!(successes.len(), 1);
    assert_eq!(successes[0].1, "Archive");
}

#[tokio::test]
async fn catalog_load_failure_fails_every_enabled_account() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));
    store.fail_catalog_load();

    let providers = Arc::new(FakeEmailProviderFactory::new());
    let status = run_single_account(&store, providers, vec![]).await;

    assert_eq!(status, RunStatus::Failed);
    let account_results = store.run_account_calls();
    assert_eq!(account_results.len(), 1);
    assert_eq!(account_results[0].1.error_kind, Some(ErrorKind::Internal));
    assert!(store.results().is_empty());
}

#[tokio::test]
async fn fetch_uses_the_stored_cursor_and_the_initial_sync_window() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));
    store.seed_sync_state(1, DEFAULT_IMAP_FOLDER, imap_cursor(50));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![],
            next_cursor: imap_cursor(51),
        })],
    );

    let status = run_single_account(&store, providers.clone(), vec![]).await;

    assert_eq!(status, RunStatus::Success);
    let calls = providers.fetch_calls(1);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].folder, DEFAULT_IMAP_FOLDER);
    assert_eq!(calls[0].cursor, Some(imap_cursor(50)));
    assert_eq!(calls[0].since, base_time() - chrono::Duration::days(14));
}

#[tokio::test]
async fn rate_limited_model_error_also_stops_the_account() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "Exam",
                "body",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    let status = run_single_account(
        &store,
        providers,
        vec![Err(support::FakeModelError::RateLimited)],
    )
    .await;

    assert_eq!(status, RunStatus::Failed);
    assert_eq!(
        store.run_account_calls()[0].1.error_kind,
        Some(ErrorKind::Ai)
    );
    assert!(store
        .sync_state(1, DEFAULT_IMAP_FOLDER)
        .unwrap()
        .cursor
        .is_none());
}

#[tokio::test]
async fn unavailable_model_error_also_stops_the_account() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "Exam",
                "body",
            )],
            next_cursor: imap_cursor(1),
        })],
    );

    let status = run_single_account(
        &store,
        providers,
        vec![Err(support::FakeModelError::Unavailable(
            "ollama down".to_string(),
        ))],
    )
    .await;

    assert_eq!(status, RunStatus::Failed);
    assert_eq!(
        store.run_account_calls()[0].1.error_kind,
        Some(ErrorKind::Ai)
    );
    assert!(store
        .sync_state(1, DEFAULT_IMAP_FOLDER)
        .unwrap()
        .cursor
        .is_none());
}

// ---------------------------------------------------------------------------
// Preconditions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn no_enabled_accounts_is_rejected_without_creating_a_run() {
    let mut account = account_with_inbox(1, "student@example.edu");
    account.enabled = false;
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    let model = Arc::new(FakeStructuredModel::new(vec![]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer,
        clock,
    );

    let error = runner.prepare(RunTrigger::Manual).err().unwrap();

    assert_eq!(error, academic_core::domain::RunError::NoEnabledAccounts);
    assert!(store.runs().is_empty(), "no run row must be created");
    assert_eq!(runner.running_run(), None, "the lock must be released");
}

#[tokio::test]
async fn ai_not_configured_is_rejected_without_creating_a_run() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::failing(
            "Add your jev API key in Settings",
        )),
        observer,
        clock,
    );

    let error = runner.prepare(RunTrigger::Manual).err().unwrap();

    assert_eq!(
        error,
        academic_core::domain::RunError::AiNotConfigured(
            "Add your jev API key in Settings".to_string()
        )
    );
    assert!(store.runs().is_empty());
    assert_eq!(runner.running_run(), None);
}

#[tokio::test]
async fn lock_is_released_after_a_failed_prepare() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::failing("no key")),
        observer,
        clock,
    );

    // Fails twice with the *same* precondition error, never `AlreadyRunning`,
    // proving the lock was released after the first failure.
    assert!(matches!(
        runner.prepare(RunTrigger::Manual),
        Err(academic_core::domain::RunError::AiNotConfigured(_))
    ));
    assert!(matches!(
        runner.prepare(RunTrigger::Manual),
        Err(academic_core::domain::RunError::AiNotConfigured(_))
    ));
}

#[tokio::test]
async fn lock_is_released_after_execute_completes() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![],
            next_cursor: imap_cursor(1),
        })],
    );
    let model = Arc::new(FakeStructuredModel::new(vec![]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer,
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    runner.execute(prepared).await;

    assert_eq!(runner.running_run(), None);
    assert!(runner.prepare(RunTrigger::Manual).is_ok());
}

#[tokio::test]
async fn already_running_while_a_run_is_executing_then_released_after() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let started = Arc::new(Notify::new());
    let proceed = Arc::new(Notify::new());
    let providers: Arc<dyn EmailProviderFactory> = Arc::new(BlockingProviderFactory {
        started: started.clone(),
        proceed: proceed.clone(),
    });

    let model = Arc::new(FakeStructuredModel::new(vec![]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = Arc::new(CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer,
        clock,
    ));

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    let run_id = prepared.run_id;

    let runner_for_task = runner.clone();
    let handle = tokio::spawn(async move { runner_for_task.execute(prepared).await });

    started.notified().await;

    assert_eq!(runner.running_run(), Some(run_id));
    assert_eq!(
        runner.prepare(RunTrigger::Manual).err().unwrap(),
        academic_core::domain::RunError::AlreadyRunning
    );

    proceed.notify_one();
    let status = handle.await.expect("execute task completes");

    assert_eq!(status, RunStatus::Success);
    assert_eq!(runner.running_run(), None);
    assert!(runner.prepare(RunTrigger::Manual).is_ok());
}

// ---------------------------------------------------------------------------
// Progress
// ---------------------------------------------------------------------------

#[tokio::test]
async fn progress_sequence_includes_every_stage_in_order() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![simple_email(
                1,
                DEFAULT_IMAP_FOLDER,
                "1",
                "prof@example.edu",
                "Exam",
                "body",
            )],
            next_cursor: imap_cursor(1),
        })],
    );
    let model = Arc::new(FakeStructuredModel::new(vec![Ok(relevant_answers())]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer.clone(),
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    runner.execute(prepared).await;

    let stages: Vec<_> = observer
        .progress_calls()
        .into_iter()
        .map(|p| p.stage)
        .collect();
    use academic_core::domain::RunStage::*;
    assert_eq!(stages, vec![Connecting, Fetching, Analyzing, Done]);
}

// ---------------------------------------------------------------------------
// Dates use the local offset
// ---------------------------------------------------------------------------

#[tokio::test]
async fn date_candidates_are_resolved_against_the_local_offset() {
    let account = account_with_inbox(1, "student@example.edu");
    let store = Arc::new(TestStore::new(
        vec![account],
        empty_catalog(),
        Settings::default(),
    ));

    // 23:30 UTC on the 28th is already 01:30 on the 29th at UTC+2: "mañana"
    // (tomorrow) must resolve against the *local* date (29th), landing on the
    // 30th, not the 29th a naive UTC-based resolution would produce.
    let received_at: DateTime<Utc> = Utc.with_ymd_and_hms(2026, 9, 28, 23, 30, 0).unwrap();
    let mut email = simple_email(
        1,
        DEFAULT_IMAP_FOLDER,
        "1",
        "prof@example.edu",
        "Reminder",
        "Nos vemos mañana en clase.",
    );
    email.received_at = received_at;

    let providers = Arc::new(FakeEmailProviderFactory::new());
    providers.queue_responses(
        1,
        vec![Ok(academic_core::domain::FetchBatch {
            emails: vec![email],
            next_cursor: imap_cursor(1),
        })],
    );

    let mut answers = relevant_answers();
    answers.insert(
        "date_0".to_string(),
        Answer::Choice {
            choice: "exam".to_string(),
            confidence: Some(0.9),
        },
    );
    let model = Arc::new(FakeStructuredModel::new(vec![Ok(answers)]));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(
        base_time(),
        FixedOffset::east_opt(2 * 3600).unwrap(),
    ));
    let runner = CheckRunner::new(
        repos(&store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer,
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    let status = runner.execute(prepared).await;

    assert_eq!(status, RunStatus::Success);
    let results = store.results();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].dates.len(), 1);
    assert_eq!(
        results[0].dates[0].date,
        NaiveDate::from_ymd_opt(2026, 9, 30).unwrap()
    );
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Runs a single-account check with `model_responses` queued on the model, and
/// returns the final [`RunStatus`]. The account/provider/observer/clock are
/// already set up on `store`/`providers`; this only wires the model and drives
/// `prepare`/`execute`.
async fn run_single_account(
    store: &Arc<TestStore>,
    providers: Arc<FakeEmailProviderFactory>,
    model_responses: Vec<Result<academic_core::ports::AnswerSet, support::FakeModelError>>,
) -> RunStatus {
    let model = Arc::new(FakeStructuredModel::new(model_responses));
    let observer = Arc::new(FakeObserver::new());
    let clock = Arc::new(FakeClock::new(base_time(), utc_offset()));
    let runner = CheckRunner::new(
        repos(store),
        providers,
        Arc::new(FakeModelProvider::new(model)),
        observer,
        clock,
    );

    let prepared = runner
        .prepare(RunTrigger::Manual)
        .expect("prepare succeeds");
    runner.execute(prepared).await
}

struct BlockingProvider {
    started: Arc<Notify>,
    proceed: Arc<Notify>,
}

#[async_trait]
impl EmailProvider for BlockingProvider {
    async fn test_connection(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn fetch_new(
        &self,
        _folder: &str,
        _cursor: Option<&SyncCursor>,
        _since: DateTime<Utc>,
    ) -> Result<academic_core::domain::FetchBatch, ProviderError> {
        self.started.notify_one();
        self.proceed.notified().await;
        Ok(academic_core::domain::FetchBatch {
            emails: vec![],
            next_cursor: imap_cursor(1),
        })
    }
}

struct BlockingProviderFactory {
    started: Arc<Notify>,
    proceed: Arc<Notify>,
}

impl EmailProviderFactory for BlockingProviderFactory {
    fn create(
        &self,
        _account: &academic_core::domain::Account,
    ) -> Result<Box<dyn EmailProvider>, ProviderError> {
        Ok(Box::new(BlockingProvider {
            started: self.started.clone(),
            proceed: self.proceed.clone(),
        }))
    }
}
