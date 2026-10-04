//! Embedded SSO sign-in for LMS platforms (spec §6).

mod capture;
mod window;

pub use window::{
    capture_credential, close_all_login_windows, close_login_window, LoginMode,
    TauriLmsAuthenticator,
};
