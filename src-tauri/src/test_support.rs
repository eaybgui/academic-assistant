//! Test fixtures: an [`AppState`] over an in-memory database, an in-memory secret
//! store and fake provider/model adapters.

use std::sync::Arc;

use academic_core::domain::{
    lms_credential_key, AIAnalysis, Account, AccountDraft, Category, FetchBatch, LmsIdentity,
    LmsUniversity, MessageOutcome, ModelDescriptor, NewResult, Priority, ProcessedMessage,
    ProviderConfig, RemoteCourse, RemoteCourseRef, RemoteItem, RunId, RunProgress, RunStatus,
    RunTrigger, Settings, SyncCursor, SyncWindow,
};
use academic_core::ports::{
    Answer, AnswerSet, EmailProvider, EmailProviderFactory, LmsAuthenticator, LmsClient,
    LmsClientFactory, LmsError, ModelError, ModelProvider, ProviderError, QuestionSet, RunObserver,
    SecretStore, StructuredModel, SystemClock,
};
use academic_infra::db::Database;
use academic_infra::secrets::InMemorySecretStore;
use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::state::{AppState, Collaborators};

pub struct Harness {
    pub state: AppState,
    pub secrets: Arc<InMemorySecretStore>,
}

/// How a fake provider behaves. Function pointers keep the fakes `Send + Sync`
/// without requiring `ProviderError`/`ModelError` to be `Clone`.
#[derive(Clone, Copy)]
pub struct FakeProviderBehavior {
    pub create: fn() -> Result<(), ProviderError>,
    pub test_connection: fn() -> Result<(), ProviderError>,
}

impl Default for FakeProviderBehavior {
    fn default() -> Self {
        Self {
            create: || Ok(()),
            test_connection: || Ok(()),
        }
    }
}

/// Scripted LMS behavior for command tests.
#[derive(Clone, Copy)]
pub struct FakeLmsBehavior {
    pub verify: fn() -> Result<LmsIdentity, LmsError>,
    pub renew: fn() -> Result<(), LmsError>,
}

impl Default for FakeLmsBehavior {
    fn default() -> Self {
        Self {
            verify: || {
                Ok(LmsIdentity {
                    user_id: "1".into(),
                    display_name: "Ana Pérez".into(),
                })
            },
            renew: || Err(LmsError::Auth("Sign-in required".into())),
        }
    }
}

/// Like the real factory, `create` fails with `Auth` when no credential is stored.
struct FakeLmsFactory {
    behavior: FakeLmsBehavior,
    secrets: Arc<dyn SecretStore>,
}

struct FakeLmsClient(FakeLmsBehavior);

struct FakeLmsAuthenticator(FakeLmsBehavior);

