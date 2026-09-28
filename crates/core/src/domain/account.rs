use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::enum_str;

pub type AccountId = i64;

/// Default OAuth tenant used for Microsoft Graph accounts when none is chosen.
pub const DEFAULT_GRAPH_TENANT: &str = "organizations";
/// Default folder name for IMAP accounts.
pub const DEFAULT_IMAP_FOLDER: &str = "INBOX";
/// Default folder name for Microsoft Graph accounts.
pub const DEFAULT_GRAPH_FOLDER: &str = "inbox";
/// Default number of days of history to fetch on an account's first sync.
pub const DEFAULT_INITIAL_SYNC_DAYS: u32 = 14;

/// Provider-specific connection configuration for an [`Account`]. Stored as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderConfig {
    Imap {
        host: String,
        port: u16,
        username: String,
    },
    MicrosoftGraph {
        tenant: String,
    },
}

impl ProviderConfig {
    pub fn kind(&self) -> ProviderKind {
        match self {
            ProviderConfig::Imap { .. } => ProviderKind::Imap,
            ProviderConfig::MicrosoftGraph { .. } => ProviderKind::MicrosoftGraph,
        }
    }
}

/// Discriminator for [`ProviderConfig`], stored separately in the database so accounts
/// can be filtered/queried by provider without deserializing the full JSON config.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    Imap,
    MicrosoftGraph,
}

enum_str!(ProviderKind {
    Imap => "imap",
    MicrosoftGraph => "microsoft_graph",
});

/// A configured mailbox that the pipeline syncs and analyzes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: AccountId,
    pub name: String,
    pub email_address: String,
    pub provider: ProviderConfig,
    pub folders: Vec<String>,
    pub default_university_id: Option<i64>,
    pub initial_sync_days: u32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Input for creating or updating an [`Account`] (no id/timestamps: those are
/// assigned by the repository).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountDraft {
    pub name: String,
    pub email_address: String,
    pub provider: ProviderConfig,
    pub folders: Vec<String>,
    pub default_university_id: Option<i64>,
    pub initial_sync_days: u32,
    pub enabled: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn provider_kind_round_trips() {
        for kind in [ProviderKind::Imap, ProviderKind::MicrosoftGraph] {
            assert_eq!(ProviderKind::from_str(kind.as_str()).unwrap(), kind);
        }
        assert_eq!(ProviderKind::Imap.as_str(), "imap");
        assert_eq!(ProviderKind::MicrosoftGraph.as_str(), "microsoft_graph");
    }

    #[test]
    fn provider_kind_rejects_unknown_value() {
        assert!(ProviderKind::from_str("bogus").is_err());
    }

    #[test]
    fn provider_config_kind_matches_variant() {
        let imap = ProviderConfig::Imap {
            host: "imap.example.com".into(),
            port: 993,
            username: "user@example.com".into(),
        };
        assert_eq!(imap.kind(), ProviderKind::Imap);

        let graph = ProviderConfig::MicrosoftGraph {
            tenant: DEFAULT_GRAPH_TENANT.into(),
        };
        assert_eq!(graph.kind(), ProviderKind::MicrosoftGraph);
    }
}
