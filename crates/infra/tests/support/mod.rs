//! Shared fixtures for the `db_*` integration test suite. Included per test binary
//! via `#[path = "support/mod.rs"] mod support;` (a `tests/` subdirectory is not
//! auto-discovered by cargo as its own test target, so this file is not itself a
//! test binary).
#![allow(dead_code)]

use academic_core::domain::{
    AIAnalysis, Account, AccountDraft, Category, MessageOutcome, ModelDescriptor, NewResult,
    Priority, ProcessedMessage, ProviderConfig, ResultDate, RunId, RunTrigger,
};
use academic_core::ports::{AccountRepository, RunRepository};
use academic_infra::db::Database;
use chrono::{DateTime, Utc};

/// An `AccountDraft` for an enabled IMAP account, with a name-derived unique email.
pub fn imap_draft(name: &str) -> AccountDraft {
    AccountDraft {
        name: name.to_string(),
        email_address: format!("{name}@example.test"),
        provider: ProviderConfig::Imap {
            host: "imap.example.test".to_string(),
            port: 993,
            username: format!("{name}@example.test"),
        },
        folders: vec!["INBOX".to_string()],
        default_university_id: None,
        initial_sync_days: 14,
        enabled: true,
    }
}

/// An `AccountDraft` for an enabled Microsoft Graph account.
pub fn graph_draft(name: &str) -> AccountDraft {
    AccountDraft {
        name: name.to_string(),
        email_address: format!("{name}@example.test"),
        provider: ProviderConfig::MicrosoftGraph {
            tenant: "organizations".to_string(),
        },
        folders: vec!["inbox".to_string()],
        default_university_id: None,
        initial_sync_days: 14,
        enabled: true,
    }
}

/// Inserts an IMAP account and returns it, for tests that only need a valid
/// `account_id` to satisfy a foreign key.
pub fn insert_account(db: &Database, name: &str) -> Account {
    db.insert(&imap_draft(name), Utc::now()).unwrap()
}

/// Creates a `running` run and returns its id, for tests that only need a valid
/// `run_id` to satisfy a foreign key.
pub fn insert_run(db: &Database) -> RunId {
    db.create(RunTrigger::Manual, Utc::now()).unwrap()
}

/// A `ProcessedMessage` with outcome `Analyzed`, ready to pass to `save_analyzed`
/// alongside a matching [`new_result`].
pub fn analyzed_message(
    account_id: i64,
    run_id: RunId,
    provider_message_id: &str,
    received_at: DateTime<Utc>,
) -> ProcessedMessage {
    ProcessedMessage {
        account_id,
        folder: "INBOX".to_string(),
        provider_message_id: provider_message_id.to_string(),
        message_id_header: Some(format!("<{provider_message_id}@example.test>")),
        content_hash: format!("hash-{provider_message_id}"),
        received_at,
        outcome: MessageOutcome::Analyzed,
        reason: None,
        run_id,
        processed_at: received_at,
    }
}

/// A minimal but complete `NewResult`, overriding just the fields a test cares
/// about via the builder-style parameters.
#[allow(clippy::too_many_arguments)]
pub fn new_result(
    account_id: i64,
    run_id: RunId,
    subject: &str,
    received_at: DateTime<Utc>,
    category: Category,
    priority: Priority,
    dates: Vec<ResultDate>,
) -> NewResult {
    NewResult {
        account_id,
        run_id,
        detected_at: received_at,
        received_at,
        subject: subject.to_string(),
        sender_name: Some("Sender Name".to_string()),
        sender_email: "sender@example.test".to_string(),
        source_url: None,
        analysis: AIAnalysis {
            university_id: None,
            course_id: None,
            category,
            priority,
            action_required: false,
            possible_change: false,
            dates,
            confidence: Some(0.9),
            model: ModelDescriptor {
                provider: "ollama".to_string(),
                model: "test-model".to_string(),
            },
        },
    }
}
