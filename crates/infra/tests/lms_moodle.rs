use academic_core::ports::{LmsClient, LmsError};
use academic_infra::lms::moodle::{decode_launch_token, MoodleClient};
use base64::Engine;
use md5::{Digest, Md5};
use serde_json::json;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const REQUIRED: [&str; 7] = [
    "core_enrol_get_users_courses",
    "mod_assign_get_assignments",
    "mod_assign_get_submission_status",
    "mod_quiz_get_quizzes_by_courses",
    "mod_quiz_get_user_attempts",
    "mod_quiz_get_user_best_grade",
    "core_calendar_get_calendar_events",
];

fn site_info(functions: &[&str]) -> serde_json::Value {
    json!({
        "userid": 42,
        "fullname": "Ana Pérez",
        "functions": functions.iter().map(|f| json!({ "name": f, "version": "1" })).collect::<Vec<_>>()
    })
}

async fn mock_site_info(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/webservice/rest/server.php"))
        .and(body_string_contains(
            "wsfunction=core_webservice_get_site_info",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

fn client(server: &MockServer) -> MoodleClient {
    MoodleClient::new(reqwest::Client::new(), server.uri(), "tok123".into())
}

#[tokio::test]
async fn verify_returns_the_user_and_sends_the_token() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/webservice/rest/server.php"))
        .and(body_string_contains("wstoken=tok123"))
        .and(body_string_contains("moodlewsrestformat=json"))
        .and(body_string_contains(
            "wsfunction=core_webservice_get_site_info",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(site_info(&REQUIRED)))
        .mount(&server)
        .await;
    let identity = client(&server).verify().await.unwrap();
    assert_eq!(
        (identity.user_id.as_str(), identity.display_name.as_str()),
        ("42", "Ana Pérez")
    );
}

#[tokio::test]
async fn verify_fails_when_a_required_function_is_missing() {
    let server = MockServer::start().await;
    mock_site_info(&server, site_info(&REQUIRED[..6])).await;
    match client(&server).verify().await {
        Err(LmsError::Protocol(m)) => {
            assert!(m.contains("core_calendar_get_calendar_events"), "{m}")
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn invalid_token_is_an_auth_error() {
    let server = MockServer::start().await;
    mock_site_info(
        &server,
        json!({ "exception": "moodle_exception", "errorcode": "invalidtoken", "message": "Invalid token" }),
    )
    .await;
    assert!(matches!(
        client(&server).verify().await,
        Err(LmsError::Auth(_))
    ));
}

#[tokio::test]
async fn unavailable_function_is_a_protocol_error_naming_it() {
    let server = MockServer::start().await;
    mock_site_info(
        &server,
        json!({ "exception": "x", "errorcode": "servicenotavailable", "message": "nope" }),
    )
    .await;
    match client(&server).verify().await {
        Err(LmsError::Protocol(m)) => assert!(m.contains("core_webservice_get_site_info")),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn list_courses_maps_enrolled_courses() {
    let server = MockServer::start().await;
    mock_site_info(&server, site_info(&REQUIRED)).await;
    Mock::given(method("POST"))
        .and(path("/webservice/rest/server.php"))
        .and(body_string_contains(
            "wsfunction=core_enrol_get_users_courses",
        ))
        .and(body_string_contains("userid=42"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            { "id": 1234, "fullname": "Cálculo II", "shortname": "CAL2" },
            { "id": 99, "fullname": "Tutoría", "shortname": "" }
        ])))
        .mount(&server)
        .await;
    let courses = client(&server).list_courses().await.unwrap();
    assert_eq!(courses.len(), 2);
    assert_eq!(courses[0].external_id, "1234");
    assert_eq!(courses[0].name, "Cálculo II");
    assert_eq!(courses[0].short_name.as_deref(), Some("CAL2"));
    assert_eq!(
        courses[0].url,
        Some(format!("{}/course/view.php?id=1234", server.uri()))
    );
    assert_eq!(courses[1].short_name, None);
}

#[tokio::test]
async fn http_errors_are_protocol_errors() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    assert!(matches!(
        client(&server).verify().await,
        Err(LmsError::Protocol(_))
    ));
}

fn payload(signature: &str, token: &str) -> String {
    base64::engine::general_purpose::STANDARD.encode(format!("{signature}:::{token}:::private"))
}

fn signature(base_url: &str, passport: &str) -> String {
    hex::encode(Md5::digest(format!("{base_url}{passport}").as_bytes()))
}

#[test]
fn launch_token_is_decoded_when_the_signature_matches() {
    let base = "https://aulavirtual.uv.es";
    let p = payload(&signature(base, "pass1"), "tok-xyz");
    assert_eq!(decode_launch_token(&p, base, "pass1").unwrap(), "tok-xyz");
}

#[test]
fn launch_token_accepts_percent_encoded_base64() {
    let base = "https://aulavirtual.uv.es";
    let p = payload(&signature(base, "pass1"), "tok-xyz")
        .replace('+', "%2B")
        .replace('/', "%2F")
        .replace('=', "%3D");
    assert_eq!(decode_launch_token(&p, base, "pass1").unwrap(), "tok-xyz");
}

#[test]
fn launch_token_with_a_wrong_signature_or_garbage_is_rejected() {
    let base = "https://aulavirtual.uv.es";
    let wrong = payload(&signature(base, "other"), "tok-xyz");
    assert!(matches!(
        decode_launch_token(&wrong, base, "pass1"),
        Err(LmsError::Auth(_))
    ));
    assert!(matches!(
        decode_launch_token("%%%", base, "pass1"),
        Err(LmsError::Auth(_))
    ));
    let no_token = base64::engine::general_purpose::STANDARD.encode("onlysignature");
    assert!(matches!(
        decode_launch_token(&no_token, base, "pass1"),
        Err(LmsError::Auth(_))
    ));
}
