//! Fixture capture tool: calls the raw Moodle/Blackboard endpoints with the
//! credential the app stored (read-only), redacts personal text and writes one
//! JSON file per endpoint under the git-ignored
//! `crates/infra/tests/fixtures/lms-local/<platform>/`.
//!
//! Configuration (environment variables):
//! - `LMS_CAPTURE_UNIVERSITY_ID`: id of the university whose credential to use.
//! - `LMS_CAPTURE_PLATFORM`: `moodle` or `blackboard`.
//! - `LMS_CAPTURE_BASE_URL`: platform base URL, e.g. `https://campus.example.edu`.
//! - `LMS_CAPTURE_MAX_COURSES` (optional, default 3).
//!
//! It never prints credentials, cookies, tokens or response bodies.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use academic_core::domain::{lms_credential_key, LmsPlatform, UniversityId};
use academic_core::ports::SecretStore;
use academic_infra::factory::http_client;
use academic_infra::lms::blackboard::decode_session;
use academic_infra::secrets::KeyringSecretStore;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use url::form_urlencoded::{byte_serialize, Serializer};

const DEFAULT_MAX_COURSES: usize = 3;
const MAX_SUBMISSION_STATUSES: usize = 3;
const MAX_QUIZZES: usize = 2;
const MAX_ATTEMPT_COLUMNS: usize = 3;
const MOODLE_CALENDAR_PAST_DAYS: i64 = 180;
const MOODLE_CALENDAR_FUTURE_DAYS: i64 = 365;
const BLACKBOARD_CALENDAR_WINDOW_DAYS: i64 = 56;
const FIXTURES_DIR: &str = "tests/fixtures/lms-local";

type Params = Vec<(String, String)>;

/// Keys (lowercase, compared case-insensitively) whose string values are always
/// replaced, even single words or ids: personal data and free-text fields.
const REDACTED_KEYS: [&str; 22] = [
    "fullname",
    "firstname",
    "lastname",
    "username",
    "email",
    "given",
    "family",
    "idnumber",
    "shortname",
    "displayname",
    "sitename",
    "name",
    "title",
    "text",
    "feedback",
    "description",
    "intro",
    "summary",
    "comment",
    "gradefordisplay",
    "externalid",
    "uuid",
];
/// Keys whose values are personal results and get a fixed value.
const RESULT_KEY_FRAGMENTS: [&str; 3] = ["grade", "score", "mark"];
/// Result-looking keys that are 0/1 configuration flags, not results.
const FLAG_KEY_PREFIXES: [&str; 2] = ["marking", "blind"];
const FLAG_KEYS: [&str; 1] = ["hidegrader"];
const REDACTED_TEXT: &str = "redacted";
const REDACTED_NUMBER: f64 = 7.5;
const REDACTED_NUMBER_TEXT: &str = "7.5";
const MAX_KEPT_STRING_LEN: usize = 60;

/// Replaces personal data while keeping the JSON shape, ids, enum-like values,
/// timestamps and URLs: any string containing whitespace or longer than 60
/// characters becomes "redacted"; strings under free-text/personal keys always
/// do; numbers (and numeric strings) under grade/score/mark keys become 7.5
/// (unless they are ids named `id`/`*id` or, for numbers, 0, -1 or config flags).
fn redact(value: &mut Value, key: Option<&str>) {
    match value {
        Value::Object(map) => {
            for (child_key, child) in map.iter_mut() {
                redact(child, Some(child_key));
            }
        }
        Value::Array(items) => {
            for item in items.iter_mut() {
                redact(item, key);
            }
        }
        Value::String(text) => {
            let key = key.map(str::to_lowercase);
            if is_personal_text(text, key.as_deref()) {
                *text = REDACTED_TEXT.to_string();
            } else if is_result_key(key.as_deref()) && is_numeric_text(text) {
                *text = REDACTED_NUMBER_TEXT.to_string();
            }
        }
        Value::Number(number) => {
            if is_personal_result(number, key) {
                *value = json!(REDACTED_NUMBER);
            }
        }
        Value::Bool(_) | Value::Null => {}
    }
}

