//! Commands: account CRUD, connection test and Microsoft sign-in.

use std::time::Duration;

use academic_core::domain::{
    account_password_key, account_refresh_token_key, Account, AccountId, ProviderConfig,
    ProviderKind,
};
use academic_infra::graph::oauth;
use academic_infra::graph::{signed_in_email, OAuthConfig, GRAPH_BASE_URL, LOGIN_BASE_URL};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::logged;
use crate::account_status::account_dto;
use crate::dto::{AccountDto, AccountInput, ConnectionTestDto};
use crate::error::CommandError;
use crate::state::AppState;
use crate::validation::{self, ValidatedAccount};

/// Callback that opens the sign-in URL in the system browser.
type OpenBrowser<'a> = dyn Fn(&str) -> Result<(), String> + Send + Sync + 'a;

const SIGN_IN_TIMEOUT: Duration = Duration::from_secs(300);
const PASSWORD_REQUIRED: &str = "Enter the password";
const PROVIDER_CHANGE_MESSAGE: &str = "The provider type can't be changed";
const CLIENT_ID_REQUIRED: &str = "Set the Microsoft client ID in Settings";
const NOT_MICROSOFT_MESSAGE: &str = "This account doesn't use Microsoft sign-in";

#[tauri::command]
pub async fn get_accounts(state: State<'_, AppState>) -> Result<Vec<AccountDto>, CommandError> {
    logged("get_accounts", accounts(&state))
}

#[tauri::command]
pub async fn add_account(
    state: State<'_, AppState>,
    input: AccountInput,
) -> Result<AccountDto, CommandError> {
    logged("add_account", create_account(&state, input))
}

#[tauri::command]
pub async fn update_account(
    state: State<'_, AppState>,
    id: AccountId,
    input: AccountInput,
) -> Result<AccountDto, CommandError> {
    logged("update_account", edit_account(&state, id, input))
}

#[tauri::command]
pub async fn remove_account(state: State<'_, AppState>, id: AccountId) -> Result<(), CommandError> {
    logged("remove_account", delete_account(&state, id))
}

#[tauri::command]
pub async fn test_account(
    state: State<'_, AppState>,
    id: AccountId,
) -> Result<ConnectionTestDto, CommandError> {
    logged("test_account", check_connection(&state, id).await)
}

#[tauri::command]
pub async fn connect_microsoft_account(
    app: AppHandle,
    state: State<'_, AppState>,
    id: AccountId,
) -> Result<AccountDto, CommandError> {
    let open_browser = move |url: &str| {
        app.opener()
            .open_url(url, None::<&str>)
            .map_err(|error| error.to_string())
    };
    logged(
        "connect_microsoft_account",
        sign_in_microsoft(&state, id, &open_browser).await,
    )
}

fn find_account(state: &AppState, id: AccountId) -> Result<Account, CommandError> {
    state
        .accounts
        .get(id)?
        .ok_or_else(|| CommandError::not_found("Account not found"))
}

fn accounts(state: &AppState) -> Result<Vec<AccountDto>, CommandError> {
    state
        .accounts
        .list()?
        .iter()
        .map(|account| account_dto(state, account))
        .collect()
}

/// Inserts the account, then stores the IMAP password. If storing the password
/// fails the inserted account is removed again so no half-configured account stays.
fn create_account(state: &AppState, input: AccountInput) -> Result<AccountDto, CommandError> {
    let ValidatedAccount { draft, password } = validation::account(input)?;
    if draft.provider.kind() == ProviderKind::Imap && password.is_none() {
        return Err(CommandError::validation(PASSWORD_REQUIRED));
    }

    let account = state.accounts.insert(&draft, state.clock.now())?;
    if let Some(password) = password {
        if let Err(error) = state
            .secrets
            .set(&account_password_key(account.id), &password)
        {
            if let Err(rollback) = state.accounts.delete(account.id) {
                tracing::warn!(account_id = account.id, %rollback, "could not roll back account");
            }
            return Err(error.into());
        }
    }
    account_dto(state, &account)
}

/// Updates the account. The provider type cannot change and the IMAP password is
/// replaced only when the input carries one.
fn edit_account(
    state: &AppState,
    id: AccountId,
    input: AccountInput,
) -> Result<AccountDto, CommandError> {
    let ValidatedAccount { draft, password } = validation::account(input)?;
    let existing = find_account(state, id)?;
    if existing.provider.kind() != draft.provider.kind() {
        return Err(CommandError::validation(PROVIDER_CHANGE_MESSAGE));
    }

    let account = state.accounts.update(id, &draft, state.clock.now())?;
    if let Some(password) = password {
        state.secrets.set(&account_password_key(id), &password)?;
    }
    account_dto(state, &account)
}

