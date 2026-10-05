//! Integration tests for `core::lms::LmsSyncRunner` against in-memory fakes of
//! every port. All data is invented.

#[path = "support/clock.rs"]
mod clock;
#[path = "support/lms.rs"]
mod lms;

use std::sync::Arc;

use academic_core::domain::{
    ItemKind, LmsCourse, LmsPlatform, LmsUniversity, LoginState, PlatformStatus, RemoteCourse,
    RemoteCourseRef, RemoteItem, SyncOutcome, SyncWindow, UniversityId,
};
use academic_core::lms::{diff_items, LmsRepositories, LmsSyncError, LmsSyncRunner};
use academic_core::ports::{LmsSyncProgress, LmsSyncResult, LmsSyncStage};
use chrono::{DateTime, FixedOffset, TimeZone, Utc};

use clock::FakeClock;
use lms::{
    FakeAuthenticator, FakeCourses, FakeItems, FakeLmsPlatform, FakeSyncObserver, FakeUniversities,
    RecordedSync, Script, ScriptedError,
};

const STORAGE_ERROR: &str = "Something went wrong while saving data";

fn now() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 28, 9, 0, 0).unwrap()
}

fn university(id: UniversityId, name: &str, state: LoginState) -> LmsUniversity {
    LmsUniversity {
        id,
        name: name.into(),
        platform: LmsPlatform::Blackboard,
        base_url: format!("https://campus{id}.example.edu"),
        login_state: state,
        account_label: Some("Label Anterior".into()),
        last_sync_at: None,
        last_sync_outcome: None,
        last_sync_error: None,
        created_at: now(),
    }
}

fn demo(id: UniversityId, state: LoginState) -> LmsUniversity {
    university(id, &format!("Universidad Demo {id}"), state)
}

fn course(id: i64, university_id: UniversityId, external: &str, tracked: bool) -> LmsCourse {
    LmsCourse {
        id,
        university_id,
        external_id: external.into(),
        name: format!("Curso {external}"),
        short_name: None,
        url: None,
        tracked,
        available: true,
        reviewed: true,
        items_synced_at: Some(now()),
    }
}

fn remote_course(external: &str) -> RemoteCourse {
    RemoteCourse {
        external_id: external.into(),
        name: format!("Curso {external}"),
        short_name: None,
        url: None,
    }
}

fn remote_item(course_external: &str, external: &str) -> RemoteItem {
    RemoteItem {
        course_external_id: course_external.into(),
        external_id: external.into(),
        kind: ItemKind::Assignment,
        title: "Ensayo de ejemplo".into(),
        due_at: Some(now() + chrono::Duration::days(7)),
        url: None,
        platform_status: PlatformStatus::NotSubmitted,
        grade: None,
    }
}

struct Harness {
    runner: Arc<LmsSyncRunner>,
    platform: Arc<FakeLmsPlatform>,
    universities: Arc<FakeUniversities>,
    courses: Arc<FakeCourses>,
    items: Arc<FakeItems>,
    observer: Arc<FakeSyncObserver>,
}

fn harness(universities: Vec<LmsUniversity>, courses: Vec<LmsCourse>) -> Harness {
    harness_with_renewal(universities, courses, Err(ScriptedError::Auth("sign in")))
}

fn harness_with_renewal(
    universities: Vec<LmsUniversity>,
    courses: Vec<LmsCourse>,
    renewal: Result<(), ScriptedError>,
) -> Harness {
    let platform = Arc::new(FakeLmsPlatform::default());
    let universities = Arc::new(FakeUniversities::new(universities));
    let courses = Arc::new(FakeCourses::new(courses));
    let items = Arc::new(FakeItems::new(vec![]));
    let observer = Arc::new(FakeSyncObserver::default());
    let runner = Arc::new(LmsSyncRunner::new(
        LmsRepositories {
            universities: universities.clone(),
            courses: courses.clone(),
            items: items.clone(),
        },
        platform.clone(),
        Arc::new(FakeAuthenticator(renewal)),
        observer.clone(),
        Arc::new(FakeClock::new(now(), FixedOffset::east_opt(0).unwrap())),
    ));
    Harness {
        runner,
        platform,
        universities,
        courses,
        items,
        observer,
    }
}

