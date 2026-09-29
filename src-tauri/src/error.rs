//! `CommandError`: the error every Tauri command returns to the frontend.
//!
//! Serializes as `{ kind, message }` (spec §10). `message` is user-facing and
//! sanitized; anything more detailed (storage errors, provider internals) is kept
//! in `detail` — never serialized, only logged.

use academic_core::domain::RunError;
use academic_core::ports::{ProviderError, RepoError, SecretError};
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
}

impl CommandError {
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(CommandErrorKind::Validation, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(CommandErrorKind::NotFound, message)
    }

    fn new(kind: CommandErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            detail: None,
        }
    }

    fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
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
            message = %self.message,
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
}
