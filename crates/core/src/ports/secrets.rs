/// Error returned by a [`SecretStore`] operation.
#[derive(Debug, thiserror::Error)]
#[error("Secure storage error: {0}")]
pub struct SecretError(pub String);

/// Access to the OS-level secure credential store. Implemented by
/// `academic-infra::secrets::KeyringSecretStore` (Windows Credential Manager) and,
/// for tests, an in-memory fake.
pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    /// Deletes the entry for `key`. Returns `Ok(())` when no entry exists.
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}