/// Deletes both possible secrets first (a missing secret is not an error), then
/// the account and, by cascade, all of its data.
fn delete_account(state: &AppState, id: AccountId) -> Result<(), CommandError> {
    state.secrets.delete(&account_password_key(id))?;
    state.secrets.delete(&account_refresh_token_key(id))?;
    Ok(state.accounts.delete(id)?)
}

async fn check_connection(
    state: &AppState,
    id: AccountId,
) -> Result<ConnectionTestDto, CommandError> {
    let account = find_account(state, id)?;
    let result = match state.factory.create(&account) {
        Ok(provider) => provider.test_connection().await,
        Err(error) => Err(error),
    };
    Ok(result.into())
}

/// Runs the interactive Microsoft sign-in and stores the refresh token, but only
/// when the signed-in mailbox is the account's own address.
async fn sign_in_microsoft(
    state: &AppState,
    id: AccountId,
    open_browser: &OpenBrowser<'_>,
) -> Result<AccountDto, CommandError> {
    let account = find_account(state, id)?;
    let ProviderConfig::MicrosoftGraph { tenant } = &account.provider else {
        return Err(CommandError::validation(NOT_MICROSOFT_MESSAGE));
    };
    let client_id = state
        .settings
        .load()?
        .microsoft_client_id
        .ok_or_else(|| CommandError::validation(CLIENT_ID_REQUIRED))?;
    let config = OAuthConfig {
        client_id,
        tenant: tenant.clone(),
        login_base_url: LOGIN_BASE_URL.to_string(),
    };

    let tokens = oauth::sign_in(&state.http, &config, open_browser, SIGN_IN_TIMEOUT).await?;
    let signed_in = signed_in_email(&state.http, GRAPH_BASE_URL, &tokens.access_token).await?;
    ensure_expected_email(&account.email_address, &signed_in)?;

    state
        .secrets
        .set(&account_refresh_token_key(id), &tokens.refresh_token)?;
    account_dto(state, &account)
}

