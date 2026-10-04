use std::sync::Arc;

use academic_core::domain::{lms_credential_key, LmsPlatform, LmsUniversity, LoginState};
use academic_core::ports::{LmsClientFactory, LmsError, SecretStore};
use academic_infra::lms::factory::DefaultLmsClientFactory;
use academic_infra::secrets::InMemorySecretStore;
use chrono::Utc;

fn uni(platform: LmsPlatform) -> LmsUniversity {
    LmsUniversity {
        id: 5,
        name: "X".into(),
        platform,
        base_url: "https://x.es".into(),
        login_state: LoginState::Ok,
        account_label: None,
        last_sync_at: None,
        last_sync_outcome: None,
        last_sync_error: None,
        created_at: Utc::now(),
    }
}

fn factory(secrets: Arc<InMemorySecretStore>) -> DefaultLmsClientFactory {
    DefaultLmsClientFactory::new(secrets, reqwest::Client::new())
}

#[test]
fn missing_credential_is_an_auth_error() {
    let f = factory(Arc::new(InMemorySecretStore::default()));
    assert!(matches!(
        f.create(&uni(LmsPlatform::Moodle)),
        Err(LmsError::Auth(_))
    ));
    assert!(matches!(
        f.create(&uni(LmsPlatform::Blackboard)),
        Err(LmsError::Auth(_))
    ));
}

#[test]
fn unreadable_blackboard_session_is_an_auth_error() {
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&lms_credential_key(LmsPlatform::Blackboard, 5), "[]garbage")
        .unwrap();
    assert!(matches!(
        factory(secrets).create(&uni(LmsPlatform::Blackboard)),
        Err(LmsError::Auth(_))
    ));
}

#[test]
fn stored_credentials_build_clients() {
    let secrets = Arc::new(InMemorySecretStore::default());
    secrets
        .set(&lms_credential_key(LmsPlatform::Moodle, 5), "tok")
        .unwrap();
    secrets
        .set(
            &lms_credential_key(LmsPlatform::Blackboard, 5),
            r#"[{"name":"BbRouter","value":"v"}]"#,
        )
        .unwrap();
    let f = factory(secrets);
    assert!(f.create(&uni(LmsPlatform::Moodle)).is_ok());
    assert!(f.create(&uni(LmsPlatform::Blackboard)).is_ok());
    assert!(f
        .create_with_credential(&uni(LmsPlatform::Moodle), "fresh")
        .is_ok());
}
