//! Integration tests for `academic_infra::graph::MicrosoftGraphEmailProvider`:
//! token refresh/caching, `test_connection`, and the delta fetch/pagination logic
//! (including the `$filter` 400 fallback) — all against `wiremock`.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use wiremock::matchers::{headers, method, path, query_param_contains, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

use academic_core::domain::{account_refresh_token_key, AccountId, SyncCursor, MAX_BATCH};
use academic_core::ports::{EmailProvider, ProviderError, SecretError, SecretStore};
use academic_infra::graph::{MicrosoftGraphEmailProvider, OAuthConfig};
use academic_infra::secrets::InMemorySecretStore;

const ACCOUNT_ID: AccountId = 1;
const FOLDER: &str = "inbox";

const MESSAGE_FIXTURE: &str = include_str!("fixtures/graph/message_exam_change.json");

fn provider(server: &MockServer, secrets: Arc<InMemorySecretStore>) -> MicrosoftGraphEmailProvider {
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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();

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
        secrets.get(&account_refresh_token_key(ACCOUNT_ID)).unwrap(),
        Some("rotated-refresh-token".to_string())
    );
}

#[tokio::test]
async fn test_connection_fails_when_no_refresh_token_is_stored() {
    let server = MockServer::start().await;
    let secrets = Arc::new(InMemorySecretStore::default());

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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();

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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
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
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
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

#[tokio::test]
async fn fetch_new_skips_a_message_that_fails_to_map_but_returns_the_others() {
    let server = MockServer::start().await;
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let delta_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=done",
        server.uri()
    );
    let mut unmappable = message("bad", "2026-09-05T10:00:00Z");
    unmappable["from"] = Value::Null; // no sender: map_message errors on this one.

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [
                message("m1", "2026-09-05T10:00:00Z"),
                unmappable,
                message("m2", "2026-09-06T10:00:00Z"),
            ],
            "@odata.deltaLink": delta_link,
        })))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch = provider
        .fetch_new(FOLDER, None, since())
        .await
        .expect("one unmappable message must not fail the whole batch");

    assert_eq!(batch.emails.len(), 2);
    assert!(batch.emails.iter().all(|e| e.provider_message_id != "bad"));
    // The cursor still advances to the end of the feed regardless of the skipped
    // message.
    assert_eq!(batch.next_cursor, SyncCursor::Graph { link: delta_link });
}

#[tokio::test]
async fn fetch_new_with_a_cursor_from_the_fallback_path_still_discards_older_messages() {
    let server = MockServer::start().await;
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    // First call: the filtered request 400s, so it falls back to an unfiltered one.
    // The saved cursor (a deltaLink) therefore carries no `$filter`.
    let saved_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=saved",
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
        .and(query_param_is_missing("$deltatoken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [message("new1", "2026-09-15T00:00:00Z")],
            "@odata.deltaLink": saved_link,
        })))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch1 = provider.fetch_new(FOLDER, None, since()).await.unwrap();
    assert_eq!(batch1.emails.len(), 1);
    assert_eq!(
        batch1.next_cursor,
        SyncCursor::Graph {
            link: saved_link.clone()
        }
    );

    // Second call (a later run): resumes from the saved cursor, which has no
    // `$filter`, so the server returns an older message too. It must be dropped.
    let next_delta_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=next",
        server.uri()
    );
    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_contains("$deltatoken", "saved"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [
                message("old", "2026-08-15T00:00:00Z"),
                message("new2", "2026-09-16T00:00:00Z"),
            ],
            "@odata.deltaLink": next_delta_link,
        })))
        .mount(&server)
        .await;

    let batch2 = provider
        .fetch_new(FOLDER, Some(&batch1.next_cursor), since())
        .await
        .unwrap();

    assert_eq!(batch2.emails.len(), 1);
    assert_eq!(batch2.emails[0].provider_message_id, "new2");
}