impl LmsClientFactory for FakeLmsFactory {
    fn create(&self, university: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError> {
        let key = lms_credential_key(university.platform, university.id);
        match self.secrets.get(&key) {
            Ok(Some(_)) => Ok(Box::new(FakeLmsClient(self.behavior))),
            _ => Err(LmsError::Auth("Sign in".into())),
        }
    }

    fn create_with_credential(
        &self,
        _university: &LmsUniversity,
        _credential: &str,
    ) -> Result<Box<dyn LmsClient>, LmsError> {
        Ok(Box::new(FakeLmsClient(self.behavior)))
    }
}

#[async_trait]
impl LmsClient for FakeLmsClient {
    async fn verify(&self) -> Result<LmsIdentity, LmsError> {
        (self.0.verify)()
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

#[async_trait]
impl LmsAuthenticator for FakeLmsAuthenticator {
    async fn renew_silently(&self, _university: &LmsUniversity) -> Result<(), LmsError> {
        (self.0.renew)()
    }
}

struct FakeFactory(FakeProviderBehavior);

struct FakeProvider(FakeProviderBehavior);

impl EmailProviderFactory for FakeFactory {
    fn create(&self, _account: &Account) -> Result<Box<dyn EmailProvider>, ProviderError> {
        (self.0.create)()?;
        Ok(Box::new(FakeProvider(self.0)))
    }
}

#[async_trait]
impl EmailProvider for FakeProvider {
    async fn test_connection(&self) -> Result<(), ProviderError> {
        (self.0.test_connection)()
    }

    async fn fetch_new(
        &self,
        _folder: &str,
        _cursor: Option<&SyncCursor>,
        _since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError> {
        Err(ProviderError::Protocol("not used in tests".into()))
    }
}

pub struct FakeModel(pub fn(&QuestionSet) -> Result<AnswerSet, ModelError>);

#[async_trait]
impl StructuredModel for FakeModel {
    fn descriptor(&self) -> ModelDescriptor {
        ModelDescriptor {
            provider: "fake".into(),
            model: "fake".into(),
        }
    }

    async fn ask(
        &self,
        _state: &serde_json::Value,
        questions: &QuestionSet,
    ) -> Result<AnswerSet, ModelError> {
        (self.0)(questions)
    }
}

struct FakeModels(Result<Arc<dyn StructuredModel>, String>);

impl ModelProvider for FakeModels {
    fn current(&self, _settings: &Settings) -> Result<Arc<dyn StructuredModel>, String> {
        self.0.clone()
    }
}

struct NoopObserver;

impl RunObserver for NoopObserver {
    fn progress(&self, _progress: &RunProgress) {}
    fn finished(&self, _run_id: RunId, _status: RunStatus) {}
}

/// Answers every asked question `yes`.
pub fn answer_yes(questions: &QuestionSet) -> Result<AnswerSet, ModelError> {
    Ok(questions
        .keys()
        .map(|id| {
            (
                id.clone(),
                Answer::Noul {
                    p_yes: 1.0,
                    confidence: None,
                },
            )
        })
        .collect())
}

pub fn harness() -> Harness {
    harness_with(
        FakeProviderBehavior::default(),
        Ok(Arc::new(FakeModel(answer_yes))),
    )
}

pub fn harness_with_lms(behavior: FakeLmsBehavior) -> Harness {
    build_harness(
        FakeProviderBehavior::default(),
        Ok(Arc::new(FakeModel(answer_yes))),
        behavior,
    )
}

pub fn harness_with(
    provider: FakeProviderBehavior,
    model: Result<Arc<dyn StructuredModel>, String>,
) -> Harness {
    build_harness(provider, model, FakeLmsBehavior::default())
}

fn build_harness(
    provider: FakeProviderBehavior,
    model: Result<Arc<dyn StructuredModel>, String>,
    lms: FakeLmsBehavior,
) -> Harness {
    let secrets = Arc::new(InMemorySecretStore::default());
    let state = AppState::new(
        Arc::new(Database::open_in_memory().expect("in-memory database opens")),
        Collaborators {
            secrets: secrets.clone(),
            factory: Arc::new(FakeFactory(provider)),
            models: Arc::new(FakeModels(model)),
            http: academic_infra::factory::http_client(),
            observer: Arc::new(NoopObserver),
            clock: Arc::new(SystemClock),
            lms_factory: Arc::new(FakeLmsFactory {
                behavior: lms,
                secrets: secrets.clone(),
            }),
            lms_authenticator: Arc::new(FakeLmsAuthenticator(lms)),
        },
    );
    Harness { state, secrets }
}

pub fn imap_draft(name: &str) -> AccountDraft {
    AccountDraft {
        name: name.into(),
        email_address: "me@uv.es".into(),
        provider: ProviderConfig::Imap {
            host: "imap.uv.es".into(),
            port: 993,
            username: "me".into(),
        },
        folders: vec!["INBOX".into()],
        default_university_id: None,
        initial_sync_days: 14,
        enabled: true,
    }
}

pub fn graph_draft(name: &str) -> AccountDraft {
    AccountDraft {
        name: name.into(),
        email_address: "me@uah.es".into(),
        provider: ProviderConfig::MicrosoftGraph {
            tenant: "organizations".into(),
        },
        folders: vec!["inbox".into()],
        default_university_id: None,
        initial_sync_days: 14,
        enabled: true,
    }
}

/// Saves one unseen analyzed result (category `task`) for `account_id` and returns
/// its id.
pub fn seed_result(state: &AppState, account_id: i64, subject: &str) -> i64 {
    let now = Utc::now();
    let run_id = state.runs.create(RunTrigger::Manual, now).unwrap();
    let message = ProcessedMessage {
        account_id,
        folder: "INBOX".into(),
        provider_message_id: subject.into(),
        message_id_header: None,
        content_hash: format!("hash-{subject}"),
        received_at: now,
        outcome: MessageOutcome::Analyzed,
        reason: None,
        run_id,
        processed_at: now,
    };
    let result = NewResult {
        account_id,
        run_id,
        detected_at: now,
        received_at: now,
        subject: subject.into(),
        sender_name: None,
        sender_email: "prof@uv.es".into(),
        source_url: None,
        analysis: AIAnalysis {
            university_id: None,
            course_id: None,
            category: Category::Task,
            priority: Priority::High,
            action_required: true,
            possible_change: false,
            dates: vec![],
            confidence: None,
            model: ModelDescriptor {
                provider: "fake".into(),
                model: "fake".into(),
            },
        },
    };
    state.results.save_analyzed(&message, &result).unwrap()
}
