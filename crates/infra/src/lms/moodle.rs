//! Moodle web-service client (mobile service token) and launch-token decoding (spec §5.1, §6.2).

use academic_core::domain::{
    Grade, ItemKind, LmsIdentity, PlatformStatus, RemoteCourse, RemoteCourseRef, RemoteItem,
    SyncWindow,
};
use academic_core::ports::{LmsClient, LmsError};
use async_trait::async_trait;
use base64::Engine;
use md5::{Digest, Md5};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use super::{connection_error, format_number, strip_tags, unix_to_utc, REQUEST_TIMEOUT};
use crate::http_util::encode_form;

/// Functions Phase 2 needs from the mobile service. `verify` fails when one is missing.
const REQUIRED_FUNCTIONS: [&str; 7] = [
    "core_enrol_get_users_courses",
    "mod_assign_get_assignments",
    "mod_assign_get_submission_status",
    "mod_quiz_get_quizzes_by_courses",
    "mod_quiz_get_user_attempts",
    "mod_quiz_get_user_best_grade",
    "core_calendar_get_calendar_events",
];

pub struct MoodleClient {
    http: reqwest::Client,
    base_url: String,
    token: String,
}

#[derive(Deserialize)]
struct SiteInfo {
    userid: i64,
    fullname: String,
    #[serde(default)]
    functions: Vec<SiteFunction>,
}

#[derive(Deserialize)]
struct SiteFunction {
    name: String,
}

#[derive(Deserialize)]
struct MoodleCourse {
    id: i64,
    fullname: String,
    #[serde(default)]
    shortname: String,
}

#[derive(Deserialize)]
struct AssignmentsResponse {
    #[serde(default)]
    courses: Vec<AssignCourse>,
}

#[derive(Deserialize)]
struct AssignCourse {
    id: i64,
    #[serde(default)]
    assignments: Vec<Assignment>,
}

#[derive(Deserialize)]
struct Assignment {
    id: i64,
    cmid: i64,
    name: String,
    #[serde(default)]
    duedate: i64,
    #[serde(default)]
    grade: f64,
    #[serde(default)]
    nosubmissions: i64,
}

#[derive(Deserialize)]
struct SubmissionStatus {
    lastattempt: Option<LastAttempt>,
    feedback: Option<Feedback>,
}

#[derive(Deserialize)]
struct LastAttempt {
    submission: Option<Submission>,
    teamsubmission: Option<Submission>,
}

#[derive(Deserialize)]
struct Submission {
    status: String,
}

#[derive(Deserialize)]
struct Feedback {
    gradefordisplay: Option<String>,
    grade: Option<FeedbackGrade>,
}

#[derive(Deserialize)]
struct FeedbackGrade {
    grade: Option<String>,
}

#[derive(Deserialize)]
struct QuizzesResponse {
    #[serde(default)]
    quizzes: Vec<Quiz>,
}

#[derive(Deserialize)]
struct Quiz {
    id: i64,
    course: i64,
    coursemodule: i64,
    name: String,
    #[serde(default)]
    timeclose: i64,
    #[serde(default)]
    grade: f64,
}

#[derive(Deserialize)]
struct Attempts {
    #[serde(default)]
    attempts: Vec<Value>,
}

#[derive(Deserialize)]
struct BestGrade {
    #[serde(default)]
    hasgrade: bool,
    grade: Option<f64>,
}

#[derive(Deserialize)]
struct EventsResponse {
    #[serde(default)]
    events: Vec<CalendarEvent>,
}

#[derive(Deserialize)]
struct CalendarEvent {
    id: i64,
    name: String,
    courseid: i64,
    #[serde(default)]
    eventtype: String,
    modulename: Option<String>,
    #[serde(default)]
    timestart: i64,
}

impl MoodleClient {
    pub fn new(http: reqwest::Client, base_url: String, token: String) -> Self {
        Self {
            http,
            base_url,
            token,
        }
    }

