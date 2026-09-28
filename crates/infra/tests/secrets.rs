//! Integration tests for `academic_infra::secrets`.

use academic_core::ports::SecretStore;
use academic_infra::secrets::{InMemorySecretStore, KeyringSecretStore};

#[test]
fn in_memory_store_returns_none_for_a_missing_key() {
    let store = InMemorySecretStore::default();
    assert_eq!(store.get("missing").unwrap(), None);
}

#[test]
fn in_memory_store_round_trips_a_value() {
    let store = InMemorySecretStore::default();

    store.set("k", "secret-value").unwrap();
    assert_eq!(store.get("k").unwrap(), Some("secret-value".to_string()));

    store.delete("k").unwrap();
    assert_eq!(store.get("k").unwrap(), None);
}

#[test]
fn in_memory_store_overwrites_an_existing_value() {
    let store = InMemorySecretStore::default();
    store.set("k", "first").unwrap();
    store.set("k", "second").unwrap();
    assert_eq!(store.get("k").unwrap(), Some("second".to_string()));
}

#[test]
fn in_memory_store_delete_of_a_missing_key_is_ok() {
    let store = InMemorySecretStore::default();
    assert!(store.delete("missing").is_ok());
}

/// Touches the real Windows Credential Manager. Run explicitly with:
/// `cargo test -p academic-infra --test secrets -- --ignored`.
#[test]
#[ignore]
fn keyring_store_round_trips_a_value() {
    let key = format!("academic-assistant/test/{:x}", rand::random::<u64>());
    let store = KeyringSecretStore;

    // Clean slate in case a previous failed run left this key behind.
    store.delete(&key).unwrap();
    assert_eq!(store.get(&key).unwrap(), None);

    store.set(&key, "test-value").unwrap();
    assert_eq!(store.get(&key).unwrap(), Some("test-value".to_string()));

    store.delete(&key).unwrap();
    assert_eq!(store.get(&key).unwrap(), None);
}
