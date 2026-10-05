use std::path::PathBuf;

use academic_core::domain::{ItemKind, PlatformStatus, RemoteCourseRef, SyncWindow};
use academic_core::ports::{LmsClient, LmsError};
use academic_infra::lms::blackboard::{BlackboardClient, SessionCookie};
use chrono::{DateTime, Duration, SecondsFormat, TimeZone, Utc};
use serde_json::{json, Value};
use wiremock::matchers::{method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const ME: &str = "/learn/api/public/v1/users/me";
const CALENDAR: &str = "/learn/api/public/v1/calendars/items";

fn client(server: &MockServer) -> BlackboardClient {
    let cookies = [SessionCookie {
        name: "BbRouter".into(),
        value: "x".into(),
    }];
    BlackboardClient::new(reqwest::Client::new(), server.uri(), &cookies)
}

fn course(id: &str) -> RemoteCourseRef {
    RemoteCourseRef {
        external_id: id.into(),
    }
}

fn at(text: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(text)
        .unwrap()
        .with_timezone(&Utc)
}

fn short_window() -> SyncWindow {
    SyncWindow {
        from: at("2026-10-01T00:00:00Z"),
        to: at("2026-10-31T00:00:00Z"),
    }
}

fn columns_path(course: &str) -> String {
    format!("/learn/api/public/v2/courses/{course}/gradebook/columns")
}

fn grades_path(course: &str, user: &str) -> String {
    format!("/learn/api/public/v1/courses/{course}/gradebook/users/{user}")
}

fn attempts_path(course: &str, column: &str) -> String {
    format!("/learn/api/public/v1/courses/{course}/gradebook/columns/{column}/attempts")
}

/// A captured error response (`{"httpStatus": 403, ...}`) is replayed as that status.
fn response_for(body: Value) -> ResponseTemplate {
    match body["httpStatus"]
        .as_u64()
        .and_then(|s| u16::try_from(s).ok())
    {
        Some(status) => ResponseTemplate::new(status),
        None => ResponseTemplate::new(200).set_body_json(body),
    }
}

async fn mock_body(server: &MockServer, url_path: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .respond_with(response_for(body))
        .mount(server)
        .await;
}

async fn mock_status(server: &MockServer, url_path: &str, status: u16) {
    Mock::given(method("GET"))
        .and(path(url_path))
        .respond_with(ResponseTemplate::new(status))
        .mount(server)
        .await;
}

async fn mock_attempts(server: &MockServer, course: &str, column: &str, user: &str, body: Value) {
    Mock::given(method("GET"))
        .and(path(attempts_path(course, column)))
        .and(query_param("userId", user))
        .respond_with(response_for(body))
        .mount(server)
        .await;
}

async fn mock_me(server: &MockServer) {
    mock_body(server, ME, json!({"id": "_9_1", "userName": "u"})).await;
}

async fn mock_empty_calendar(server: &MockServer) {
    mock_body(server, CALENDAR, json!({"results": []})).await;
}

#[tokio::test]
async fn maps_columns_with_attempts_and_grades() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(
        &server,
        &columns_path("_10_1"),
        json!({"results": [
            {"id": "_1_1", "name": "Memoria", "contentId": "_100_1",
             "score": {"possible": 10},
             "grading": {"type": "Attempts", "due": "2026-10-15T21:59:00.000Z"},
             "scoreProviderHandle": "resource/x-bb-assignment"},
            {"id": "_2_1", "name": "Test 1", "contentId": "_101_1", "score": {"possible": 20},
             "grading": {"type": "Attempts"}, "scoreProviderHandle": "resource/x-bb-asmt-test-link"},
            {"id": "_3_1", "name": "Total", "score": {"possible": 100}, "grading": {"type": "Calculated"}},
            {"id": "_4_1", "name": "Participación", "score": {"possible": 5}, "grading": {"type": "Manual"}}
        ]}),
    )
    .await;
    mock_body(
        &server,
        &grades_path("_10_1", "_9_1"),
        json!({"results": [{"columnId": "_1_1", "score": 8.5, "status": "Graded"}]}),
    )
    .await;
    mock_attempts(
        &server,
        "_10_1",
        "_1_1",
        "_9_1",
        json!({"results": [{"id": "a", "status": "Completed"}]}),
    )
    .await;
    mock_attempts(
        &server,
        "_10_1",
        "_2_1",
        "_9_1",
        json!({"results": [{"id": "b", "status": "InProgress"}]}),
    )
    .await;
    mock_empty_calendar(&server).await;

    let items = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await
        .unwrap();

    let ids: Vec<&str> = items.iter().map(|i| i.external_id.as_str()).collect();
    assert_eq!(ids, ["column:_1_1", "column:_2_1", "column:_4_1"]);
    let outline = format!("{}/ultra/courses/_10_1/outline", server.uri());
    assert!(items
        .iter()
        .all(|i| i.course_external_id == "_10_1" && i.url.as_deref() == Some(outline.as_str())));

    let memoria = &items[0];
    assert_eq!(memoria.title, "Memoria");
    assert_eq!(memoria.kind, ItemKind::Assignment);
    assert_eq!(memoria.due_at, Some(at("2026-10-15T21:59:00Z")));
    assert_eq!(memoria.platform_status, PlatformStatus::Graded);
    let grade = memoria.grade.as_ref().unwrap();
    assert_eq!(grade.display, "8.5 / 10");
    assert_eq!((grade.score, grade.max), (Some(8.5), Some(10.0)));

    let test = &items[1];
    assert_eq!(test.kind, ItemKind::Test);
    assert_eq!(
        (test.due_at, test.platform_status, test.grade.clone()),
        (None, PlatformStatus::NotSubmitted, None)
    );

    let other = &items[2];
    assert_eq!(other.kind, ItemKind::Other);
    assert_eq!(other.platform_status, PlatformStatus::NotApplicable);

    let requests = server.received_requests().await.unwrap();
    assert!(
        !requests
            .iter()
            .any(|r| r.url.path() == attempts_path("_10_1", "_4_1")),
        "a column without contentId has no attempts request"
    );
}

