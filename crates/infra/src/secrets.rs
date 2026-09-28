//! `KeyringSecretStore` (Windows Credential Manager) and `InMemorySecretStore` (Task T7).
//!
//! See `docs/infra/secure-storage.md` for details.

use std::collections::HashMap;
use std::sync::Mutex;

use academic_core::domain::SECRET_SERVICE;
use academic_core::ports::{SecretError, SecretStore};

/// Secret store backed by the OS credential store. On Windows this is the Credential
/// Manager: each key is stored as one generic credential with service `SECRET_SERVICE`
/// and target/user set to the key itself.
pub struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        match new_entry(key)?.get_secret() {
            Ok(bytes) => String::from_utf8(bytes)
                .map(Some)
                .map_err(|_| SecretError("stored secret was not valid UTF-8".to_string())),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(err) => Err(map_keyring_error(err)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        // Stored as raw UTF-8 bytes via `set_secret` rather than `set_password`, which
        // re-encodes the value as UTF-16 and caps it at 1280 chars on Windows — too
        // small for some Entra refresh tokens.
        new_entry(key)?
            .set_secret(value.as_bytes())
            .map_err(map_keyring_error)
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        match new_entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(err) => Err(map_keyring_error(err)),
        }
    }
}

fn new_entry(key: &str) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new(SECRET_SERVICE, key).map_err(map_keyring_error)
}

/// Never includes the secret value: a `keyring::Error` only describes the platform
/// failure (e.g. "no matching entry"), never the credential that was stored or read.
/// `SecretError`'s own `Display` already prefixes "Secure storage error: ", so the
/// message here carries no prefix of its own.
fn map_keyring_error(err: keyring::Error) -> SecretError {
    SecretError(err.to_string())
}

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
