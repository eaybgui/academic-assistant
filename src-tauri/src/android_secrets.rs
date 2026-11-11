//! [`SecretStore`] backed by the Kotlin `AcademicPlugin`, which keeps values
//! encrypted under an Android Keystore key.
//!
//! The calls block until Kotlin answers, so they must never run on the Android
//! main thread. Every command that touches secrets is `async` and runs on the
//! async runtime, and `setup` only builds the store without using it.

use academic_core::ports::{SecretError, SecretStore};
use serde::{Deserialize, Serialize};
use tauri::plugin::PluginHandle;
use tauri::Runtime;

#[derive(Serialize)]
struct KeyArgs<'a> {
    key: &'a str,
}

#[derive(Serialize)]
struct SetArgs<'a> {
    key: &'a str,
    value: &'a str,
}

#[derive(Deserialize)]
struct SecretValue {
    value: Option<String>,
}

pub struct AndroidSecretStore<R: Runtime> {
    plugin: PluginHandle<R>,
}

impl<R: Runtime> AndroidSecretStore<R> {
    pub fn new(plugin: PluginHandle<R>) -> Self {
        Self { plugin }
    }
}

fn secret_error(error: impl std::fmt::Display) -> SecretError {
    SecretError(error.to_string())
}

impl<R: Runtime> SecretStore for AndroidSecretStore<R> {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        let found: SecretValue = self
            .plugin
            .run_mobile_plugin("secretGet", KeyArgs { key })
            .map_err(secret_error)?;
        Ok(found.value)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        self.plugin
            .run_mobile_plugin::<()>("secretSet", SetArgs { key, value })
            .map_err(secret_error)
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.plugin
            .run_mobile_plugin::<()>("secretDelete", KeyArgs { key })
            .map_err(secret_error)
    }
}