/// `key` is lowercase.
fn is_personal_text(text: &str, key: Option<&str>) -> bool {
    key.is_some_and(|k| REDACTED_KEYS.contains(&k))
        || text.chars().any(char::is_whitespace)
        || text.chars().count() > MAX_KEPT_STRING_LEN
}

/// `key` is lowercase. Ids (`id`, `gradeid`, ...) are never results.
fn is_result_key(key: Option<&str>) -> bool {
    key.is_some_and(|k| RESULT_KEY_FRAGMENTS.iter().any(|f| k.contains(f)) && !k.ends_with("id"))
}

fn is_numeric_text(text: &str) -> bool {
    text.replace(',', ".").parse::<f64>().is_ok()
}

fn is_personal_result(number: &serde_json::Number, key: Option<&str>) -> bool {
    let key = key.map(str::to_lowercase);
    let is_flag = key.as_deref().is_some_and(|k| {
        FLAG_KEYS.contains(&k) || FLAG_KEY_PREFIXES.iter().any(|p| k.starts_with(p))
    });
    let is_sentinel = matches!(number.as_f64(), Some(n) if n == 0.0 || n == -1.0);
    is_result_key(key.as_deref()) && !is_flag && !is_sentinel
}

struct CaptureConfig {
    university_id: UniversityId,
    platform: LmsPlatform,
    base_url: String,
    max_courses: usize,
}

fn required_env(name: &str) -> Result<String, String> {
    std::env::var(name)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("missing environment variable {name}"))
}

impl CaptureConfig {
    fn from_env() -> Result<Self, String> {
        let university_id = required_env("LMS_CAPTURE_UNIVERSITY_ID")?
            .parse()
            .map_err(|_| "LMS_CAPTURE_UNIVERSITY_ID must be an integer".to_string())?;
        let platform = required_env("LMS_CAPTURE_PLATFORM")?
            .parse()
            .map_err(|_| "LMS_CAPTURE_PLATFORM must be `moodle` or `blackboard`".to_string())?;
        let base_url = required_env("LMS_CAPTURE_BASE_URL")?
            .trim_end_matches('/')
            .to_string();
        let max_courses = match std::env::var("LMS_CAPTURE_MAX_COURSES") {
            Ok(raw) => raw
                .parse()
                .map_err(|_| "LMS_CAPTURE_MAX_COURSES must be an integer".to_string())?,
            Err(_) => DEFAULT_MAX_COURSES,
        };
        Ok(Self {
            university_id,
            platform,
            base_url,
            max_courses,
        })
    }
}

/// Writes redacted, pretty-printed captures into one platform directory.
struct FixtureWriter {
    dir: PathBuf,
    written: usize,
}

impl FixtureWriter {
    fn new(platform: LmsPlatform) -> Result<Self, String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(FIXTURES_DIR)
            .join(platform.as_str());
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("could not create {}: {e}", dir.display()))?;
        Ok(Self { dir, written: 0 })
    }

    fn write(&mut self, name: &str, response: &Value) -> Result<(), String> {
        let mut value = response.clone();
        redact(&mut value, None);
        let body = serde_json::to_string_pretty(&value)
            .map_err(|e| format!("could not serialize {name}: {e}"))?;
        let path = self.dir.join(format!("{name}.json"));
        std::fs::write(&path, &body)
            .map_err(|e| format!("could not write {}: {e}", path.display()))?;
        println!("wrote {} ({} bytes)", path.display(), body.len());
        self.written += 1;
        Ok(())
    }
}

/// Reads the response as JSON; a non-2xx status becomes `{"httpStatus": <code>}`.
async fn read_response(response: reqwest::Response) -> Result<Value, String> {
    let status = response.status();
    if !status.is_success() {
        return Ok(json!({ "httpStatus": status.as_u16() }));
    }
    let text = response
        .text()
        .await
        .map_err(|e| format!("could not read response: {}", e.without_url()))?;
    Ok(serde_json::from_str(&text)
        .unwrap_or_else(|_| json!({ "httpStatus": status.as_u16(), "invalidJson": true })))
}

