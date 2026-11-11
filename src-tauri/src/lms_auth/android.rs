//! Android sign-in: the Kotlin `AcademicPlugin` opens a separate screen with a
//! `WebView` and reports how it ended. The credential is decoded here, so the
//! screen never returns anything but the redirect URL or the cookie header, and
//! nothing in this module logs them.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use academic_core::domain::{LmsPlatform, LmsUniversity, UniversityId};
use academic_core::ports::LmsError;
use serde::Serialize;
use tauri::AppHandle;

use super::capture::{blackboard_start_url, moodle_launch_url, random_passport};
use super::sign_in_result::{
    blackboard_done_prefix, credential_from_outcome, SignInOutcome, SignInReply,
};
use super::LoginMode;

const MOODLE_SCHEME: &str = "moodlemobile";
/// Slack over the screen's own timeout, so the plugin normally answers first.
const PLUGIN_GRACE: Duration = Duration::from_secs(15);

/// The app shows one sign-in screen at a time.
static SIGN_IN_OPEN: AtomicBool = AtomicBool::new(false);

/// Marks the sign-in screen as open until dropped, on every exit path.
struct OpenGuard;

impl OpenGuard {
    fn acquire() -> Option<Self> {
        SIGN_IN_OPEN
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| Self)
    }
}

impl Drop for OpenGuard {
    fn drop(&mut self) {
        SIGN_IN_OPEN.store(false, Ordering::Release);
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SignInArgs<'a> {
    url: &'a str,
    intercept_scheme: Option<&'a str>,
    done_url_prefix: Option<String>,
    cookie_url: Option<&'a str>,
    silent: bool,
    timeout_ms: u64,
    title: String,
}

pub async fn capture_credential(
    app: &AppHandle,
    university: &LmsUniversity,
    mode: LoginMode,
) -> Result<String, LmsError> {
    let Some(_open) = OpenGuard::acquire() else {
        return Err(LmsError::Auth(match mode {
            LoginMode::Silent => "Sign-in required".into(),
            LoginMode::Interactive => "A sign-in window for this university is already open".into(),
        }));
    };
    let base_url = university.base_url.as_str();
    let passport = random_passport();
    let start = match university.platform {
        LmsPlatform::Moodle => moodle_launch_url(base_url, &passport),
        LmsPlatform::Blackboard => blackboard_start_url(base_url),
    };
    let is_moodle = university.platform == LmsPlatform::Moodle;
    let args = SignInArgs {
        url: &start,
        intercept_scheme: is_moodle.then_some(MOODLE_SCHEME),
        done_url_prefix: (!is_moodle).then(|| blackboard_done_prefix(base_url)),
        cookie_url: (!is_moodle).then_some(base_url),
        silent: mode == LoginMode::Silent,
        timeout_ms: mode.timeout().as_millis() as u64,
        title: format!("Sign in to {}", university.name),
    };
    tracing::info!(
        university_id = university.id,
        ?mode,
        "lms sign-in screen opened"
    );
    let plugin = tauri_plugin_academic_android::handle(app);
    let call = plugin.run_mobile_plugin_async::<SignInReply>("signIn", args);
    let outcome = match tokio::time::timeout(mode.timeout() + PLUGIN_GRACE, call).await {
        Ok(reply) => reply
            .map_err(|_| LmsError::Protocol("Could not open the sign-in screen".into()))?
            .into_outcome()?,
        Err(_) => {
            // The plugin never answered: close whatever it may have opened.
            run_in_background(app, "cancelSignIn");
            SignInOutcome::TimedOut
        }
    };
    let result = credential_from_outcome(outcome, university.platform, base_url, &passport, mode);
    tracing::info!(
        university_id = university.id,
        stage = if result.is_ok() {
            "captured"
        } else {
            "not captured"
        },
        "lms sign-in screen closed"
    );
    result
}

/// Runs a plugin command that returns nothing without blocking the caller, which may
/// be the main thread. A failure is only worth a log line without details.
fn run_in_background(app: &AppHandle, command: &'static str) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let outcome = tauri_plugin_academic_android::handle(&app)
            .run_mobile_plugin_async::<()>(command, ())
            .await;
        if outcome.is_err() {
            tracing::warn!(command, "android sign-in plugin command failed");
        }
    });
}

/// Finishes the open sign-in screen, which then reports "cancelled". There is only
/// one screen, so the university id is not needed.
pub fn close_login_window(app: &AppHandle, _id: UniversityId) {
    run_in_background(app, "cancelSignIn");
}

pub fn close_all_login_windows(app: &AppHandle) {
    run_in_background(app, "cancelSignIn");
}

/// Removes every `WebView` cookie and the storage of the sites the sign-in screen loaded (never the app's own storage).
pub fn forget_sign_in(app: &AppHandle) {
    run_in_background(app, "clearSignInData");
}
