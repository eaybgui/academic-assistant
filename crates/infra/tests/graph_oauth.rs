//! Integration tests for `academic_infra::graph::oauth`: PKCE, the loopback-based
//! `sign_in` flow, and token/refresh error mapping — all against `wiremock`.

use std::time::Duration;

use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use academic_core::ports::ProviderError;
use academic_infra::graph::oauth;
use academic_infra::graph::OAuthConfig;

fn test_config(login_base_url: String) -> OAuthConfig {
    OAuthConfig {
        client_id: "test-client-id".into(),
        tenant: "organizations".into(),
        login_base_url,
    }
}

#[tokio::test]
async fn refresh_sends_expected_form_fields() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/organizations/oauth2/v2.0/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("client_id=test-client-id"))
        .and(body_string_contains("refresh_token=old-refresh-token"))
        .and(body_string_contains(
            "scope=offline_access+Mail.Read+User.Read",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "new-access-token",
            "refresh_token": "new-refresh-token",
            "expires_in": 3600
        })))
        .expect(1)
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let config = test_config(server.uri());

    let tokens = oauth::refresh(&http, &config, "old-refresh-token")
        .await
        .expect("refresh should succeed");

    assert_eq!(tokens.access_token, "new-access-token");
    assert_eq!(tokens.refresh_token, "new-refresh-token");
    assert!(tokens.expires_at > chrono::Utc::now());
}

#[tokio::test]
async fn refresh_maps_invalid_grant_to_auth_with_reconnect_message() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/organizations/oauth2/v2.0/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": "AADSTS70008: The provided grant has expired."
        })))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let config = test_config(server.uri());

    let err = oauth::refresh(&http, &config, "expired-refresh-token")
        .await
        .expect_err("expired refresh token should fail");

    assert!(matches!(
        err,
        ProviderError::Auth(msg) if msg == "Your sign-in expired. Reconnect the account"
    ));
}

#[tokio::test]
async fn refresh_maps_admin_consent_error_to_auth() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/organizations/oauth2/v2.0/token"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": "invalid_grant",
            "error_description": "AADSTS65001: the user or administrator has not consented to use the application."
        })))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let config = test_config(server.uri());

    let err = oauth::refresh(&http, &config, "some-refresh-token")
        .await
        .expect_err("admin consent error should fail");

    assert!(matches!(
        err,
        ProviderError::Auth(msg) if msg == "Your organization requires admin approval for this app"
    ));
}

/// End-to-end `sign_in`: `open_browser` doesn't launch a real browser — it parses
/// `redirect_uri` and `state` out of the authorize URL it's given and performs the
/// GET the browser would eventually make, directly against the loopback listener.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_in_completes_via_loopback_redirect_and_token_exchange() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/organizations/oauth2/v2.0/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains("code=test-auth-code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "signed-in-access-token",
            "refresh_token": "signed-in-refresh-token",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let config = test_config(server.uri());

    let handle = tokio::runtime::Handle::current();
    let callback_client = reqwest::Client::new();
    let open_browser = move |authorize_url: &str| -> Result<(), String> {
        let (redirect_uri, state) = extract_redirect_and_state(authorize_url)?;
        let callback_url = format!("{redirect_uri}/?code=test-auth-code&state={state}");
        let client = callback_client.clone();
        handle.spawn(async move {
            let _ = client.get(callback_url).send().await;
        });
        Ok(())
    };

    let tokens = oauth::sign_in(&http, &config, &open_browser, Duration::from_secs(5))
        .await
        .expect("sign_in should succeed");

    assert_eq!(tokens.access_token, "signed-in-access-token");
    assert_eq!(tokens.refresh_token, "signed-in-refresh-token");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sign_in_fails_when_redirect_state_does_not_match() {
    let server = MockServer::start().await;
    // The token endpoint must never be called when state mismatches.
    Mock::given(method("POST"))
        .and(path("/organizations/oauth2/v2.0/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "should-not-be-used",
            "refresh_token": "should-not-be-used",
            "expires_in": 3600
        })))
        .expect(0)
        .mount(&server)
        .await;

    let http = reqwest::Client::new();
    let config = test_config(server.uri());

    let handle = tokio::runtime::Handle::current();
    let callback_client = reqwest::Client::new();
    let open_browser = move |authorize_url: &str| -> Result<(), String> {
        let (redirect_uri, _state) = extract_redirect_and_state(authorize_url)?;
        // Deliberately send the wrong state.
        let callback_url = format!("{redirect_uri}/?code=test-auth-code&state=wrong-state");
        let client = callback_client.clone();
        handle.spawn(async move {
            let _ = client.get(callback_url).send().await;
        });
        Ok(())
    };

    let err = oauth::sign_in(&http, &config, &open_browser, Duration::from_secs(5))
        .await
        .expect_err("mismatched state should fail sign-in");

    assert!(matches!(err, ProviderError::Auth(_)));
    server.verify().await;
}

fn extract_redirect_and_state(authorize_url: &str) -> Result<(String, String), String> {
    let parsed = url::Url::parse(authorize_url).map_err(|e| e.to_string())?;
    let pairs: std::collections::HashMap<String, String> =
        parsed.query_pairs().into_owned().collect();
    let redirect_uri = pairs
        .get("redirect_uri")
        .cloned()
        .ok_or_else(|| "authorize URL is missing redirect_uri".to_string())?;
    let state = pairs
        .get("state")
        .cloned()
        .ok_or_else(|| "authorize URL is missing state".to_string())?;
    Ok((redirect_uri, state))
}
