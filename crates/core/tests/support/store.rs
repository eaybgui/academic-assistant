use std::collections::HashMap;
use std::sync::Mutex;

use academic_core::domain::{
    Account, AccountDraft, AccountId, Catalog, Counters, DashboardCounts, NewResult, Page,
    ProcessedMessage, ResultQuery, RunAccountResult, RunAccountView, RunId, RunRecord, RunStatus,
    RunTrigger, Settings, StoredResult, SyncCursor, SyncState, SyncStatus,
};
use academic_core::ports::{
    AccountRepository, CatalogRepository, ProcessedMessageRepository, RepoError, RepoResult,
    ResultRepository, RunRepository, SettingsRepository, SyncStateRepository,
};
use chrono::{DateTime, NaiveDate, Utc};

/// One in-memory "database" implementing every repository port the pipeline
/// depends on, so a single `Arc<TestStore>` can back every field of
/// `pipeline::Repositories` and stay consistent (e.g. a message written via
/// `ResultRepository::save_analyzed` is visible to a later
/// `ProcessedMessageRepository::exists` check, exactly as it would be with a
/// single shared SQLite connection).
pub struct TestStore {
    accounts: Mutex<Vec<Account>>,
    next_account_id: Mutex<AccountId>,

    catalog: Mutex<Catalog>,
    catalog_fails: Mutex<bool>,

    sync_states: Mutex<HashMap<(AccountId, String), SyncState>>,
    sync_successes: Mutex<Vec<(AccountId, String, SyncCursor)>>,
    sync_failures: Mutex<Vec<(AccountId, String, String)>>,

    processed_messages: Mutex<Vec<ProcessedMessage>>,

    results: Mutex<Vec<StoredResult>>,
    next_result_id: Mutex<i64>,

    runs: Mutex<Vec<RunRecord>>,
    next_run_id: Mutex<RunId>,
    run_account_calls: Mutex<Vec<(RunId, RunAccountResult)>>,

    settings: Mutex<Settings>,
}

impl TestStore {
    pub fn new(accounts: Vec<Account>, catalog: Catalog, settings: Settings) -> Self {
        let next_account_id = accounts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
        Self {
            accounts: Mutex::new(accounts),
            next_account_id: Mutex::new(next_account_id),
            catalog: Mutex::new(catalog),
            catalog_fails: Mutex::new(false),
            sync_states: Mutex::new(HashMap::new()),
            sync_successes: Mutex::new(Vec::new()),
            sync_failures: Mutex::new(Vec::new()),
            processed_messages: Mutex::new(Vec::new()),
            results: Mutex::new(Vec::new()),
            next_result_id: Mutex::new(1),
            runs: Mutex::new(Vec::new()),
            next_run_id: Mutex::new(1),
            run_account_calls: Mutex::new(Vec::new()),
            settings: Mutex::new(settings),
        }
    }

    /// Seeds a sync cursor for `account_id`/`folder`, as if a previous run had
    /// already succeeded.
    pub fn seed_sync_state(&self, account_id: AccountId, folder: &str, cursor: SyncCursor) {
        self.sync_states.lock().unwrap().insert(
            (account_id, folder.to_string()),
            SyncState {
                account_id,
                folder: folder.to_string(),
                cursor: Some(cursor),
                last_run_at: None,
                last_success_at: None,
                status: SyncStatus::Ok,
                last_error: None,
            },
        );
    }

    /// Seeds a `processed_messages` row directly, as if a previous run had
    /// already looked at this message.
    pub fn seed_processed_message(&self, message: ProcessedMessage) {
        self.processed_messages.lock().unwrap().push(message);
    }

    /// Makes `CatalogRepository::catalog` fail with a `Storage` error.
    pub fn fail_catalog_load(&self) {
        *self.catalog_fails.lock().unwrap() = true;
    }

    pub fn sync_state(&self, account_id: AccountId, folder: &str) -> Option<SyncState> {
        self.sync_states
            .lock()
            .unwrap()
            .get(&(account_id, folder.to_string()))
            .cloned()
    }