    async fn call<T: DeserializeOwned>(
        &self,
        function: &str,
        args: &[(&str, &str)],
    ) -> Result<T, LmsError> {
        let mut form: Vec<(&str, &str)> = vec![
            ("wstoken", self.token.as_str()),
            ("wsfunction", function),
            ("moodlewsrestformat", "json"),
        ];
        form.extend_from_slice(args);
        let response = self
            .http
            .post(format!("{}/webservice/rest/server.php", self.base_url))
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(encode_form(&form))
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|e| connection_error(&e))?;
        let status = response.status();
        if !status.is_success() {
            return Err(LmsError::Protocol(format!(
                "{function} returned HTTP {}",
                status.as_u16()
            )));
        }
        let value: Value = response.json().await.map_err(|_| {
            LmsError::Protocol(format!("{function} returned an unexpected response"))
        })?;
        if let Some(code) = value.get("errorcode").and_then(Value::as_str) {
            return Err(map_error_code(function, code));
        }
        serde_json::from_value(value)
            .map_err(|_| LmsError::Protocol(format!("{function} returned an unexpected response")))
    }

    async fn call_with_params<T: DeserializeOwned>(
        &self,
        function: &str,
        params: &[(String, String)],
    ) -> Result<T, LmsError> {
        let args: Vec<(&str, &str)> = params
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        self.call(function, &args).await
    }

    async fn fetch_assignments(&self, course_ids: &[i64]) -> Result<Vec<RemoteItem>, LmsError> {
        let response: AssignmentsResponse = self
            .call_with_params(
                "mod_assign_get_assignments",
                &indexed_params("courseids", course_ids),
            )
            .await?;
        let mut items = Vec::new();
        for course in response.courses {
            for assignment in course.assignments {
                let status: SubmissionStatus = self
                    .call(
                        "mod_assign_get_submission_status",
                        &[("assignid", assignment.id.to_string().as_str())],
                    )
                    .await?;
                let (platform_status, grade) = assignment_outcome(&assignment, &status);
                items.push(RemoteItem {
                    course_external_id: course.id.to_string(),
                    external_id: format!("assign:{}", assignment.id),
                    kind: ItemKind::Assignment,
                    title: assignment.name,
                    due_at: unix_to_utc(assignment.duedate),
                    url: Some(format!(
                        "{}/mod/assign/view.php?id={}",
                        self.base_url, assignment.cmid
                    )),
                    platform_status,
                    grade,
                });
            }
        }
        Ok(items)
    }

    async fn fetch_quizzes(&self, course_ids: &[i64]) -> Result<Vec<RemoteItem>, LmsError> {
        let response: QuizzesResponse = self
            .call_with_params(
                "mod_quiz_get_quizzes_by_courses",
                &indexed_params("courseids", course_ids),
            )
            .await?;
        let mut items = Vec::new();
        for quiz in response.quizzes {
            let quiz_id = quiz.id.to_string();
            let attempts: Attempts = self
                .call(
                    "mod_quiz_get_user_attempts",
                    &[("quizid", quiz_id.as_str()), ("status", "finished")],
                )
                .await?;
            let best: BestGrade = self
                .call(
                    "mod_quiz_get_user_best_grade",
                    &[("quizid", quiz_id.as_str())],
                )
                .await?;
            let grade = quiz_grade(&quiz, &best);
            let platform_status = if grade.is_some() {
                PlatformStatus::Graded
            } else if attempts.attempts.is_empty() {
                PlatformStatus::NotSubmitted
            } else {
                PlatformStatus::Submitted
            };
            items.push(RemoteItem {
                course_external_id: quiz.course.to_string(),
                external_id: format!("quiz:{}", quiz.id),
                kind: ItemKind::Quiz,
                title: quiz.name,
                due_at: unix_to_utc(quiz.timeclose),
                url: Some(format!(
                    "{}/mod/quiz/view.php?id={}",
                    self.base_url, quiz.coursemodule
                )),
                platform_status,
                grade,
            });
        }
        Ok(items)
    }

    async fn fetch_events(
        &self,
        course_ids: &[i64],
        window: &SyncWindow,
    ) -> Result<Vec<RemoteItem>, LmsError> {
        let mut params = indexed_params("events[courseids]", course_ids);
        params.extend([
            ("options[userevents]".to_string(), "0".to_string()),
            ("options[siteevents]".to_string(), "0".to_string()),
            (
                "options[timestart]".to_string(),
                window.from.timestamp().to_string(),
            ),
            (
                "options[timeend]".to_string(),
                window.to.timestamp().to_string(),
            ),
        ]);
        let response: EventsResponse = self
            .call_with_params("core_calendar_get_calendar_events", &params)
            .await?;
        Ok(response
            .events
            .into_iter()
            .filter(is_course_level_event)
            .map(|event| RemoteItem {
                course_external_id: event.courseid.to_string(),
                external_id: format!("event:{}", event.id),
                kind: ItemKind::Event,
                title: event.name,
                due_at: unix_to_utc(event.timestart),
                url: Some(format!(
                    "{}/calendar/view.php?view=day&time={}",
                    self.base_url, event.timestart
                )),
                platform_status: PlatformStatus::NotApplicable,
                grade: None,
            })
            .collect())
    }

    async fn site_info(&self) -> Result<SiteInfo, LmsError> {
        self.call("core_webservice_get_site_info", &[]).await
    }
}