#[tokio::test]
async fn fetch_new_stops_following_next_links_once_less_than_a_page_of_room_remains() {
    let server = MockServer::start().await;
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let link = |token: &str| {
        format!(
            "{}/me/mailFolders/inbox/messages/delta?$skiptoken={token}",
            server.uri()
        )
    };
    let page = |prefix: &str| -> Vec<Value> {
        (0..50)
            .map(|i| message(&format!("{prefix}-{i:02}"), "2026-09-05T10:00:00Z"))
            .collect()
    };

    // Four pages of 50 fill MAX_BATCH exactly; the fifth must never be requested.
    let pages = [
        ("first", None, link("p1")),
        ("second", Some("p1"), link("p2")),
        ("third", Some("p2"), link("p3")),
        ("fourth", Some("p3"), link("p4")),
    ];
    for (prefix, token, next) in &pages {
        let mock = Mock::given(method("GET")).and(path("/me/mailFolders/inbox/messages/delta"));
        let mock = match token {
            Some(token) => mock.and(query_param_contains("$skiptoken", *token)),
            None => mock.and(query_param_is_missing("$skiptoken")),
        };
        mock.respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": page(prefix),
            "@odata.nextLink": next,
        })))
        .expect(1)
        .mount(&server)
        .await;
    }
    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .and(query_param_contains("$skiptoken", "p4"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch = provider.fetch_new(FOLDER, None, since()).await.unwrap();

    assert_eq!(batch.emails.len(), MAX_BATCH);
    assert_eq!(batch.next_cursor, SyncCursor::Graph { link: link("p4") });
    server.verify().await;
}

#[tokio::test]
async fn fetch_new_returns_emails_oldest_first_regardless_of_server_order() {
    let server = MockServer::start().await;
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    let delta_link = format!(
        "{}/me/mailFolders/inbox/messages/delta?$deltatoken=done",
        server.uri()
    );
    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "value": [
                message("later", "2026-09-10T00:00:00Z"),
                message("earlier", "2026-09-05T00:00:00Z"),
                message("middle", "2026-09-07T00:00:00Z"),
            ],
            "@odata.deltaLink": delta_link,
        })))
        .mount(&server)
        .await;

    let provider = provider(&server, secrets);
    let batch = provider.fetch_new(FOLDER, None, since()).await.unwrap();

    let ids: Vec<&str> = batch
        .emails
        .iter()
        .map(|e| e.provider_message_id.as_str())
        .collect();
    assert_eq!(ids, vec!["earlier", "middle", "later"]);
}

#[tokio::test]
async fn fetch_new_times_out_when_microsoft_graph_is_too_slow() {
    let server = MockServer::start().await;
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "seed-refresh-token")
        .unwrap();
    mount_token_endpoint(&server, "access-1", "refresh-2").await;

    Mock::given(method("GET"))
        .and(path("/me/mailFolders/inbox/messages/delta"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(300)))
        .mount(&server)
        .await;

    // The production timeout (60s) would make this test far too slow; shrink it via
    // the test-only override (see `academic_infra::graph::request_timeout`).
    std::env::set_var("ACADEMIC_ASSISTANT_GRAPH_TEST_TIMEOUT_MS", "50");
    let provider = provider(&server, secrets);
    let result = provider.fetch_new(FOLDER, None, since()).await;
    std::env::remove_var("ACADEMIC_ASSISTANT_GRAPH_TEST_TIMEOUT_MS");

    let err = result.expect_err("a slow Graph response must time out");
    assert!(matches!(err, ProviderError::Connection(_)));
}

/// A [`SecretStore`] wrapping [`InMemorySecretStore`] that counts `set` calls, used
/// to verify the provider only rewrites the stored refresh token when it changed.
#[derive(Default)]
struct CountingSecretStore {
    inner: InMemorySecretStore,
    set_calls: AtomicUsize,
}

impl SecretStore for CountingSecretStore {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        self.inner.get(key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.set_calls.fetch_add(1, Ordering::SeqCst);
        self.inner.set(key, value)
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.inner.delete(key)
    }
}

#[tokio::test]
async fn refresh_does_not_rewrite_the_secret_store_when_the_refresh_token_is_unchanged() {
    let server = MockServer::start().await;
    let secrets = Arc::new(CountingSecretStore::default());
    secrets
        .set(&account_refresh_token_key(ACCOUNT_ID), "same-refresh-token")
        .unwrap();
    // The counter only tracks calls made by the provider itself, not this setup.
    secrets.set_calls.store(0, Ordering::SeqCst);

    // The token response omits `refresh_token`: Microsoft did not rotate it.
    Mock::given(method("POST"))
        .and(path("/organizations/oauth2/v2.0/token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "access-1",
            "expires_in": 3600
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/me"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"mail": "user@uv.es"})))
        .mount(&server)
        .await;

    let oauth = OAuthConfig {
        client_id: "test-client-id".into(),
        tenant: "organizations".into(),
        login_base_url: server.uri(),
    };
    let provider = MicrosoftGraphEmailProvider::new(
        ACCOUNT_ID,
        reqwest::Client::new(),
        oauth,
        server.uri(),
        secrets.clone(),
    );

    provider.test_connection().await.unwrap();

    assert_eq!(secrets.set_calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        secrets
            .inner
            .get(&account_refresh_token_key(ACCOUNT_ID))
            .unwrap(),
        Some("same-refresh-token".to_string())
    );
}