#[tokio::test]
async fn completed_attempts_without_a_grade_are_submitted() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(
        &server,
        &columns_path("_10_1"),
        json!({"results": [
            {"id": "_1_1", "name": "Entrega", "contentId": "_100_1", "score": {"possible": 10},
             "grading": {"type": "Attempts"}, "scoreProviderHandle": "resource/x-bb-assignment"}
        ]}),
    )
    .await;
    mock_body(
        &server,
        &grades_path("_10_1", "_9_1"),
        json!({"results": []}),
    )
    .await;
    mock_attempts(
        &server,
        "_10_1",
        "_1_1",
        "_9_1",
        json!({"results": [
            {"id": "a", "status": "Abandoned"}, {"id": "b", "status": "NeedsGrading"}
        ]}),
    )
    .await;
    mock_empty_calendar(&server).await;

    let items = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].platform_status, PlatformStatus::Submitted);
    assert_eq!(items[0].grade, None);
}

#[tokio::test]
async fn attempts_or_grades_answering_404_mean_none() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(
        &server,
        &columns_path("_10_1"),
        json!({"results": [
            {"id": "_1_1", "name": "Memoria", "contentId": "_100_1", "score": {"possible": 10},
             "grading": {"type": "Attempts"}, "scoreProviderHandle": "resource/x-bb-assignment"}
        ]}),
    )
    .await;
    mock_status(&server, &grades_path("_10_1", "_9_1"), 404).await;
    mock_status(&server, &attempts_path("_10_1", "_1_1"), 404).await;
    mock_empty_calendar(&server).await;

    let items = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].external_id, "column:_1_1");
    assert_eq!(items[0].platform_status, PlatformStatus::NotSubmitted);
    assert_eq!(items[0].grade, None);
}

