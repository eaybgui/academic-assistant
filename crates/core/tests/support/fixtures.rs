use std::collections::HashMap;

use academic_core::domain::{
    Account, AccountId, Address, Email, EmailBody, ProviderConfig, DEFAULT_IMAP_FOLDER,
};
use academic_core::ports::{Answer, AnswerSet};
use chrono::{DateTime, TimeZone, Utc};

/// A fixed "now" used across tests so date-related assertions are deterministic:
/// 2026-09-28 09:00:00 UTC.
pub fn base_time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 28, 9, 0, 0).unwrap()
}

/// An enabled IMAP account with no default university (so, paired with an empty
/// [`test_catalog`], the analyzer's question set is always just the five
/// mandatory questions — see [`relevant_answers`]).
pub fn account(id: AccountId, email_address: &str, folders: &[&str]) -> Account {
    Account {
        id,
        name: format!("Account {id}"),
        email_address: email_address.to_string(),
        provider: ProviderConfig::Imap {
            host: "imap.example.edu".to_string(),
            port: 993,
            username: email_address.to_string(),
        },
        folders: folders.iter().map(|f| f.to_string()).collect(),
        default_university_id: None,
        initial_sync_days: 14,
        enabled: true,
        created_at: base_time(),
        updated_at: base_time(),
    }
}

/// [`account`] with a single `INBOX` folder.
pub fn account_with_inbox(id: AccountId, email_address: &str) -> Account {
    account(id, email_address, &[DEFAULT_IMAP_FOLDER])
}

/// An email in `folder`, identified by `provider_message_id`, from `sender`.
#[allow(clippy::too_many_arguments)]
pub fn email(
    account_id: AccountId,
    folder: &str,
    provider_message_id: &str,
    message_id_header: Option<&str>,
    sender: &str,
    subject: &str,
    body: &str,
) -> Email {
    Email {
        account_id,
        folder: folder.to_string(),
        provider_message_id: provider_message_id.to_string(),
        message_id_header: message_id_header.map(str::to_string),
        sender: Address {
            name: Some("Sender".to_string()),
            email: sender.to_string(),
        },
        recipients: vec![],
        subject: subject.to_string(),
        received_at: base_time(),
        body: EmailBody::Text(body.to_string()),
        source_url: None,
    }
}

/// [`email`] with a message id header derived from `provider_message_id`.
pub fn simple_email(
    account_id: AccountId,
    folder: &str,
    provider_message_id: &str,
    sender: &str,
    subject: &str,
    body: &str,
) -> Email {
    email(
        account_id,
        folder,
        provider_message_id,
        Some(&format!("<{provider_message_id}@example.edu>")),
        sender,
        subject,
        body,
    )
}

/// Answers to the five mandatory questions (`relevant`, `category`,
/// `action_required`, `possible_change`, `priority`) that assemble into a
/// `Relevant` outcome. Valid whenever the account has no default university and
/// the catalog is empty (no `university`/`course` questions) and the email has no
/// date candidates (no `date_i` questions) — see [`account`].
pub fn relevant_answers() -> AnswerSet {
    HashMap::from([
        (
            "relevant".to_string(),
            Answer::Noul {
                p_yes: 0.9,
                confidence: Some(0.9),
            },
        ),
        (
            "category".to_string(),
            Answer::Choice {
                choice: "exam".to_string(),
                confidence: Some(0.9),
            },
        ),
        (
            "action_required".to_string(),
            Answer::Noul {
                p_yes: 0.2,
                confidence: Some(0.9),
            },
        ),
        (
            "possible_change".to_string(),
            Answer::Noul {
                p_yes: 0.2,
                confidence: Some(0.9),
            },
        ),
        (
            "priority".to_string(),
            Answer::Score {
                level: 1,
                confidence: Some(0.9),
            },
        ),
    ])
}

/// [`relevant_answers`] with `relevant.p_yes` dropped below
/// `analyzer::RELEVANCE_THRESHOLD`, assembling into `Irrelevant`.
pub fn irrelevant_answers() -> AnswerSet {
    let mut answers = relevant_answers();
    answers.insert(
        "relevant".to_string(),
        Answer::Noul {
            p_yes: 0.1,
            confidence: Some(0.9),
        },
    );
    answers
}
