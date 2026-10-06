//! Blackboard Learn public REST client using the user's session cookies (spec §5.2).

use std::collections::{HashMap, HashSet};

use academic_core::domain::{
    Grade, ItemKind, LmsIdentity, PlatformStatus, RemoteCourse, RemoteCourseRef, RemoteItem,
    SyncWindow,
};
use academic_core::ports::{LmsClient, LmsError};
use async_trait::async_trait;
use chrono::{DateTime, Duration, SecondsFormat, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::form_urlencoded;

use super::{connection_error, format_number, REQUEST_TIMEOUT};

/// Upper bound on followed `paging.nextPage` links, against a misbehaving server.
const MAX_PAGES: usize = 50;
const API_PREFIX: &str = "/learn/api/";
/// The calendar endpoint rejects ranges spanning more than 16 weeks, measured in the server's
/// local time. A 112-day chunk that crosses a daylight-saving change becomes 16 weeks plus an
/// hour there, so chunks stay at 15 weeks, safely below the limit.
const CALENDAR_CHUNK_DAYS: i64 = 105;
/// Attempt statuses that mean nothing has been handed in.
const UNSUBMITTED_ATTEMPT_STATUSES: [&str; 4] =
    ["InProgress", "NotAttempted", "Abandoned", "Canceled"];

/// One captured session cookie. The stored credential is a JSON array of these.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCookie {
    pub name: String,
    pub value: String,
}

/// Hand-written so that logging a cookie never prints its secret value.
impl std::fmt::Debug for SessionCookie {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionCookie")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .finish()
    }
}

pub fn encode_session(cookies: &[SessionCookie]) -> String {
    serde_json::to_string(cookies).expect("cookies always serialize")
}

/// An unreadable or empty session is treated as an expired one.
pub fn decode_session(json: &str) -> Result<Vec<SessionCookie>, LmsError> {
    serde_json::from_str::<Vec<SessionCookie>>(json)
        .ok()
        .filter(|cookies| !cookies.is_empty())
        .ok_or_else(|| LmsError::Auth("The saved Blackboard session is not usable".into()))
}

pub struct BlackboardClient {
    http: reqwest::Client,
    base_url: String,
    cookie_header: String,
}

#[derive(Deserialize)]
struct Me {
    id: String,
    #[serde(rename = "userName")]
    user_name: Option<String>,
    name: Option<PersonName>,
}

#[derive(Deserialize)]
struct PersonName {
    given: Option<String>,
    family: Option<String>,
}

#[derive(Deserialize)]
struct Page<T> {
    results: Vec<T>,
    paging: Option<Paging>,
}

#[derive(Deserialize)]
struct Paging {
    #[serde(rename = "nextPage")]
    next_page: Option<String>,
}

#[derive(Deserialize)]
struct Membership {
    #[serde(rename = "courseId")]
    course_id: String,
    course: Option<CourseInfo>,
}

#[derive(Deserialize)]
struct CourseInfo {
    #[serde(rename = "courseId")]
    code: Option<String>,
    name: String,
}

#[derive(Deserialize)]
struct Column {
    id: String,
    name: String,
    #[serde(rename = "contentId")]
    content_id: Option<String>,
    score: Option<ColumnScore>,
    grading: Option<Grading>,
    #[serde(rename = "scoreProviderHandle")]
    provider_handle: Option<String>,
}

#[derive(Deserialize)]
struct ColumnScore {
    possible: Option<f64>,
}

#[derive(Deserialize)]
struct Grading {
    #[serde(rename = "type")]
    kind: Option<String>,
    due: Option<String>,
}

#[derive(Deserialize)]
struct UserGrade {
    #[serde(rename = "columnId")]
    column_id: String,
    score: Option<f64>,
}

#[derive(Deserialize)]
struct Attempt {
    status: Option<String>,
}

#[derive(Deserialize)]
struct CalendarItem {
    id: String,
    #[serde(rename = "calendarId")]
    calendar_id: Option<String>,
    title: String,
    start: Option<String>,
}

