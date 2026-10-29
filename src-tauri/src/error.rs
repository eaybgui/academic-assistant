//! `CommandError`: the error every Tauri command returns to the frontend.
//!
//! Serializes as `{ kind, message }` (spec §10). `message` is user-facing and
//! sanitized; anything more detailed (storage errors, provider internals) is kept
//! in `detail` — never serialized, only logged.

use academic_core::domain::RunError;
use academic_core::lms::{LmsSyncError, SessionError};
use academic_core::ports::{LmsError, ProviderError, RepoError, SecretError};
use serde::Serialize;

const STORAGE_MESSAGE: &str = "Something went wrong while saving data";
const SECRET_STORAGE_MESSAGE: &str = "Something went wrong while accessing secure storage";
const NOT_FOUND_MESSAGE: &str = "Item not found";
const CONFLICT_MESSAGE: &str = "An item with that name already exists";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandErrorKind {
    Validation,
    NotFound,
    Conflict,
    Auth,
    Connection,
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CommandError {
    pub kind: CommandErrorKind,
    pub message: String,
    #[serde(skip)]
    detail: Option<String>,
    /// Replaces `message` in the log line when `message` carries personal data
    /// (e.g. email addresses) that must not reach the log file.
    #[serde(skip)]
    log_message: Option<String>,
}

impl CommandError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(CommandErrorKind::Validation, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(CommandErrorKind::NotFound, message)
    }

    pub fn auth(message: impl Into<String>) -> Self {
        Self::new(CommandErrorKind::Auth, message)
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(CommandErrorKind::Conflict, message)
    }

    fn new(kind: CommandErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
            log_message: None,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Logs `log_message` instead of the user-facing message.
    pub fn with_log_message(mut self, log_message: impl Into<String>) -> Self {
        self.log_message = Some(log_message.into());
        self
    }

    pub(crate) fn message_for_log(&self) -> &str {
        self.log_message.as_deref().unwrap_or(&self.message)
    }

    fn internal(message: &str, detail: impl Into<String>) -> Self {
        Self::new(CommandErrorKind::Internal, message).with_detail(detail)
    }

    /// Logs the failure of `command` at `warn` (kind, message and internal detail
    /// only; no arguments, so no secrets, subjects or bodies) and returns `self`
    /// so it can be used inside `map_err`.
    pub fn logged(self, command: &str) -> Self {
        tracing::warn!(
            command,
            kind = ?self.kind,
            message = %self.message_for_log(),
            detail = self.detail.as_deref().unwrap_or(""),
            "command failed"
        );
        self
    }
}

impl From<RepoError> for CommandError {
    fn from(error: RepoError) -> Self {
        match error {
            RepoError::NotFound => Self::not_found(NOT_FOUND_MESSAGE),
            RepoError::Conflict(detail) => {
                Self::new(CommandErrorKind::Conflict, CONFLICT_MESSAGE).with_detail(detail)
            }
            RepoError::Storage(detail) => Self::internal(STORAGE_MESSAGE, detail),
        }
    }
}

impl From<RunError> for CommandError {
    fn from(error: RunError) -> Self {
        let message = error.to_string();
        match error {
            RunError::AlreadyRunning => Self::new(CommandErrorKind::Conflict, message),
            RunError::NoEnabledAccounts | RunError::AiNotConfigured(_) => Self::validation(message),
            RunError::Storage(detail) => Self::internal(STORAGE_MESSAGE, detail),
        }
    }
}

impl From<ProviderError> for CommandError {
    fn from(error: ProviderError) -> Self {
        let kind = match error {
            ProviderError::Auth(_) => CommandErrorKind::Auth,
            ProviderError::Connection(_) => CommandErrorKind::Connection,
            ProviderError::Protocol(_) => CommandErrorKind::Internal,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<LmsError> for CommandError {
    fn from(error: LmsError) -> Self {
        let kind = match error {
            LmsError::Auth(_) => CommandErrorKind::Auth,
            LmsError::Connection(_) => CommandErrorKind::Connection,
            LmsError::Protocol(_) => CommandErrorKind::Internal,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<SessionError> for CommandError {
    fn from(error: SessionError) -> Self {
        match error {
            SessionError::LoginRequired(_) => Self::new(CommandErrorKind::Auth, error.to_string()),
            SessionError::Lms(e) => e.into(),
            SessionError::Storage(detail) => Self::internal(STORAGE_MESSAGE, detail),
            SessionError::Secret(detail) => Self::internal(SECRET_STORAGE_MESSAGE, detail),
        }
    }
}

impl From<LmsSyncError> for CommandError {
    fn from(error: LmsSyncError) -> Self {
        let message = error.to_string();
        match error {
            LmsSyncError::AlreadyRunning => Self::conflict(message),
            LmsSyncError::NothingToSync => Self::validation(message),
            LmsSyncError::Storage(detail) => Self::internal(STORAGE_MESSAGE, detail),
        }
    }
}

impl From<SecretError> for CommandError {
    fn from(error: SecretError) -> Self {
        Self::internal(SECRET_STORAGE_MESSAGE, error.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_as_kind_and_message_only() {
        let error = CommandError::validation("Enter a name").with_detail("internal");
        assert_eq!(
            serde_json::to_value(&error).unwrap(),
            json!({ "kind": "validation", "message": "Enter a name" })
        );
    }

    #[test]
    fn the_log_message_replaces_the_user_message_in_logs_only() {
        let error = CommandError::validation("Signed in as a@b.es, expected c@d.es")
            .with_log_message("signed-in account mismatch");
        assert_eq!(error.message_for_log(), "signed-in account mismatch");
        assert_eq!(error.message, "Signed in as a@b.es, expected c@d.es");
        assert_eq!(
            serde_json::to_value(&error).unwrap()["message"],
            "Signed in as a@b.es, expected c@d.es"
        );
        assert_eq!(
            CommandError::validation("Enter a name").message_for_log(),
            "Enter a name"
        );
    }

    #[test]
    fn conflict_builds_a_conflict_error() {
        let error = CommandError::conflict("This platform is already added");
        assert_eq!(error.kind, CommandErrorKind::Conflict);
        assert_eq!(error.message, "This platform is already added");
    }

    #[test]
    fn kinds_serialize_snake_case() {
        let value = serde_json::to_value(CommandError::not_found("x")).unwrap();
        assert_eq!(value["kind"], "not_found");
    }

    #[test]
    fn repo_not_found_maps_to_not_found() {
        let error = CommandError::from(RepoError::NotFound);
        assert_eq!(error.kind, CommandErrorKind::NotFound);
    }

    #[test]
    fn repo_conflict_maps_to_conflict_and_hides_detail() {
        let error = CommandError::from(RepoError::Conflict("UNIQUE constraint failed".into()));
        assert_eq!(error.kind, CommandErrorKind::Conflict);
        assert_eq!(error.message, CONFLICT_MESSAGE);
        assert_eq!(error.detail.as_deref(), Some("UNIQUE constraint failed"));
    }

    #[test]
    fn repo_storage_maps_to_generic_internal() {
        let error = CommandError::from(RepoError::Storage("disk I/O error".into()));
        assert_eq!(error.kind, CommandErrorKind::Internal);
        assert_eq!(error.message, STORAGE_MESSAGE);
        assert_eq!(error.detail.as_deref(), Some("disk I/O error"));
    }

    #[test]
    fn run_errors_map_per_spec() {
        let running = CommandError::from(RunError::AlreadyRunning);
        assert_eq!(running.kind, CommandErrorKind::Conflict);
        assert_eq!(running.message, "A check is already running");

        let none = CommandError::from(RunError::NoEnabledAccounts);
        assert_eq!(none.kind, CommandErrorKind::Validation);

        let ai = CommandError::from(RunError::AiNotConfigured("Add your jev API key".into()));
        assert_eq!(ai.kind, CommandErrorKind::Validation);
        assert!(ai.message.contains("Add your jev API key"));

        let storage = CommandError::from(RunError::Storage("boom".into()));
        assert_eq!(storage.kind, CommandErrorKind::Internal);
        assert_eq!(storage.message, STORAGE_MESSAGE);
    }

    #[test]
    fn provider_errors_map_by_kind() {
        let auth = CommandError::from(ProviderError::Auth("bad".into()));
        assert_eq!(auth.kind, CommandErrorKind::Auth);
        let connection = CommandError::from(ProviderError::Connection("down".into()));
        assert_eq!(connection.kind, CommandErrorKind::Connection);
        let protocol = CommandError::from(ProviderError::Protocol("odd".into()));
        assert_eq!(protocol.kind, CommandErrorKind::Internal);
    }

    #[test]
    fn secret_errors_map_to_generic_internal() {
        let error = CommandError::from(SecretError("locked".into()));
        assert_eq!(error.kind, CommandErrorKind::Internal);
        assert_eq!(error.message, SECRET_STORAGE_MESSAGE);
        assert_eq!(error.detail.as_deref(), Some("locked"));
    }

    #[test]
    fn lms_errors_map_by_kind_keeping_the_message() {
        let auth = CommandError::from(LmsError::Auth("Sign-in cancelled".into()));
        assert_eq!(auth.kind, CommandErrorKind::Auth);
        assert_eq!(auth.message, "Sign-in cancelled");
        let connection = CommandError::from(LmsError::Connection("down".into()));
        assert_eq!(connection.kind, CommandErrorKind::Connection);
        let protocol = CommandError::from(LmsError::Protocol("odd".into()));
        assert_eq!(protocol.kind, CommandErrorKind::Internal);
    }

    #[test]
    fn session_errors_map_per_variant() {
        let login = CommandError::from(SessionError::LoginRequired("UAH".into()));
        assert_eq!(login.kind, CommandErrorKind::Auth);
        assert_eq!(login.message, "UAH needs you to sign in again");

        let lms = CommandError::from(SessionError::Lms(LmsError::Connection("down".into())));
        assert_eq!(lms.kind, CommandErrorKind::Connection);

        let storage = CommandError::from(SessionError::Storage("x".into()));
        assert_eq!(storage.kind, CommandErrorKind::Internal);
        assert_eq!(storage.message, STORAGE_MESSAGE);

        let secret = CommandError::from(SessionError::Secret("locked".into()));
        assert_eq!(secret.message, SECRET_STORAGE_MESSAGE);
        assert_eq!(secret.detail.as_deref(), Some("locked"));
    }

    #[test]
    fn lms_sync_error_mapping() {
        let running = CommandError::from(LmsSyncError::AlreadyRunning);
        assert_eq!(running.kind, CommandErrorKind::Conflict);
        assert_eq!(running.message, "A sync is already running");

        let nothing = CommandError::from(LmsSyncError::NothingToSync);
        assert_eq!(nothing.kind, CommandErrorKind::Validation);
        assert_eq!(nothing.message, "Sign in to a university first");

        let storage = CommandError::from(LmsSyncError::Storage("disk full".into()));
        assert_eq!(storage.kind, CommandErrorKind::Internal);
        assert_eq!(storage.message, STORAGE_MESSAGE);
        assert_eq!(storage.detail.as_deref(), Some("disk full"));
    }
}