/// Items of a JSON array (empty when the value is not an array).
fn items(value: &Value) -> impl Iterator<Item = &Value> {
    value.as_array().into_iter().flatten()
}

fn indexed(prefix: &str, values: &[i64]) -> Params {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| (format!("{prefix}[{i}]"), v.to_string()))
        .collect()
}

fn timestamp(days_from_now: i64) -> i64 {
    (Utc::now() + Duration::days(days_from_now)).timestamp()
}

// ---------------------------------------------------------------- Moodle

struct Moodle {
    http: reqwest::Client,
    endpoint: String,
    token: String,
}

impl Moodle {
    async fn call(&self, function: &str, params: Params) -> Result<Value, String> {
        let mut form = Serializer::new(String::new());
        form.append_pair("wstoken", &self.token)
            .append_pair("wsfunction", function)
            .append_pair("moodlewsrestformat", "json");
        for (name, value) in &params {
            form.append_pair(name, value);
        }
        let response = self
            .http
            .post(&self.endpoint)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(form.finish())
            .send()
            .await
            .map_err(|e| format!("request {function} failed: {}", e.without_url()))?;
        read_response(response).await
    }
}

async fn capture_moodle(config: &CaptureConfig, token: String) -> Result<usize, String> {
    let moodle = Moodle {
        http: http_client(),
        endpoint: format!("{}/webservice/rest/server.php", config.base_url),
        token,
    };
    let mut out = FixtureWriter::new(LmsPlatform::Moodle)?;

    // In memory only: the site info identifies the user and is never written.
    let site_info = moodle.call("core_webservice_get_site_info", vec![]).await?;
    let user_id = site_info
        .get("userid")
        .and_then(Value::as_i64)
        .ok_or("could not read the user id from the site info (is the token valid?)")?;

    let courses = moodle
        .call(
            "core_enrol_get_users_courses",
            vec![("userid".into(), user_id.to_string())],
        )
        .await?;
    out.write("courses", &courses)?;
    let course_ids: Vec<i64> = items(&courses)
        .filter_map(|c| c.get("id")?.as_i64())
        .take(config.max_courses)
        .collect();
    if course_ids.is_empty() {
        return Ok(out.written);
    }

    let assignments = moodle
        .call(
            "mod_assign_get_assignments",
            indexed("courseids", &course_ids),
        )
        .await?;
    out.write("assignments", &assignments)?;
    for (n, assign_id) in pick_assignments(&assignments).into_iter().enumerate() {
        let status = moodle
            .call(
                "mod_assign_get_submission_status",
                vec![("assignid".into(), assign_id.to_string())],
            )
            .await?;
        out.write(&format!("submission_status_{n}"), &status)?;
    }

    let quizzes = moodle
        .call(
            "mod_quiz_get_quizzes_by_courses",
            indexed("courseids", &course_ids),
        )
        .await?;
    out.write("quizzes", &quizzes)?;
    let quiz_ids = items(&quizzes["quizzes"])
        .filter_map(|q| q.get("id")?.as_i64())
        .take(MAX_QUIZZES);
    for (n, quiz_id) in quiz_ids.enumerate() {
        let quiz_param = ("quizid".to_string(), quiz_id.to_string());
        let attempts = moodle
            .call(
                "mod_quiz_get_user_attempts",
                vec![quiz_param.clone(), ("status".into(), "finished".into())],
            )
            .await?;
        out.write(&format!("quiz_attempts_{n}"), &attempts)?;
        let best = moodle
            .call("mod_quiz_get_user_best_grade", vec![quiz_param])
            .await?;
        out.write(&format!("quiz_best_grade_{n}"), &best)?;
    }

    let mut calendar_params = indexed("events[courseids]", &course_ids);
    calendar_params.extend([
        ("options[userevents]".into(), "0".into()),
        ("options[siteevents]".into(), "0".into()),
        (
            "options[timestart]".into(),
            timestamp(-MOODLE_CALENDAR_PAST_DAYS).to_string(),
        ),
        (
            "options[timeend]".into(),
            timestamp(MOODLE_CALENDAR_FUTURE_DAYS).to_string(),
        ),
    ]);
    let events = moodle
        .call("core_calendar_get_calendar_events", calendar_params)
        .await?;
    out.write("calendar_events", &events)?;
    Ok(out.written)
}

