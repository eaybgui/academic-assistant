//! Integration tests for `academic_infra::factory`: `DefaultEmailProviderFactory` and
//! `DefaultModelProvider` build the right adapter (or the documented error) for every
//! account/settings combination, without performing any network I/O.

use std::sync::Arc;

use chrono::Utc;

use academic_core::domain::{
    account_password_key, Account, AiProviderKind, ProviderConfig, Settings, DEFAULT_GRAPH_TENANT,
    JEV_API_KEY,
};
use academic_core::ports::{
    EmailProviderFactory, ModelProvider, ProviderError, SecretStore, SettingsRepository,
};
use academic_infra::db::Database;
use academic_infra::factory::{http_client, DefaultEmailProviderFactory, DefaultModelProvider};
use academic_infra::secrets::InMemorySecretStore;

fn imap_account(id: i64) -> Account {
    Account {
        id,
        name: "Test IMAP".to_string(),
        email_address: "student@example.test".to_string(),
        provider: ProviderConfig::Imap {
            host: "imap.example.test".to_string(),
            port: 993,
            username: "student@example.test".to_string(),
        },
        folders: vec!["INBOX".to_string()],
        default_university_id: None,
        initial_sync_days: 14,
        enabled: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

fn graph_account(id: i64) -> Account {
    Account {
        id,
        name: "Test Graph".to_string(),
        email_address: "student@uv.es".to_string(),
        provider: ProviderConfig::MicrosoftGraph {
            tenant: DEFAULT_GRAPH_TENANT.to_string(),
        },
        folders: vec!["inbox".to_string()],
        default_university_id: None,
        initial_sync_days: 14,
        enabled: true,
        created_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

#[test]
fn imap_factory_builds_a_provider_when_the_password_is_stored() {
    let secrets = Arc::new(InMemorySecretStore::default());
    let account = imap_account(1);
    secrets
        .set(&account_password_key(account.id), "s3cret")
        .unwrap();

    let settings: Arc<dyn SettingsRepository> = Arc::new(Database::open_in_memory().unwrap());
    let factory = DefaultEmailProviderFactory::new(secrets, settings, http_client());

    assert!(factory.create(&account).is_ok());
}

#[test]
fn imap_factory_fails_when_the_password_is_missing() {
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    let settings: Arc<dyn SettingsRepository> = Arc::new(Database::open_in_memory().unwrap());
    let factory = DefaultEmailProviderFactory::new(secrets, settings, http_client());
    let account = imap_account(2);

    let err = match factory.create(&account) {
        Ok(_) => panic!("missing password should fail"),
        Err(err) => err,
    };

    assert!(
        matches!(&err, ProviderError::Auth(msg) if msg == "The password is missing. Edit the account to set it")
    );
}

#[test]
fn graph_factory_builds_a_provider_when_the_client_id_is_configured() {
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    let db = Database::open_in_memory().unwrap();
    let settings = Settings {
        microsoft_client_id: Some("client-id".to_string()),
        ..Settings::default()
    };
    db.save(&settings).unwrap();
    let settings: Arc<dyn SettingsRepository> = Arc::new(db);
    let factory = DefaultEmailProviderFactory::new(secrets, settings, http_client());
    let account = graph_account(3);

    assert!(factory.create(&account).is_ok());
}

#[test]
fn graph_factory_fails_when_the_client_id_is_not_configured() {
    let secrets: Arc<dyn SecretStore> = Arc::new(InMemorySecretStore::default());
    let settings: Arc<dyn SettingsRepository> = Arc::new(Database::open_in_memory().unwrap());
    let factory = DefaultEmailProviderFactory::new(secrets, settings, http_client());
    let account = graph_account(4);

    let err = match factory.create(&account) {
        Ok(_) => panic!("missing client id should fail"),
        Err(err) => err,
    };

    assert!(
        matches!(&err, ProviderError::Auth(msg) if msg == "Set the Microsoft client ID in Settings")
    );
}

#[test]
fn model_provider_builds_ollama_from_settings_by_default() {
    let secrets = Arc::new(InMemorySecretStore::default());
    let provider = DefaultModelProvider::new(secrets, http_client());
    let settings = Settings::default();

    let model = provider.current(&settings).expect("ollama needs no secret");
    let descriptor = model.descriptor();

    assert_eq!(descriptor.provider, "ollama");
    assert_eq!(descriptor.model, settings.ollama_model);
}

#[test]
fn model_provider_builds_jev_when_the_key_is_stored() {
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets.set(JEV_API_KEY, "jev-key").unwrap();
    let provider = DefaultModelProvider::new(secrets, http_client());
    let settings = Settings {
        ai_provider: AiProviderKind::Jev,
        jev_model: "jev-custom".to_string(),
        ..Settings::default()
    };

    let model = provider.current(&settings).expect("key is stored");
    let descriptor = model.descriptor();

    assert_eq!(descriptor.provider, "jev");
    assert_eq!(descriptor.model, "jev-custom");
}

#[test]
fn model_provider_fails_when_jev_is_selected_without_a_key() {
    let secrets = Arc::new(InMemorySecretStore::default());
    let provider = DefaultModelProvider::new(secrets, http_client());
    let settings = Settings {
        ai_provider: AiProviderKind::Jev,
        ..Settings::default()
    };

    let err = match provider.current(&settings) {
        Ok(_) => panic!("missing key should fail"),
        Err(err) => err,
    };

    assert_eq!(err, "Add your jev API key in Settings");
}

#[test]
fn http_client_can_be_built_repeatedly_without_network_io() {
    let _ = http_client();
    let _ = http_client();
}
