//! Opens the platform's sign-in page in a dedicated window with a persistent,
//! separate browser profile and captures the credential (spec §6.1–§6.3, §6.5).
//! Never logs URLs seen in the window.

use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;

use academic_core::domain::{LmsPlatform, LmsUniversity};
use academic_core::lms::{adopt_credential, SessionError};
use academic_core::ports::{LmsAuthenticator, LmsClientFactory, LmsError, SecretStore};
use academic_infra::lms::moodle::decode_launch_token;
use async_trait::async_trait;
use tauri::webview::PageLoadEvent;
use tauri::{
    AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tokio::sync::mpsc;

use super::capture::{
    blackboard_start_url, encode_session_cookies, is_blackboard_signed_in, moodle_launch_url,
    moodle_token_payload, random_passport, CookieCaptureError, RawCookie,
};

const AUTH_PROFILE_DIR: &str = "lms-auth";
const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const SILENT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMode {
    Interactive,
    Silent,
}

/// Destroys the login window when dropped, so it is closed on every exit path,
/// including the `capture_credential` future being dropped before it completes.
struct LoginWindow(WebviewWindow);

impl Deref for LoginWindow {
    type Target = WebviewWindow;

    fn deref(&self) -> &WebviewWindow {
        &self.0
    }
}

impl Drop for LoginWindow {
    fn drop(&mut self) {
        let _ = self.0.destroy();
    }
}

enum Signal {
    MoodlePayload(String),
    BlackboardSignedIn,
    Closed,
}

pub async fn capture_credential(
    app: &AppHandle,
    university: &LmsUniversity,
    mode: LoginMode,
) -> Result<String, LmsError> {
    let label = format!("lms-login-{}", university.id);
    if let Some(existing) = app.get_webview_window(&label) {
        if mode == LoginMode::Silent {
            return Err(LmsError::Auth("Sign-in required".into()));
        }
        let _ = existing.set_focus();
        return Err(LmsError::Auth(
            "A sign-in window for this university is already open".into(),
        ));
    }
    let profile = app
        .path()
        .app_data_dir()
        .map_err(|e| LmsError::Protocol(format!("App data directory unavailable: {e}")))?
        .join(AUTH_PROFILE_DIR);
    let passport = random_passport();
    let base_url = university.base_url.clone();
    let start = match university.platform {
        LmsPlatform::Moodle => moodle_launch_url(&base_url, &passport),
        LmsPlatform::Blackboard => blackboard_start_url(&base_url),
    };
    let start: Url = start
        .parse()
        .map_err(|_| LmsError::Protocol("Invalid platform URL".into()))?;

    let (tx, mut rx) = mpsc::unbounded_channel::<Signal>();
    let mut builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(start))
        .title(format!("Sign in to {}", university.name))
        .inner_size(520.0, 720.0)
        .data_directory(profile)
        .visible(mode == LoginMode::Interactive);
    builder = match university.platform {
        LmsPlatform::Moodle => {
            let tx = tx.clone();
            builder.on_navigation(move |url| match moodle_token_payload(url.as_str()) {
                Some(payload) => {
                    let _ = tx.send(Signal::MoodlePayload(payload.to_string()));
                    false // never navigate to the custom scheme
                }
                None => true,
            })
        }
        LmsPlatform::Blackboard => {
            let tx = tx.clone();
            let base_url = base_url.clone();
            builder.on_page_load(move |_window, payload| {
                if payload.event() == PageLoadEvent::Finished
                    && is_blackboard_signed_in(payload.url().as_str(), &base_url)
                {
                    let _ = tx.send(Signal::BlackboardSignedIn);
                }
            })
        }
    };
    let window = LoginWindow(
        builder
            .build()
            .map_err(|e| LmsError::Protocol(format!("Could not open the sign-in window: {e}")))?,
    );
    {
        let tx = tx.clone();
        window.on_window_event(move |event| {
            if matches!(event, WindowEvent::Destroyed) {
                let _ = tx.send(Signal::Closed);
            }
        });
    }
    tracing::info!(
        university_id = university.id,
        ?mode,
        "lms sign-in window opened"
    );

    let timeout = match mode {
        LoginMode::Interactive => INTERACTIVE_TIMEOUT,
        LoginMode::Silent => SILENT_TIMEOUT,
    };
    let signal = tokio::time::timeout(timeout, rx.recv()).await;
    let result = match signal {
        Ok(Some(Signal::MoodlePayload(payload))) => {
            decode_launch_token(&payload, &base_url, &passport)
        }
        Ok(Some(Signal::BlackboardSignedIn)) => read_session(&window, &base_url).await,
        Ok(Some(Signal::Closed)) | Ok(None) => Err(LmsError::Auth("Sign-in cancelled".into())),
        Err(_) => Err(LmsError::Auth(match mode {
            LoginMode::Interactive => "Sign-in timed out".into(),
            LoginMode::Silent => "Sign-in required".into(),
        })),
    };
    drop(window);
    let stage = if result.is_ok() {
        "captured"
    } else {
        "not captured"
    };
    tracing::info!(
        university_id = university.id,
        stage,
        "lms sign-in window closed"
    );
    result
}

/// Reads the window's cookies off the main thread (WebView2 deadlocks when cookies
/// are read from the event loop thread) and encodes the platform host's cookies.
async fn read_session(window: &WebviewWindow, base_url: &str) -> Result<String, LmsError> {
    let url: Url = base_url
        .parse()
        .map_err(|_| LmsError::Protocol("Invalid platform URL".into()))?;
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let window = window.clone();
    let cookies = tauri::async_runtime::spawn_blocking(move || window.cookies_for_url(url))
        .await
        .map_err(|_| LmsError::Protocol("Could not read the Blackboard session".into()))?
        .map_err(|_| LmsError::Protocol("Could not read the Blackboard session".into()))?;
    let raw = cookies
        .into_iter()
        .map(|c| RawCookie {
            name: c.name().to_string(),
            value: c.value().to_string(),
            domain: c.domain().map(str::to_string),
        })
        .collect();
    encode_session_cookies(raw, &host).map_err(|error| {
        if let CookieCaptureError::TooLarge(names) = &error {
            tracing::warn!(cookie_names = ?names, "blackboard session exceeds the secret size limit");
        }
        LmsError::Auth(error.to_string())
    })
}

/// Silent renewal for `ensure_session` (spec §6.5).
pub struct TauriLmsAuthenticator {
    app: AppHandle,
    factory: Arc<dyn LmsClientFactory>,
    secrets: Arc<dyn SecretStore>,
}

impl TauriLmsAuthenticator {
    pub fn new(
        app: AppHandle,
        factory: Arc<dyn LmsClientFactory>,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            app,
            factory,
            secrets,
        }
    }
}

#[async_trait]
impl LmsAuthenticator for TauriLmsAuthenticator {
    async fn renew_silently(&self, university: &LmsUniversity) -> Result<(), LmsError> {
        let credential = capture_credential(&self.app, university, LoginMode::Silent).await?;
        adopt_credential(
            university,
            &credential,
            self.factory.as_ref(),
            self.secrets.as_ref(),
        )
        .await
        .map(|_| ())
        .map_err(|error| match error {
            SessionError::Lms(e) => e,
            other => LmsError::Protocol(other.to_string()),
        })
    }
}
