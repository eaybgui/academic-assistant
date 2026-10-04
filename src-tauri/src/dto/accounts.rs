//! Account DTOs: the account view, the add/edit input and connection test results.

use academic_core::domain::{Account, ErrorKind, ProviderConfig, SyncState, SyncStatus};
use academic_core::ports::ProviderError;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderDto {
    Imap {
        host: String,
        port: u16,
        username: String,
    },
    MicrosoftGraph {
        tenant: String,
    },
}

impl From<&ProviderConfig> for ProviderDto {
    fn from(provider: &ProviderConfig) -> Self {
        match provider {
            ProviderConfig::Imap {
                host,
                port,
                username,
            } => Self::Imap {
                host: host.clone(),
                port: *port,
                username: username.clone(),
            },
            ProviderConfig::MicrosoftGraph { tenant } => Self::MicrosoftGraph {
                tenant: tenant.clone(),
            },
        }
    }
}

/// Aggregate of an account's per-folder sync states.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LastSyncDto {
    pub at: Option<DateTime<Utc>>,
    pub status: SyncStatus,
    pub error: Option<String>,
}

impl LastSyncDto {
    /// `at` is the latest `last_run_at`; `status` is `error` if any folder errored,
    /// else `ok` if any folder succeeded, else `never`; `error` is the first
    /// folder error message.
    pub fn from_states(states: &[SyncState]) -> Self {
        let at = states.iter().filter_map(|state| state.last_run_at).max();
        let first_error = states
            .iter()
            .find(|state| state.status == SyncStatus::Error);
        let status = if first_error.is_some() {
            SyncStatus::Error
        } else if states.iter().any(|state| state.status == SyncStatus::Ok) {
            SyncStatus::Ok
        } else {
            SyncStatus::Never
        };
        Self {
            at,
            status,
            error: first_error.and_then(|state| state.last_error.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub id: i64,
    pub name: String,
    pub email_address: String,
    pub provider: ProviderDto,
    pub folders: Vec<String>,
    pub default_university_id: Option<i64>,
    pub initial_sync_days: u32,
    pub enabled: bool,
    pub credentials_configured: bool,
    pub last_sync: LastSyncDto,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AccountDto {
    pub fn new(account: &Account, credentials_configured: bool, last_sync: LastSyncDto) -> Self {
        Self {
            id: account.id,
            name: account.name.clone(),
            email_address: account.email_address.clone(),
            provider: (&account.provider).into(),
            folders: account.folders.clone(),
            default_university_id: account.default_university_id,
            initial_sync_days: account.initial_sync_days,
            enabled: account.enabled,
            credentials_configured,
            last_sync,
            created_at: account.created_at,
            updated_at: account.updated_at,
        }
    }
}

/// Provider part of the add/edit input. `port` is signed so an out-of-range value
/// is reported as a validation error instead of an opaque deserialization failure.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderInput {
    Imap {
        host: String,
        port: i64,
        username: String,
        password: Option<String>,
    },
    MicrosoftGraph {
        tenant: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInput {
    pub name: String,
    pub email_address: String,
    pub folders: Vec<String>,
    pub default_university_id: Option<i64>,
    pub initial_sync_days: i64,
    pub enabled: bool,
    pub provider: ProviderInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionTestDto {
    pub ok: bool,
    pub error_kind: Option<ErrorKind>,
    pub message: Option<String>,
}

impl ConnectionTestDto {
    pub fn success() -> Self {
        Self {
            ok: true,
            error_kind: None,
            message: None,
        }
    }

    pub fn failure(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            error_kind: Some(kind),
            message: Some(message.into()),
        }
    }
}

impl From<Result<(), ProviderError>> for ConnectionTestDto {
    fn from(result: Result<(), ProviderError>) -> Self {
        match result {
            Ok(()) => Self::success(),
            Err(error) => Self::failure(error.kind(), error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn at(hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 28, hour, 0, 0).unwrap()
    }

    fn state(status: SyncStatus, last_run: Option<u32>, error: Option<&str>) -> SyncState {
        SyncState {
            account_id: 1,
            folder: "INBOX".into(),
            cursor: None,
            last_run_at: last_run.map(at),
            last_success_at: None,
            status,
            last_error: error.map(str::to_string),
        }
    }

    #[test]
    fn last_sync_without_states_is_never() {
        assert_eq!(
            LastSyncDto::from_states(&[]),
            LastSyncDto {
                at: None,
                status: SyncStatus::Never,
                error: None
            }
        );
    }

    #[test]
    fn last_sync_is_ok_when_any_folder_succeeded() {
        let states = [
            state(SyncStatus::Never, None, None),
            state(SyncStatus::Ok, Some(8), None),
            state(SyncStatus::Ok, Some(10), None),
        ];
        let sync = LastSyncDto::from_states(&states);
        assert_eq!(sync.status, SyncStatus::Ok);
        assert_eq!(sync.at, Some(at(10)));
        assert_eq!(sync.error, None);
    }

    #[test]
    fn last_sync_reports_the_first_error_and_wins_over_ok() {
        let states = [
            state(SyncStatus::Ok, Some(9), None),
            state(SyncStatus::Error, Some(7), Some("first")),
            state(SyncStatus::Error, Some(8), Some("second")),
        ];
        let sync = LastSyncDto::from_states(&states);
        assert_eq!(sync.status, SyncStatus::Error);
        assert_eq!(sync.error.as_deref(), Some("first"));
        assert_eq!(sync.at, Some(at(9)));
    }

    #[test]
    fn imap_input_parses_without_password() {
        let input: AccountInput = serde_json::from_value(json!({
            "name": "UV",
            "emailAddress": "me@uv.es",
            "folders": ["INBOX"],
            "defaultUniversityId": null,
            "initialSyncDays": 14,
            "enabled": true,
            "provider": { "type": "imap", "host": "imap.uv.es", "port": 993, "username": "me" }
        }))
        .unwrap();
        assert_eq!(
            input.provider,
            ProviderInput::Imap {
                host: "imap.uv.es".into(),
                port: 993,
                username: "me".into(),
                password: None,
            }
        );
        assert_eq!(input.default_university_id, None);
    }

    #[test]
    fn imap_input_parses_with_password_and_university() {
        let input: AccountInput = serde_json::from_value(json!({
            "name": "UV",
            "emailAddress": "me@uv.es",
            "folders": ["INBOX", "Courses"],
            "defaultUniversityId": 3,
            "initialSyncDays": 30,
            "enabled": false,
            "provider": {
                "type": "imap", "host": "imap.uv.es", "port": 993,
                "username": "me", "password": "test-password"
            }
        }))
        .unwrap();
        assert_eq!(input.default_university_id, Some(3));
        assert_eq!(input.initial_sync_days, 30);
        assert!(!input.enabled);
        assert!(matches!(
            input.provider,
            ProviderInput::Imap { password: Some(ref p), .. } if p == "test-password"
        ));
    }

    #[test]
    fn graph_input_parses() {
        let input: AccountInput = serde_json::from_value(json!({
            "name": "UAH",
            "emailAddress": "me@uah.es",
            "folders": ["inbox"],
            "defaultUniversityId": 2,
            "initialSyncDays": 14,
            "enabled": true,
            "provider": { "type": "microsoft_graph", "tenant": "organizations" }
        }))
        .unwrap();
        assert_eq!(
            input.provider,
            ProviderInput::MicrosoftGraph {
                tenant: "organizations".into()
            }
        );
    }

    #[test]
    fn account_dto_matches_the_typescript_shape() {
        let account = Account {
            id: 5,
            name: "UV".into(),
            email_address: "me@uv.es".into(),
            provider: ProviderConfig::Imap {
                host: "imap.uv.es".into(),
                port: 993,
                username: "me".into(),
            },
            folders: vec!["INBOX".into()],
            default_university_id: None,
            initial_sync_days: 14,
            enabled: true,
            created_at: at(8),
            updated_at: at(9),
        };
        let dto = AccountDto::new(
            &account,
            true,
            LastSyncDto {
                at: Some(at(10)),
                status: SyncStatus::Error,
                error: Some("boom".into()),
            },
        );
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            json!({
                "id": 5,
                "name": "UV",
                "emailAddress": "me@uv.es",
                "provider": { "type": "imap", "host": "imap.uv.es", "port": 993, "username": "me" },
                "folders": ["INBOX"],
                "defaultUniversityId": null,
                "initialSyncDays": 14,
                "enabled": true,
                "credentialsConfigured": true,
                "lastSync": { "at": "2026-09-28T10:00:00Z", "status": "error", "error": "boom" },
                "createdAt": "2026-09-28T08:00:00Z",
                "updatedAt": "2026-09-28T09:00:00Z"
            })
        );
    }

    #[test]
    fn graph_provider_dto_is_tagged_microsoft_graph() {
        let dto = ProviderDto::from(&ProviderConfig::MicrosoftGraph {
            tenant: "organizations".into(),
        });
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            json!({ "type": "microsoft_graph", "tenant": "organizations" })
        );
    }

    #[test]
    fn connection_test_dto_shapes() {
        assert_eq!(
            serde_json::to_value(ConnectionTestDto::success()).unwrap(),
            json!({ "ok": true, "errorKind": null, "message": null })
        );
        let failed = ConnectionTestDto::from(Err(ProviderError::Connection("timeout".into())));
        assert_eq!(
            serde_json::to_value(failed).unwrap(),
            json!({
                "ok": false,
                "errorKind": "connection",
                "message": "Connection failed: timeout"
            })
        );
    }

    #[test]
    fn connection_test_maps_protocol_errors_to_protocol_kind() {
        let dto = ConnectionTestDto::from(Err(ProviderError::Protocol("odd".into())));
        assert_eq!(dto.error_kind, Some(ErrorKind::Protocol));
    }
}
