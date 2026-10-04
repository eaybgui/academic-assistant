//! Blackboard Learn public REST client using the user's session cookies (spec §5.2).

use academic_core::domain::{LmsIdentity, RemoteCourse};
use academic_core::ports::{LmsClient, LmsError};
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{connection_error, REQUEST_TIMEOUT};

/// Upper bound on followed `paging.nextPage` links, against a misbehaving server.
const MAX_PAGES: usize = 50;
const API_PREFIX: &str = "/learn/api/";

/// One captured session cookie. The stored credential is a JSON array of these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCookie {
    pub name: String,
    pub value: String,
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

    /// GET `path` (starting with `/learn/api/`). A 401/403, or a redirect that lands
    /// outside the API (the login page), means the session is no longer valid.
    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, LmsError> {
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
        if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
            return Err(expired());
        }
        if !response.url().path().starts_with(API_PREFIX) {
            return Err(expired());
        }
        let endpoint = path.split('?').next().unwrap_or(path);
        if !status.is_success() {
            return Err(LmsError::Protocol(format!(
                "{endpoint} returned HTTP {}",
                status.as_u16()
            )));
        }
        response
            .json()
            .await
            .map_err(|_| LmsError::Protocol(format!("{endpoint} returned an unexpected response")))
    }

    async fn get_all<T: DeserializeOwned>(&self, first: &str) -> Result<Vec<T>, LmsError> {
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
            let page: Page<T> = self.get(&path).await?;
            items.extend(page.results);
            next = page
                .paging
                .and_then(|p| p.next_page)
                .filter(|p| p.starts_with(API_PREFIX));
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
}
