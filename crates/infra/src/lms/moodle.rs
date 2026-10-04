//! Moodle web-service client (mobile service token) and launch-token decoding (spec §5.1, §6.2).

use academic_core::domain::{LmsIdentity, RemoteCourse};
use academic_core::ports::{LmsClient, LmsError};
use async_trait::async_trait;
use base64::Engine;
use md5::{Digest, Md5};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use super::{connection_error, REQUEST_TIMEOUT};
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

    async fn site_info(&self) -> Result<SiteInfo, LmsError> {
        self.call("core_webservice_get_site_info", &[]).await
    }
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
