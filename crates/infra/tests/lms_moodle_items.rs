use std::path::PathBuf;

use academic_core::domain::{ItemKind, PlatformStatus, RemoteCourseRef, SyncWindow};
use academic_core::ports::{LmsClient, LmsError};
use academic_infra::lms::moodle::MoodleClient;
use chrono::{TimeZone, Utc};
use serde_json::{json, Value};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

const ENDPOINT: &str = "/webservice/rest/server.php";

fn form_param(request: &Request, name: &str) -> Option<String> {
    url::form_urlencoded::parse(&request.body)
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.into_owned())
}

/// Mounts a response for `function`, restricted to requests whose form carries every `params` pair.
async fn mock_fn(server: &MockServer, function: &str, params: &[(&str, &str)], body: Value) {
    let function = function.to_string();
    let params: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Mock::given(move |request: &Request| {
        request.url.path() == ENDPOINT
            && form_param(request, "wsfunction").as_deref() == Some(function.as_str())
            && params
                .iter()
                .all(|(k, v)| form_param(request, k).as_deref() == Some(v.as_str()))
    })
    .respond_with(ResponseTemplate::new(200).set_body_json(body))
    .mount(server)
    .await;
}

fn client(server: &MockServer) -> MoodleClient {
    MoodleClient::new(reqwest::Client::new(), server.uri(), "tok".into())
}

fn course(id: &str) -> RemoteCourseRef {
    RemoteCourseRef {
        external_id: id.into(),
    }
}

fn window() -> SyncWindow {
    SyncWindow {
        from: Utc.timestamp_opt(1_700_000_000, 0).unwrap(),
        to: Utc.timestamp_opt(1_800_000_000, 0).unwrap(),
    }
}

async fn mock_empty_quizzes_and_events(server: &MockServer) {
    mock_fn(
        server,
        "mod_quiz_get_quizzes_by_courses",
        &[],
        json!({"quizzes": []}),
    )
    .await;
    mock_fn(
        server,
        "core_calendar_get_calendar_events",
        &[],
        json!({"events": []}),
    )
    .await;
}

fn assignments_body(assignments: Value) -> Value {
    json!({"courses": [{"id": 12, "assignments": assignments}]})
}

#[tokio::test]
async fn maps_assignments_with_status_grade_and_missing_due_date() {
    let server = MockServer::start().await;
    mock_fn(
        &server,
        "mod_assign_get_assignments",
        &[],
        assignments_body(json!([
            {"id": 5, "cmid": 50, "name": "P1", "duedate": 1_791_000_000, "grade": 10, "nosubmissions": 0},
            {"id": 6, "cmid": 60, "name": "P2", "duedate": 0, "grade": 10, "nosubmissions": 0}
        ])),
    )
    .await;
    mock_fn(
        &server,
        "mod_assign_get_submission_status",
        &[("assignid", "5")],
        json!({
            "lastattempt": {"submission": {"status": "submitted"}},
            "feedback": {"gradefordisplay": "<span>8,50</span> / 10,00", "grade": {"grade": "8.50000"}}
        }),
    )
    .await;
    mock_fn(
        &server,
        "mod_assign_get_submission_status",
        &[("assignid", "6")],
        json!({"lastattempt": {"submission": {"status": "new"}}}),
    )
    .await;
    mock_empty_quizzes_and_events(&server).await;

    let items = client(&server)
        .fetch_items(&[course("12")], &window())
        .await
        .unwrap();

    let p1 = items.iter().find(|i| i.external_id == "assign:5").unwrap();
    assert_eq!(p1.course_external_id, "12");
    assert_eq!(p1.kind, ItemKind::Assignment);
    assert_eq!(p1.title, "P1");
    assert_eq!(p1.due_at.unwrap().timestamp(), 1_791_000_000);
    assert_eq!(
        p1.url.as_deref(),
        Some(format!("{}/mod/assign/view.php?id=50", server.uri()).as_str())
    );
    assert_eq!(p1.platform_status, PlatformStatus::Graded);
    let grade = p1.grade.as_ref().unwrap();
    assert_eq!(grade.display, "8,50 / 10,00");
    assert_eq!((grade.score, grade.max), (Some(8.5), Some(10.0)));
    let p2 = items.iter().find(|i| i.external_id == "assign:6").unwrap();
    assert_eq!(
        (p2.due_at, p2.platform_status, p2.grade.clone()),
        (None, PlatformStatus::NotSubmitted, None)
    );
}