    pub fn sync_successes(&self) -> Vec<(AccountId, String, SyncCursor)> {
        self.sync_successes.lock().unwrap().clone()
    }

    pub fn sync_failures(&self) -> Vec<(AccountId, String, String)> {
        self.sync_failures.lock().unwrap().clone()
    }

    pub fn processed_messages(&self) -> Vec<ProcessedMessage> {
        self.processed_messages.lock().unwrap().clone()
    }

    pub fn results(&self) -> Vec<StoredResult> {
        self.results.lock().unwrap().clone()
    }

    pub fn runs(&self) -> Vec<RunRecord> {
        self.runs.lock().unwrap().clone()
    }

    pub fn run_account_calls(&self) -> Vec<(RunId, RunAccountResult)> {
        self.run_account_calls.lock().unwrap().clone()
    }

    fn account_name(&self, account_id: AccountId) -> String {
        self.accounts
            .lock()
            .unwrap()
            .iter()
            .find(|a| a.id == account_id)
            .map(|a| a.name.clone())
            .unwrap_or_default()
    }
}

impl AccountRepository for TestStore {
    fn list(&self) -> RepoResult<Vec<Account>> {
        Ok(self.accounts.lock().unwrap().clone())
    }

    fn get(&self, id: AccountId) -> RepoResult<Option<Account>> {
        Ok(self
            .accounts
            .lock()
            .unwrap()
            .iter()
            .find(|a| a.id == id)
            .cloned())
    }

    fn insert(&self, draft: &AccountDraft, now: DateTime<Utc>) -> RepoResult<Account> {
        let mut next_id = self.next_account_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        let account = Account {
            id,
            name: draft.name.clone(),
            email_address: draft.email_address.clone(),
            provider: draft.provider.clone(),
            folders: draft.folders.clone(),
            default_university_id: draft.default_university_id,
            initial_sync_days: draft.initial_sync_days,
            enabled: draft.enabled,
            created_at: now,
            updated_at: now,
        };
        self.accounts.lock().unwrap().push(account.clone());
        Ok(account)
    }

    fn update(
        &self,
        id: AccountId,
        draft: &AccountDraft,
        now: DateTime<Utc>,
    ) -> RepoResult<Account> {
        let mut accounts = self.accounts.lock().unwrap();
        let account = accounts
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or(RepoError::NotFound)?;
        account.name = draft.name.clone();
        account.email_address = draft.email_address.clone();
        account.provider = draft.provider.clone();
        account.folders = draft.folders.clone();
        account.default_university_id = draft.default_university_id;
        account.initial_sync_days = draft.initial_sync_days;
        account.enabled = draft.enabled;
        account.updated_at = now;
        Ok(account.clone())
    }

    fn delete(&self, id: AccountId) -> RepoResult<()> {
        self.accounts.lock().unwrap().retain(|a| a.id != id);
        Ok(())
    }
}

impl CatalogRepository for TestStore {
    fn catalog(&self) -> RepoResult<Catalog> {
        if *self.catalog_fails.lock().unwrap() {
            return Err(RepoError::Storage("catalog load failed".to_string()));
        }
        Ok(self.catalog.lock().unwrap().clone())
    }
}

impl SyncStateRepository for TestStore {
    fn get(&self, account_id: AccountId, folder: &str) -> RepoResult<Option<SyncState>> {
        Ok(self.sync_state(account_id, folder))
    }

    fn list_for_account(&self, account_id: AccountId) -> RepoResult<Vec<SyncState>> {
        Ok(self
            .sync_states
            .lock()
            .unwrap()
            .values()
            .filter(|s| s.account_id == account_id)
            .cloned()
            .collect())
    }

    fn record_success(
        &self,
        account_id: AccountId,
        folder: &str,
        cursor: &SyncCursor,
        at: DateTime<Utc>,
    ) -> RepoResult<()> {
        let mut states = self.sync_states.lock().unwrap();
        states.insert(
            (account_id, folder.to_string()),
            SyncState {
                account_id,
                folder: folder.to_string(),
                cursor: Some(cursor.clone()),
                last_run_at: Some(at),
                last_success_at: Some(at),
                status: SyncStatus::Ok,
                last_error: None,
            },
        );
        self.sync_successes
            .lock()
            .unwrap()
            .push((account_id, folder.to_string(), cursor.clone()));
        Ok(())
    }