fn parse_course_ids(courses: &[RemoteCourseRef]) -> Result<Vec<i64>, LmsError> {
    courses
        .iter()
        .map(|course| {
            course
                .external_id
                .parse()
                .map_err(|_| LmsError::Protocol("Invalid Moodle course id".into()))
        })
        .collect()
}

/// `name[0]=v0, name[1]=v1, ...` form parameters for a Moodle array argument.
fn indexed_params(name: &str, values: &[i64]) -> Vec<(String, String)> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (format!("{name}[{index}]"), value.to_string()))
        .collect()
}

/// Status and grade of an assignment. Moodle returns a `feedback` placeholder
/// ("-" and a negative grade) for unmarked work, so it only counts as graded when
/// the display text is real and the numeric grade is not negative.
fn assignment_outcome(
    assignment: &Assignment,
    status: &SubmissionStatus,
) -> (PlatformStatus, Option<Grade>) {
    if let Some(grade) = status
        .feedback
        .as_ref()
        .and_then(|feedback| feedback_grade(feedback, assignment))
    {
        return (PlatformStatus::Graded, Some(grade));
    }
    let submitted = status.lastattempt.as_ref().is_some_and(|attempt| {
        [&attempt.submission, &attempt.teamsubmission]
            .into_iter()
            .flatten()
            .any(|submission| submission.status == "submitted")
    });
    let platform_status = if assignment.nosubmissions == 1 {
        PlatformStatus::NotApplicable
    } else if submitted {
        PlatformStatus::Submitted
    } else {
        PlatformStatus::NotSubmitted
    };
    (platform_status, None)
}

fn feedback_grade(feedback: &Feedback, assignment: &Assignment) -> Option<Grade> {
    let display = strip_tags(feedback.gradefordisplay.as_deref()?);
    if display.is_empty() || display == "-" {
        return None;
    }
    let score: f64 = feedback
        .grade
        .as_ref()?
        .grade
        .as_deref()?
        .replace(',', ".")
        .parse()
        .ok()?;
    (score >= 0.0).then(|| Grade {
        display,
        score: Some(score),
        max: Some(assignment.grade).filter(|max| *max > 0.0),
    })
}

fn quiz_grade(quiz: &Quiz, best: &BestGrade) -> Option<Grade> {
    let score = best.grade.filter(|_| best.hasgrade)?;
    let max = Some(quiz.grade).filter(|max| *max > 0.0);
    let display = match max {
        Some(max) => format!("{} / {}", format_number(score), format_number(max)),
        None => format_number(score),
    };
    Some(Grade {
        display,
        score: Some(score),
        max,
    })
}