#[tokio::test]
async fn team_submissions_and_offline_assignments() {
    let server = MockServer::start().await;
    mock_fn(
        &server,
        "mod_assign_get_assignments",
        &[],
        assignments_body(json!([
            {"id": 7, "cmid": 70, "name": "Team", "duedate": 0, "grade": 10, "nosubmissions": 0, "teamsubmission": 1},
            {"id": 8, "cmid": 80, "name": "Offline", "duedate": 0, "grade": 10, "nosubmissions": 1}
        ])),
    )
    .await;
    mock_fn(
        &server,
        "mod_assign_get_submission_status",
        &[("assignid", "7")],
        json!({"lastattempt": {"teamsubmission": {"status": "submitted"}}}),
    )
    .await;
    mock_fn(
        &server,
        "mod_assign_get_submission_status",
        &[("assignid", "8")],
        json!({"lastattempt": {}}),
    )
    .await;
    mock_empty_quizzes_and_events(&server).await;

    let items = client(&server)
        .fetch_items(&[course("12")], &window())
        .await
        .unwrap();

    let status_of = |id: &str| {
        items
            .iter()
            .find(|i| i.external_id == id)
            .unwrap()
            .platform_status
    };
    assert_eq!(status_of("assign:7"), PlatformStatus::Submitted);
    assert_eq!(status_of("assign:8"), PlatformStatus::NotApplicable);
}

#[tokio::test]
async fn placeholder_feedback_without_a_grade_is_not_graded() {
    let server = MockServer::start().await;
    mock_fn(
        &server,
        "mod_assign_get_assignments",
        &[],
        assignments_body(json!([
            {"id": 9, "cmid": 90, "name": "P9", "duedate": 0, "grade": 10, "nosubmissions": 0}
        ])),
    )
    .await;
    mock_fn(
        &server,
        "mod_assign_get_submission_status",
        &[("assignid", "9")],
        json!({
            "lastattempt": {"submission": {"status": "new"}},
            "feedback": {"gradefordisplay": "-", "grade": {"grade": "-1.00000"}}
        }),
    )
    .await;
    mock_empty_quizzes_and_events(&server).await;

    let items = client(&server)
        .fetch_items(&[course("12")], &window())
        .await
        .unwrap();

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].platform_status, PlatformStatus::NotSubmitted);
    assert_eq!(items[0].grade, None);
}

#[tokio::test]
async fn maps_quizzes_with_finished_attempts_and_best_grade() {
    let server = MockServer::start().await;
    mock_fn(
        &server,
        "mod_assign_get_assignments",
        &[],
        json!({"courses": []}),
    )
    .await;
    mock_fn(
        &server,
        "mod_quiz_get_quizzes_by_courses",
        &[],
        json!({"quizzes": [
            {"id": 3, "course": 12, "coursemodule": 30, "name": "Q1", "timeclose": 1_791_000_000, "grade": 10},
            {"id": 4, "course": 12, "coursemodule": 40, "name": "Q2", "timeclose": 0, "grade": 10}
        ]}),
    )
    .await;
    mock_fn(
        &server,
        "mod_quiz_get_user_attempts",
        &[("quizid", "3"), ("status", "finished")],
        json!({"attempts": [{"id": 1, "state": "finished"}]}),
    )
    .await;
    mock_fn(
        &server,
        "mod_quiz_get_user_best_grade",
        &[("quizid", "3")],
        json!({"hasgrade": true, "grade": 7.25}),
    )
    .await;
    mock_fn(
        &server,
        "mod_quiz_get_user_attempts",
        &[("quizid", "4"), ("status", "finished")],
        json!({"attempts": []}),
    )
    .await;
    mock_fn(
        &server,
        "mod_quiz_get_user_best_grade",
        &[("quizid", "4")],
        json!({"hasgrade": false}),
    )
    .await;
    mock_fn(
        &server,
        "core_calendar_get_calendar_events",
        &[],
        json!({"events": []}),
    )
    .await;

    let items = client(&server)
        .fetch_items(&[course("12")], &window())
        .await
        .unwrap();

    let q1 = items.iter().find(|i| i.external_id == "quiz:3").unwrap();
    assert_eq!(q1.kind, ItemKind::Quiz);
    assert_eq!(q1.course_external_id, "12");
    assert_eq!(q1.platform_status, PlatformStatus::Graded);
    assert_eq!(q1.due_at.unwrap().timestamp(), 1_791_000_000);
    assert_eq!(
        q1.url.as_deref(),
        Some(format!("{}/mod/quiz/view.php?id=30", server.uri()).as_str())
    );
    let grade = q1.grade.as_ref().unwrap();
    assert_eq!(grade.display, "7.25 / 10");
    assert_eq!((grade.score, grade.max), (Some(7.25), Some(10.0)));
    let q2 = items.iter().find(|i| i.external_id == "quiz:4").unwrap();
    assert_eq!(
        (q2.due_at, q2.platform_status, q2.grade.clone()),
        (None, PlatformStatus::NotSubmitted, None)
    );
}

