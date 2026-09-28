//! Test-only support shared by `graph_*.rs` integration tests.
//!
//! `FakeSecretStore` is a minimal in-memory `SecretStore` used only because
//! `academic_infra::secrets::InMemorySecretStore` (Task T7) is implemented in a
//! parallel worktree and was not available while this task was written. Once T7 is
//! merged, these tests can switch to the real `InMemorySecretStore` and this file
//! can be deleted.

use std::collections::HashMap;
use std::sync::Mutex;

use academic_core::ports::{SecretError, SecretStore};

/// An in-memory `SecretStore` fake for tests: not persisted, not encrypted.
#[derive(Default)]
pub struct FakeSecretStore {
    entries: Mutex<HashMap<String, String>>,
}

impl FakeSecretStore {
    pub fn new() -> Self {
        Self::default()
    }

    /// Seeds `key` with `value` before the store is used by the code under test.
    pub fn seed(&self, key: &str, value: &str) {
        self.entries
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
    }

    /// Returns the current value for `key`, if any. Used to assert what the code
    /// under test wrote (e.g. a rotated refresh token).
    pub fn get_value(&self, key: &str) -> Option<String> {
        self.entries.lock().unwrap().get(key).cloned()
    }
}

impl SecretStore for FakeSecretStore {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        Ok(self.entries.lock().unwrap().get(key).cloned())
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.entries
            .lock()
            .unwrap()
            .insert(key.to_string(), value.to_string());
        Ok(())
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.entries.lock().unwrap().remove(key);
        Ok(())
    }
}
