//! `LmsSyncRunner`: refreshes courses and items of every signed-in university
//! (spec §6, §7). Universities are synced one after another and fail
//! independently; nothing is written for a university whose sync fails before
//! the save step.

use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::{diff_items, ensure_session, SessionError, SyncChanges};
use crate::domain::{
    LmsCourse, LmsUniversity, LoginState, RemoteCourseRef, SyncOutcome, SyncWindow, UniversityId,
};
use crate::pipeline::lock::{RunLock, RunLockGuard};
use crate::ports::{
    Clock, CourseRepository, LmsAuthenticator, LmsClientFactory, LmsError, LmsItemRepository,
    LmsSyncObserver, LmsSyncProgress, LmsSyncResult, LmsSyncStage, RepoError, UniversityRepository,
};

/// Shown when a repository operation fails; repository details are not user-facing.
const STORAGE_ERROR_MESSAGE: &str = "Something went wrong while saving data";

#[derive(Debug, thiserror::Error)]
pub enum LmsSyncError {
    #[error("A sync is already running")]
    AlreadyRunning,
    #[error("Sign in to a university first")]
    NothingToSync,
    #[error("Storage error: {0}")]
    Storage(String),
}

/// Every repository port the sync depends on, bundled for [`LmsSyncRunner::new`].
pub struct LmsRepositories {
    pub universities: Arc<dyn UniversityRepository>,
    pub courses: Arc<dyn CourseRepository>,
    pub items: Arc<dyn LmsItemRepository>,
}

/// Syncs universities one after another. Holds a lock so at most one sync is
/// ever in progress.
pub struct LmsSyncRunner {
    repos: LmsRepositories,
    factory: Arc<dyn LmsClientFactory>,
    authenticator: Arc<dyn LmsAuthenticator>,
    observer: Arc<dyn LmsSyncObserver>,
    clock: Arc<dyn Clock>,
    lock: RunLock,
}

/// The universities selected by [`LmsSyncRunner::prepare`], with the lock held
/// until [`LmsSyncRunner::execute`] finishes (or the value is dropped).
pub struct PreparedSync {
    universities: Vec<LmsUniversity>,
    _lock: RunLockGuard,
}

/// Why a university's sync stopped early.
enum Failure {
    LoginRequired(String),
    Failed(String),
}

impl Failure {
    fn storage() -> Self {
        Failure::Failed(STORAGE_ERROR_MESSAGE.to_string())
    }
}

impl From<RepoError> for Failure {
    fn from(_: RepoError) -> Self {
        Failure::storage()
    }
}

