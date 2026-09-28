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
use super::{OAuthConfig, TokenSet, SCOPES};

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

/// Builds the `/authorize` URL the system browser is opened at.
pub fn authorize_url(config: &OAuthConfig, redirect_uri: &str, pkce: &Pkce, state: &str) -> String {
    let mut url = Url::parse(&format!(
        "{}/{}/oauth2/v2.0/authorize",
        config.login_base_url, config.tenant
    ))
    .expect("login_base_url and tenant form a valid absolute URL");

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

/// Exchanges a refresh token for a new access token (and, when Microsoft rotates it,
/// a new refresh token — the caller is responsible for persisting it).
pub async fn refresh(
    http: &reqwest::Client,
    config: &OAuthConfig,
    refresh_token: &str,
) -> Result<TokenSet, ProviderError> {
    request_token(
        http,
        config,
        &[
            ("client_id", config.client_id.as_str()),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("scope", SCOPES),
        ],
    )
    .await
}

async fn exchange_code(
    http: &reqwest::Client,
    config: &OAuthConfig,
    redirect_uri: &str,
    code: &str,
    verifier: &str,
) -> Result<TokenSet, ProviderError> {
    request_token(
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
    )
    .await
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

async fn request_token(
    http: &reqwest::Client,
    config: &OAuthConfig,
    form: &[(&str, &str)],
) -> Result<TokenSet, ProviderError> {
    let url = format!(
        "{}/{}/oauth2/v2.0/token",
        config.login_base_url, config.tenant
    );

    let response = http
        .post(url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(encode_form(form))
        .send()
        .await
        .map_err(map_transport_error)?;

    let status = response.status();
    let body = response.text().await.map_err(map_transport_error)?;

    if !status.is_success() {
        return Err(map_token_error_body(status, &body));
    }

    let parsed: TokenResponse = serde_json::from_str(&body).map_err(|_| {
        ProviderError::Protocol("Microsoft returned an invalid token response".into())
    })?;

    let refresh_token = parsed.refresh_token.ok_or_else(|| {
        ProviderError::Protocol("Microsoft did not return a refresh token".into())
    })?;

    Ok(TokenSet {
        access_token: parsed.access_token,
        refresh_token,
        expires_at: Utc::now() + ChronoDuration::seconds(parsed.expires_in.max(0)),
    })
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
}
