//! Turns what the Android sign-in screen reports into the credential to store.
//! Target-independent so it is tested on every platform. Nothing here logs or
//! returns a token, cookie or URL.

use academic_core::domain::LmsPlatform;
use academic_core::ports::LmsError;
use academic_infra::lms::moodle::decode_launch_token;
use serde::Deserialize;
use tauri::Url;

use super::capture::{encode_session_cookies, moodle_token_payload, CookieCaptureError, RawCookie};
use super::LoginMode;

fn unexpected() -> LmsError {
    LmsError::Protocol("The sign-in screen returned an unexpected result".into())
}

/// How the sign-in screen ended.
#[derive(Debug, PartialEq, Eq)]
pub enum SignInOutcome {
    /// Moodle: the intercepted `moodlemobile://` redirect.
    Redirect(String),
    /// Blackboard: the `Cookie` header of the platform address after sign-in.
    SignedIn(String),
    Cancelled,
    TimedOut,
}

/// The plugin's reply to `signIn`.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignInReply {
    status: String,
    url: Option<String>,
    cookie_header: Option<String>,
}

impl SignInReply {
    pub fn into_outcome(self) -> Result<SignInOutcome, LmsError> {
        match self.status.as_str() {
            "redirect" => self.url.map(SignInOutcome::Redirect).ok_or_else(unexpected),
            "signedIn" => self
                .cookie_header
                .map(SignInOutcome::SignedIn)
                .ok_or_else(unexpected),
            "cancelled" => Ok(SignInOutcome::Cancelled),
            "timeout" => Ok(SignInOutcome::TimedOut),
            _ => Err(unexpected()),
        }
    }
}

/// Where an Ultra page (only reachable after sign-in) starts.
pub fn blackboard_done_prefix(base_url: &str) -> String {
    format!("{base_url}/ultra/")
}

/// Splits a `Cookie` header into cookies. The header has no domain, so every
/// cookie belongs to the address it was read for. Empty or invalid parts are skipped.
pub fn parse_cookie_header(header: &str) -> Vec<RawCookie> {
    header
        .split(';')
        .filter_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| RawCookie {
                name: name.to_string(),
                value: value.trim().to_string(),
                domain: None,
            })
        })
        .collect()
}

/// The credential to store for a finished sign-in, or the same error the desktop
/// sign-in window reports for that ending.
pub fn credential_from_outcome(
    outcome: SignInOutcome,
    platform: LmsPlatform,
    base_url: &str,
    passport: &str,
    mode: LoginMode,
) -> Result<String, LmsError> {
    match (outcome, platform) {
        (SignInOutcome::Redirect(url), LmsPlatform::Moodle) => {
            let payload = moodle_token_payload(&url).ok_or_else(unexpected)?;
            decode_launch_token(payload, base_url, passport)
        }
        (SignInOutcome::SignedIn(header), LmsPlatform::Blackboard) => {
            let host = base_url
                .parse::<Url>()
                .ok()
                .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
                .ok_or_else(|| LmsError::Protocol("Invalid platform URL".into()))?;
            encode_session_cookies(parse_cookie_header(&header), &host).map_err(|error| {
                if let CookieCaptureError::TooLarge(names) = &error {
                    tracing::warn!(cookie_names = ?names, "blackboard session exceeds the secret size limit");
                }
                LmsError::Auth(error.to_string())
            })
        }
        (SignInOutcome::Cancelled, _) => Err(LmsError::Auth("Sign-in cancelled".into())),
        (SignInOutcome::TimedOut, _) => Err(LmsError::Auth(match mode {
            LoginMode::Interactive => "Sign-in timed out".into(),
            LoginMode::Silent => "Sign-in required".into(),
        })),
        _ => Err(unexpected()),
    }
}

#[cfg(test)]
mod tests {
    use super::super::capture::encode_session_cookies;
    use super::*;

    const MOODLE_BASE: &str = "https://moodle.demo.example";
    const BB_BASE: &str = "https://campus.demo.example";
    // base64 of "<md5(MOODLE_BASE + "pass1")>:::tok-xyz:::private"
    const MOODLE_PAYLOAD: &str =
        "ZWM0N2EyYWIwZDcyOGZjN2RmZjA5NjM3YjQ0ZDc1ZGU6Ojp0b2steHl6Ojo6cHJpdmF0ZQ==";

    fn cookie_pairs(cookies: &[RawCookie]) -> Vec<(&str, &str)> {
        cookies
            .iter()
            .map(|c| (c.name.as_str(), c.value.as_str()))
            .collect()
    }

    fn raw(name: &str, value: &str) -> RawCookie {
        RawCookie {
            name: name.into(),
            value: value.into(),
            domain: None,
        }
    }