impl Harness {
    async fn sync(&self, university_id: Option<UniversityId>) -> Vec<LmsSyncResult> {
        let prepared = self
            .runner
            .prepare(university_id)
            .expect("prepare succeeds");
        self.runner.execute(prepared).await
    }
}

fn stages(observer: &FakeSyncObserver, university_id: UniversityId) -> Vec<LmsSyncStage> {
    observer
        .progress_calls()
        .iter()
        .filter(|p| p.university_id == university_id)
        .map(|p| p.stage)
        .collect()
}

#[tokio::test]
async fn syncs_courses_and_items_and_records_success() {
    let tracked = course(10, 1, "C-100", true);
    let mut unavailable = course(11, 1, "C-101", true);
    unavailable.available = false;
    let untracked = course(12, 1, "C-102", false);
    let h = harness(
        vec![demo(1, LoginState::Ok)],
        vec![tracked.clone(), unavailable, untracked],
    );
    let remote_items = vec![remote_item("C-100", "assign:1")];
    h.platform.script(
        1,
        Script {
            courses: Ok(vec![remote_course("C-100"), remote_course("C-102")]),
            items: Ok(remote_items.clone()),
            ..Script::default()
        },
    );

    let results = h.sync(None).await;

    assert_eq!(
        h.courses.upserts(),
        vec![(
            1,
            vec![remote_course("C-100"), remote_course("C-102")],
            now()
        )]
    );
    let fetches = h.platform.fetch_calls();
    assert_eq!(fetches.len(), 1);
    assert_eq!(
        fetches[0].courses,
        vec![RemoteCourseRef {
            external_id: "C-100".into()
        }]
    );
    assert_eq!(fetches[0].window, SyncWindow::around(now()));

    let expected_changes = diff_items(&[tracked], &[], &remote_items, &SyncWindow::around(now()));
    assert_eq!(expected_changes.new_items, 1);
    assert_eq!(h.items.applied(), vec![(1, expected_changes, now())]);

    assert_eq!(
        h.universities.syncs(),
        vec![RecordedSync {
            university_id: 1,
            at: now(),
            outcome: SyncOutcome::Success,
            error: None,
        }]
    );
    assert_eq!(
        results,
        vec![LmsSyncResult {
            university_id: 1,
            university_name: "Universidad Demo 1".into(),
            outcome: SyncOutcome::Success,
            new_items: 1,
            due_changed: 0,
            removed: 0,
            error: None,
        }]
    );
    assert_eq!(
        stages(&h.observer, 1),
        vec![
            LmsSyncStage::Courses,
            LmsSyncStage::Items,
            LmsSyncStage::Saving,
            LmsSyncStage::Done
        ]
    );
    assert_eq!(
        h.observer.progress_calls()[0],
        LmsSyncProgress {
            university_id: 1,
            university_name: "Universidad Demo 1".into(),
            stage: LmsSyncStage::Courses,
        }
    );
    assert_eq!(h.observer.finished_calls(), vec![results]);
}

#[tokio::test]
async fn no_tracked_courses_skips_fetch_but_succeeds() {
    let h = harness(
        vec![demo(1, LoginState::Ok)],
        vec![course(10, 1, "C-100", false)],
    );

    let results = h.sync(None).await;

    assert!(h.platform.fetch_calls().is_empty());
    assert_eq!(results[0].outcome, SyncOutcome::Success);
    assert_eq!(
        (
            results[0].new_items,
            results[0].due_changed,
            results[0].removed
        ),
        (0, 0, 0)
    );
    assert_eq!(h.universities.syncs()[0].outcome, SyncOutcome::Success);
    assert_eq!(h.observer.finished_calls().len(), 1);
}