#[tokio::test]
async fn keeps_only_course_and_group_events_without_a_module() {
    let server = MockServer::start().await;
    mock_fn(
        &server,
        "mod_assign_get_assignments",
        &[],
        json!({"courses": []}),
    )
    .await;
    mock_fn(
        &server,
        "mod_quiz_get_quizzes_by_courses",
        &[],
        json!({"quizzes": []}),
    )
    .await;
    mock_fn(
        &server,
        "core_calendar_get_calendar_events",
        &[],
        json!({"events": [
            {"id": 90, "name": "E1", "courseid": 12, "eventtype": "course", "modulename": null, "timestart": 1_791_000_000},
            {"id": 91, "name": "E2", "courseid": 12, "eventtype": "group", "modulename": "", "timestart": 1_791_003_600},
            {"id": 92, "name": "E3", "courseid": 12, "eventtype": "due", "modulename": "assign", "timestart": 1_791_000_000},
            {"id": 93, "name": "E4", "courseid": 12, "eventtype": "user", "timestart": 1_791_000_000},
            {"id": 94, "name": "E5", "courseid": 12, "eventtype": "attendance", "modulename": "attendance", "timestart": 1_791_000_000}
        ]}),
    )
    .await;

    let items = client(&server)
        .fetch_items(&[course("12")], &window())
        .await
        .unwrap();

    let ids: Vec<&str> = items.iter().map(|i| i.external_id.as_str()).collect();
    assert_eq!(ids, ["event:90", "event:91"]);
    let e1 = &items[0];
    assert_eq!(e1.kind, ItemKind::Event);
    assert_eq!(e1.platform_status, PlatformStatus::NotApplicable);
    assert_eq!(e1.course_external_id, "12");
    assert_eq!(e1.due_at.unwrap().timestamp(), 1_791_000_000);
    assert_eq!(
        e1.url.as_deref(),
        Some(
            format!(
                "{}/calendar/view.php?view=day&time=1791000000",
                server.uri()
            )
            .as_str()
        )
    );

    let requests = server.received_requests().await.unwrap();
    let events_request = requests
        .iter()
        .find(|r| {
            form_param(r, "wsfunction").as_deref() == Some("core_calendar_get_calendar_events")
        })
        .unwrap();
    let body = String::from_utf8(events_request.body.clone()).unwrap();
    assert!(body.contains("events%5Bcourseids%5D%5B0%5D=12"), "{body}");
    assert!(body.contains("options%5Buserevents%5D=0"), "{body}");
    assert!(body.contains("options%5Bsiteevents%5D=0"), "{body}");
    assert!(body.contains("options%5Btimestart%5D=1700000000"), "{body}");
    assert!(body.contains("options%5Btimeend%5D=1800000000"), "{body}");
}