#[tokio::test]
async fn other_http_errors_fail_the_fetch() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_status(&server, &columns_path("_10_1"), 500).await;
    let result = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await;
    assert!(matches!(result, Err(LmsError::Protocol(_))));

    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_status(&server, &columns_path("_10_1"), 401).await;
    let result = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await;
    assert!(matches!(result, Err(LmsError::Auth(_))));
}

#[tokio::test]
async fn a_missing_columns_list_means_no_items() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_status(&server, &columns_path("_10_1"), 404).await;
    mock_empty_calendar(&server).await;

    let items = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await
        .unwrap();

    assert!(items.is_empty());
}

#[tokio::test]
async fn a_course_without_gradebook_access_is_skipped() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_status(&server, &columns_path("_10_1"), 403).await;
    mock_body(
        &server,
        &columns_path("_11_1"),
        json!({"results": [
            {"id": "_1_1", "name": "Memoria", "contentId": "_100_1", "score": {"possible": 10},
             "grading": {"type": "Attempts"}, "scoreProviderHandle": "resource/x-bb-assignment"}
        ]}),
    )
    .await;
    mock_body(
        &server,
        &grades_path("_11_1", "_9_1"),
        json!({"results": []}),
    )
    .await;
    mock_attempts(&server, "_11_1", "_1_1", "_9_1", json!({"results": []})).await;
    mock_empty_calendar(&server).await;

    let items = client(&server)
        .fetch_items(&[course("_10_1"), course("_11_1")], &short_window())
        .await
        .unwrap();

    let ids: Vec<&str> = items.iter().map(|i| i.external_id.as_str()).collect();
    assert_eq!(ids, ["column:_1_1"]);
    assert_eq!(items[0].course_external_id, "_11_1");
}

#[tokio::test]
async fn forbidden_user_grades_or_attempts_mean_none() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(
        &server,
        &columns_path("_10_1"),
        json!({"results": [
            {"id": "_1_1", "name": "Memoria", "contentId": "_100_1", "score": {"possible": 10},
             "grading": {"type": "Attempts"}, "scoreProviderHandle": "resource/x-bb-assignment"}
        ]}),
    )
    .await;
    mock_status(&server, &grades_path("_10_1", "_9_1"), 403).await;
    mock_status(&server, &attempts_path("_10_1", "_1_1"), 403).await;
    mock_empty_calendar(&server).await;

    let items = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].platform_status, PlatformStatus::NotSubmitted);
    assert_eq!(items[0].grade, None);
}

#[tokio::test]
async fn forbidden_outside_the_gradebook_is_still_an_expired_session() {
    let server = MockServer::start().await;
    mock_status(&server, ME, 403).await;
    let result = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await;
    assert!(matches!(result, Err(LmsError::Auth(_))));

    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(&server, &columns_path("_10_1"), json!({"results": []})).await;
    mock_status(&server, CALENDAR, 403).await;
    let result = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await;
    assert!(matches!(result, Err(LmsError::Auth(_))));
}

#[tokio::test]
async fn forbidden_on_a_later_page_is_an_error() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    let next = format!("{}?page=2", columns_path("_10_1"));
    Mock::given(method("GET"))
        .and(path(columns_path("_10_1")))
        .and(query_param("page", "2"))
        .respond_with(ResponseTemplate::new(403))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(columns_path("_10_1")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [],
            "paging": {"nextPage": next}
        })))
        .mount(&server)
        .await;

    let result = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await;

    assert!(matches!(result, Err(LmsError::Auth(_))));
}

