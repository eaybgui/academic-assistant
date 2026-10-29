//! In-memory fakes for the ports `core::lms::LmsSyncRunner` depends on, used by
//! `tests/lms_sync.rs`. All data is invented.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use academic_core::domain::{
    LmsCourse, LmsIdentity, LmsItem, LmsPlatform, LmsUniversity, LoginState, RemoteCourse,
    RemoteCourseRef, RemoteItem, SyncOutcome, SyncWindow, TrackedItem, TrackerQuery, UniversityId,
};
use academic_core::lms::SyncChanges;
use academic_core::ports::{
    CourseRepository, LmsAuthenticator, LmsClient, LmsClientFactory, LmsError, LmsItemRepository,
    LmsSyncObserver, LmsSyncProgress, LmsSyncResult, RepoError, RepoResult, UniversityRepository,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// A cloneable stand-in for [`LmsError`] (which is not `Clone`) so scripts can be
/// stored and replayed.
#[derive(Clone)]
pub enum ScriptedError {
    Auth(&'static str),
    Connection(&'static str),
}

impl ScriptedError {
    fn to_lms_error(&self) -> LmsError {
        match self {
            ScriptedError::Auth(message) => LmsError::Auth((*message).to_string()),
            ScriptedError::Connection(message) => LmsError::Connection((*message).to_string()),
        }
    }
}

/// What the fake platform answers for one university.
#[derive(Clone)]
pub struct Script {
    pub verify: Result<(), ScriptedError>,
    pub courses: Result<Vec<RemoteCourse>, ScriptedError>,
    pub items: Result<Vec<RemoteItem>, ScriptedError>,
}

impl Default for Script {
    fn default() -> Self {
        Self {
            verify: Ok(()),
            courses: Ok(vec![]),
            items: Ok(vec![]),
        }
    }
}

/// One recorded `fetch_items` call.
#[derive(Debug, Clone, PartialEq)]
pub struct FetchCall {
    pub university_id: UniversityId,
    pub courses: Vec<RemoteCourseRef>,
    pub window: SyncWindow,
}

/// Client factory (and the clients it builds) driven by a per-university [`Script`].
#[derive(Default)]
pub struct FakeLmsPlatform {
    scripts: Mutex<HashMap<UniversityId, Script>>,
    fetch_calls: Arc<Mutex<Vec<FetchCall>>>,
}

impl FakeLmsPlatform {
    pub fn script(&self, university_id: UniversityId, script: Script) {
        self.scripts.lock().unwrap().insert(university_id, script);
    }

    pub fn fetch_calls(&self) -> Vec<FetchCall> {
        self.fetch_calls.lock().unwrap().clone()
    }
}

struct FakeClient {
    university_id: UniversityId,
    script: Script,
    fetch_calls: Arc<Mutex<Vec<FetchCall>>>,
}

impl LmsClientFactory for FakeLmsPlatform {
    fn create(&self, university: &LmsUniversity) -> Result<Box<dyn LmsClient>, LmsError> {
        let script = self
            .scripts
            .lock()
            .unwrap()
            .get(&university.id)
            .cloned()
            .unwrap_or_default();
        Ok(Box::new(FakeClient {
            university_id: university.id,
            script,
            fetch_calls: self.fetch_calls.clone(),
        }))
    }

    fn create_with_credential(
        &self,
        _university: &LmsUniversity,
        _credential: &str,
    ) -> Result<Box<dyn LmsClient>, LmsError> {
        unreachable!("sync never adopts a credential")
    }
}

#[async_trait]
impl LmsClient for FakeClient {
    async fn verify(&self) -> Result<LmsIdentity, LmsError> {
        self.script
            .verify
            .as_ref()
            .map(|()| LmsIdentity {
                user_id: "_1_1".into(),
                display_name: "Ana Pérez".into(),
            })
            .map_err(ScriptedError::to_lms_error)
    }

    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError> {
        self.script
            .courses
            .clone()
            .map_err(|error| error.to_lms_error())
    }

    async fn fetch_items(
        &self,
        courses: &[RemoteCourseRef],
        window: &SyncWindow,
    ) -> Result<Vec<RemoteItem>, LmsError> {
        self.fetch_calls.lock().unwrap().push(FetchCall {
            university_id: self.university_id,
            courses: courses.to_vec(),
            window: *window,
        });
        self.script
            .items
            .clone()
            .map_err(|error| error.to_lms_error())
    }
}

/// Silent renewal always answers with the scripted result.
pub struct FakeAuthenticator(pub Result<(), ScriptedError>);

#[async_trait]
impl LmsAuthenticator for FakeAuthenticator {
    async fn renew_silently(&self, _university: &LmsUniversity) -> Result<(), LmsError> {
        self.0.clone().map_err(|error| error.to_lms_error())
    }
}

/// One recorded `record_sync` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedSync {
    pub university_id: UniversityId,
    pub at: DateTime<Utc>,
    pub outcome: SyncOutcome,
    pub error: Option<String>,
}

/// University repository over a vector; records `set_login` and `record_sync`.
pub struct FakeUniversities {
    universities: Mutex<Vec<LmsUniversity>>,
    logins: Mutex<Vec<(UniversityId, LoginState, Option<String>)>>,
    syncs: Mutex<Vec<RecordedSync>>,
    fail_record_sync: Mutex<bool>,
}

impl FakeUniversities {
    pub fn new(universities: Vec<LmsUniversity>) -> Self {
        Self {
            universities: Mutex::new(universities),
            logins: Mutex::default(),
            syncs: Mutex::default(),
            fail_record_sync: Mutex::default(),
        }
    }

    pub fn logins(&self) -> Vec<(UniversityId, LoginState, Option<String>)> {
        self.logins.lock().unwrap().clone()
    }

    pub fn syncs(&self) -> Vec<RecordedSync> {
        self.syncs.lock().unwrap().clone()
    }

    pub fn fail_record_sync(&self) {
        *self.fail_record_sync.lock().unwrap() = true;
    }
}

impl UniversityRepository for FakeUniversities {
    fn list_universities(&self) -> RepoResult<Vec<LmsUniversity>> {
        Ok(self.universities.lock().unwrap().clone())
    }

    fn get_university(&self, id: UniversityId) -> RepoResult<Option<LmsUniversity>> {
        Ok(self
            .universities
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.id == id)
            .cloned())
    }

    fn insert_university(
        &self,
        _name: &str,
        _platform: LmsPlatform,
        _base_url: &str,
        _now: DateTime<Utc>,
    ) -> RepoResult<LmsUniversity> {
        unreachable!()
    }

    fn rename_university(&self, _id: UniversityId, _name: &str) -> RepoResult<LmsUniversity> {
        unreachable!()
    }

    fn delete_university(&self, _id: UniversityId) -> RepoResult<()> {
        unreachable!()
    }

    fn set_login(
        &self,
        id: UniversityId,
        state: LoginState,
        account_label: Option<&str>,
    ) -> RepoResult<LmsUniversity> {
        self.logins
            .lock()
            .unwrap()
            .push((id, state, account_label.map(str::to_string)));
        let mut universities = self.universities.lock().unwrap();
        let university = universities
            .iter_mut()
            .find(|u| u.id == id)
            .ok_or(RepoError::NotFound)?;
        university.login_state = state;
        university.account_label = account_label.map(str::to_string);
        Ok(university.clone())
    }

    fn record_sync(
        &self,
        id: UniversityId,
        at: DateTime<Utc>,
        outcome: SyncOutcome,
        error: Option<&str>,
    ) -> RepoResult<()> {
        if *self.fail_record_sync.lock().unwrap() {
            return Err(RepoError::Storage("disk full".into()));
        }
        self.syncs.lock().unwrap().push(RecordedSync {
            university_id: id,
            at,
            outcome,
            error: error.map(str::to_string),
        });
        Ok(())
    }
}

/// One recorded `upsert_remote_courses` call.
pub type UpsertCall = (UniversityId, Vec<RemoteCourse>, DateTime<Utc>);

/// Course repository over a vector; records `upsert_remote_courses` calls. The
/// stored courses are preset by the test (upserting does not alter them).
pub struct FakeCourses {
    courses: Mutex<Vec<LmsCourse>>,
    upserts: Mutex<Vec<UpsertCall>>,
}

impl FakeCourses {
    pub fn new(courses: Vec<LmsCourse>) -> Self {
        Self {
            courses: Mutex::new(courses),
            upserts: Mutex::default(),
        }
    }

    pub fn upserts(&self) -> Vec<UpsertCall> {
        self.upserts.lock().unwrap().clone()
    }
}

impl CourseRepository for FakeCourses {
    fn list_courses(&self, university_id: UniversityId) -> RepoResult<Vec<LmsCourse>> {
        Ok(self
            .courses
            .lock()
            .unwrap()
            .iter()
            .filter(|c| c.university_id == university_id)
            .cloned()
            .collect())
    }

    fn upsert_remote_courses(
        &self,
        university_id: UniversityId,
        courses: &[RemoteCourse],
        now: DateTime<Utc>,
    ) -> RepoResult<()> {
        self.upserts
            .lock()
            .unwrap()
            .push((university_id, courses.to_vec(), now));
        Ok(())
    }

    fn set_tracked_courses(
        &self,
        _university_id: UniversityId,
        _course_ids: &[i64],
    ) -> RepoResult<Vec<LmsCourse>> {
        unreachable!()
    }
}

/// One recorded `apply_sync` call.
pub type ApplyCall = (UniversityId, SyncChanges, DateTime<Utc>);

/// Item repository over a vector; records `apply_sync` calls and can be told to fail.
pub struct FakeItems {
    items: Mutex<Vec<LmsItem>>,
    applied: Mutex<Vec<ApplyCall>>,
    fail_apply: Mutex<bool>,
}

impl FakeItems {
    pub fn new(items: Vec<LmsItem>) -> Self {
        Self {
            items: Mutex::new(items),
            applied: Mutex::default(),
            fail_apply: Mutex::default(),
        }
    }

    pub fn applied(&self) -> Vec<ApplyCall> {
        self.applied.lock().unwrap().clone()
    }

    pub fn fail_apply(&self) {
        *self.fail_apply.lock().unwrap() = true;
    }
}

impl LmsItemRepository for FakeItems {
    fn list_items_for_university(&self, university_id: UniversityId) -> RepoResult<Vec<LmsItem>> {
        Ok(self
            .items
            .lock()
            .unwrap()
            .iter()
            .filter(|i| i.university_id == university_id)
            .cloned()
            .collect())
    }

    fn apply_sync(
        &self,
        university_id: UniversityId,
        changes: &SyncChanges,
        now: DateTime<Utc>,
    ) -> RepoResult<()> {
        if *self.fail_apply.lock().unwrap() {
            return Err(RepoError::Storage("disk full".into()));
        }
        self.applied
            .lock()
            .unwrap()
            .push((university_id, changes.clone(), now));
        Ok(())
    }

    fn query_items(&self, _query: &TrackerQuery) -> RepoResult<Vec<TrackedItem>> {
        unreachable!()
    }

    fn get_item(&self, _id: i64) -> RepoResult<Option<TrackedItem>> {
        unreachable!()
    }

    fn set_item_done(&self, _id: i64, _done: bool) -> RepoResult<TrackedItem> {
        unreachable!()
    }

    fn set_item_hidden(&self, _id: i64, _hidden: bool) -> RepoResult<TrackedItem> {
        unreachable!()
    }

    fn set_item_note(&self, _id: i64, _note: Option<&str>) -> RepoResult<TrackedItem> {
        unreachable!()
    }

    fn dismiss_item_change(&self, _id: i64) -> RepoResult<TrackedItem> {
        unreachable!()
    }

    fn dismiss_all_changes(&self, _university_id: Option<UniversityId>) -> RepoResult<u64> {
        unreachable!()
    }
}

type FinishedProbe = Box<dyn Fn() -> bool + Send + Sync>;

/// Records every progress/finished call. An optional probe is evaluated at the
/// moment `finished` is called and its answer recorded.
#[derive(Default)]
pub struct FakeSyncObserver {
    progress: Mutex<Vec<LmsSyncProgress>>,
    finished: Mutex<Vec<Vec<LmsSyncResult>>>,
    probe: Mutex<Option<FinishedProbe>>,
    probe_answers: Mutex<Vec<bool>>,
}

impl FakeSyncObserver {
    pub fn progress_calls(&self) -> Vec<LmsSyncProgress> {
        self.progress.lock().unwrap().clone()
    }

    pub fn finished_calls(&self) -> Vec<Vec<LmsSyncResult>> {
        self.finished.lock().unwrap().clone()
    }

    pub fn set_finished_probe(&self, probe: FinishedProbe) {
        *self.probe.lock().unwrap() = Some(probe);
    }

    pub fn probe_answers(&self) -> Vec<bool> {
        self.probe_answers.lock().unwrap().clone()
    }
}

impl LmsSyncObserver for FakeSyncObserver {
    fn progress(&self, progress: &LmsSyncProgress) {
        self.progress.lock().unwrap().push(progress.clone());
    }

    fn finished(&self, results: &[LmsSyncResult]) {
        if let Some(probe) = self.probe.lock().unwrap().as_ref() {
            self.probe_answers.lock().unwrap().push(probe());
        }
        self.finished.lock().unwrap().push(results.to_vec());
    }
}