#[tokio::test]
async fn any_failed_call_fails_the_whole_fetch() {
    let server = MockServer::start().await;
    mock_fn(
        &server,
        "mod_assign_get_assignments",
        &[],
        assignments_body(json!([
            {"id": 5, "cmid": 50, "name": "P1", "duedate": 0, "grade": 10, "nosubmissions": 0}
        ])),
    )
    .await;
    mock_fn(
        &server,
        "mod_assign_get_submission_status",
        &[],
        json!({"exception": "x", "errorcode": "invalidtoken"}),
    )
    .await;
    mock_empty_quizzes_and_events(&server).await;

    let result = client(&server)
        .fetch_items(&[course("12")], &window())
        .await;

    assert!(matches!(result, Err(LmsError::Auth(_))));
}

#[tokio::test]
async fn non_numeric_course_ids_are_rejected() {
    let server = MockServer::start().await;

    let result = client(&server)
        .fetch_items(&[course("abc")], &window())
        .await;

    assert!(matches!(result, Err(LmsError::Protocol(_))));
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn no_courses_means_no_requests() {
    let server = MockServer::start().await;

    let items = client(&server).fetch_items(&[], &window()).await.unwrap();

    assert!(items.is_empty());
    assert!(server.received_requests().await.unwrap().is_empty());
}

fn read_fixture(name: &str) -> Option<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/lms-local/moodle")
        .join(name);
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Numbered fixture files `<prefix>_0.json`, `<prefix>_1.json`, ... in order.
fn numbered_fixtures(prefix: &str) -> Vec<Value> {
    (0..)
        .map_while(|n| read_fixture(&format!("{prefix}_{n}.json")))
        .collect()
}

/// Mounts the numbered fixtures for `function`, one per id in order (matched on `id_param`),
/// and the first fixture for every other id.
async fn mount_numbered(
    server: &MockServer,
    function: &str,
    id_param: &str,
    ids: &[i64],
    fixtures: &[Value],
) {
    let Some(first) = fixtures.first() else {
        return;
    };
    for (id, fixture) in ids.iter().zip(fixtures) {
        mock_fn(
            server,
            function,
            &[(id_param, id.to_string().as_str())],
            fixture.clone(),
        )
        .await;
    }
    mock_fn(server, function, &[], first.clone()).await;
}

fn ids_of(value: &Value, key: &str) -> Vec<i64> {
    value[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry["id"].as_i64())
        .collect()
}

#[tokio::test]
async fn captured_fixtures_parse() {
    let Some(assignments) = read_fixture("assignments.json") else {
        eprintln!("skipping: no local Moodle captures");
        return;
    };
    let quizzes = read_fixture("quizzes.json").unwrap_or_else(|| json!({"quizzes": []}));
    let events = read_fixture("calendar_events.json").unwrap_or_else(|| json!({"events": []}));

    let course_ids = ids_of(&assignments, "courses");
    let assign_ids: Vec<i64> = assignments["courses"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|c| ids_of(c, "assignments"))
        .collect();
    let quiz_ids = ids_of(&quizzes, "quizzes");

    let server = MockServer::start().await;
    mock_fn(&server, "mod_assign_get_assignments", &[], assignments).await;
    mock_fn(&server, "mod_quiz_get_quizzes_by_courses", &[], quizzes).await;
    mock_fn(&server, "core_calendar_get_calendar_events", &[], events).await;
    mount_numbered(
        &server,
        "mod_assign_get_submission_status",
        "assignid",
        &assign_ids,
        &numbered_fixtures("submission_status"),
    )
    .await;
    mount_numbered(
        &server,
        "mod_quiz_get_user_attempts",
        "quizid",
        &quiz_ids,
        &numbered_fixtures("quiz_attempts"),
    )
    .await;
    mount_numbered(
        &server,
        "mod_quiz_get_user_best_grade",
        "quizid",
        &quiz_ids,
        &numbered_fixtures("quiz_best_grade"),
    )
    .await;

    let courses: Vec<RemoteCourseRef> = course_ids
        .iter()
        .map(|id| course(&id.to_string()))
        .collect();
    let items = client(&server)
        .fetch_items(&courses, &window())
        .await
        .unwrap();

    assert!(!items.is_empty());
    let mut ids: Vec<&str> = items.iter().map(|i| i.external_id.as_str()).collect();
    ids.sort_unstable();
    let total = ids.len();
    ids.dedup();
    assert_eq!(ids.len(), total, "external ids must be unique");
}
