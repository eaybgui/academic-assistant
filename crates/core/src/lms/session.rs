//! Session check and credential adoption shared by sign-in, "Check connection" and
//! (from M3) sync (spec §6.4, §6.5, §18.4).

use crate::domain::{lms_credential_key, LmsIdentity, LmsUniversity, LoginState};
use crate::ports::{
    LmsAuthenticator, LmsClientFactory, LmsError, RepoError, SecretError, SecretStore,
    UniversityRepository,
};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    /// The user has to sign in interactively. Holds the university name.
    #[error("{0} needs you to sign in again")]
    LoginRequired(String),
    #[error(transparent)]
    Lms(#[from] LmsError),
    #[error("Storage error: {0}")]
    Storage(String),
    #[error("Secure storage error: {0}")]
    Secret(String),
}

impl From<RepoError> for SessionError {
    fn from(error: RepoError) -> Self {
        SessionError::Storage(error.to_string())
    }
}

impl From<SecretError> for SessionError {
    fn from(error: SecretError) -> Self {
        SessionError::Secret(error.0)
    }
}

/// Verifies the stored credential. On `Auth`, tries one silent renewal and verifies
/// again. Updates `login_state` (`Ok` with the platform's display name, or `Expired`
/// keeping the previous label). Connection/protocol errors leave the state unchanged.
pub async fn ensure_session(
    university: &LmsUniversity,
    factory: &dyn LmsClientFactory,
    authenticator: &dyn LmsAuthenticator,
    universities: &dyn UniversityRepository,
) -> Result<LmsUniversity, SessionError> {
    if university.login_state == LoginState::Never {
        return Err(SessionError::LoginRequired(university.name.clone()));
    }
    match verify(university, factory).await {
        Ok(identity) => return mark_ok(universities, university, &identity),
        Err(LmsError::Auth(_)) => {}
        Err(other) => return Err(other.into()),
    }
    match authenticator.renew_silently(university).await {
        Ok(()) => {}
        Err(LmsError::Auth(_)) => return mark_expired(universities, university),
        Err(other) => return Err(other.into()),
    }
    match verify(university, factory).await {
        Ok(identity) => mark_ok(universities, university, &identity),
        Err(LmsError::Auth(_)) => mark_expired(universities, university),
        Err(other) => Err(other.into()),
    }
}

/// Verifies a freshly captured credential and, only if the platform accepts it,
/// stores it. Does not change `login_state` (the caller decides).
pub async fn adopt_credential(
    university: &LmsUniversity,
    credential: &str,
    factory: &dyn LmsClientFactory,
    secrets: &dyn SecretStore,
) -> Result<LmsIdentity, SessionError> {
    let identity = factory
        .create_with_credential(university, credential)?
        .verify()
        .await?;
    secrets.set(
        &lms_credential_key(university.platform, university.id),
        credential,
    )?;
    Ok(identity)
}

async fn verify(
    university: &LmsUniversity,
    factory: &dyn LmsClientFactory,
) -> Result<LmsIdentity, LmsError> {
    factory.create(university)?.verify().await
}

fn mark_ok(
    universities: &dyn UniversityRepository,
    university: &LmsUniversity,
    identity: &LmsIdentity,
) -> Result<LmsUniversity, SessionError> {
    Ok(universities.set_login(university.id, LoginState::Ok, Some(&identity.display_name))?)
}

