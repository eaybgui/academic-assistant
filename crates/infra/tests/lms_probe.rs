use academic_core::domain::LmsPlatform;
use academic_infra::lms::probe::{probe, ProbeError};
use serde_json::json;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn moodle_config(server: &MockServer, data: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/lib/ajax/service-nologin.php"))
        .and(query_param("info", "tool_mobile_get_public_config"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!([{ "error": false, "data": data }])),
        )
        .mount(server)
        .await;
}

#[tokio::test]
async fn moodle_with_mobile_services_passes() {
    let server = MockServer::start().await;
    moodle_config(
        &server,
        json!({ "enablewebservices": 1, "enablemobilewebservice": 1 }),
    )
    .await;
    probe(&reqwest::Client::new(), LmsPlatform::Moodle, &server.uri())
        .await
        .unwrap();
}

#[tokio::test]
async fn moodle_without_mobile_services_is_a_mismatch_with_a_clear_message() {
    let server = MockServer::start().await;
    moodle_config(
        &server,
        json!({ "enablewebservices": 1, "enablemobilewebservice": 0 }),
    )
    .await;
    match probe(&reqwest::Client::new(), LmsPlatform::Moodle, &server.uri()).await {
        Err(ProbeError::Mismatch(m)) => {
            assert_eq!(m, "Mobile web services are disabled on this Moodle site")
        }
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn a_non_moodle_site_is_a_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    match probe(&reqwest::Client::new(), LmsPlatform::Moodle, &server.uri()).await {
        Err(ProbeError::Mismatch(m)) => assert_eq!(m, "This URL is not a Moodle site"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn blackboard_version_endpoint_passes_and_others_mismatch() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/learn/api/public/v1/system/version"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "learn": { "major": 4001 } })),
        )
        .mount(&server)
        .await;
    probe(
        &reqwest::Client::new(),
        LmsPlatform::Blackboard,
        &server.uri(),
    )
    .await
    .unwrap();

    let other = MockServer::start().await;
    match probe(
        &reqwest::Client::new(),
        LmsPlatform::Blackboard,
        &other.uri(),
    )
    .await
    {
        Err(ProbeError::Mismatch(m)) => assert_eq!(m, "This URL is not a Blackboard Learn site"),
        r => panic!("{r:?}"),
    }
}

#[tokio::test]
async fn unreachable_host_is_a_connection_error() {
    let result = probe(
        &reqwest::Client::new(),
        LmsPlatform::Moodle,
        "https://127.0.0.1:9",
    )
    .await;
    assert!(matches!(result, Err(ProbeError::Connection(_))));
}