    #[test]
    fn parse_cookie_header_splits_and_trims() {
        let cookies = parse_cookie_header("a=1; b=2=3; ;c");
        assert_eq!(cookie_pairs(&cookies), vec![("a", "1"), ("b", "2=3")]);
        assert!(cookies.iter().all(|c| c.domain.is_none()));
    }

    #[test]
    fn blackboard_signed_in_outcome_encodes_the_session() {
        let expected = encode_session_cookies(
            vec![raw("BbRouter", "x"), raw("JSESSIONID", "y")],
            "campus.demo.example",
        )
        .unwrap();
        let got = credential_from_outcome(
            SignInOutcome::SignedIn("BbRouter=x; JSESSIONID=y".into()),
            LmsPlatform::Blackboard,
            BB_BASE,
            "unused",
            LoginMode::Interactive,
        );
        assert_eq!(got.unwrap(), expected);
    }

    #[test]
    fn empty_cookie_header_is_an_auth_error() {
        let got = credential_from_outcome(
            SignInOutcome::SignedIn(String::new()),
            LmsPlatform::Blackboard,
            BB_BASE,
            "unused",
            LoginMode::Interactive,
        );
        assert!(matches!(got, Err(LmsError::Auth(_))));
    }

    #[test]
    fn moodle_redirect_outcome_decodes_the_token() {
        let got = credential_from_outcome(
            SignInOutcome::Redirect(format!("moodlemobile://token={MOODLE_PAYLOAD}")),
            LmsPlatform::Moodle,
            MOODLE_BASE,
            "pass1",
            LoginMode::Interactive,
        );
        assert_eq!(got.unwrap(), "tok-xyz");
    }

    #[test]
    fn cancelled_and_timed_out_map_like_the_desktop() {
        let run = |outcome, mode| {
            credential_from_outcome(outcome, LmsPlatform::Moodle, MOODLE_BASE, "p", mode)
                .unwrap_err()
        };
        for mode in [LoginMode::Interactive, LoginMode::Silent] {
            let error = run(SignInOutcome::Cancelled, mode);
            assert!(matches!(error, LmsError::Auth(_)));
            assert_eq!(error.to_string(), "Sign-in cancelled");
        }
        let error = run(SignInOutcome::TimedOut, LoginMode::Interactive);
        assert!(matches!(error, LmsError::Auth(_)));
        assert_eq!(error.to_string(), "Sign-in timed out");
        let error = run(SignInOutcome::TimedOut, LoginMode::Silent);
        assert!(matches!(error, LmsError::Auth(_)));
        assert_eq!(error.to_string(), "Sign-in required");
    }

    #[test]
    fn mismatched_outcome_is_a_protocol_error() {
        let got = credential_from_outcome(
            SignInOutcome::Redirect("moodlemobile://token=x".into()),
            LmsPlatform::Blackboard,
            BB_BASE,
            "p",
            LoginMode::Interactive,
        );
        assert!(matches!(got, Err(LmsError::Protocol(_))));
        let got = credential_from_outcome(
            SignInOutcome::SignedIn("a=b".into()),
            LmsPlatform::Moodle,
            MOODLE_BASE,
            "p",
            LoginMode::Interactive,
        );
        assert!(matches!(got, Err(LmsError::Protocol(_))));
    }

    #[test]
    fn done_prefix_is_the_ultra_path() {
        assert_eq!(
            blackboard_done_prefix("https://x.example"),
            "https://x.example/ultra/"
        );
    }

    #[test]
    fn reply_status_maps_to_outcome() {
        let reply = |json: &str| serde_json::from_str::<SignInReply>(json).unwrap();
        assert_eq!(
            reply(r#"{"status":"redirect","url":"moodlemobile://token=x"}"#)
                .into_outcome()
                .unwrap(),
            SignInOutcome::Redirect("moodlemobile://token=x".into())
        );
        assert_eq!(
            reply(r#"{"status":"signedIn","cookieHeader":"a=b"}"#)
                .into_outcome()
                .unwrap(),
            SignInOutcome::SignedIn("a=b".into())
        );
        assert_eq!(
            reply(r#"{"status":"cancelled"}"#).into_outcome().unwrap(),
            SignInOutcome::Cancelled
        );
        assert_eq!(
            reply(r#"{"status":"timeout"}"#).into_outcome().unwrap(),
            SignInOutcome::TimedOut
        );
        assert!(reply(r#"{"status":"redirect"}"#).into_outcome().is_err());
        assert!(reply(r#"{"status":"other"}"#).into_outcome().is_err());
    }
}
