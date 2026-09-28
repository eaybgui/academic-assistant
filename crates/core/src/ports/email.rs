use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::domain::{Account, ErrorKind, FetchBatch, SyncCursor};

/// Error returned by an [`EmailProvider`] operation.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("Authentication failed: {0}")]
    Auth(String),
    #[error("Connection failed: {0}")]
    Connection(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
}

impl ProviderError {
    pub fn kind(&self) -> ErrorKind {
        match self {
            ProviderError::Auth(_) => ErrorKind::Auth,
            ProviderError::Connection(_) => ErrorKind::Connection,
            ProviderError::Protocol(_) => ErrorKind::Protocol,
        }
    }
}

/// A read-only connection to one account's mailbox: connectivity check and
/// incremental fetch. Implemented per provider (IMAP, Microsoft Graph) in `academic-infra`.
#[async_trait]
pub trait EmailProvider: Send + Sync {
    async fn test_connection(&self) -> Result<(), ProviderError>;

    async fn fetch_new(
        &self,
        folder: &str,
        cursor: Option<&SyncCursor>,
        since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError>;
}

/// Builds the right [`EmailProvider`] for an [`Account`] based on its [`crate::domain::ProviderConfig`].
pub trait EmailProviderFactory: Send + Sync {
    fn create(&self, account: &Account) -> Result<Box<dyn EmailProvider>, ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_error_kind_maps_each_variant() {
        assert_eq!(ProviderError::Auth("x".into()).kind(), ErrorKind::Auth);
        assert_eq!(
            ProviderError::Connection("x".into()).kind(),
            ErrorKind::Connection
        );
        assert_eq!(
            ProviderError::Protocol("x".into()).kind(),
            ErrorKind::Protocol
        );
    }
}