fn mark_expired(
    universities: &dyn UniversityRepository,
    university: &LmsUniversity,
) -> Result<LmsUniversity, SessionError> {
    universities.set_login(
        university.id,
        LoginState::Expired,
        university.account_label.as_deref(),
    )?;
    Err(SessionError::LoginRequired(university.name.clone()))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use async_trait::async_trait;
    use chrono::{DateTime, Utc};

    use super::*;
    use crate::domain::{
        LmsPlatform, RemoteCourse, RemoteCourseRef, RemoteItem, SyncOutcome, SyncWindow,
    };
    use crate::ports::{LmsClient, RepoError, RepoResult, SecretError};

    fn university(state: LoginState) -> LmsUniversity {
        LmsUniversity {
            id: 7,
            name: "UAH".into(),
            platform: LmsPlatform::Blackboard,
            base_url: "https://uah.blackboard.com".into(),
            login_state: state,
            account_label: Some("Old Name".into()),
            last_sync_at: None,
            last_sync_outcome: None,
            last_sync_error: None,
            created_at: Utc::now(),
        }
    }

    fn identity() -> LmsIdentity {
        LmsIdentity {
            user_id: "_1_1".into(),
            display_name: "Ana Pérez".into(),
        }
    }

    /// Each `verify()` pops the next scripted result (shared by every client built).
    #[derive(Default)]
    struct FakeFactory {
        verify_results: Mutex<VecDeque<Result<LmsIdentity, LmsError>>>,
        credentials_seen: Mutex<Vec<String>>,
    }
    struct FakeClient(Result<LmsIdentity, LmsError>);

    #[async_trait]
    impl LmsClient for FakeClient {
        async fn verify(&self) -> Result<LmsIdentity, LmsError> {
            match &self.0 {
                Ok(id) => Ok(id.clone()),
                Err(LmsError::Auth(m)) => Err(LmsError::Auth(m.clone())),
                Err(LmsError::Connection(m)) => Err(LmsError::Connection(m.clone())),
                Err(LmsError::Protocol(m)) => Err(LmsError::Protocol(m.clone())),
            }
        }
        async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError> {
            Ok(vec![])
        }
        async fn fetch_items(
            &self,
            _courses: &[RemoteCourseRef],
            _window: &SyncWindow,
        ) -> Result<Vec<RemoteItem>, LmsError> {
            Ok(vec![])
        }
    }
    impl FakeFactory {
        fn scripted(results: Vec<Result<LmsIdentity, LmsError>>) -> Self {
            Self {
                verify_results: Mutex::new(results.into()),
                ..Default::default()
            }
        }
        fn next(&self) -> Box<dyn LmsClient> {
            let result = self
                .verify_results
                .lock()
                .unwrap()
                .pop_front()
                .expect("scripted result");
            Box::new(FakeClient(result))
        }
    }
    impl LmsClientFactory for FakeFactory {
        fn create(&self, _u: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError> {
            Ok(self.next())
        }
        fn create_with_credential(
            &self,
            _u: &LmsUniversity,
            c: &str,
        ) -> Result<Box<dyn LmsClient>, LmsError> {
            self.credentials_seen.lock().unwrap().push(c.to_string());
            Ok(self.next())
        }
    }

    struct FakeAuthenticator {
        result: Mutex<Option<Result<(), LmsError>>>,
        calls: Mutex<u32>,
    }
    impl FakeAuthenticator {
        fn returning(result: Result<(), LmsError>) -> Self {
            Self {
                result: Mutex::new(Some(result)),
                calls: Mutex::new(0),
            }
        }
    }
    #[async_trait]
    impl LmsAuthenticator for FakeAuthenticator {
        async fn renew_silently(&self, _u: &LmsUniversity) -> Result<(), LmsError> {
            *self.calls.lock().unwrap() += 1;
            self.result
                .lock()
                .unwrap()
                .take()
                .expect("renewal scripted once")
        }
    }

    /// Records `set_login` calls; returns the university with the new state.
    struct FakeUniversities {
        base: LmsUniversity,
        logins: Mutex<Vec<(LoginState, Option<String>)>>,
    }
    impl FakeUniversities {
        fn new(base: LmsUniversity) -> Self {
            Self {
                base,
                logins: Mutex::new(vec![]),
            }
        }
    }
    impl UniversityRepository for FakeUniversities {
        fn list_universities(&self) -> RepoResult<Vec<LmsUniversity>> {
            unreachable!()
        }
        fn get_university(&self, _id: i64) -> RepoResult<Option<LmsUniversity>> {
            unreachable!()
        }
        fn insert_university(
            &self,
            _n: &str,
            _p: LmsPlatform,
            _b: &str,
            _t: DateTime<Utc>,
        ) -> RepoResult<LmsUniversity> {
            unreachable!()
        }
        fn rename_university(&self, _id: i64, _n: &str) -> RepoResult<LmsUniversity> {
            unreachable!()
        }
        fn delete_university(&self, _id: i64) -> RepoResult<()> {
            unreachable!()
        }
        fn set_login(
            &self,
            _id: i64,
            state: LoginState,
            label: Option<&str>,
        ) -> RepoResult<LmsUniversity> {
            self.logins
                .lock()
                .unwrap()
                .push((state, label.map(str::to_string)));
            Ok(LmsUniversity {
                login_state: state,
                account_label: label.map(str::to_string),
                ..self.base.clone()
            })
        }
        fn record_sync(
            &self,
            _id: i64,
            _a: DateTime<Utc>,
            _o: SyncOutcome,
            _e: Option<&str>,
        ) -> RepoResult<()> {
            unreachable!()
        }
    }

    #[derive(Default)]
    struct FakeSecrets {
        stored: Mutex<Vec<(String, String)>>,
        fail: bool,
    }
    impl SecretStore for FakeSecrets {
        fn get(&self, _k: &str) -> Result<Option<String>, SecretError> {
            Ok(None)
        }
        fn set(&self, k: &str, v: &str) -> Result<(), SecretError> {
            if self.fail {
                return Err(SecretError("locked".into()));
            }
            self.stored.lock().unwrap().push((k.into(), v.into()));
            Ok(())
        }
        fn delete(&self, _k: &str) -> Result<(), SecretError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn valid_session_marks_ok_with_the_platform_name() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![Ok(identity())]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        let updated = ensure_session(&u, &factory, &auth, &repo).await.unwrap();
        assert_eq!(updated.login_state, LoginState::Ok);
        assert_eq!(updated.account_label.as_deref(), Some("Ana Pérez"));
        assert_eq!(*auth.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn expired_session_is_renewed_silently_then_verified_again() {
        let u = university(LoginState::Ok);
        let factory =
            FakeFactory::scripted(vec![Err(LmsError::Auth("expired".into())), Ok(identity())]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        let updated = ensure_session(&u, &factory, &auth, &repo).await.unwrap();
        assert_eq!(updated.login_state, LoginState::Ok);
        assert_eq!(*auth.calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn failed_silent_renewal_marks_expired_and_keeps_the_label() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![Err(LmsError::Auth("expired".into()))]);
        let auth = FakeAuthenticator::returning(Err(LmsError::Auth("Sign-in required".into())));
        let repo = FakeUniversities::new(u.clone());
        let error = ensure_session(&u, &factory, &auth, &repo)
            .await
            .unwrap_err();
        assert!(matches!(error, SessionError::LoginRequired(ref name) if name == "UAH"));
        assert_eq!(
            repo.logins.lock().unwrap().as_slice(),
            &[(LoginState::Expired, Some("Old Name".to_string()))]
        );
    }

    #[tokio::test]
    async fn auth_failure_after_renewal_marks_expired() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![
            Err(LmsError::Auth("expired".into())),
            Err(LmsError::Auth("still rejected".into())),
        ]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        assert!(matches!(
            ensure_session(&u, &factory, &auth, &repo).await,
            Err(SessionError::LoginRequired(_))
        ));
        assert_eq!(repo.logins.lock().unwrap()[0].0, LoginState::Expired);
    }

    #[tokio::test]
    async fn connection_errors_do_not_change_the_login_state() {
        let u = university(LoginState::Ok);
        let factory = FakeFactory::scripted(vec![Err(LmsError::Connection("offline".into()))]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        assert!(matches!(
            ensure_session(&u, &factory, &auth, &repo).await,
            Err(SessionError::Lms(LmsError::Connection(_)))
        ));
        assert!(repo.logins.lock().unwrap().is_empty());
        assert_eq!(*auth.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn never_signed_in_requires_login_without_any_request() {
        let u = university(LoginState::Never);
        let factory = FakeFactory::scripted(vec![]);
        let auth = FakeAuthenticator::returning(Ok(()));
        let repo = FakeUniversities::new(u.clone());
        assert!(matches!(
            ensure_session(&u, &factory, &auth, &repo).await,
            Err(SessionError::LoginRequired(_))
        ));
        assert_eq!(*auth.calls.lock().unwrap(), 0);
    }

    #[tokio::test]
    async fn adopt_credential_verifies_before_storing() {
        let u = university(LoginState::Never);
        let factory = FakeFactory::scripted(vec![Ok(identity())]);
        let secrets = FakeSecrets::default();
        let id = adopt_credential(&u, "[{\"name\":\"a\",\"value\":\"b\"}]", &factory, &secrets)
            .await
            .unwrap();
        assert_eq!(id.display_name, "Ana Pérez");
        assert_eq!(
            secrets.stored.lock().unwrap()[0].0,
            "academic-assistant/university/7/blackboard-session"
        );
    }

    #[tokio::test]
    async fn rejected_credential_is_never_stored() {
        let u = university(LoginState::Never);
        let factory = FakeFactory::scripted(vec![Err(LmsError::Auth("rejected".into()))]);
        let secrets = FakeSecrets::default();
        assert!(adopt_credential(&u, "x", &factory, &secrets).await.is_err());
        assert!(secrets.stored.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn secret_store_failure_is_reported() {
        let u = university(LoginState::Never);
        let factory = FakeFactory::scripted(vec![Ok(identity())]);
        let secrets = FakeSecrets {
            fail: true,
            ..Default::default()
        };
        assert!(matches!(
            adopt_credential(&u, "x", &factory, &secrets).await,
            Err(SessionError::Secret(_))
        ));
    }

    #[test]
    fn repo_errors_become_storage_errors() {
        assert!(matches!(
            SessionError::from(RepoError::NotFound),
            SessionError::Storage(_)
        ));
    }
}