/// Up to three assignment ids, preferring the ones that have a due date.
fn pick_assignments(assignments: &Value) -> Vec<i64> {
    let mut found: Vec<(i64, i64)> = items(&assignments["courses"])
        .flat_map(|course| items(&course["assignments"]))
        .filter_map(|a| {
            let due = a.get("duedate").and_then(Value::as_i64).unwrap_or(0);
            Some((a.get("id")?.as_i64()?, due))
        })
        .collect();
    found.sort_by_key(|(_, due)| *due <= 0);
    found
        .into_iter()
        .take(MAX_SUBMISSION_STATUSES)
        .map(|(id, _)| id)
        .collect()
}

// ------------------------------------------------------------ Blackboard

struct Blackboard {
    http: reqwest::Client,
    base_url: String,
    cookie_header: String,
}

impl Blackboard {
    async fn get(&self, path_and_query: &str) -> Result<Value, String> {
        let response = self
            .http
            .get(format!("{}{path_and_query}", self.base_url))
            .header("Cookie", &self.cookie_header)
            .send()
            .await
            .map_err(|e| format!("request failed: {}", e.without_url()))?;
        read_response(response).await
    }
}

fn path_segment(raw: &str) -> String {
    byte_serialize(raw.as_bytes()).collect()
}

fn query(pairs: &[(&str, &str)]) -> String {
    let mut serializer = Serializer::new(String::new());
    serializer.extend_pairs(pairs);
    serializer.finish()
}

fn iso(days_from_now: i64) -> String {
    (Utc::now() + Duration::days(days_from_now))
        .format("%Y-%m-%dT%H:%M:%S.000Z")
        .to_string()
}

async fn capture_blackboard(config: &CaptureConfig, session_json: &str) -> Result<usize, String> {
    let cookies = decode_session(session_json).map_err(|e| e.to_string())?;
    let blackboard = Blackboard {
        http: http_client(),
        base_url: config.base_url.clone(),
        cookie_header: cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; "),
    };
    let mut out = FixtureWriter::new(LmsPlatform::Blackboard)?;

    // In memory only: the profile identifies the user and is never written.
    let me = blackboard.get("/learn/api/public/v1/users/me").await?;
    let user_id = me
        .get("id")
        .and_then(Value::as_str)
        .ok_or("could not read the user id (is the session still valid?)")?
        .to_string();

    let courses = blackboard
        .get("/learn/api/public/v1/users/me/courses?expand=course&limit=100")
        .await?;
    out.write("courses", &courses)?;
    let course_ids: Vec<String> = items(&courses["results"])
        .filter_map(|c| Some(c.get("courseId")?.as_str()?.to_string()))
        .take(config.max_courses)
        .collect();

    for (n, course_id) in course_ids.iter().enumerate() {
        let course = path_segment(course_id);
        let columns = blackboard
            .get(&format!(
                "/learn/api/public/v2/courses/{course}/gradebook/columns"
            ))
            .await?;
        out.write(&format!("columns_{n}"), &columns)?;

        let grades = blackboard
            .get(&format!(
                "/learn/api/public/v1/courses/{course}/gradebook/users/{}",
                path_segment(&user_id)
            ))
            .await?;
        out.write(&format!("user_grades_{n}"), &grades)?;

        let content_column_ids = items(&columns["results"])
            .filter(|c| c.get("contentId").is_some())
            .filter_map(|c| c.get("id")?.as_str())
            .take(MAX_ATTEMPT_COLUMNS);
        for (m, column_id) in content_column_ids.enumerate() {
            let attempts = blackboard
                .get(&format!(
                    "/learn/api/public/v2/courses/{course}/gradebook/columns/{}/attempts?{}",
                    path_segment(column_id),
                    query(&[("userId", &user_id)])
                ))
                .await?;
            out.write(&format!("attempts_{n}_{m}"), &attempts)?;
        }
    }

    let since = iso(-BLACKBOARD_CALENDAR_WINDOW_DAYS);
    let until = iso(BLACKBOARD_CALENDAR_WINDOW_DAYS);
    let calendar = blackboard
        .get(&format!(
            "/learn/api/public/v1/calendars/items?{}",
            query(&[("type", "Course"), ("since", &since), ("until", &until)])
        ))
        .await?;
    out.write("calendar_items", &calendar)?;
    Ok(out.written)
}