fn http_error(endpoint: &str, status: u16) -> LmsError {
    LmsError::Protocol(format!("{endpoint} returned HTTP {status}"))
}

fn endpoint_of(path: &str) -> &str {
    path.split('?').next().unwrap_or(path)
}

fn parse_instant(text: &str) -> Result<DateTime<Utc>, LmsError> {
    DateTime::parse_from_rfc3339(text)
        .map(|instant| instant.with_timezone(&Utc))
        .map_err(|_| LmsError::Protocol("Blackboard returned an invalid date".into()))
}

/// The kind of a gradebook column, or `None` for columns that are not deliverables
/// (attendance). Ultra uses `x-bb-assessment` for both assignments and tests.
fn item_kind(provider_handle: Option<&str>) -> Option<ItemKind> {
    match provider_handle {
        Some("resource/x-bb-attendance") => None,
        Some("resource/x-bb-assignment" | "resource/x-bb-assessment") => Some(ItemKind::Assignment),
        Some(handle) if handle.contains("test") || handle.contains("asmt") => Some(ItemKind::Test),
        _ => Some(ItemKind::Other),
    }
}

fn grade_of(score: f64, possible: Option<f64>) -> Grade {
    let max = possible.filter(|p| *p > 0.0);
    let display = match max {
        Some(max) => format!("{} / {}", format_number(score), format_number(max)),
        None => format_number(score),
    };
    Grade {
        display,
        score: Some(score),
        max,
    }
}

fn query(pairs: &[(&str, &str)]) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(pairs)
        .finish()
}

/// Consecutive `[start, end)` ranges of at most `CALENDAR_CHUNK_DAYS` covering `window`.
fn calendar_chunks(window: &SyncWindow) -> Vec<(DateTime<Utc>, DateTime<Utc>)> {
    let mut chunks = Vec::new();
    let mut start = window.from;
    while start < window.to {
        let end = (start + Duration::days(CALENDAR_CHUNK_DAYS)).min(window.to);
        chunks.push((start, end));
        start = end;
    }
    chunks
}

impl BlackboardClient {
    pub fn new(http: reqwest::Client, base_url: String, cookies: &[SessionCookie]) -> Self {
        let cookie_header = cookies
            .iter()
            .map(|c| format!("{}={}", c.name, c.value))
            .collect::<Vec<_>>()
            .join("; ");
        Self {
            http,
            base_url,
            cookie_header,
        }
    }

