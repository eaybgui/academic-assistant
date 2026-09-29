//! Account status derived from secrets and sync state: whether credentials are
//! configured, the aggregated last sync, and the dashboard "needs attention" list.

use academic_core::domain::{
    account_password_key, account_refresh_token_key, Account, ProviderConfig, SyncStatus,
};
use academic_core::ports::{SecretError, SecretStore};

use crate::dto::{AccountDto, AttentionDto, LastSyncDto};
use crate::error::CommandError;
use crate::state::AppState;

const PASSWORD_MISSING: &str = "The password is missing. Edit the account to set it";
const SIGN_IN_REQUIRED: &str = "Sign in to this account";
const SYNC_FAILED: &str = "The last sync failed";

/// True when the secret the account needs is stored: the IMAP password or the
/// Microsoft refresh token.
pub fn credentials_configured(
    secrets: &dyn SecretStore,
    account: &Account,
) -> Result<bool, SecretError> {
    let key = match account.provider {
        ProviderConfig::Imap { .. } => account_password_key(account.id),
        ProviderConfig::MicrosoftGraph { .. } => account_refresh_token_key(account.id),
    };
    Ok(secrets.get(&key)?.is_some())
}

pub fn account_dto(state: &AppState, account: &Account) -> Result<AccountDto, CommandError> {
    let configured = credentials_configured(state.secrets.as_ref(), account)?;
    let last_sync = LastSyncDto::from_states(&state.sync.list_for_account(account.id)?);
    Ok(AccountDto::new(account, configured, last_sync))
}

/// Enabled accounts that need the user's attention, one entry per account. A
/// failed sync wins over missing credentials.
pub fn attention(state: &AppState) -> Result<Vec<AttentionDto>, CommandError> {
    let mut entries = Vec::new();
    for account in state.accounts.list()?.iter().filter(|a| a.enabled) {
        if let Some(message) = attention_message(state, account)? {
            entries.push(AttentionDto {
                account_id: account.id,
                account_name: account.name.clone(),
                message,
            });
        }
    }
    Ok(entries)
}

fn attention_message(state: &AppState, account: &Account) -> Result<Option<String>, CommandError> {
    let sync_error = state
        .sync
        .list_for_account(account.id)?
        .into_iter()
        .find(|sync| sync.status == SyncStatus::Error);
    if let Some(sync) = sync_error {
        return Ok(Some(sync.last_error.unwrap_or_else(|| SYNC_FAILED.into())));
    }
    if credentials_configured(state.secrets.as_ref(), account)? {
        return Ok(None);
    }
    Ok(Some(
        match account.provider {
            ProviderConfig::Imap { .. } => PASSWORD_MISSING,
            ProviderConfig::MicrosoftGraph { .. } => SIGN_IN_REQUIRED,
        }
        .to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{graph_draft, harness, imap_draft, Harness};
    use academic_core::domain::{SyncCursor, SyncStatus};
    use chrono::Utc;

    fn add(harness: &Harness, draft: &academic_core::domain::AccountDraft) -> Account {
        harness.state.accounts.insert(draft, Utc::now()).unwrap()
    }

    fn cursor() -> SyncCursor {
        SyncCursor::Imap {
            uidvalidity: 1,
            last_uid: 1,
        }
    }

    #[test]
    fn credentials_follow_the_provider_kind() {
        let h = harness();
        let imap = add(&h, &imap_draft("IMAP"));
        let graph = add(&h, &graph_draft("Graph"));
        let secrets = h.state.secrets.as_ref();
        assert!(!credentials_configured(secrets, &imap).unwrap());
        assert!(!credentials_configured(secrets, &graph).unwrap());

        secrets
            .set(&account_password_key(imap.id), "test-pw")
            .unwrap();
        secrets
            .set(&account_refresh_token_key(graph.id), "test-token")
            .unwrap();
        assert!(credentials_configured(secrets, &imap).unwrap());
        assert!(credentials_configured(secrets, &graph).unwrap());
    }

    #[test]
    fn account_dto_aggregates_sync_state_and_credentials() {
        let h = harness();
        let account = add(&h, &imap_draft("IMAP"));
        h.secrets
            .set(&account_password_key(account.id), "test-pw")
            .unwrap();
        h.state
            .sync
            .record_success(account.id, "INBOX", &cursor(), Utc::now())
            .unwrap();

        let dto = account_dto(&h.state, &account).unwrap();
        assert!(dto.credentials_configured);
        assert_eq!(dto.last_sync.status, SyncStatus::Ok);
        assert!(dto.last_sync.at.is_some());
    }

    #[test]
    fn attention_lists_missing_credentials_per_provider() {
        let h = harness();
        let imap = add(&h, &imap_draft("IMAP"));
        let graph = add(&h, &graph_draft("Graph"));

        let entries = attention(&h.state).unwrap();
        assert_eq!(
            entries,
            vec![
                AttentionDto {
                    account_id: imap.id,
                    account_name: "IMAP".into(),
                    message: PASSWORD_MISSING.into()
                },
                AttentionDto {
                    account_id: graph.id,
                    account_name: "Graph".into(),
                    message: SIGN_IN_REQUIRED.into()
                },
            ]
        );
    }

    #[test]
    fn sync_error_wins_over_missing_credentials_and_yields_one_entry() {
        let h = harness();
        let account = add(&h, &imap_draft("IMAP"));
        h.state
            .sync
            .record_failure(
                account.id,
                "INBOX",
                "Connection failed: timeout",
                Utc::now(),
            )
            .unwrap();

        let entries = attention(&h.state).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message, "Connection failed: timeout");
    }

    #[test]
    fn healthy_and_disabled_accounts_need_no_attention() {
        let h = harness();
        let healthy = add(&h, &imap_draft("Healthy"));
        h.secrets
            .set(&account_password_key(healthy.id), "test-pw")
            .unwrap();
        let mut disabled = imap_draft("Disabled");
        disabled.enabled = false;
        add(&h, &disabled);

        assert!(attention(&h.state).unwrap().is_empty());
    }
}
