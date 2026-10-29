use academic_core::ports::{LmsClient, LmsError};
use academic_infra::lms::blackboard::{
    decode_session, encode_session, BlackboardClient, SessionCookie,
};
use serde_json::json;
use wiremock::matchers::{headers, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn cookies() -> Vec<SessionCookie> {
    vec![
        SessionCookie {
            name: "BbRouter".into(),
            value: "expires:1,id:abc".into(),
        },
        SessionCookie {
            name: "JSESSIONID".into(),
            value: "J1".into(),
        },
    ]
}

fn client(server: &MockServer) -> BlackboardClient {
    BlackboardClient::new(reqwest::Client::new(), server.uri(), &cookies())
}

#[tokio::test]
async fn verify_sends_the_cookies_and_builds_the_display_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me"))
        // wiremock splits request header values on commas, and the BbRouter value has one.
        .and(headers(
            "cookie",
            vec!["BbRouter=expires:1", "id:abc; JSESSIONID=J1"],
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": "_1234_1", "userName": "ana.perez", "name": { "given": "Ana", "family": "Pérez" }
        })))
        .mount(&server)
        .await;
    let identity = client(&server).verify().await.unwrap();
    assert_eq!(
        (identity.user_id.as_str(), identity.display_name.as_str()),
        ("_1234_1", "Ana Pérez")
    );
}

#[tokio::test]
async fn verify_falls_back_to_the_user_name() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "id": "_1_1", "userName": "ana.perez" })),
        )
        .mount(&server)
        .await;
    assert_eq!(
        client(&server).verify().await.unwrap().display_name,
        "ana.perez"
    );
}

#[tokio::test]
async fn unauthorized_is_an_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(401).set_body_json(json!({ "status": 401 })))
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server).verify().await,
        Err(LmsError::Auth(_))
    ));
}

#[tokio::test]
async fn a_redirect_away_from_the_api_is_an_auth_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/webapps/login/"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/webapps/login/"))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>login</html>"))
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server).verify().await,
        Err(LmsError::Auth(_))
    ));
}

#[tokio::test]
async fn list_courses_follows_paging() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me/courses"))
        .and(query_param("offset", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [ { "courseId": "_2_1", "course": { "id": "_2_1", "courseId": "RED-2026", "name": "Redes" } } ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me/courses"))
        .and(query_param("expand", "course"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [ { "courseId": "_10_1", "course": { "id": "_10_1", "courseId": "ISW-2026", "name": "Ingeniería del Software" } } ],
            "paging": { "nextPage": "/learn/api/public/v1/users/me/courses?expand=course&limit=100&offset=100" }
        })))
        .mount(&server)
        .await;
    let courses = client(&server).list_courses().await.unwrap();
    assert_eq!(courses.len(), 2);
    assert_eq!(courses[0].external_id, "_10_1");
    assert_eq!(courses[0].short_name.as_deref(), Some("ISW-2026"));
    assert_eq!(
        courses[0].url,
        Some(format!("{}/ultra/courses/_10_1/outline", server.uri()))
    );
    assert_eq!(courses[1].name, "Redes");
}

#[test]
fn sessions_round_trip_and_garbage_is_an_auth_error() {
    let json = encode_session(&cookies());
    assert_eq!(decode_session(&json).unwrap(), cookies());
    assert!(matches!(decode_session("not json"), Err(LmsError::Auth(_))));
    assert!(matches!(decode_session("[]"), Err(LmsError::Auth(_))));
}

#[tokio::test]
async fn a_next_page_link_outside_the_api_is_a_protocol_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/users/me/courses"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [ { "courseId": "_1_1", "course": { "id": "_1_1", "courseId": "A", "name": "A" } } ],
            "paging": { "nextPage": "https://evil.example/learn/api/x" }
        })))
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server).list_courses().await,
        Err(LmsError::Protocol(_))
    ));
}
