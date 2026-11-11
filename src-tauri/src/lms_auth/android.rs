//! Android sign-in. Placeholder until the native sign-in screen replaces it.

use academic_core::domain::{LmsUniversity, UniversityId};
use academic_core::ports::LmsError;
use tauri::AppHandle;

use super::LoginMode;

pub async fn capture_credential(
    _app: &AppHandle,
    _university: &LmsUniversity,
    _mode: LoginMode,
) -> Result<String, LmsError> {
    Err(LmsError::Protocol("Sign-in is not available yet".into()))
}

pub fn close_login_window(_app: &AppHandle, _id: UniversityId) {}

pub fn close_all_login_windows(_app: &AppHandle) {}
