use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use academic_core::domain::{Account, AccountId, FetchBatch, SyncCursor};
use academic_core::ports::{EmailProvider, EmailProviderFactory, ProviderError};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// A [`ProviderError`]-shaped value that can be cloned and stored ahead of time
/// (unlike [`ProviderError`] itself), then turned into one at call time.
#[derive(Clone)]
pub enum FakeProviderError {
    Auth(String),
    Connection(String),
    Protocol(String),
}

impl FakeProviderError {
    fn into_provider_error(self) -> ProviderError {
        match self {
            FakeProviderError::Auth(m) => ProviderError::Auth(m),
            FakeProviderError::Connection(m) => ProviderError::Connection(m),
            FakeProviderError::Protocol(m) => ProviderError::Protocol(m),
        }
    }
}

/// One recorded `fetch_new` call, for assertions on cursor/since plumbing.
#[derive(Debug, Clone)]
pub struct FetchCall {
    pub folder: String,
    pub cursor: Option<SyncCursor>,
    pub since: DateTime<Utc>,
}

/// A queue of canned `fetch_new` responses for one account's folder(s). Each call
/// pops the next queued response; an empty queue yields an empty batch (a
/// reasonable default: "nothing new"), so tests only need to queue the responses
/// they care about.
struct FakeProvider {
    responses: Mutex<VecDeque<Result<FetchBatch, FakeProviderError>>>,
    calls: Mutex<Vec<FetchCall>>,
}

#[async_trait]
impl EmailProvider for FakeProvider {
    async fn test_connection(&self) -> Result<(), ProviderError> {
        Ok(())
    }

    async fn fetch_new(
        &self,
        folder: &str,
        cursor: Option<&SyncCursor>,
        since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError> {
        self.calls.lock().unwrap().push(FetchCall {
            folder: folder.to_string(),
            cursor: cursor.cloned(),
            since,
        });
        match self.responses.lock().unwrap().pop_front() {
            Some(Ok(batch)) => Ok(batch),
            Some(Err(error)) => Err(error.into_provider_error()),
            None => Ok(FetchBatch {
                emails: Vec::new(),
                next_cursor: SyncCursor::Imap {
                    uidvalidity: 1,
                    last_uid: 0,
                },
            }),
        }
    }
}

/// Delegates to a shared [`FakeProvider`] so the same recorded `calls` are
/// visible to the test regardless of how many times `EmailProviderFactory::create`
/// is called for the account (the pipeline may create a provider once per
/// account per run).
struct SharedProvider(Arc<FakeProvider>);

#[async_trait]
impl EmailProvider for SharedProvider {
    async fn test_connection(&self) -> Result<(), ProviderError> {
        self.0.test_connection().await
    }

    async fn fetch_new(
        &self,
        folder: &str,
        cursor: Option<&SyncCursor>,
        since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError> {
        self.0.fetch_new(folder, cursor, since).await
    }
}

/// Builds a canned [`EmailProvider`] per account (or fails `create` for accounts
/// registered via [`FakeEmailProviderFactory::fail_creation_for`]).
#[derive(Default)]
pub struct FakeEmailProviderFactory {
    providers: Mutex<HashMap<AccountId, Arc<FakeProvider>>>,
    creation_failures: Mutex<HashMap<AccountId, FakeProviderError>>,
}

impl FakeEmailProviderFactory {
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues `responses` (consumed in order, one per `fetch_new` call) for
    /// `account_id`'s provider.
    pub fn queue_responses(
        &self,
        account_id: AccountId,
        responses: Vec<Result<FetchBatch, FakeProviderError>>,
    ) {
        let mut providers = self.providers.lock().unwrap();
        let provider = providers.entry(account_id).or_insert_with(|| {
            Arc::new(FakeProvider {
                responses: Mutex::new(VecDeque::new()),
                calls: Mutex::new(Vec::new()),
            })
        });
        provider.responses.lock().unwrap().extend(responses);
    }

    /// Makes `EmailProviderFactory::create` fail for `account_id` with `error`.
    pub fn fail_creation_for(&self, account_id: AccountId, error: FakeProviderError) {
        self.creation_failures
            .lock()
            .unwrap()
            .insert(account_id, error);
    }

    /// Every `fetch_new` call recorded for `account_id`, in order.
    pub fn fetch_calls(&self, account_id: AccountId) -> Vec<FetchCall> {
        self.providers
            .lock()
            .unwrap()
            .get(&account_id)
            .map(|provider| provider.calls.lock().unwrap().clone())
            .unwrap_or_default()
    }
}

impl EmailProviderFactory for FakeEmailProviderFactory {
    fn create(&self, account: &Account) -> Result<Box<dyn EmailProvider>, ProviderError> {
        if let Some(error) = self.creation_failures.lock().unwrap().get(&account.id) {
            return Err(error.clone().into_provider_error());
        }
        let mut providers = self.providers.lock().unwrap();
        let provider = providers.entry(account.id).or_insert_with(|| {
            Arc::new(FakeProvider {
                responses: Mutex::new(VecDeque::new()),
                calls: Mutex::new(Vec::new()),
            })
        });
        Ok(Box::new(SharedProvider(provider.clone())))
    }
}
