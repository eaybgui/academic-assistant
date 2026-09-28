//! `EmailProviderFactory` and `StructuredModel` selection from settings (Task T13).
//!
//! See `docs/infra/factories.md` for the full contract (secret keys read, error
//! messages, and how each provider kind is built).

use std::sync::Arc;
use std::time::Duration;

use academic_core::domain::{
    account_password_key, Account, AiProviderKind, ProviderConfig, Settings, JEV_API_KEY,
};
use academic_core::ports::{
    EmailProvider, EmailProviderFactory, ModelProvider, ProviderError, SecretStore,
    SettingsRepository, StructuredModel,
};

use crate::graph::{MicrosoftGraphEmailProvider, OAuthConfig, GRAPH_BASE_URL, LOGIN_BASE_URL};
use crate::imap::ImapEmailProvider;
use crate::jev::{JevModel, JEV_BASE_URL};
use crate::ollama::OllamaModel;

/// The `User-Agent` sent on every request made by [`http_client`]'s client.
const USER_AGENT: &str = concat!("academic-assistant/", env!("CARGO_PKG_VERSION"));
/// Time allowed to establish the TCP+TLS connection before giving up. Per-request
/// timeouts (waiting for a response once connected) are set by each adapter.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Builds the `reqwest::Client` shared by every adapter this crate constructs:
/// rustls TLS, this app's `User-Agent`, and a 10 second connect timeout.
pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .expect("http client configuration is static and always valid")
}

/// Default [`EmailProviderFactory`]: builds an [`ImapEmailProvider`] or
/// [`MicrosoftGraphEmailProvider`] from an account's [`ProviderConfig`], reading the
/// IMAP password / Graph client id from the [`SecretStore`] / [`SettingsRepository`].
pub struct DefaultEmailProviderFactory {
    secrets: Arc<dyn SecretStore>,
    settings: Arc<dyn SettingsRepository>,
    http: reqwest::Client,
}

impl DefaultEmailProviderFactory {
    pub fn new(
        secrets: Arc<dyn SecretStore>,
        settings: Arc<dyn SettingsRepository>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            secrets,
            settings,
            http,
        }
    }
}

impl EmailProviderFactory for DefaultEmailProviderFactory {
    fn create(&self, account: &Account) -> Result<Box<dyn EmailProvider>, ProviderError> {
        match &account.provider {
            ProviderConfig::Imap {
                host,
                port,
                username,
            } => {
                let password = self
                    .secrets
                    .get(&account_password_key(account.id))
                    .map_err(|_| protocol_secrets_error())?
                    .ok_or_else(missing_password_error)?;
                Ok(Box::new(ImapEmailProvider::new(
                    account.id,
                    host.clone(),
                    *port,
                    username.clone(),
                    password,
                )))
            }
            ProviderConfig::MicrosoftGraph { tenant } => {
                let settings = self
                    .settings
                    .load()
                    .map_err(|_| protocol_settings_error())?;
                let client_id = settings
                    .microsoft_client_id
                    .ok_or_else(missing_client_id_error)?;
                let oauth = OAuthConfig {
                    client_id,
                    tenant: tenant.clone(),
                    login_base_url: LOGIN_BASE_URL.to_string(),
                };
                Ok(Box::new(MicrosoftGraphEmailProvider::new(
                    account.id,
                    self.http.clone(),
                    oauth,
                    GRAPH_BASE_URL.to_string(),
                    self.secrets.clone(),
                )))
            }
        }
    }
}

fn missing_password_error() -> ProviderError {
    ProviderError::Auth("The password is missing. Edit the account to set it".to_string())
}

fn missing_client_id_error() -> ProviderError {
    ProviderError::Auth("Set the Microsoft client ID in Settings".to_string())
}

fn protocol_settings_error() -> ProviderError {
    ProviderError::Protocol("Could not load settings".to_string())
}

fn protocol_secrets_error() -> ProviderError {
    ProviderError::Protocol("Could not read secure storage".to_string())
}

/// Default [`ModelProvider`]: builds an [`OllamaModel`] or [`JevModel`] from
/// [`Settings::ai_provider`], reading the jev API key from the [`SecretStore`] when needed.
pub struct DefaultModelProvider {
    secrets: Arc<dyn SecretStore>,
    http: reqwest::Client,
}

impl DefaultModelProvider {
    pub fn new(secrets: Arc<dyn SecretStore>, http: reqwest::Client) -> Self {
        Self { secrets, http }
    }
}

impl ModelProvider for DefaultModelProvider {
    fn current(&self, settings: &Settings) -> Result<Arc<dyn StructuredModel>, String> {
        match settings.ai_provider {
            AiProviderKind::Ollama => Ok(Arc::new(OllamaModel::new(
                self.http.clone(),
                settings.ollama_base_url.clone(),
                settings.ollama_model.clone(),
            ))),
            AiProviderKind::Jev => {
                let key = self
                    .secrets
                    .get(JEV_API_KEY)
                    .map_err(|_| "Could not read secure storage".to_string())?
                    .ok_or_else(|| "Add your jev API key in Settings".to_string())?;
                Ok(Arc::new(JevModel::new(
                    self.http.clone(),
                    JEV_BASE_URL.to_string(),
                    key,
                    settings.jev_model.clone(),
                )))
            }
        }
    }
}
