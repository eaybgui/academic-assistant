//! Commands: account CRUD, connection test and Microsoft sign-in.

use std::time::Duration;

use academic_core::domain::{
    account_password_key, account_refresh_token_key, Account, AccountDraft, AccountId,
    ProviderConfig, ProviderKind,
};
use academic_infra::graph::oauth;
use academic_infra::graph::{
    signed_in_identity, OAuthConfig, SignedInIdentity, GRAPH_BASE_URL, LOGIN_BASE_URL,
};
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
/// Logged instead of the user-facing mismatch message, which contains addresses.
const MISMATCH_LOG_MESSAGE: &str = "signed-in account mismatch";

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

/// Inserts the account, then prepares its secrets (see [`store_new_account_secrets`]).
/// If that fails the inserted account is removed again so no half-configured
/// account stays.
fn create_account(state: &AppState, input: AccountInput) -> Result<AccountDto, CommandError> {
    let ValidatedAccount { draft, password } = validation::account(input)?;
    if draft.provider.kind() == ProviderKind::Imap && password.is_none() {
        return Err(CommandError::validation(PASSWORD_REQUIRED));
    }

    let account = state.accounts.insert(&draft, state.clock.now())?;
    if let Err(error) = store_new_account_secrets(state, account.id, password.as_deref()) {
        if let Err(rollback) = state.accounts.delete(account.id) {
            tracing::warn!(account_id = account.id, %rollback, "could not roll back account");
        }
        return Err(error);
    }
    account_dto(state, &account)
}

/// Deletes any secret already stored under the new account's keys (defense in
/// depth: a new account must never inherit a leftover password or refresh token),
/// then stores the IMAP password when there is one.
fn store_new_account_secrets(
    state: &AppState,
    id: AccountId,
    password: Option<&str>,
) -> Result<(), CommandError> {
    state.secrets.delete(&account_password_key(id))?;
    state.secrets.delete(&account_refresh_token_key(id))?;
    if let Some(password) = password {
        state.secrets.set(&account_password_key(id), password)?;
    }
    Ok(())
}

/// Updates the account. The provider type cannot change and the IMAP password is
/// replaced only when the input carries one. A Microsoft account whose email or
/// tenant changes loses its refresh token: the stored sign-in belongs to the old
/// mailbox, so the account must be signed in again.
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
    if invalidates_sign_in(&existing, &draft) {
        state.secrets.delete(&account_refresh_token_key(id))?;
    }
    account_dto(state, &account)
}

/// True when `draft` changes the mailbox a Microsoft account is signed in to: its
/// email address (case-insensitive) or its tenant.
fn invalidates_sign_in(existing: &Account, draft: &AccountDraft) -> bool {
    match (&existing.provider, &draft.provider) {
        (
            ProviderConfig::MicrosoftGraph { tenant: old },
            ProviderConfig::MicrosoftGraph { tenant: new },
        ) => {
            old != new
                || !existing
                    .email_address
                    .eq_ignore_ascii_case(&draft.email_address)
        }
        _ => false,
    }
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
    let identity = signed_in_identity(&state.http, GRAPH_BASE_URL, &tokens.access_token).await?;
    store_sign_in(state, id, &identity, &tokens.refresh_token)
}

/// Stores the refresh token for account `id` when the signed-in identity is the
/// account's own address. The account is loaded again here because the sign-in
/// waits on the user: it may have been removed or edited meanwhile, and nothing is
/// stored for an account that no longer exists.
fn store_sign_in(
    state: &AppState,
    id: AccountId,
    identity: &SignedInIdentity,
    refresh_token: &str,
) -> Result<AccountDto, CommandError> {
    let account = find_account(state, id)?;
    ensure_expected_identity(&account.email_address, identity)?;
    state
        .secrets
        .set(&account_refresh_token_key(id), refresh_token)?;
    account_dto(state, &account)
}

