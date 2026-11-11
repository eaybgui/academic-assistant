//! Embedded SSO sign-in for LMS platforms (spec §6).
//!
//! Every target exposes the same names. Desktop captures the credential in a
//! dedicated webview window; Android has its own implementation in `android`.

use std::sync::Arc;
use std::time::Duration;

use academic_core::domain::LmsUniversity;
use academic_core::lms::{adopt_credential, SessionError};
use academic_core::ports::{LmsAuthenticator, LmsClientFactory, LmsError, SecretStore};
use async_trait::async_trait;
use tauri::AppHandle;

#[cfg(target_os = "android")]
mod android;
mod capture;
// Only the Android sign-in uses these on every path; desktop compiles them for the tests.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
mod sign_in_result;
#[cfg(not(target_os = "android"))]
mod window;

#[cfg(target_os = "android")]
pub use android::{
    capture_credential, close_all_login_windows, close_login_window, forget_sign_in,
};
#[cfg(not(target_os = "android"))]
pub use window::{capture_credential, close_all_login_windows, close_login_window};

const INTERACTIVE_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const SILENT_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginMode {
    Interactive,
    Silent,
}

impl LoginMode {
    fn timeout(self) -> Duration {
        match self {
            LoginMode::Interactive => INTERACTIVE_TIMEOUT,
            LoginMode::Silent => SILENT_TIMEOUT,
        }
    }
}

/// Forgets the sign-in browser state when a university is signed out. The desktop
/// keeps a separate profile per window, so there is nothing to clear.
#[cfg(not(target_os = "android"))]
pub fn forget_sign_in(_app: &AppHandle) {}

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
