//! Integration tests for `academic_infra::graph::MicrosoftGraphEmailProvider`:
//! token refresh/caching, `test_connection`, and the delta fetch/pagination logic
//! (including the `$filter` 400 fallback) — all against `wiremock`.

mod graph_support;

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use wiremock::matchers::{headers, method, path, query_param_contains, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use academic_core::domain::{account_refresh_token_key, AccountId, SyncCursor, MAX_BATCH};
use academic_core::ports::{EmailProvider, ProviderError};
use academic_infra::graph::{MicrosoftGraphEmailProvider, OAuthConfig};

use graph_support::FakeSecretStore;

const ACCOUNT_ID: AccountId = 1;
const FOLDER: &str = "inbox";

const MESSAGE_FIXTURE: &str = include_str!("fixtures/graph/message_exam_change.json");

fn provider(server: &MockServer, secrets: Arc<FakeSecretStore>) -> MicrosoftGraphEmailProvider {
    let oauth = OAuthConfig {
        client_id: "test-client-id".into(),
        tenant: "organizations".into(),
        login_base_url: server.uri(),
    };
    MicrosoftGraphEmailProvider::new(
        ACCOUNT_ID,
        reqwest::Client::new(),
        oauth,
        server.uri(),
        secrets,
    )
}

async fn mount_token_endpoint(server: &MockServer, access_token: &str, refresh_token: &str) {
    Mock::given(method("POST"))
        .and(path("/organizations/oauth2/v2.0/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": access_token,
            "refresh_token": refresh_token,
            "expires_in": 3600
        })))
        .mount(server)
        .await;
}

fn message(id: &str, received_at: &str) -> Value {
    json!({
        "id": id,
        "internetMessageId": format!("<{id}@uv.es>"),
        "subject": "Aviso",
        "from": {"emailAddress": {"name": "Secretaría", "address": "secretaria@uv.es"}},
        "toRecipients": [{"emailAddress": {"name": "Ana", "address": "ana@uv.es"}}],
        "ccRecipients": [],
        "receivedDateTime": received_at,
        "body": {"contentType": "text", "content": "Contenido."},
        "webLink": "https://outlook.office.com/mail/deeplink"
    })
}

#[tokio::test]
async fn test_connection_refreshes_the_token_and_calls_me() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");

    mount_token_endpoint(&server, "access-1", "rotated-refresh-token").await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"mail": "user@uv.es"})))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets.clone());
    provider
        .test_connection()
        .await
        .expect("test_connection should succeed");

    assert_eq!(
        secrets.get_value(&account_refresh_token_key(ACCOUNT_ID)),
        Some("rotated-refresh-token".to_string())
    );
}

#[tokio::test]
async fn test_connection_fails_when_no_refresh_token_is_stored() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());

    let provider = provider(&server, secrets);
    let err = provider
        .test_connection()
        .await
        .expect_err("missing refresh token should fail");

    assert!(matches!(err, ProviderError::Auth(msg) if msg == "Sign in to this account"));
}

#[tokio::test]
async fn access_token_is_cached_and_not_refreshed_on_every_call() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");

    Mock::given(method("POST"))
        .and(path("/organizations/oauth2/v2.0/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "access-1",
            "refresh_token": "refresh-2",
            "expires_in": 3600
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"mail": "user@uv.es"})))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    provider.test_connection().await.unwrap();
    provider.test_connection().await.unwrap();

    server.verify().await;
}

fn since() -> DateTime<Utc> {
    "2026-09-01T00:00:00Z".parse().unwrap()
}

#[tokio::test]
async fn fetch_new_first_request_has_filter_and_prefer_header_and_maps_the_message() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let fixture_message: Value = serde_json::from_str(MESSAGE_FIXTURE).unwrap();
    let delta_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=done",
        server.uri()
    );

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_contains("$filter", "receivedDateTime ge"))
        // `HeaderExactMatcher` splits comma-separated values, so the `Prefer` header
        // (a single value with an embedded comma) is matched as its two parts.
        .and(headers(
            "Prefer",
            vec!["outlook.body-content-type=\"text\"", "odata.maxpagesize=50"],
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [fixture_message],
            "@odata.deltaLink": delta_link,
        })))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch = provider
        .fetch_new(FOLDER, None, since())
        .await
        .expect("fetch_new should succeed");

    assert_eq!(batch.emails.len(), 1);
    let email = &batch.emails[0];
    assert_eq!(email.provider_message_id, "AAMkAGI2VGU-exam-change-001");
    assert_eq!(email.sender.email, "secretaria.matematicas@uv.es");
    assert_eq!(email.subject, "Cambio de fecha del examen de Cálculo II");
    assert_eq!(
        email.message_id_header.as_deref(),
        Some("exam-change-20260920@uv.es")
    );
    assert_eq!(batch.next_cursor, SyncCursor::Graph { link: delta_link });
}

