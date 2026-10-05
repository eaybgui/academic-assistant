use async_trait::async_trait;

use crate::domain::{
    ErrorKind, LmsIdentity, LmsUniversity, RemoteCourse, RemoteCourseRef, RemoteItem, SyncWindow,
};

/// Error returned by an LMS client, factory or authenticator. Every message is
/// user-facing and never contains a token, cookie or URL from the login window.
#[derive(Debug, thiserror::Error)]
pub enum LmsError {
    /// Missing, unreadable, expired or rejected credential: sign-in is needed.
    #[error("{0}")]
    Auth(String),
    #[error("{0}")]
    Connection(String),
    /// Unexpected response or a platform function that is not available.
    #[error("{0}")]
    Protocol(String),
}

impl LmsError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            LmsError::Auth(_) => ErrorKind::Auth,
            LmsError::Connection(_) => ErrorKind::Connection,
            LmsError::Protocol(_) => ErrorKind::Protocol,
        }
    }
}

/// Read-only access to one university's platform as the signed-in user.
#[async_trait]
pub trait LmsClient: Send + Sync {
    /// Who the credential belongs to; also checks the platform exposes what we need.
    async fn verify(&self) -> Result<LmsIdentity, LmsError>;
    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError>;
    /// Assignments, quizzes, gradebook items and course events of `courses`.
    /// All or nothing: any failed request fails the whole call.
    async fn fetch_items(
        &self,
        courses: &[RemoteCourseRef],
        window: &SyncWindow,
    ) -> Result<Vec<RemoteItem>, LmsError>;
}

pub trait LmsClientFactory: Send + Sync {
    /// Builds a client with the credential stored in `SecretStore`. A missing or
    /// unreadable credential is `LmsError::Auth`.
    fn create(&self, university: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError>;
    /// Builds a client with a credential that is not stored yet (used to verify a
    /// freshly captured credential before storing it).
    fn create_with_credential(
        &self,
        university: &LmsUniversity,
        credential: &str,
    ) -> Result<Box<dyn LmsClient>, LmsError>;
}

/// Renews a university's credential without user interaction (hidden login window).
#[async_trait]
pub trait LmsAuthenticator: Send + Sync {
    /// On success the new credential is already verified and stored.
    /// `Err(LmsError::Auth)` when the user has to sign in interactively.
    async fn renew_silently(&self, university: &LmsUniversity) -> Result<(), LmsError>;
}