/// Course and group events not tied to an activity: assignments and quizzes already
/// appear as items of their own.
fn is_course_level_event(event: &CalendarEvent) -> bool {
    matches!(event.eventtype.as_str(), "course" | "group")
        && event.modulename.as_deref().is_none_or(str::is_empty)
}

/// Moodle reports an expired token (`validuntil`) as `accessexception` from the
/// verify call (`core_webservice_get_site_info`), so that case needs a new sign-in.
fn map_error_code(function: &str, code: &str) -> LmsError {
    match code {
        "invalidtoken" => LmsError::Auth("Your Moodle session has expired".into()),
        "accessexception" if function == "core_webservice_get_site_info" => {
            LmsError::Auth("Your Moodle session has expired".into())
        }
        "servicenotavailable" | "accessexception" => {
            LmsError::Protocol(format!("{function} is not available on this Moodle site"))
        }
        other => LmsError::Protocol(format!("{function} failed ({other})")),
    }
}

#[async_trait]
impl LmsClient for MoodleClient {
    async fn verify(&self) -> Result<LmsIdentity, LmsError> {
        let info = self.site_info().await?;
        if let Some(missing) = REQUIRED_FUNCTIONS
            .iter()
            .find(|required| !info.functions.iter().any(|f| f.name == **required))
        {
            return Err(LmsError::Protocol(format!(
                "{missing} is not enabled for the Moodle mobile service on this site"
            )));
        }
        Ok(LmsIdentity {
            user_id: info.userid.to_string(),
            display_name: info.fullname,
        })
    }

    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError> {
        let user_id = self.site_info().await?.userid.to_string();
        let courses: Vec<MoodleCourse> = self
            .call(
                "core_enrol_get_users_courses",
                &[("userid", user_id.as_str())],
            )
            .await?;
        Ok(courses
            .into_iter()
            .map(|c| RemoteCourse {
                external_id: c.id.to_string(),
                url: Some(format!("{}/course/view.php?id={}", self.base_url, c.id)),
                short_name: Some(c.shortname.trim().to_string()).filter(|s| !s.is_empty()),
                name: c.fullname,
            })
            .collect())
    }

    /// Assignments, then quizzes, then course events.
    async fn fetch_items(
        &self,
        courses: &[RemoteCourseRef],
        window: &SyncWindow,
    ) -> Result<Vec<RemoteItem>, LmsError> {
        let course_ids = parse_course_ids(courses)?;
        if course_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut items = self.fetch_assignments(&course_ids).await?;
        items.extend(self.fetch_quizzes(&course_ids).await?);
        items.extend(self.fetch_events(&course_ids, window).await?);
        Ok(items)
    }
}

/// Decodes the payload of a `moodlemobile://token=<payload>` redirect. The payload is
/// base64 of `signature:::token[:::privatetoken]`; `signature` must equal
/// `md5(base_url + passport)`. Returns only the token (the private token is dropped).
pub fn decode_launch_token(
    payload: &str,
    base_url: &str,
    passport: &str,
) -> Result<String, LmsError> {
    let invalid = || LmsError::Auth("Moodle returned an invalid sign-in token".into());
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(percent_decode_base64(payload.trim()))
        .map_err(|_| invalid())?;
    let text = String::from_utf8(decoded).map_err(|_| invalid())?;
    let mut parts = text.split(":::");
    let (Some(signature), Some(token)) = (parts.next(), parts.next()) else {
        return Err(invalid());
    };
    if token.is_empty() {
        return Err(invalid());
    }
    let expected = hex::encode(Md5::digest(format!("{base_url}{passport}").as_bytes()));
    if !signature.eq_ignore_ascii_case(&expected) {
        return Err(LmsError::Auth(
            "The Moodle sign-in could not be verified".into(),
        ));
    }
    Ok(token.to_string())
}

/// Undoes percent-encoding of the three base64 characters a webview may escape.
fn percent_decode_base64(value: &str) -> String {
    value
        .replace("%2B", "+")
        .replace("%2b", "+")
        .replace("%2F", "/")
        .replace("%2f", "/")
        .replace("%3D", "=")
        .replace("%3d", "=")
}