/// Accepts the sign-in when `expected` equals the identity's `mail` or
/// `userPrincipalName`, ignoring case and surrounding whitespace.
fn ensure_expected_identity(
    expected: &str,
    identity: &SignedInIdentity,
) -> Result<(), CommandError> {
    let expected = expected.trim();
    let candidates = [&identity.mail, &identity.user_principal_name];
    let matches = candidates
        .iter()
        .copied()
        .flatten()
        .any(|candidate| candidate.trim().eq_ignore_ascii_case(expected));
    if matches {
        return Ok(());
    }
    let signed_in = candidates.iter().copied().flatten().next();
    Err(CommandError::validation(format!(
        "Signed in as {}, expected {expected}",
        signed_in.map_or("", String::as_str)
    ))
    .with_log_message(MISMATCH_LOG_MESSAGE))
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

    fn identity(mail: Option<&str>, upn: Option<&str>) -> SignedInIdentity {
        SignedInIdentity {
            mail: mail.map(str::to_string),
            user_principal_name: upn.map(str::to_string),
        }
    }

    #[test]
    fn the_account_email_may_match_mail_or_user_principal_name() {
        let both = identity(Some("alias@uah.es"), Some("Me@UAH.es"));
        assert!(ensure_expected_identity("me@uah.es", &both).is_ok());
        assert!(ensure_expected_identity(" ALIAS@uah.es ", &both).is_ok());
        assert!(ensure_expected_identity("me@uah.es", &identity(None, Some("me@uah.es"))).is_ok());
    }

    #[test]
    fn a_different_signed_in_identity_is_rejected_without_logging_the_addresses() {
        let error = ensure_expected_identity(
            "me@uah.es",
            &identity(Some("other@uah.es"), Some("upn@uah.es")),
        )
        .unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(
            error.message,
            "Signed in as other@uah.es, expected me@uah.es"
        );
        assert_eq!(error.message_for_log(), "signed-in account mismatch");
    }

    #[test]
    fn signing_in_stores_the_refresh_token_when_the_identity_matches() {
        let h = harness();
        let id = create_account(&h.state, graph_input()).unwrap().id;

        let dto = store_sign_in(
            &h.state,
            id,
            &identity(None, Some("me@uah.es")),
            "test-token",
        )
        .unwrap();

        assert!(dto.credentials_configured);
        assert_eq!(
            h.secrets.get(&account_refresh_token_key(id)).unwrap(),
            Some("test-token".to_string())
        );
    }

    #[test]
    fn signing_in_as_someone_else_stores_nothing() {
        let h = harness();
        let id = create_account(&h.state, graph_input()).unwrap().id;

        let error = store_sign_in(
            &h.state,
            id,
            &identity(Some("other@uah.es"), None),
            "test-token",
        )
        .unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(h.secrets.get(&account_refresh_token_key(id)).unwrap(), None);
    }

    #[test]
    fn signing_in_to_an_account_removed_meanwhile_stores_nothing() {
        let h = harness();
        let id = create_account(&h.state, graph_input()).unwrap().id;
        delete_account(&h.state, id).unwrap();

        let error = store_sign_in(
            &h.state,
            id,
            &identity(Some("me@uah.es"), None),
            "test-token",
        )
        .unwrap_err();

        assert_eq!(error.kind, CommandErrorKind::NotFound);
        assert_eq!(h.secrets.get(&account_refresh_token_key(id)).unwrap(), None);
    }

    #[test]
    fn a_new_account_never_inherits_leftover_secrets() {
        let h = harness();
        // The first account created in a fresh database gets id 1.
        h.secrets.set(&account_password_key(1), "old-pw").unwrap();
        h.secrets
            .set(&account_refresh_token_key(1), "old-token")
            .unwrap();

        let id = create_account(&h.state, graph_input()).unwrap().id;

        assert_eq!(id, 1);
        assert_eq!(stored_password(&h, id), None);
        assert_eq!(h.secrets.get(&account_refresh_token_key(id)).unwrap(), None);
    }

    fn signed_in_graph_account(h: &Harness) -> AccountId {
        let id = create_account(&h.state, graph_input()).unwrap().id;
        h.secrets
            .set(&account_refresh_token_key(id), "test-token")
            .unwrap();
        id
    }

    fn refresh_token_kept(h: &Harness, id: AccountId) -> bool {
        h.secrets
            .get(&account_refresh_token_key(id))
            .unwrap()
            .is_some()
    }

    #[test]
    fn editing_a_graph_accounts_email_or_tenant_drops_the_refresh_token() {
        let h = harness();

        let id = signed_in_graph_account(&h);
        let mut input = graph_input();
        input.email_address = "new@uah.es".into();
        let dto = edit_account(&h.state, id, input).unwrap();
        assert!(!dto.credentials_configured);
        assert!(!refresh_token_kept(&h, id));

        h.secrets
            .set(&account_refresh_token_key(id), "test-token")
            .unwrap();
        let mut input = graph_input();
        input.email_address = "new@uah.es".into();
        input.provider = ProviderInput::MicrosoftGraph {
            tenant: "contoso.onmicrosoft.com".into(),
        };
        edit_account(&h.state, id, input).unwrap();
        assert!(!refresh_token_kept(&h, id));
    }

    #[test]
    fn editing_a_graph_account_keeps_the_refresh_token_when_the_mailbox_is_unchanged() {
        let h = harness();
        let id = signed_in_graph_account(&h);

        let mut input = graph_input();
        input.name = "Renamed".into();
        input.email_address = "ME@UAH.ES".into();
        edit_account(&h.state, id, input).unwrap();

        assert!(refresh_token_kept(&h, id));
    }
}