#[tokio::test]
async fn maps_assessment_attendance_and_manual_columns() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(
        &server,
        &columns_path("_10_1"),
        json!({"results": [
            {"id": "_1_1", "name": "Práctica", "contentId": "_100_1", "score": {"possible": 10},
             "grading": {"type": "Attempts"}, "scoreProviderHandle": "resource/x-bb-assessment"},
            {"id": "_2_1", "name": "Asistencia", "contentId": "_101_1", "score": {"possible": 10},
             "grading": {"type": "Manual"}, "scoreProviderHandle": "resource/x-bb-attendance"},
            {"id": "_3_1", "name": "Participación", "score": {"possible": 5},
             "grading": {"type": "Manual"}},
            {"id": "_4_1", "name": "Actitud", "score": {"possible": 5},
             "grading": {"type": "Manual"}}
        ]}),
    )
    .await;
    mock_body(
        &server,
        &grades_path("_10_1", "_9_1"),
        json!({"results": [{"columnId": "_4_1", "score": 4.0}]}),
    )
    .await;
    mock_attempts(&server, "_10_1", "_1_1", "_9_1", json!({"results": []})).await;
    mock_empty_calendar(&server).await;

    let items = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await
        .unwrap();

    let summary: Vec<_> = items
        .iter()
        .map(|i| (i.external_id.as_str(), i.kind, i.platform_status))
        .collect();
    assert_eq!(
        summary,
        [
            (
                "column:_1_1",
                ItemKind::Assignment,
                PlatformStatus::NotSubmitted
            ),
            (
                "column:_3_1",
                ItemKind::Other,
                PlatformStatus::NotApplicable
            ),
            ("column:_4_1", ItemKind::Other, PlatformStatus::Graded),
        ]
    );
    let requests = server.received_requests().await.unwrap();
    assert!(
        !requests
            .iter()
            .any(|r| r.url.path() == attempts_path("_10_1", "_2_1")),
        "an attendance column is skipped before any attempts request"
    );
}

#[tokio::test]
async fn an_invalid_due_date_is_a_protocol_error() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(
        &server,
        &columns_path("_10_1"),
        json!({"results": [
            {"id": "_1_1", "name": "Memoria", "score": {"possible": 10},
             "grading": {"type": "Attempts", "due": "mañana"}}
        ]}),
    )
    .await;
    mock_body(
        &server,
        &grades_path("_10_1", "_9_1"),
        json!({"results": []}),
    )
    .await;
    mock_empty_calendar(&server).await;

    let result = client(&server)
        .fetch_items(&[course("_10_1")], &short_window())
        .await;

    assert!(matches!(result, Err(LmsError::Protocol(_))));
}

#[tokio::test]
async fn course_events_are_fetched_in_chunks_and_deduplicated() {
    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(&server, &columns_path("_10_1"), json!({"results": []})).await;
    mock_body(
        &server,
        &grades_path("_10_1", "_9_1"),
        json!({"results": []}),
    )
    .await;
    mock_body(
        &server,
        CALENDAR,
        json!({"results": [
            {"id": "_e1_1", "calendarId": "_10_1", "title": "Examen",
             "start": "2026-11-03T08:00:00.000Z", "type": "Course"},
            {"id": "_e2_1", "calendarId": "_other_1", "title": "x",
             "start": "2026-11-04T08:00:00.000Z", "type": "Course"}
        ]}),
    )
    .await;
    let from = at("2026-01-01T00:00:00Z");
    let window = SyncWindow {
        from,
        to: from + Duration::days(300),
    };

    let items = client(&server)
        .fetch_items(&[course("_10_1")], &window)
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    let event = &items[0];
    assert_eq!(event.external_id, "calendar:_e1_1");
    assert_eq!(event.course_external_id, "_10_1");
    assert_eq!(event.kind, ItemKind::Event);
    assert_eq!(event.title, "Examen");
    assert_eq!(event.platform_status, PlatformStatus::NotApplicable);
    assert_eq!(event.due_at, Some(at("2026-11-03T08:00:00Z")));
    assert_eq!(
        event.url.as_deref(),
        Some(format!("{}/ultra/calendar", server.uri()).as_str())
    );

    let requests = server.received_requests().await.unwrap();
    let ranges: Vec<(String, String)> = requests
        .iter()
        .filter(|r| r.url.path() == CALENDAR)
        .map(|r| {
            let param = |name: &str| {
                r.url
                    .query_pairs()
                    .find(|(key, _)| key == name)
                    .map(|(_, value)| value.into_owned())
                    .unwrap()
            };
            assert_eq!(param("type"), "Course");
            (param("since"), param("until"))
        })
        .collect();
    let format = |t: DateTime<Utc>| t.to_rfc3339_opts(SecondsFormat::Secs, true);
    assert_eq!(
        ranges,
        [
            (format(from), format(from + Duration::days(112))),
            (
                format(from + Duration::days(112)),
                format(from + Duration::days(224))
            ),
            (format(from + Duration::days(224)), format(window.to)),
        ]
    );
}