fn ensure_expected_email(expected: &str, signed_in: &str) -> Result<(), CommandError> {
    if expected.eq_ignore_ascii_case(signed_in) {
        Ok(())
    } else {
        Err(CommandError::validation(format!(
            "Signed in as {signed_in}, expected {expected}"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::ProviderInput;
    use crate::error::CommandErrorKind;
    use crate::test_support::{harness, harness_with, FakeProviderBehavior, Harness};
    use academic_core::domain::ErrorKind;
    use academic_core::ports::{ProviderError, SecretStore};

    fn imap_input(password: Option<&str>) -> AccountInput {
        AccountInput {
            name: "UV IMAP".into(),
            email_address: "me@uv.es".into(),
            folders: vec!["INBOX".into()],
            default_university_id: None,
            initial_sync_days: 14,
            enabled: true,
            provider: ProviderInput::Imap {
                host: "imap.uv.es".into(),
                port: 993,
                username: "me".into(),
                password: password.map(str::to_string),
            },
        }
    }

    fn graph_input() -> AccountInput {
        AccountInput {
            provider: ProviderInput::MicrosoftGraph {
                tenant: "organizations".into(),
            },
            email_address: "me@uah.es".into(),
            ..imap_input(None)
        }
    }

    fn stored_password(h: &Harness, id: AccountId) -> Option<String> {
        h.secrets.get(&account_password_key(id)).unwrap()
    }

    #[test]
    fn adding_an_imap_account_stores_the_password_outside_the_database() {
        let h = harness();
        let dto = create_account(&h.state, imap_input(Some("test-password"))).unwrap();

        assert!(dto.credentials_configured);
        assert_eq!(
            stored_password(&h, dto.id).as_deref(),
            Some("test-password")
        );
        assert!(!serde_json::to_string(&dto)
            .unwrap()
            .contains("test-password"));
        assert_eq!(accounts(&h.state).unwrap().len(), 1);
    }

    #[test]
    fn adding_an_imap_account_requires_a_password() {
        let h = harness();
        for password in [None, Some("")] {
            let error = create_account(&h.state, imap_input(password)).unwrap_err();
            assert_eq!(error.kind, CommandErrorKind::Validation);
            assert_eq!(error.message, "Enter the password");
        }
        assert!(accounts(&h.state).unwrap().is_empty());
    }

    #[test]
    fn adding_a_graph_account_needs_no_password_and_is_not_signed_in_yet() {
        let h = harness();
        let dto = create_account(&h.state, graph_input()).unwrap();
        assert!(!dto.credentials_configured);
    }

    #[test]
    fn invalid_input_is_rejected_before_anything_is_stored() {
        let h = harness();
        let mut input = imap_input(Some("test-password"));
        input.name = " ".into();
        assert_eq!(
            create_account(&h.state, input).unwrap_err().message,
            "Enter a name"
        );
        assert!(accounts(&h.state).unwrap().is_empty());
    }

    #[test]
    fn editing_keeps_the_password_unless_a_new_one_is_given() {
        let h = harness();
        let id = create_account(&h.state, imap_input(Some("first-password")))
            .unwrap()
            .id;

        let mut input = imap_input(None);
        input.name = "Renamed".into();
        let dto = edit_account(&h.state, id, input).unwrap();
        assert_eq!(dto.name, "Renamed");
        assert_eq!(stored_password(&h, id).as_deref(), Some("first-password"));

        edit_account(&h.state, id, imap_input(Some("second-password"))).unwrap();
        assert_eq!(stored_password(&h, id).as_deref(), Some("second-password"));
    }

    #[test]
    fn the_provider_type_cannot_change() {
        let h = harness();
        let id = create_account(&h.state, imap_input(Some("pw"))).unwrap().id;
        let error = edit_account(&h.state, id, graph_input()).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(error.message, "The provider type can't be changed");
    }

    #[test]
    fn editing_a_missing_account_is_not_found() {
        let h = harness();
        let error = edit_account(&h.state, 42, imap_input(None)).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[test]
    fn removing_an_account_deletes_both_secrets_and_the_row() {
        let h = harness();
        let id = create_account(&h.state, imap_input(Some("pw"))).unwrap().id;
        h.secrets
            .set(&account_refresh_token_key(id), "test-token")
            .unwrap();

        delete_account(&h.state, id).unwrap();
        assert_eq!(stored_password(&h, id), None);
        assert_eq!(h.secrets.get(&account_refresh_token_key(id)).unwrap(), None);
        assert!(accounts(&h.state).unwrap().is_empty());
    }

    #[test]
    fn removing_a_missing_account_is_not_found() {
        let h = harness();
        let error = delete_account(&h.state, 42).unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[tokio::test]
    async fn testing_an_account_that_connects_succeeds() {
        let h = harness();
        let id = create_account(&h.state, imap_input(Some("pw"))).unwrap().id;
        assert_eq!(
            check_connection(&h.state, id).await.unwrap(),
            ConnectionTestDto::success()
        );
    }

    #[tokio::test]
    async fn testing_an_account_maps_provider_errors_to_kinds() {
        let behavior = FakeProviderBehavior {
            create: || Ok(()),
            test_connection: || Err(ProviderError::Auth("wrong password".into())),
        };
        let h = harness_with(behavior, Err("unused".into()));
        let id = create_account(&h.state, imap_input(Some("pw"))).unwrap().id;
        let dto = check_connection(&h.state, id).await.unwrap();
        assert!(!dto.ok);
        assert_eq!(dto.error_kind, Some(ErrorKind::Auth));
        assert_eq!(
            dto.message.as_deref(),
            Some("Authentication failed: wrong password")
        );
    }

    #[tokio::test]
    async fn testing_an_account_reports_factory_errors() {
        let behavior = FakeProviderBehavior {
            create: || Err(ProviderError::Connection("no route".into())),
            test_connection: || Ok(()),
        };
        let h = harness_with(behavior, Err("unused".into()));
        let id = create_account(&h.state, imap_input(Some("pw"))).unwrap().id;
        let dto = check_connection(&h.state, id).await.unwrap();
        assert_eq!(dto.error_kind, Some(ErrorKind::Connection));
    }

    #[tokio::test]
    async fn testing_a_missing_account_is_not_found() {
        let h = harness();
        let error = check_connection(&h.state, 1).await.unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[tokio::test]
    async fn microsoft_sign_in_requires_a_client_id() {
        let h = harness();
        let id = create_account(&h.state, graph_input()).unwrap().id;
        let never_opened = |_: &str| -> Result<(), String> { panic!("browser must not open") };
        let error = sign_in_microsoft(&h.state, id, &never_opened)
            .await
            .unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(error.message, "Set the Microsoft client ID in Settings");
    }

    #[tokio::test]
    async fn microsoft_sign_in_is_rejected_for_imap_accounts() {
        let h = harness();
        let id = create_account(&h.state, imap_input(Some("pw"))).unwrap().id;
        let never_opened = |_: &str| -> Result<(), String> { panic!("browser must not open") };
        let error = sign_in_microsoft(&h.state, id, &never_opened)
            .await
            .unwrap_err();
        assert_eq!(error.message, "This account doesn't use Microsoft sign-in");
    }

    #[test]
    fn the_signed_in_email_must_match_case_insensitively() {
        assert!(ensure_expected_email("Me@UAH.es", "me@uah.es").is_ok());
        let error = ensure_expected_email("me@uah.es", "other@uah.es").unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(
            error.message,
            "Signed in as other@uah.es, expected me@uah.es"
        );
    }
}
