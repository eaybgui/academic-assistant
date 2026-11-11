//! `KeyringSecretStore` (Windows Credential Manager, desktop only) and
//! `InMemorySecretStore` (Task T7).
//!
//! See `docs/infra/secure-storage.md` for details.

use std::collections::HashMap;
use std::sync::Mutex;

use academic_core::ports::{SecretError, SecretStore};

#[cfg(not(target_os = "android"))]
mod keyring_store;

#[cfg(not(target_os = "android"))]
pub use keyring_store::KeyringSecretStore;

/// Largest secret the Windows Credential Manager accepts: its
/// `CRED_MAX_CREDENTIAL_BLOB_SIZE`, in bytes.
pub const MAX_SECRET_BYTES: usize = 2560;

/// In-memory [`SecretStore`], for tests. Used by this crate's own tests and by other
/// tasks' tests that need a `SecretStore` without touching the OS credential store.
#[derive(Default)]
pub struct InMemorySecretStore {
    values: Mutex<HashMap<String, String>>,
}

impl InMemorySecretStore {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, String>> {
        self.values
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl SecretStore for InMemorySecretStore {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        Ok(self.lock().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.lock().insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.lock().remove(key);
        Ok(())
    }
}
