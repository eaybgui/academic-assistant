use chrono::{DateTime, NaiveDate, Utc};

use crate::domain::{
    Account, AccountDraft, AccountId, Catalog, Counters, DashboardCounts, LmsCourse, LmsItem,
    LmsPlatform, LmsUniversity, LoginState, NewResult, Page, ProcessedMessage, RemoteCourse,
    ResultQuery, RunAccountResult, RunId, RunRecord, RunStatus, RunTrigger, Settings, StoredResult,
    SyncCursor, SyncOutcome, SyncState, TrackedItem, TrackerQuery, UniversityId,
};
use crate::lms::SyncChanges;

/// Error returned by a repository operation.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("Not found")]
    NotFound,
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Storage error: {0}")]
    Storage(String),
}

pub type RepoResult<T> = Result<T, RepoError>;

/// Persistence for configured accounts.
pub trait AccountRepository: Send + Sync {
    fn list(&self) -> RepoResult<Vec<Account>>;
    fn get(&self, id: AccountId) -> RepoResult<Option<Account>>;
    fn insert(&self, draft: &AccountDraft, now: DateTime<Utc>) -> RepoResult<Account>;
    fn update(
        &self,
        id: AccountId,
        draft: &AccountDraft,
        now: DateTime<Utc>,
    ) -> RepoResult<Account>;
    fn delete(&self, id: AccountId) -> RepoResult<()>;
}

/// Read-only projection of universities and tracked courses, used by the email
/// analyzer as its course catalog (spec §11).
pub trait CatalogRepository: Send + Sync {
    fn catalog(&self) -> RepoResult<Catalog>;
}

/// Persistence for per-account, per-folder sync cursors.
pub trait SyncStateRepository: Send + Sync {
    fn get(&self, account_id: AccountId, folder: &str) -> RepoResult<Option<SyncState>>;
    fn list_for_account(&self, account_id: AccountId) -> RepoResult<Vec<SyncState>>;
    fn record_success(
        &self,
        account_id: AccountId,
        folder: &str,
        cursor: &SyncCursor,
        at: DateTime<Utc>,
    ) -> RepoResult<()>;
    fn record_failure(
        &self,
        account_id: AccountId,
        folder: &str,
        error: &str,
        at: DateTime<Utc>,
    ) -> RepoResult<()>;
}

/// Persistence for the dedup/audit trail of every email the pipeline has looked at.
pub trait ProcessedMessageRepository: Send + Sync {
    fn exists(
        &self,
        account_id: AccountId,
        folder: &str,
        provider_message_id: &str,
    ) -> RepoResult<bool>;
    fn is_duplicate(
        &self,
        account_id: AccountId,
        message_id_header: Option<&str>,
        content_hash: &str,
    ) -> RepoResult<bool>;
    fn insert(&self, message: &ProcessedMessage) -> RepoResult<i64>;
}

/// Persistence and querying for analyzed results shown in the UI.
pub trait ResultRepository: Send + Sync {
    /// Inserts the processed message (outcome Analyzed), the result and its dates in one transaction.
    fn save_analyzed(&self, message: &ProcessedMessage, result: &NewResult) -> RepoResult<i64>;
    fn query(&self, query: &ResultQuery, today: NaiveDate) -> RepoResult<Page<StoredResult>>;
    fn get(&self, id: i64) -> RepoResult<Option<StoredResult>>;
    fn set_seen(&self, id: i64, seen: bool, at: DateTime<Utc>) -> RepoResult<StoredResult>;
    fn mark_all_seen(&self, at: DateTime<Utc>) -> RepoResult<u64>;
    fn unseen_counts(&self) -> RepoResult<DashboardCounts>;
}

/// Persistence for check run history.
pub trait RunRepository: Send + Sync {
    fn create(&self, trigger: RunTrigger, started_at: DateTime<Utc>) -> RepoResult<RunId>;
    fn record_account(&self, run_id: RunId, result: &RunAccountResult) -> RepoResult<()>;
    fn finish(
        &self,
        run_id: RunId,
        status: RunStatus,
        finished_at: DateTime<Utc>,
        accounts_processed: u32,
        counters: &Counters,
        error_summary: Option<&str>,
    ) -> RepoResult<()>;
    fn get(&self, run_id: RunId) -> RepoResult<Option<RunRecord>>;
    fn latest(&self) -> RepoResult<Option<RunRecord>>;
    fn history(&self, limit: u32) -> RepoResult<Vec<RunRecord>>;
    /// Sets every `running` run to `failed` with error_summary "Interrupted"; returns rows changed.
    fn mark_interrupted(&self, at: DateTime<Utc>) -> RepoResult<u64>;
}

