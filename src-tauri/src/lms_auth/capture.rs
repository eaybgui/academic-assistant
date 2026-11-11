//! Pure helpers for the LMS login window: start URLs, redirect/landing detection,
//! and turning captured cookies into the stored session (spec §6).

use academic_core::ports::LmsError;
use academic_infra::lms::blackboard::{encode_session, SessionCookie};
use academic_infra::secrets::MAX_SECRET_BYTES;
use rand::distr::{Alphanumeric, SampleString};

use super::sign_in_result::blackboard_done_prefix;

const MOODLE_TOKEN_PREFIX: &str = "moodlemobile://token=";
const PASSPORT_LENGTH: usize = 32;

pub fn random_passport() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), PASSPORT_LENGTH)
}

pub fn moodle_launch_url(base_url: &str, passport: &str) -> String {
    format!(
        "{base_url}/admin/tool/mobile/launch.php?service=moodle_mobile_app&passport={passport}&urlscheme=moodlemobile"
    )
}

/// The payload after `moodlemobile://token=`, if `url` is the launch redirect.
pub fn moodle_token_payload(url: &str) -> Option<&str> {
    url.strip_prefix(MOODLE_TOKEN_PREFIX)
}

pub fn blackboard_start_url(base_url: &str) -> String {
    format!("{base_url}/ultra")
}

/// True once the window shows an Ultra page (e.g. `/ultra/stream`), which only
/// happens after a successful sign-in. `/ultra` itself redirects to the login page.
#[cfg_attr(target_os = "android", allow(dead_code))] // the Android screen applies the same prefix natively
pub fn is_blackboard_signed_in(url: &str, base_url: &str) -> bool {
    url.strip_prefix(&blackboard_done_prefix(base_url))
        .is_some_and(|rest| !rest.is_empty())
}

/// A cookie as read from the webview: name, value and optional domain attribute.
pub struct RawCookie {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum CookieCaptureError {
    #[error("No Blackboard session was found after sign-in")]
    Empty,
    /// Holds the cookie names (never values) for the log.
    #[error("The Blackboard session is too large to store securely")]
    TooLarge(Vec<String>),
}

/// Keeps the cookies that belong to `host` and encodes them as the stored session.
pub fn encode_session_cookies(
    cookies: Vec<RawCookie>,
    host: &str,
) -> Result<String, CookieCaptureError> {
    let kept: Vec<SessionCookie> = cookies
        .into_iter()
        .filter(|c| domain_matches(c.domain.as_deref(), host))
        .map(|c| SessionCookie {
            name: c.name,
            value: c.value,
        })
        .collect();
    if kept.is_empty() {
        return Err(CookieCaptureError::Empty);
    }
    let encoded = encode_session(&kept);
    if encoded.len() > MAX_SECRET_BYTES {
        return Err(CookieCaptureError::TooLarge(
            kept.into_iter().map(|c| c.name).collect(),
        ));
    }
    Ok(encoded)
}

/// [`encode_session_cookies`] with the failure mapped to the sign-in error. The log
/// line for an oversized session carries cookie names only.
pub fn session_credential(cookies: Vec<RawCookie>, host: &str) -> Result<String, LmsError> {
    encode_session_cookies(cookies, host).map_err(|error| {
        if let CookieCaptureError::TooLarge(names) = &error {
            tracing::warn!(cookie_names = ?names, "blackboard session exceeds the secret size limit");
        }
        LmsError::Auth(error.to_string())
    })
}

fn domain_matches(domain: Option<&str>, host: &str) -> bool {
    match domain.map(|d| d.trim_start_matches('.').to_ascii_lowercase()) {
        None => true,
        Some(d) => host == d || host.ends_with(&format!(".{d}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(name: &str, value: &str, domain: Option<&str>) -> RawCookie {
        RawCookie {
            name: name.into(),
            value: value.into(),
            domain: domain.map(str::to_string),
        }
    }

    #[test]
    fn moodle_urls_and_payload_detection() {
        assert_eq!(
            moodle_launch_url("https://aulavirtual.uv.es", "abc"),
            "https://aulavirtual.uv.es/admin/tool/mobile/launch.php?service=moodle_mobile_app&passport=abc&urlscheme=moodlemobile"
        );
        assert_eq!(
            moodle_token_payload("moodlemobile://token=QUJD"),
            Some("QUJD")
        );
        assert_eq!(moodle_token_payload("https://aulavirtual.uv.es/my/"), None);
        let passport = random_passport();
        assert_eq!(passport.len(), 32);
        assert!(passport.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn blackboard_landing_detection() {
        let base = "https://uah.blackboard.com";
        assert!(is_blackboard_signed_in(
            "https://uah.blackboard.com/ultra/stream",
            base
        ));
        assert!(!is_blackboard_signed_in(
            "https://uah.blackboard.com/ultra",
            base
        ));
        assert!(!is_blackboard_signed_in(
            "https://uah.blackboard.com/ultra/",
            base
        ));
        assert!(!is_blackboard_signed_in(
            "https://uah.blackboard.com/webapps/login/?action=relogin",
            base
        ));
        assert!(!is_blackboard_signed_in(
            "https://login.microsoftonline.com/ultra/x",
            base
        ));
    }

    #[test]
    fn only_cookies_for_the_platform_host_are_kept() {
        let json = encode_session_cookies(
            vec![
                raw("BbRouter", "v1", Some("uah.blackboard.com")),
                raw("JSESSIONID", "v2", None),
                raw("AWSALB", "v3", Some(".blackboard.com")),
                raw("ESTSAUTH", "ms", Some(".login.microsoftonline.com")),
            ],
            "uah.blackboard.com",
        )
        .unwrap();
        assert!(
            json.contains("BbRouter") && json.contains("JSESSIONID") && json.contains("AWSALB")
        );
        assert!(!json.contains("ESTSAUTH"));
    }

    #[test]
    fn empty_or_oversized_sessions_are_rejected_without_values() {
        assert_eq!(
            encode_session_cookies(vec![raw("x", "y", Some("other.com"))], "uah.blackboard.com"),
            Err(CookieCaptureError::Empty)
        );
        let big = "v".repeat(MAX_SECRET_BYTES);
        assert_eq!(
            encode_session_cookies(vec![raw("BbRouter", &big, None)], "uah.blackboard.com"),
            Err(CookieCaptureError::TooLarge(vec!["BbRouter".into()]))
        );
    }

    #[test]
    fn session_credential_maps_failures_to_auth_errors() {
        let empty = session_credential(vec![], "uah.blackboard.com").unwrap_err();
        assert!(matches!(&empty, LmsError::Auth(_)));
        assert_eq!(empty.to_string(), CookieCaptureError::Empty.to_string());
        let big = "v".repeat(MAX_SECRET_BYTES);
        let too_large = session_credential(vec![raw("BbRouter", &big, None)], "uah.blackboard.com")
            .unwrap_err();
        assert!(matches!(&too_large, LmsError::Auth(_)));
        assert!(!too_large.to_string().contains(&big));
        assert!(session_credential(vec![raw("a", "b", None)], "uah.blackboard.com").is_ok());
    }
}
