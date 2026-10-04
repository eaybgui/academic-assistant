//! Authorization Code + PKCE sign-in against Microsoft's identity platform (v2
//! endpoints), and refresh-token exchange. No client secret is used (public client).

use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use chrono::{Duration as ChronoDuration, Utc};
use rand::{rng, Rng};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use academic_core::ports::ProviderError;

use super::loopback::LoopbackServer;
use super::mapping::{map_oauth_error, map_token_error_body, map_transport_error};
use super::{OAuthConfig, TokenSet, DEFAULT_REQUEST_TIMEOUT, SCOPES};

/// A PKCE verifier/challenge pair, generated fresh for every sign-in attempt.
/// The challenge method is always `S256`.
pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

/// Generates a new random PKCE verifier (32 random bytes, base64url-encoded, giving
/// the required 43+ characters) and its `S256` challenge.
pub fn new_pkce() -> Pkce {
    let verifier = random_url_safe_token(32);
    let digest = Sha256::digest(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(digest);
    Pkce {
        verifier,
        challenge,
    }
}

fn random_url_safe_token(byte_len: usize) -> String {
    let mut bytes = vec![0u8; byte_len];
    rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// True when `tenant` looks like a Microsoft tenant id/domain
/// (`^[A-Za-z0-9.-]+$` — a GUID, a verified domain, or a well-known alias like
/// `organizations`/`common`/`consumers`), i.e. it is safe to interpolate into a URL
/// path segment. Also used to validate the tenant when an account is saved.
pub fn is_valid_tenant(tenant: &str) -> bool {
    !tenant.is_empty()
        && tenant
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// Rejects a tenant that fails [`is_valid_tenant`] before it is interpolated into
/// a URL path segment. Called by every path that builds a login/token URL from an
/// [`OAuthConfig`] (`sign_in`, `request_token`), so [`authorize_url`] can assume
/// `config.tenant` is already valid and stay infallible.
fn validate_tenant(tenant: &str) -> Result<(), ProviderError> {
    if is_valid_tenant(tenant) {
        Ok(())
    } else {
        Err(ProviderError::Auth("Invalid Microsoft tenant".into()))
    }
}

/// Builds the `/authorize` URL the system browser is opened at.
///
/// Assumes `config.tenant` has already been validated by the caller (every public
/// entry point that builds a URL from an `OAuthConfig` — `sign_in`, `refresh` —
/// calls [`validate_tenant`] first); never panics even if that assumption somehow
/// doesn't hold, falling back to the raw, unparsed URL instead.
pub fn authorize_url(config: &OAuthConfig, redirect_uri: &str, pkce: &Pkce, state: &str) -> String {
    let base = format!(
        "{}/{}/oauth2/v2.0/authorize",
        config.login_base_url, config.tenant
    );
    let Ok(mut url) = Url::parse(&base) else {
        return base;
    };

    url.query_pairs_mut()
        .append_pair("client_id", &config.client_id)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_mode", "query")
        .append_pair("scope", SCOPES)
        .append_pair("state", state)
        .append_pair("code_challenge", &pkce.challenge)
        .append_pair("code_challenge_method", "S256")
        .append_pair("prompt", "select_account");

    url.to_string()
}

/// Runs an interactive sign-in: binds a loopback listener, opens the system browser
/// at the authorize URL via `open_browser`, waits (up to `timeout`) for the redirect,
/// checks `state`, and exchanges the authorization code for tokens.
pub async fn sign_in(
    http: &reqwest::Client,
    config: &OAuthConfig,
    open_browser: &(dyn Fn(&str) -> Result<(), String> + Send + Sync),
    timeout: Duration,
) -> Result<TokenSet, ProviderError> {
    validate_tenant(&config.tenant)?;

    let server = LoopbackServer::bind().await?;
    let redirect_uri = server.redirect_uri();
    let pkce = new_pkce();
    let state = random_url_safe_token(16);

    let url = authorize_url(config, &redirect_uri, &pkce, &state);
    open_browser(&url).map_err(ProviderError::Auth)?;

    let redirect = server.wait_for_redirect(timeout).await?;

    if redirect.state.as_deref() != Some(state.as_str()) {
        return Err(ProviderError::Auth(
            "Sign-in response did not match this request".into(),
        ));
    }

    if let Some(error) = redirect.error {
        return Err(
            map_oauth_error(&error, redirect.error_description.as_deref()).unwrap_or_else(|| {
                ProviderError::Auth("Microsoft sign-in was not completed".into())
            }),
        );
    }

    let code = redirect.code.ok_or_else(|| {
        ProviderError::Protocol("Microsoft did not return an authorization code".into())
    })?;

    exchange_code(http, config, &redirect_uri, &code, &pkce.verifier).await
}

/// Exchanges a refresh token for a new access token.
///
/// Microsoft does not always rotate the refresh token on this grant: when the
/// response omits one, the token passed in is still valid and is reused (the caller
/// is responsible for persisting whichever refresh token comes back, rotated or
/// not).
pub async fn refresh(
    http: &reqwest::Client,
    config: &OAuthConfig,
    refresh_token: &str,
) -> Result<TokenSet, ProviderError> {
    refresh_with_timeout(http, config, refresh_token, DEFAULT_REQUEST_TIMEOUT).await
}

/// [`refresh`] with an explicit per-request `timeout`.
pub(super) async fn refresh_with_timeout(
    http: &reqwest::Client,
    config: &OAuthConfig,
    refresh_token: &str,
    timeout: Duration,
) -> Result<TokenSet, ProviderError> {
    let response = request_token(
        http,
        config,
        &[
            ("client_id", config.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", SCOPES),
        ],
        timeout,
    )
    .await?;

    Ok(TokenSet {
        access_token: response.access_token,
        refresh_token: response
            .refresh_token
            .unwrap_or_else(|| refresh_token.to_string()),
        expires_at: Utc::now() + ChronoDuration::seconds(response.expires_in.max(0)),
    })
}

async fn exchange_code(
    http: &reqwest::Client,
    config: &OAuthConfig,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<TokenSet, ProviderError> {
    let response = request_token(
        http,
        config,
        &[
            ("client_id", config.client_id.as_str()),
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
            ("scope", SCOPES),
        ],
        DEFAULT_REQUEST_TIMEOUT,
    )
    .await?;

    // Unlike a refresh, the first token exchange has no previous refresh token to
    // fall back to: a missing one here means `offline_access` wasn't actually
    // granted, a genuine (if unlikely) protocol problem.
    let refresh_token = response.refresh_token.ok_or_else(|| {
        ProviderError::Protocol("Microsoft did not return a refresh token".into())
    })?;

    Ok(TokenSet {
        access_token: response.access_token,
        refresh_token,
        expires_at: Utc::now() + ChronoDuration::seconds(response.expires_in.max(0)),
    })
}

/// Encodes `pairs` as an `application/x-www-form-urlencoded` body. Used instead of
/// `reqwest`'s `form` feature, which is not enabled for this workspace (only `json`
/// and `rustls` are); `url`'s encoder produces the same result.
fn encode_form(pairs: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: i64,
}

/// Posts `form` to the `/token` endpoint and returns the raw response. Shared by
/// [`refresh`] and `exchange_code`, which each decide differently how to handle a
/// response with no `refresh_token`.
async fn request_token(
    http: &reqwest::Client,
    config: &OAuthConfig,
    form: &[(&str, &str)],
    timeout: Duration,
) -> Result<TokenResponse, ProviderError> {
    validate_tenant(&config.tenant)?;

    let url = format!(
        "{}/{}/oauth2/v2.0/token",
        config.login_base_url, config.tenant
    );

    let response = http
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .timeout(timeout)
        .body(encode_form(form))
        .send()
        .await
        .map_err(map_transport_error)?;

    let status = response.status();
    let body = response.text().await.map_err(map_transport_error)?;

    if !status.is_success() {
        return Err(map_token_error_body(status, &body));
    }

    serde_json::from_str(&body)
        .map_err(|_| ProviderError::Protocol("Microsoft returned an invalid token response".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pkce_challenge_is_base64url_sha256_of_verifier() {
        let pkce = new_pkce();
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(pkce.verifier.as_bytes()));
        assert_eq!(pkce.challenge, expected);
        // No padding characters, and URL-safe alphabet only.
        assert!(!pkce.challenge.contains('='));
        assert!(!pkce.verifier.contains('='));
    }

    #[test]
    fn new_pkce_generates_different_verifiers_each_time() {
        let a = new_pkce();
        let b = new_pkce();
        assert_ne!(a.verifier, b.verifier);
    }

    #[test]
    fn encode_form_url_encodes_keys_and_values() {
        let body = encode_form(&[("a", "1 2"), ("b", "x&y")]);
        assert_eq!(body, "a=1+2&b=x%26y");
    }

    #[test]
    fn authorize_url_contains_required_parameters() {
        let config = OAuthConfig {
            client_id: "client-123".into(),
            tenant: "organizations".into(),
            login_base_url: "https://login.microsoftonline.com".into(),
        };
        let pkce = new_pkce();
        let url = authorize_url(&config, "http://localhost:5555", &pkce, "state-abc");

        assert!(url
            .starts_with("https://login.microsoftonline.com/organizations/oauth2/v2.0/authorize?"));
        let parsed = Url::parse(&url).unwrap();
        let pairs: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs.get("client_id").unwrap(), "client-123");
        assert_eq!(pairs.get("response_type").unwrap(), "code");
        assert_eq!(pairs.get("redirect_uri").unwrap(), "http://localhost:5555");
        assert_eq!(pairs.get("response_mode").unwrap(), "query");
        assert_eq!(pairs.get("scope").unwrap(), SCOPES);
        assert_eq!(pairs.get("state").unwrap(), "state-abc");
        assert_eq!(pairs.get("code_challenge").unwrap(), &pkce.challenge);
        assert_eq!(pairs.get("code_challenge_method").unwrap(), "S256");
        assert_eq!(pairs.get("prompt").unwrap(), "select_account");
    }

    #[test]
    fn authorize_url_never_panics_even_with_an_unvalidated_tenant() {
        // authorize_url itself performs no validation (callers do); it must still
        // never panic if an invalid tenant somehow reaches it.
        let config = OAuthConfig {
            client_id: "client-123".into(),
            tenant: "not a valid tenant".into(),
            login_base_url: "https://login.microsoftonline.com".into(),
        };
        let pkce = new_pkce();
        let _ = authorize_url(&config, "http://localhost:5555", &pkce, "state-abc");
    }

    #[test]
    fn validate_tenant_accepts_common_tenant_forms() {
        assert!(validate_tenant("organizations").is_ok());
        assert!(validate_tenant("common").is_ok());
        assert!(validate_tenant("contoso.onmicrosoft.com").is_ok());
        assert!(validate_tenant("11111111-2222-3333-4444-555555555555").is_ok());
    }

    #[test]
    fn validate_tenant_rejects_empty_and_unsafe_input() {
        assert!(matches!(validate_tenant(""), Err(ProviderError::Auth(_))));
        assert!(validate_tenant("../../etc/passwd").is_err());
        assert!(validate_tenant("tenant with spaces").is_err());
        assert!(validate_tenant("tenant/with/slash").is_err());
    }
}
