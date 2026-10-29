use super::account::AccountId;
use super::lms::{LmsPlatform, UniversityId};

/// Service name under which all this app's secrets are stored in the OS credential store.
pub const SECRET_SERVICE: &str = "academic-assistant";
/// Secret store key for the jev API key.
pub const JEV_API_KEY: &str = "academic-assistant/jev-api-key";

/// Secret store key for an IMAP account's password.
pub fn account_password_key(id: AccountId) -> String {
    format!("academic-assistant/account/{id}/password")
}

/// Secret store key for a Microsoft Graph account's OAuth refresh token.
pub fn account_refresh_token_key(id: AccountId) -> String {
    format!("academic-assistant/account/{id}/refresh-token")
}

/// Secret store key for a university's LMS credential (Moodle web-service token or
/// Blackboard session cookies as JSON).
pub fn lms_credential_key(platform: LmsPlatform, id: UniversityId) -> String {
    let suffix = match platform {
        LmsPlatform::Moodle => "moodle-token",
        LmsPlatform::Blackboard => "blackboard-session",
    };
    format!("academic-assistant/university/{id}/{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lms_credential_key_depends_on_platform_and_id() {
        assert_eq!(
            lms_credential_key(LmsPlatform::Moodle, 3),
            "academic-assistant/university/3/moodle-token"
        );
        assert_eq!(
            lms_credential_key(LmsPlatform::Blackboard, 4),
            "academic-assistant/university/4/blackboard-session"
        );
    }

    #[test]
    fn account_password_key_includes_id() {
        assert_eq!(
            account_password_key(42),
            "academic-assistant/account/42/password"
        );
    }

    #[test]
    fn account_refresh_token_key_includes_id() {
        assert_eq!(
            account_refresh_token_key(42),
            "academic-assistant/account/42/refresh-token"
        );
    }
}
