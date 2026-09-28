use super::account::AccountId;

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

#[cfg(test)]
mod tests {
    use super::*;

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