#[tokio::test]
async fn fetch_new_follows_next_link_until_delta_link() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let next_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$skip=1",
        server.uri()
    );
    let delta_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=done",
        server.uri()
    );

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_contains("$filter", "receivedDateTime"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [message("m1", "2026-09-05T10:00:00Z")],
            "@odata.nextLink": next_link,
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_is_missing("$filter"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [message("m2", "2026-09-06T10:00:00Z")],
            "@odata.deltaLink": delta_link,
        })))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch = provider.fetch_new(FOLDER, None, since()).await.unwrap();

    assert_eq!(batch.emails.len(), 2);
    assert_eq!(batch.emails[0].provider_message_id, "m1");
    assert_eq!(batch.emails[1].provider_message_id, "m2");
    assert_eq!(batch.next_cursor, SyncCursor::Graph { link: delta_link });
}

#[tokio::test]
async fn fetch_new_resumes_directly_from_a_cursor_link_without_adding_filter() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let cursor_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$skiptoken=resume",
        server.uri()
    );
    let delta_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=done",
        server.uri()
    );

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_contains("$skiptoken", "resume"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [message("m1", "2026-09-05T10:00:00Z")],
            "@odata.deltaLink": delta_link,
        })))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let cursor = SyncCursor::Graph { link: cursor_link };
    let batch = provider
        .fetch_new(FOLDER, Some(&cursor), since())
        .await
        .unwrap();

    assert_eq!(batch.emails.len(), 1);
    assert_eq!(batch.next_cursor, SyncCursor::Graph { link: delta_link });
}

#[tokio::test]
async fn fetch_new_ignores_removed_entries() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let delta_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=done",
        server.uri()
    );
    let removed = json!({"id": "removed-1", "@removed": {"reason": "deleted"}});

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [message("m1", "2026-09-05T10:00:00Z"), removed],
            "@odata.deltaLink": delta_link,
        })))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch = provider.fetch_new(FOLDER, None, since()).await.unwrap();

    assert_eq!(batch.emails.len(), 1);
    assert_eq!(batch.emails[0].provider_message_id, "m1");
}

#[tokio::test]
async fn fetch_new_stops_at_max_batch_and_returns_the_next_link_as_cursor() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let messages: Vec<Value> = (0..MAX_BATCH)
        .map(|i| message(&format!("m{i}"), "2026-09-05T10:00:00Z"))
        .collect();
    let next_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$skiptoken=never-fetched",
        server.uri()
    );

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_contains("$filter", "receivedDateTime"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": messages,
            "@odata.nextLink": next_link,
        })))
        .expect(1)
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch = provider.fetch_new(FOLDER, None, since()).await.unwrap();

    assert_eq!(batch.emails.len(), MAX_BATCH);
    assert_eq!(batch.next_cursor, SyncCursor::Graph { link: next_link });
    server.verify().await;
}

#[tokio::test]
async fn fetch_new_retries_without_filter_on_400_and_discards_older_messages() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let delta_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=done",
        server.uri()
    );

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_contains("$filter", "receivedDateTime"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {"code": "BadRequest", "message": "Cannot filter on this property"}
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_is_missing("$filter"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [
                message("old", "2026-08-15T00:00:00Z"),
                message("new", "2026-09-15T00:00:00Z"),
            ],
            "@odata.deltaLink": delta_link,
        })))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch = provider.fetch_new(FOLDER, None, since()).await.unwrap();

    assert_eq!(batch.emails.len(), 1);
    assert_eq!(batch.emails[0].provider_message_id, "new");
}

#[tokio::test]
async fn fetch_new_maps_401_to_auth() {
    let server = MockServer::start().await;
    let secrets = Arc::new(FakeSecretStore::new());
    secrets.seed(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token");
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let err = provider.fetch_new(FOLDER, None, since()).await.unwrap_err();

    assert!(matches!(err, ProviderError::Auth(_)));
}