// ------------------------------------------------------------------ main

async fn run() -> Result<usize, String> {
    let config = CaptureConfig::from_env()?;
    let key = lms_credential_key(config.platform, config.university_id);
    let credential = KeyringSecretStore
        .get(&key)
        .map_err(|e| e.to_string())?
        .ok_or(
            "no stored credential for that university and platform; log in from the app first",
        )?;
    match config.platform {
        LmsPlatform::Moodle => capture_moodle(&config, credential).await,
        LmsPlatform::Blackboard => capture_blackboard(&config, &credential).await,
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(count) => {
            println!("done: {count} file(s) written");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn keeps_shape_ids_enums_and_timestamps() {
        let mut v = json!({
            "id": "_20_1", "status": "Completed", "due": "2026-10-15T21:59:00.000Z",
            "scoreProviderHandle": "resource/x-bb-assignment", "duedate": 1791000000,
            "url": "https://uah.blackboard.com/ultra/courses/_1_1/outline"
        });
        let before = v.clone();
        redact(&mut v, None);
        assert_eq!(v, before);
    }

    #[test]
    fn replaces_free_text_and_personal_keys() {
        let mut v = json!({
            "name": "Práctica 2 — Árboles AVL", "fullname": "Ana", "shortname": "CAL2",
            "intro": "x".repeat(80), "nested": [{ "title": "Examen parcial" }]
        });
        redact(&mut v, None);
        assert_eq!(v["name"], "redacted");
        assert_eq!(v["fullname"], "redacted");
        assert_eq!(v["shortname"], "redacted");
        assert_eq!(v["intro"], "redacted");
        assert_eq!(v["nested"][0]["title"], "redacted");
    }

    #[test]
    fn replaces_single_word_free_text_and_external_ids() {
        let mut v = json!({ "title": "Calculo", "externalId": "ABC123", "uuid": "u1",
                            "gradefordisplay": "A" });
        redact(&mut v, None);
        assert_eq!(v["title"], "redacted");
        assert_eq!(v["externalId"], "redacted");
        assert_eq!(v["uuid"], "redacted");
        assert_eq!(v["gradefordisplay"], "redacted");
    }

    #[test]
    fn replaces_numeric_string_grades_but_not_string_ids() {
        let mut v = json!({ "grade": "8.50000", "rawScore": "9,5", "gradeid": "55",
                            "gradeStatus": "Completed" });
        redact(&mut v, None);
        assert_eq!(v["grade"], "7.5");
        assert_eq!(v["rawScore"], "7.5");
        assert_eq!(v["gradeid"], "55");
        assert_eq!(v["gradeStatus"], "Completed");
    }

    #[test]
    fn keeps_numeric_config_flags_that_look_like_results() {
        let mut v = json!({ "markingworkflow": 1, "markingallocation": 1, "blindmarking": 1,
                            "hidegrader": 1 });
        let before = v.clone();
        redact(&mut v, None);
        assert_eq!(v, before);
    }

    #[test]
    fn replaces_result_numbers_but_not_ids_or_sentinels() {
        let mut v = json!({ "grade": 8.25, "score": 10, "gradeid": 55, "possible": 10,
                            "bestgrade": 0, "rawgrade": -1, "displayGrade": { "score": 9 } });
        redact(&mut v, None);
        assert_eq!(v["grade"], 7.5);
        assert_eq!(v["score"], 7.5);
        assert_eq!(v["gradeid"], 55);
        assert_eq!(v["possible"], 10);
        assert_eq!(v["bestgrade"], 0);
        assert_eq!(v["rawgrade"], -1);
        assert_eq!(v["displayGrade"]["score"], 7.5);
    }
}
