//! Embedded SSO sign-in for LMS platforms (spec §6).

mod capture;
mod window;

pub use window::{capture_credential, LoginMode, TauriLmsAuthenticator};