#[tokio::test]
async fn login_required_before_fetch() {
    let h = harness(
        vec![demo(1, LoginState::Ok)],
        vec![course(10, 1, "C-100", true)],
    );
    h.platform.script(
        1,
        Script {
            verify: Err(ScriptedError::Auth("expired")),
            ..Script::default()
        },
    );

    let results = h.sync(None).await;

    assert_eq!(results[0].outcome, SyncOutcome::LoginRequired);
    let message = results[0].error.clone().expect("error message");
    assert!(message.contains("Universidad Demo 1"));
    assert_eq!(
        h.universities.syncs(),
        vec![RecordedSync {
            university_id: 1,
            at: now(),
            outcome: SyncOutcome::LoginRequired,
            error: Some(message),
        }]
    );
    assert_eq!(
        stages(&h.observer, 1),
        vec![LmsSyncStage::Courses, LmsSyncStage::LoginRequired]
    );
    assert!(h.items.applied().is_empty());
    assert!(h.platform.fetch_calls().is_empty());
    assert!(h.courses.upserts().is_empty());
}

#[tokio::test]
async fn auth_error_during_fetch_marks_expired() {
    let h = harness(
        vec![demo(1, LoginState::Ok)],
        vec![course(10, 1, "C-100", true)],
    );
    h.platform.script(
        1,
        Script {
            items: Err(ScriptedError::Auth("session rejected")),
            ..Script::default()
        },
    );

    let results = h.sync(None).await;

    assert_eq!(
        h.universities.logins().last(),
        Some(&(1, LoginState::Expired, Some("Ana Pérez".to_string())))
    );
    assert_eq!(results[0].outcome, SyncOutcome::LoginRequired);
    assert_eq!(
        h.universities.syncs()[0].outcome,
        SyncOutcome::LoginRequired
    );
    assert_eq!(
        stages(&h.observer, 1),
        vec![
            LmsSyncStage::Courses,
            LmsSyncStage::Items,
            LmsSyncStage::LoginRequired
        ]
    );
    assert!(h.items.applied().is_empty());
}

#[tokio::test]
async fn connection_error_fails_only_that_university() {
    let h = harness(
        vec![demo(1, LoginState::Ok), demo(2, LoginState::Ok)],
        vec![course(10, 1, "C-100", true), course(20, 2, "C-200", true)],
    );
    h.platform.script(
        1,
        Script {
            items: Err(ScriptedError::Connection("The server is unreachable")),
            ..Script::default()
        },
    );
    h.platform.script(
        2,
        Script {
            items: Ok(vec![remote_item("C-200", "assign:9")]),
            ..Script::default()
        },
    );

    let results = h.sync(None).await;

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].university_id, 1);
    assert_eq!(results[0].outcome, SyncOutcome::Failed);
    assert_eq!(
        results[0].error.as_deref(),
        Some("The server is unreachable")
    );
    assert_eq!(results[1].university_id, 2);
    assert_eq!(results[1].outcome, SyncOutcome::Success);
    assert_eq!(results[1].new_items, 1);

    let applied = h.items.applied();
    assert_eq!(applied.len(), 1);
    assert_eq!(applied[0].0, 2);
    assert_eq!(
        h.universities.syncs(),
        vec![
            RecordedSync {
                university_id: 1,
                at: now(),
                outcome: SyncOutcome::Failed,
                error: Some("The server is unreachable".into()),
            },
            RecordedSync {
                university_id: 2,
                at: now(),
                outcome: SyncOutcome::Success,
                error: None,
            },
        ]
    );
    assert_eq!(
        stages(&h.observer, 1),
        vec![
            LmsSyncStage::Courses,
            LmsSyncStage::Items,
            LmsSyncStage::Failed
        ]
    );
    assert_eq!(h.observer.finished_calls(), vec![results]);
}