    fn record_failure(
        &self,
        account_id: AccountId,
        folder: &str,
        error: &str,
        at: DateTime<Utc>,
    ) -> RepoResult<()> {
        let mut states = self.sync_states.lock().unwrap();
        let existing_cursor = states
            .get(&(account_id, folder.to_string()))
            .and_then(|s| s.cursor.clone());
        states.insert(
            (account_id, folder.to_string()),
            SyncState {
                account_id,
                folder: folder.to_string(),
                cursor: existing_cursor,
                last_run_at: Some(at),
                last_success_at: None,
                status: SyncStatus::Error,
                last_error: Some(error.to_string()),
            },
        );
        self.sync_failures.lock().unwrap().push((
            account_id,
            folder.to_string(),
            error.to_string(),
        ));
        Ok(())
    }
}

impl ProcessedMessageRepository for TestStore {
    fn exists(
        &self,
        account_id: AccountId,
        folder: &str,
        provider_message_id: &str,
    ) -> RepoResult<bool> {
        Ok(self.processed_messages.lock().unwrap().iter().any(|m| {
            m.account_id == account_id
                && m.folder == folder
                && m.provider_message_id == provider_message_id
        }))
    }

    fn is_duplicate(
        &self,
        account_id: AccountId,
        message_id_header: Option<&str>,
        content_hash: &str,
    ) -> RepoResult<bool> {
        Ok(self.processed_messages.lock().unwrap().iter().any(|m| {
            m.account_id == account_id
                && (m.content_hash == content_hash
                    || (message_id_header.is_some()
                        && m.message_id_header.as_deref() == message_id_header))
        }))
    }

    fn insert(&self, message: &ProcessedMessage) -> RepoResult<i64> {
        let mut messages = self.processed_messages.lock().unwrap();
        messages.push(message.clone());
        Ok(messages.len() as i64)
    }
}

impl ResultRepository for TestStore {
    fn save_analyzed(&self, message: &ProcessedMessage, result: &NewResult) -> RepoResult<i64> {
        self.processed_messages
            .lock()
            .unwrap()
            .push(message.clone());

        let mut next_id = self.next_result_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;

        let catalog = self.catalog.lock().unwrap();
        let university_name = result
            .analysis
            .university_id
            .and_then(|uid| catalog.universities.iter().find(|u| u.id == uid))
            .map(|u| u.name.clone());
        let course_name = result
            .analysis
            .course_id
            .and_then(|cid| catalog.courses.iter().find(|c| c.id == cid))
            .map(|c| c.name.clone());
        drop(catalog);

        let stored = StoredResult {
            id,
            account_id: result.account_id,
            account_name: self.account_name(result.account_id),
            subject: result.subject.clone(),
            sender_name: result.sender_name.clone(),
            sender_email: result.sender_email.clone(),
            university_name,
            course_name,
            category: result.analysis.category,
            priority: result.analysis.priority,
            action_required: result.analysis.action_required,
            possible_change: result.analysis.possible_change,
            dates: result.analysis.dates.clone(),
            received_at: result.received_at,
            detected_at: result.detected_at,
            source_url: result.source_url.clone(),
            message_id_header: message.message_id_header.clone(),
            seen: false,
            seen_at: None,
        };
        self.results.lock().unwrap().push(stored);
        Ok(id)
    }

    fn query(&self, _query: &ResultQuery, _today: NaiveDate) -> RepoResult<Page<StoredResult>> {
        let items = self.results.lock().unwrap().clone();
        let total = items.len() as u64;
        Ok(Page { items, total })
    }