/// Persistence for user settings.
pub trait SettingsRepository: Send + Sync {
    /// Missing keys fall back to `Settings::default()` values.
    fn load(&self) -> RepoResult<Settings>;
    fn save(&self, settings: &Settings) -> RepoResult<()>;
}

/// Persistence for LMS universities.
pub trait UniversityRepository: Send + Sync {
    fn list_universities(&self) -> RepoResult<Vec<LmsUniversity>>;
    fn get_university(&self, id: UniversityId) -> RepoResult<Option<LmsUniversity>>;
    /// Duplicate name or base URL → `RepoError::Conflict`.
    fn insert_university(
        &self,
        name: &str,
        platform: LmsPlatform,
        base_url: &str,
        now: DateTime<Utc>,
    ) -> RepoResult<LmsUniversity>;
    fn rename_university(&self, id: UniversityId, name: &str) -> RepoResult<LmsUniversity>;
    /// Cascades to its courses (and, from M3, items). Missing → `NotFound`.
    fn delete_university(&self, id: UniversityId) -> RepoResult<()>;
    fn set_login(
        &self,
        id: UniversityId,
        state: LoginState,
        account_label: Option<&str>,
    ) -> RepoResult<LmsUniversity>;
    fn record_sync(
        &self,
        id: UniversityId,
        at: DateTime<Utc>,
        outcome: SyncOutcome,
        error: Option<&str>,
    ) -> RepoResult<()>;
}

/// Persistence for courses discovered on LMS platforms.
pub trait CourseRepository: Send + Sync {
    /// Ordered by name.
    fn list_courses(&self, university_id: UniversityId) -> RepoResult<Vec<LmsCourse>>;
    /// In one transaction: inserts unknown courses (untracked, unreviewed, available),
    /// updates name/short name/url of known ones, marks every returned course
    /// available and every other course of the university unavailable.
    /// Missing university → `NotFound`.
    fn upsert_remote_courses(
        &self,
        university_id: UniversityId,
        courses: &[RemoteCourse],
        now: DateTime<Utc>,
    ) -> RepoResult<()>;
    /// Exactly `course_ids` become tracked; every course of the university becomes
    /// reviewed. An id that is not a course of this university → `NotFound` and no
    /// change. Returns the university's courses.
    fn set_tracked_courses(
        &self,
        university_id: UniversityId,
        course_ids: &[i64],
    ) -> RepoResult<Vec<LmsCourse>>;
}

/// Persistence for LMS items (spec §7, §8).
pub trait LmsItemRepository: Send + Sync {
    fn list_items_for_university(&self, university_id: UniversityId) -> RepoResult<Vec<LmsItem>>;
    /// One transaction: inserts (first_seen_at = updated_at = now), updates (platform fields,
    /// course, change; removed_at cleared; updated_at = now), removals (removed_at = now),
    /// and `items_synced_at = now` for `synced_course_ids`. Personal fields never change.
    fn apply_sync(
        &self,
        university_id: UniversityId,
        changes: &SyncChanges,
        now: DateTime<Utc>,
    ) -> RepoResult<()>;
    /// Items of tracked, available courses, ordered by due date (undated last), then title.
    fn query_items(&self, query: &TrackerQuery) -> RepoResult<Vec<TrackedItem>>;
    fn get_item(&self, id: i64) -> RepoResult<Option<TrackedItem>>;
    fn set_item_done(&self, id: i64, done: bool) -> RepoResult<TrackedItem>;
    fn set_item_hidden(&self, id: i64, hidden: bool) -> RepoResult<TrackedItem>;
    fn set_item_note(&self, id: i64, note: Option<&str>) -> RepoResult<TrackedItem>;
    fn dismiss_item_change(&self, id: i64) -> RepoResult<TrackedItem>;
    /// Clears every badge (of one university, or all). Returns rows changed.
    fn dismiss_all_changes(&self, university_id: Option<UniversityId>) -> RepoResult<u64>;
}
