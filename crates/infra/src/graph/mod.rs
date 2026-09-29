//! Microsoft Graph email provider and OAuth (PKCE, loopback redirect) (Task T9).
//!
//! See `docs/infra/microsoft-graph.md` for the app-registration steps a user must
//! complete before an account of this kind can sign in.

mod loopback;
mod mapping;
pub mod oauth;
mod provider;

use std::fmt;
use std::time::Duration;

use chrono::{DateTime, Utc};

use academic_core::ports::ProviderError;

pub use provider::MicrosoftGraphEmailProvider;

/// Base URL for Microsoft's identity platform (OAuth v2 authorize/token endpoints).
pub const LOGIN_BASE_URL: &str = "https://login.microsoftonline.com";
/// Base URL for the Microsoft Graph API this app talks to.
pub const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";
/// OAuth scopes requested at sign-in: offline access (to receive a refresh token),
/// reading mail, and reading the signed-in user's profile (to confirm the account's
/// email address after sign-in).
pub const SCOPES: &str = "offline_access Mail.Read User.Read";

/// Default per-request timeout applied to every Graph/OAuth HTTP request (spec §8.2):
/// 60 seconds.
const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// The per-request timeout used by every Graph/OAuth HTTP request in this module.
/// Normally [`DEFAULT_REQUEST_TIMEOUT`]; overridable via the
/// `ACADEMIC_ASSISTANT_GRAPH_TEST_TIMEOUT_MS` environment variable (milliseconds) so
/// integration tests can actually trigger a timeout (e.g. with wiremock's
/// `set_delay`) without waiting a full minute. Not part of the public API —
/// production code never sets this variable.
pub(crate) fn request_timeout() -> Duration {
    std::env::var("ACADEMIC_ASSISTANT_GRAPH_TEST_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_REQUEST_TIMEOUT)
}

/// Per-account OAuth configuration: which app registration and tenant to use, and
/// which identity platform to talk to (overridable in tests).
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub tenant: String,
    pub login_base_url: String,
}

/// A freshly obtained or refreshed pair of Microsoft Graph tokens. The access token
/// is only ever kept in memory; the refresh token is persisted to the `SecretStore`.
///
/// `Debug` redacts both `access_token` and `refresh_token` — never log or display
/// this value directly.
#[derive(Clone)]
pub struct TokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: DateTime<Utc>,
}

impl fmt::Debug for TokenSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenSet")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Calls `GET /me` and returns the signed-in user's email address: `mail` when set,
/// otherwise `userPrincipalName`. Used both by [`MicrosoftGraphEmailProvider::test_connection`]
/// and, right after sign-in, to confirm the token belongs to the expected account.
pub async fn signed_in_email(
    http: &reqwest::Client,
    graph_base_url: &str,
    access_token: &str,
) -> Result<String, ProviderError> {
    let response = http
        .get(format!("{graph_base_url}/me"))
        .bearer_auth(access_token)
        .timeout(request_timeout())
        .send()
        .await
        .map_err(mapping::map_transport_error)?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(mapping::map_graph_error_status(status, &body));
    }

    let body: serde_json::Value = response.json().await.map_err(|_| {
        ProviderError::Protocol("Microsoft Graph returned an invalid profile response".into())
    })?;

    mapping::parse_profile_email(&body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_set_debug_redacts_both_tokens() {
        let tokens = TokenSet {
            access_token: "super-secret-access-token".into(),
            refresh_token: "super-secret-refresh-token".into(),
            expires_at: Utc::now(),
        };
        let debug = format!("{tokens:?}");
        assert!(!debug.contains("super-secret-access-token"));
        assert!(!debug.contains("super-secret-refresh-token"));
        assert!(debug.contains("<redacted>"));
    }
}