    /// GET `path` (starting with `/learn/api/`); `None` when the resource answers 404,
    /// or 403 when `forbidden_is_none`. Otherwise a 401/403, or a redirect that lands
    /// outside the API (the login page), means the session is no longer valid.
    async fn get_optional<T: DeserializeOwned>(
        &self,
        path: &str,
        forbidden_is_none: bool,
    ) -> Result<Option<T>, LmsError> {
        let expired = || LmsError::Auth("Your Blackboard session has expired".into());
        let response = self
            .http
            .get(format!("{}{}", self.base_url, path))
            .header(reqwest::header::COOKIE, &self.cookie_header)
            .header(reqwest::header::ACCEPT, "application/json")
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|e| connection_error(&e))?;
        let status = response.status();
        if status == reqwest::StatusCode::FORBIDDEN && forbidden_is_none {
            return Ok(None);
        }
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(expired());
        }
        if !response.url().path().starts_with(API_PREFIX) {
            return Err(expired());
        }
        if status == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        let endpoint = endpoint_of(path);
        if !status.is_success() {
            return Err(http_error(endpoint, status.as_u16()));
        }
        response
            .json()
            .await
            .map(Some)
            .map_err(|_| LmsError::Protocol(format!("{endpoint} returned an unexpected response")))
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, LmsError> {
        self.get_optional(path, false)
            .await?
            .ok_or_else(|| http_error(endpoint_of(path), 404))
    }

    async fn get_all<T: DeserializeOwned>(&self, first: &str) -> Result<Vec<T>, LmsError> {
        self.get_pages(first, false).await
    }

    /// Like `get_all`, but a 404 or 403 on the first page means there is nothing to list.
    /// For course-scoped gradebook endpoints, where a 403 means the student has no
    /// gradebook in that course rather than an expired session.
    async fn get_all_or_empty<T: DeserializeOwned>(&self, first: &str) -> Result<Vec<T>, LmsError> {
        self.get_pages(first, true).await
    }

    async fn get_pages<T: DeserializeOwned>(
        &self,
        first: &str,
        empty_if_missing: bool,
    ) -> Result<Vec<T>, LmsError> {
        let mut items = Vec::new();
        let mut next = Some(first.to_string());
        let mut pages = 0;
        while let Some(path) = next.take() {
            pages += 1;
            if pages > MAX_PAGES {
                return Err(LmsError::Protocol(
                    "Blackboard returned too many pages".into(),
                ));
            }
            let page: Page<T> = match self
                .get_optional(&path, empty_if_missing && pages == 1)
                .await?
            {
                Some(page) => page,
                None if empty_if_missing && pages == 1 => return Ok(items),
                None => return Err(http_error(endpoint_of(&path), 404)),
            };
            items.extend(page.results);
            next = match page.paging.and_then(|p| p.next_page) {
                None => None,
                Some(link) if link.is_empty() => None,
                Some(link) if link.starts_with(API_PREFIX) => Some(link),
                Some(_) => {
                    return Err(LmsError::Protocol(
                        "Blackboard returned an unexpected page link".into(),
                    ))
                }
            };
        }
        Ok(items)
    }

    async fn fetch_course_columns(
        &self,
        course_id: &str,
        user_id: &str,
    ) -> Result<Vec<RemoteItem>, LmsError> {
        let gradebook = format!("/learn/api/public/v1/courses/{course_id}/gradebook");
        let columns: Vec<Column> = self
            .get_all_or_empty(&format!(
                "/learn/api/public/v2/courses/{course_id}/gradebook/columns"
            ))
            .await?;
        let scores: HashMap<String, f64> = self
            .get_all_or_empty::<UserGrade>(&format!("{gradebook}/users/{user_id}"))
            .await?
            .into_iter()
            .filter_map(|grade| Some((grade.column_id, grade.score?)))
            .collect();
        let url = format!("{}/ultra/courses/{course_id}/outline", self.base_url);

        let mut items = Vec::new();
        for column in columns {
            let Some(kind) = item_kind(column.provider_handle.as_deref()) else {
                continue;
            };
            let grading = column.grading.as_ref();
            if grading.and_then(|g| g.kind.as_deref()) == Some("Calculated") {
                continue;
            }
            let due_at = grading
                .and_then(|g| g.due.as_deref())
                .map(parse_instant)
                .transpose()?;
            let possible = column.score.as_ref().and_then(|s| s.possible);
            let grade = scores
                .get(&column.id)
                .map(|score| grade_of(*score, possible));
            let platform_status = if grade.is_some() {
                PlatformStatus::Graded
            } else if column.content_id.is_none() {
                PlatformStatus::NotApplicable
            } else if self
                .has_submitted_attempt(&gradebook, &column, user_id)
                .await?
            {
                PlatformStatus::Submitted
            } else {
                PlatformStatus::NotSubmitted
            };
            items.push(RemoteItem {
                course_external_id: course_id.to_string(),
                external_id: format!("column:{}", column.id),
                kind,
                title: column.name,
                due_at,
                url: Some(url.clone()),
                platform_status,
                grade,
            });
        }
        Ok(items)
    }

    async fn has_submitted_attempt(
        &self,
        gradebook: &str,
        column: &Column,
        user_id: &str,
    ) -> Result<bool, LmsError> {
        let attempts: Vec<Attempt> = self
            .get_all_or_empty(&format!(
                "{gradebook}/columns/{}/attempts?{}",
                column.id,
                query(&[("userId", user_id)])
            ))
            .await?;
        Ok(attempts.iter().any(|attempt| {
            attempt
                .status
                .as_deref()
                .is_some_and(|status| !UNSUBMITTED_ATTEMPT_STATUSES.contains(&status))
        }))
    }

    async fn fetch_events(
        &self,
        courses: &[RemoteCourseRef],
        window: &SyncWindow,
    ) -> Result<Vec<RemoteItem>, LmsError> {
        let tracked: HashSet<&str> = courses.iter().map(|c| c.external_id.as_str()).collect();
        let url = format!("{}/ultra/calendar", self.base_url);
        let mut seen = HashSet::new();
        let mut items = Vec::new();
        for (since, until) in calendar_chunks(window) {
            let since = since.to_rfc3339_opts(SecondsFormat::Secs, true);
            let until = until.to_rfc3339_opts(SecondsFormat::Secs, true);
            let events: Vec<CalendarItem> = self
                .get_all(&format!(
                    "/learn/api/public/v1/calendars/items?{}",
                    query(&[("type", "Course"), ("since", &since), ("until", &until)])
                ))
                .await?;
            for event in events {
                let Some(course_id) = event.calendar_id.filter(|id| tracked.contains(id.as_str()))
                else {
                    continue;
                };
                let external_id = format!("calendar:{}", event.id);
                if !seen.insert(external_id.clone()) {
                    continue;
                }
                items.push(RemoteItem {
                    course_external_id: course_id,
                    external_id,
                    kind: ItemKind::Event,
                    title: event.title,
                    due_at: event.start.as_deref().map(parse_instant).transpose()?,
                    url: Some(url.clone()),
                    platform_status: PlatformStatus::NotApplicable,
                    grade: None,
                });
            }
        }
        Ok(items)
    }
}