    fn get(&self, id: i64) -> RepoResult<Option<StoredResult>> {
        Ok(self
            .results
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.id == id)
            .cloned())
    }

    fn set_seen(&self, id: i64, seen: bool, at: DateTime<Utc>) -> RepoResult<StoredResult> {
        let mut results = self.results.lock().unwrap();
        let result = results
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(RepoError::NotFound)?;
        result.seen = seen;
        result.seen_at = if seen { Some(at) } else { None };
        Ok(result.clone())
    }

    fn mark_all_seen(&self, at: DateTime<Utc>) -> RepoResult<u64> {
        let mut results = self.results.lock().unwrap();
        let mut changed = 0u64;
        for result in results.iter_mut() {
            if !result.seen {
                result.seen = true;
                result.seen_at = Some(at);
                changed += 1;
            }
        }
        Ok(changed)
    }

    fn unseen_counts(&self) -> RepoResult<DashboardCounts> {
        Ok(DashboardCounts::default())
    }
}

impl RunRepository for TestStore {
    fn create(&self, trigger: RunTrigger, started_at: DateTime<Utc>) -> RepoResult<RunId> {
        let mut next_id = self.next_run_id.lock().unwrap();
        let id = *next_id;
        *next_id += 1;
        self.runs.lock().unwrap().push(RunRecord {
            id,
            trigger,
            started_at,
            finished_at: None,
            status: RunStatus::Running,
            accounts_processed: 0,
            counters: Counters::default(),
            error_summary: None,
            accounts: Vec::new(),
        });
        Ok(id)
    }

    fn record_account(&self, run_id: RunId, result: &RunAccountResult) -> RepoResult<()> {
        self.run_account_calls
            .lock()
            .unwrap()
            .push((run_id, result.clone()));
        let account_name = self.account_name(result.account_id);
        let mut runs = self.runs.lock().unwrap();
        let run = runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or(RepoError::NotFound)?;
        run.accounts.push(RunAccountView {
            account_name,
            result: result.clone(),
        });
        Ok(())
    }

    fn finish(
        &self,
        run_id: RunId,
        status: RunStatus,
        finished_at: DateTime<Utc>,
        accounts_processed: u32,
        counters: &Counters,
        error_summary: Option<&str>,
    ) -> RepoResult<()> {
        let mut runs = self.runs.lock().unwrap();
        let run = runs
            .iter_mut()
            .find(|r| r.id == run_id)
            .ok_or(RepoError::NotFound)?;
        run.status = status;
        run.finished_at = Some(finished_at);
        run.accounts_processed = accounts_processed;
        run.counters = *counters;
        run.error_summary = error_summary.map(str::to_string);
        Ok(())
    }

    fn get(&self, run_id: RunId) -> RepoResult<Option<RunRecord>> {
        Ok(self
            .runs
            .lock()
            .unwrap()
            .iter()
            .find(|r| r.id == run_id)
            .cloned())
    }

    fn latest(&self) -> RepoResult<Option<RunRecord>> {
        Ok(self
            .runs
            .lock()
            .unwrap()
            .iter()
            .max_by_key(|r| r.id)
            .cloned())
    }

    fn history(&self, limit: u32) -> RepoResult<Vec<RunRecord>> {
        let mut runs = self.runs.lock().unwrap().clone();
        runs.sort_by_key(|r| std::cmp::Reverse(r.id));
        runs.truncate(limit as usize);
        Ok(runs)
    }

    fn mark_interrupted(&self, at: DateTime<Utc>) -> RepoResult<u64> {
        let mut runs = self.runs.lock().unwrap();
        let mut changed = 0u64;
        for run in runs.iter_mut() {
            if run.status == RunStatus::Running {
                run.status = RunStatus::Failed;
                run.finished_at = Some(at);
                run.error_summary = Some("Interrupted".to_string());
                changed += 1;
            }
        }
        Ok(changed)
    }
}

impl SettingsRepository for TestStore {
    fn load(&self) -> RepoResult<Settings> {
        Ok(self.settings.lock().unwrap().clone())
    }

    fn save(&self, settings: &Settings) -> RepoResult<()> {
        *self.settings.lock().unwrap() = settings.clone();
        Ok(())
    }
}