impl LmsSyncRunner {
    pub fn new(
        repos: LmsRepositories,
        factory: Arc<dyn LmsClientFactory>,
        authenticator: Arc<dyn LmsAuthenticator>,
        observer: Arc<dyn LmsSyncObserver>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            repos,
            factory,
            authenticator,
            observer,
            clock,
            lock: RunLock::new(),
        }
    }

    /// Selects the target universities (the given one, or every one; those never
    /// signed in are excluded) and takes the lock. A missing id or an empty
    /// target set is `NothingToSync`.
    pub fn prepare(
        &self,
        university_id: Option<UniversityId>,
    ) -> Result<PreparedSync, LmsSyncError> {
        let guard = self
            .lock
            .try_acquire()
            .ok_or(LmsSyncError::AlreadyRunning)?;
        let candidates = self.candidates(university_id)?;
        let universities: Vec<LmsUniversity> = candidates
            .into_iter()
            .filter(|university| university.login_state != LoginState::Never)
            .collect();
        if universities.is_empty() {
            return Err(LmsSyncError::NothingToSync);
        }
        Ok(PreparedSync {
            universities,
            _lock: guard,
        })
    }

    /// Syncs every prepared university, then releases the lock and notifies the
    /// observer exactly once. Never panics on a platform or storage error.
    pub async fn execute(&self, prepared: PreparedSync) -> Vec<LmsSyncResult> {
        let PreparedSync {
            universities,
            _lock: lock_guard,
        } = prepared;

        let mut results = Vec::with_capacity(universities.len());
        for university in &universities {
            results.push(self.sync_university(university).await);
        }
        drop(lock_guard);
        self.observer.finished(&results);
        results
    }

    pub fn is_running(&self) -> bool {
        self.lock.is_busy()
    }

    fn candidates(
        &self,
        university_id: Option<UniversityId>,
    ) -> Result<Vec<LmsUniversity>, LmsSyncError> {
        let storage = |error: RepoError| LmsSyncError::Storage(error.to_string());
        match university_id {
            Some(id) => Ok(self
                .repos
                .universities
                .get_university(id)
                .map_err(storage)?
                .into_iter()
                .collect()),
            None => self.repos.universities.list_universities().map_err(storage),
        }
    }

    /// Runs one university's sync and records its outcome.
    async fn sync_university(&self, university: &LmsUniversity) -> LmsSyncResult {
        let now = self.clock.now();
        let attempt = self.fetch_and_save(university, now).await;
        self.finish_university(university, attempt, now)
    }

    async fn fetch_and_save(
        &self,
        university: &LmsUniversity,
        now: DateTime<Utc>,
    ) -> Result<SyncChanges, Failure> {
        self.report(university, LmsSyncStage::Courses);
        let university = self.checked_session(university).await?;
        let client = self
            .factory
            .create(&university)
            .map_err(|error| self.lms_failure(&university, error))?;

        let remote_courses = client
            .list_courses()
            .await
            .map_err(|error| self.lms_failure(&university, error))?;
        self.repos
            .courses
            .upsert_remote_courses(university.id, &remote_courses, now)?;

        let tracked: Vec<LmsCourse> = self
            .repos
            .courses
            .list_courses(university.id)?
            .into_iter()
            .filter(|course| course.tracked && course.available)
            .collect();
        let remote_items = if tracked.is_empty() {
            Vec::new()
        } else {
            self.report(&university, LmsSyncStage::Items);
            let refs: Vec<RemoteCourseRef> = tracked
                .iter()
                .map(|course| RemoteCourseRef {
                    external_id: course.external_id.clone(),
                })
                .collect();
            client
                .fetch_items(&refs, &SyncWindow::around(now))
                .await
                .map_err(|error| self.lms_failure(&university, error))?
        };

        self.report(&university, LmsSyncStage::Saving);
        let stored = self.repos.items.list_items_for_university(university.id)?;
        let changes = diff_items(&tracked, &stored, &remote_items);
        self.repos.items.apply_sync(university.id, &changes, now)?;
        Ok(changes)
    }

    /// Verifies the stored credential (renewing it silently when possible).
    async fn checked_session(&self, university: &LmsUniversity) -> Result<LmsUniversity, Failure> {
        ensure_session(
            university,
            self.factory.as_ref(),
            self.authenticator.as_ref(),
            self.repos.universities.as_ref(),
        )
        .await
        .map_err(|error| match error {
            SessionError::LoginRequired(_) => Failure::LoginRequired(error.to_string()),
            SessionError::Lms(error) => self.lms_failure(university, error),
            SessionError::Storage(_) => Failure::storage(),
            SessionError::Secret(_) => Failure::Failed(error.to_string()),
        })
    }

    /// Maps a platform error to a failure. A rejected credential also marks the
    /// university's login as expired (keeping its account label).
    fn lms_failure(&self, university: &LmsUniversity, error: LmsError) -> Failure {
        match error {
            LmsError::Auth(_) => {
                match self.repos.universities.set_login(
                    university.id,
                    LoginState::Expired,
                    university.account_label.as_deref(),
                ) {
                    Ok(_) => Failure::LoginRequired(
                        SessionError::LoginRequired(university.name.clone()).to_string(),
                    ),
                    Err(_) => Failure::storage(),
                }
            }
            other => Failure::Failed(other.to_string()),
        }
    }

    /// Records the university's outcome, reports the final stage and builds its
    /// result. If recording fails the university is reported as failed.
    fn finish_university(
        &self,
        university: &LmsUniversity,
        attempt: Result<SyncChanges, Failure>,
        now: DateTime<Utc>,
    ) -> LmsSyncResult {
        let (outcome, error, changes) = match attempt {
            Ok(changes) => (SyncOutcome::Success, None, changes),
            Err(Failure::LoginRequired(message)) => (
                SyncOutcome::LoginRequired,
                Some(message),
                SyncChanges::default(),
            ),
            Err(Failure::Failed(message)) => {
                (SyncOutcome::Failed, Some(message), SyncChanges::default())
            }
        };

        let recorded =
            self.repos
                .universities
                .record_sync(university.id, now, outcome, error.as_deref());
        let (outcome, error) = match recorded {
            Ok(()) => (outcome, error),
            Err(_) => (SyncOutcome::Failed, Some(STORAGE_ERROR_MESSAGE.to_string())),
        };

        self.report(
            university,
            match outcome {
                SyncOutcome::Success => LmsSyncStage::Done,
                SyncOutcome::Failed => LmsSyncStage::Failed,
                SyncOutcome::LoginRequired => LmsSyncStage::LoginRequired,
            },
        );
        LmsSyncResult {
            university_id: university.id,
            university_name: university.name.clone(),
            outcome,
            new_items: changes.new_items,
            due_changed: changes.due_changed,
            removed: changes.removed,
            error,
        }
    }

    fn report(&self, university: &LmsUniversity, stage: LmsSyncStage) {
        self.observer.progress(&LmsSyncProgress {
            university_id: university.id,
            university_name: university.name.clone(),
            stage,
        });
    }
}