#[async_trait]
impl LmsClient for BlackboardClient {
    async fn verify(&self) -> Result<LmsIdentity, LmsError> {
        let me: Me = self.get("/learn/api/public/v1/users/me").await?;
        let full_name = me
            .name
            .map(|n| {
                format!(
                    "{} {}",
                    n.given.unwrap_or_default(),
                    n.family.unwrap_or_default()
                )
            })
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty());
        let display_name = full_name.or(me.user_name).unwrap_or_else(|| me.id.clone());
        Ok(LmsIdentity {
            user_id: me.id,
            display_name,
        })
    }

    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError> {
        let memberships: Vec<Membership> = self
            .get_all("/learn/api/public/v1/users/me/courses?expand=course&limit=100")
            .await?;
        Ok(memberships
            .into_iter()
            .filter_map(|m| {
                let course = m.course?;
                Some(RemoteCourse {
                    url: Some(format!(
                        "{}/ultra/courses/{}/outline",
                        self.base_url, m.course_id
                    )),
                    external_id: m.course_id,
                    short_name: course.code.filter(|c| !c.trim().is_empty()),
                    name: course.name,
                })
            })
            .collect())
    }

    /// Gradebook columns of every course, then the course events inside `window`.
    async fn fetch_items(
        &self,
        courses: &[RemoteCourseRef],
        window: &SyncWindow,
    ) -> Result<Vec<RemoteItem>, LmsError> {
        if courses.is_empty() {
            return Ok(Vec::new());
        }
        let user_id = self.verify().await?.user_id;
        let mut items = Vec::new();
        for course in courses {
            items.extend(
                self.fetch_course_columns(&course.external_id, &user_id)
                    .await?,
            );
        }
        items.extend(self.fetch_events(courses, window).await?);
        Ok(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_redacts_the_cookie_value() {
        let cookie = SessionCookie {
            name: "BbRouter".into(),
            value: "super-secret".into(),
        };
        let printed = format!("{cookie:?}");
        assert!(printed.contains("BbRouter"), "{printed}");
        assert!(!printed.contains("super-secret"), "{printed}");
    }
}