#[tokio::test]
async fn no_courses_means_no_requests() {
    let server = MockServer::start().await;

    let items = client(&server)
        .fetch_items(&[], &short_window())
        .await
        .unwrap();

    assert!(items.is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
}

fn read_fixture(name: &str) -> Option<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lms-local/blackboard")
        .join(name);
    let mut value: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    // A captured next-page link would point at a path the mocks do not serve.
    if let Some(object) = value.as_object_mut() {
        object.remove("paging");
    }
    Some(value)
}

fn ids_of(value: &Value, key: &str) -> Vec<String> {
    value["results"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry[key].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn captured_fixtures_parse() {
    let Some(first_columns) = read_fixture("columns_0.json") else {
        eprintln!("skipping: no local Blackboard captures");
        return;
    };
    let Some(courses) = read_fixture("courses.json") else {
        eprintln!("skipping: no local Blackboard course list");
        return;
    };
    let course_ids = ids_of(&courses, "courseId");

    let server = MockServer::start().await;
    mock_me(&server).await;
    mock_body(
        &server,
        CALENDAR,
        read_fixture("calendar_items.json").unwrap_or_else(|| json!({"results": []})),
    )
    .await;
    for (n, course_id) in course_ids.iter().enumerate() {
        let columns = if n == 0 {
            first_columns.clone()
        } else {
            read_fixture(&format!("columns_{n}.json")).unwrap_or_else(|| json!({"results": []}))
        };
        let with_content: Vec<String> = ids_of(&columns, "id")
            .into_iter()
            .zip(columns["results"].as_array().into_iter().flatten())
            .filter(|(_, column)| column["contentId"].is_string())
            .map(|(id, _)| id)
            .collect();
        mock_body(&server, &columns_path(course_id), columns).await;
        mock_body(
            &server,
            &grades_path(course_id, "_9_1"),
            read_fixture(&format!("user_grades_{n}.json"))
                .unwrap_or_else(|| json!({"results": []})),
        )
        .await;
        for (m, column_id) in with_content.iter().enumerate() {
            let attempts = read_fixture(&format!("attempts_{n}_{m}.json"))
                .unwrap_or_else(|| json!({"results": []}));
            mock_attempts(&server, course_id, column_id, "_9_1", attempts).await;
        }
    }
    // Any attempts request without its own capture sees no attempts.
    Mock::given(method("GET"))
        .and(path_regex(r"/gradebook/columns/[^/]+/attempts$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results": []})))
        .mount(&server)
        .await;

    let refs: Vec<RemoteCourseRef> = course_ids.iter().map(|id| course(id)).collect();
    let window = SyncWindow {
        from: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        to: Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
    };
    let items = client(&server).fetch_items(&refs, &window).await.unwrap();

    let mut ids: Vec<&str> = items.iter().map(|i| i.external_id.as_str()).collect();
    ids.sort_unstable();
    let total = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), total, "external ids must be unique");
}