#[tokio::test]
async fn never_signed_in_universities_are_skipped() {
    let h = harness(
        vec![demo(1, LoginState::Never), demo(2, LoginState::Ok)],
        vec![],
    );

    let results = h.sync(None).await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].university_id, 2);
    assert!(h
        .observer
        .progress_calls()
        .iter()
        .all(|p| p.university_id == 2));
}

#[tokio::test]
async fn a_given_university_id_syncs_only_that_university() {
    let h = harness(
        vec![demo(1, LoginState::Ok), demo(2, LoginState::Ok)],
        vec![],
    );

    let results = h.sync(Some(2)).await;

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].university_id, 2);
}

#[tokio::test]
async fn prepare_errors() {
    let h = harness(
        vec![demo(1, LoginState::Never), demo(2, LoginState::Ok)],
        vec![],
    );
    assert!(matches!(
        h.runner.prepare(Some(1)).err(),
        Some(LmsSyncError::NothingToSync)
    ));
    assert!(matches!(
        h.runner.prepare(Some(99)).err(),
        Some(LmsSyncError::NothingToSync)
    ));
    assert!(!h.runner.is_running());

    let first = h.runner.prepare(None).expect("first prepare succeeds");
    assert!(h.runner.is_running());
    assert!(matches!(
        h.runner.prepare(None).err(),
        Some(LmsSyncError::AlreadyRunning)
    ));

    h.runner.execute(first).await;
    assert!(!h.runner.is_running());
    assert!(h.runner.prepare(None).is_ok());

    let nothing = harness(vec![demo(1, LoginState::Never)], vec![]);
    assert!(matches!(
        nothing.runner.prepare(None).err(),
        Some(LmsSyncError::NothingToSync)
    ));
    assert!(!nothing.runner.is_running());
}

#[tokio::test]
async fn storage_failure_is_a_failed_outcome() {
    let h = harness(
        vec![demo(1, LoginState::Ok)],
        vec![course(10, 1, "C-100", true)],
    );
    h.platform.script(
        1,
        Script {
            items: Ok(vec![remote_item("C-100", "assign:1")]),
            ..Script::default()
        },
    );
    h.items.fail_apply();

    let results = h.sync(None).await;

    assert_eq!(results[0].outcome, SyncOutcome::Failed);
    assert_eq!(results[0].error.as_deref(), Some(STORAGE_ERROR));
    assert_eq!(
        h.universities.syncs(),
        vec![RecordedSync {
            university_id: 1,
            at: now(),
            outcome: SyncOutcome::Failed,
            error: Some(STORAGE_ERROR.into()),
        }]
    );
    assert_eq!(
        stages(&h.observer, 1),
        vec![
            LmsSyncStage::Courses,
            LmsSyncStage::Items,
            LmsSyncStage::Saving,
            LmsSyncStage::Failed
        ]
    );
}

#[tokio::test]
async fn a_failing_record_sync_turns_success_into_failed_without_panicking() {
    let h = harness(vec![demo(1, LoginState::Ok)], vec![]);
    h.universities.fail_record_sync();

    let results = h.sync(None).await;

    assert_eq!(results[0].outcome, SyncOutcome::Failed);
    assert_eq!(results[0].error.as_deref(), Some(STORAGE_ERROR));
    assert_eq!(stages(&h.observer, 1).last(), Some(&LmsSyncStage::Failed));
    assert_eq!(h.observer.finished_calls().len(), 1);
}

#[tokio::test]
async fn the_lock_is_released_before_finished_is_called() {
    let h = harness(vec![demo(1, LoginState::Ok)], vec![]);
    let runner = Arc::downgrade(&h.runner);
    h.observer.set_finished_probe(Box::new(move || {
        runner.upgrade().expect("runner is alive").is_running()
    }));

    h.sync(None).await;

    assert_eq!(h.observer.probe_answers(), vec![false]);
}
