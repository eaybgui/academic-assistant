use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::account::AccountId;
use super::enum_str;

/// Maximum number of emails a single [`FetchBatch`] may contain.
pub const MAX_BATCH: usize = 200;

/// A named or bare email address (a `From`/`To`/`Cc` entry).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Address {
    pub name: Option<String>,
    pub email: String,
}

/// The normalized body of an email, tagged by the format it was extracted in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmailBody {
    Text(String),
    Html(String),
}

/// An email as fetched from a provider, before filtering or analysis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Email {
    pub account_id: AccountId,
    pub folder: String,
    pub provider_message_id: String,
    pub message_id_header: Option<String>,
    pub sender: Address,
    pub recipients: Vec<Address>,
    pub subject: String,
    pub received_at: DateTime<Utc>,
    pub body: EmailBody,
    pub source_url: Option<String>,
}

/// Opaque provider-specific bookmark for incremental sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SyncCursor {
    Imap { uidvalidity: u32, last_uid: u32 },
    Graph { link: String },
}

/// A page of emails fetched from a provider, plus the cursor to resume from next time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchBatch {
    pub emails: Vec<Email>,
    pub next_cursor: SyncCursor,
}

/// Outcome of the most recent sync attempt for an account's folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStatus {
    Never,
    Ok,
    Error,
}

enum_str!(SyncStatus {
    Never => "never",
    Ok => "ok",
    Error => "error",
});

/// Per-account, per-folder sync bookkeeping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncState {
    pub account_id: AccountId,
    pub folder: String,
    pub cursor: Option<SyncCursor>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_success_at: Option<DateTime<Utc>>,
    pub status: SyncStatus,
    pub last_error: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn sync_status_round_trips() {
        for status in [SyncStatus::Never, SyncStatus::Ok, SyncStatus::Error] {
            assert_eq!(SyncStatus::from_str(status.as_str()).unwrap(), status);
        }
    }

    #[test]
    fn sync_cursor_json_round_trip_imap() {
        let cursor = SyncCursor::Imap {
            uidvalidity: 42,
            last_uid: 1000,
        };
        let json = serde_json::to_string(&cursor).unwrap();
        assert_eq!(json, r#"{"type":"imap","uidvalidity":42,"last_uid":1000}"#);
        let round_tripped: SyncCursor = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, cursor);
    }

    #[test]
    fn sync_cursor_json_round_trip_graph() {
        let cursor = SyncCursor::Graph {
            link: "https://graph.microsoft.com/delta-link".into(),
        };
        let json = serde_json::to_string(&cursor).unwrap();
        let round_tripped: SyncCursor = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, cursor);
    }
}
