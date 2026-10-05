//! Commands: LMS sync, tracker list and the user's personal item fields.

use std::sync::Arc;

use academic_core::domain::{TrackedItem, TrackerQuery, UniversityId};
use academic_core::ports::{LmsSyncObserver, LmsSyncResult};
use tauri::async_runtime::JoinHandle;
use tauri::State;

use super::logged;
use super::universities::universities;
use crate::dto::{LmsItemDto, TrackerDto, TrackerQueryInput};
use crate::error::CommandError;
use crate::state::AppState;
use crate::validation;

#[tauri::command]
pub async fn lms_sync(
    state: State<'_, AppState>,
    university_id: Option<UniversityId>,
) -> Result<(), CommandError> {
    logged("lms_sync", start_sync(&state, university_id))
}

#[tauri::command]
pub async fn get_tracker(
    state: State<'_, AppState>,
    query: TrackerQueryInput,
) -> Result<TrackerDto, CommandError> {
    logged("get_tracker", tracker(&state, query))
}

#[tauri::command]
pub async fn set_item_done(
    state: State<'_, AppState>,
    id: i64,
    value: bool,
) -> Result<LmsItemDto, CommandError> {
    logged("set_item_done", set_done(&state, id, value))
}

#[tauri::command]
pub async fn set_item_hidden(
    state: State<'_, AppState>,
    id: i64,
    value: bool,
) -> Result<LmsItemDto, CommandError> {
    logged("set_item_hidden", set_hidden(&state, id, value))
}

#[tauri::command]
pub async fn set_item_note(
    state: State<'_, AppState>,
    id: i64,
    note: Option<String>,
) -> Result<LmsItemDto, CommandError> {
    logged("set_item_note", set_note(&state, id, note))
}

#[tauri::command]
pub async fn dismiss_item_change(
    state: State<'_, AppState>,
    id: i64,
) -> Result<LmsItemDto, CommandError> {
    logged("dismiss_item_change", dismiss_change(&state, id))
}

#[tauri::command]
pub async fn dismiss_all_changes(
    state: State<'_, AppState>,
    university_id: Option<UniversityId>,
) -> Result<u64, CommandError> {
    logged("dismiss_all_changes", dismiss_all(&state, university_id))
}

/// Selects the universities and takes the sync lock, then executes the sync in the
/// background. Progress and completion are reported through the sync observer; a
/// supervisor task reports completion if the execution task panics.
fn start_sync(state: &AppState, university_id: Option<UniversityId>) -> Result<(), CommandError> {
    let prepared = state.lms_sync.prepare(university_id)?;
    tracing::info!("lms sync started");
    let runner = state.lms_sync.clone();
    let execution = tauri::async_runtime::spawn(async move { runner.execute(prepared).await });
    tauri::async_runtime::spawn(supervise_sync(execution, state.lms_observer.clone()));
    Ok(())
}

/// Waits for the sync's execution task. If it panicked or was cancelled the
/// observer never heard `finished`, so it is told with no results and the UI
/// stops waiting.
async fn supervise_sync(
    execution: JoinHandle<Vec<LmsSyncResult>>,
    observer: Arc<dyn LmsSyncObserver>,
) {
    let Err(error) = execution.await else {
        return;
    };
    tracing::error!(%error, "lms sync ended abnormally");
    observer.finished(&[]);
}

fn tracker(state: &AppState, query: TrackerQueryInput) -> Result<TrackerDto, CommandError> {
    let now = state.clock.now();
    let items = state.items.query_items(&TrackerQuery::from(query))?;
    Ok(TrackerDto {
        items: items
            .iter()
            .map(|item| LmsItemDto::new(item, now))
            .collect(),
        universities: universities(state)?,
        sync_running: state.lms_sync.is_running(),
    })
}

fn item_dto(state: &AppState, item: &TrackedItem) -> LmsItemDto {
    LmsItemDto::new(item, state.clock.now())
}

fn set_done(state: &AppState, id: i64, done: bool) -> Result<LmsItemDto, CommandError> {
    Ok(item_dto(state, &state.items.set_item_done(id, done)?))
}

fn set_hidden(state: &AppState, id: i64, hidden: bool) -> Result<LmsItemDto, CommandError> {
    Ok(item_dto(state, &state.items.set_item_hidden(id, hidden)?))
}

fn set_note(state: &AppState, id: i64, note: Option<String>) -> Result<LmsItemDto, CommandError> {
    let note = validation::item_note(note)?;
    Ok(item_dto(
        state,
        &state.items.set_item_note(id, note.as_deref())?,
    ))
}

fn dismiss_change(state: &AppState, id: i64) -> Result<LmsItemDto, CommandError> {
    Ok(item_dto(state, &state.items.dismiss_item_change(id)?))
}

fn dismiss_all(state: &AppState, university_id: Option<UniversityId>) -> Result<u64, CommandError> {
    Ok(state.items.dismiss_all_changes(university_id)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use crate::test_support::{harness, harness_with_lms, FakeLmsBehavior, Harness};
    use academic_core::domain::{
        lms_credential_key, EffectiveStatus, ItemChange, ItemKind, LmsPlatform, LoginState,
        PlatformStatus, RemoteCourse, RemoteItem, SyncOutcome,
    };
    use academic_core::lms::{NewItem, SyncChanges};
    use academic_core::ports::{LmsError, SecretStore};
    use chrono::{DateTime, Duration, Utc};
    use serde_json::json;
    use std::time::Duration as StdDuration;

    fn remote_item(
        external_id: &str,
        title: &str,
        kind: ItemKind,
        due_at: Option<DateTime<Utc>>,
    ) -> RemoteItem {
        RemoteItem {
            course_external_id: "10".into(),
            external_id: external_id.into(),
            kind,
            title: title.into(),
            due_at,
            url: None,
            platform_status: PlatformStatus::NotSubmitted,
            grade: None,
        }
    }

    /// A signed-in university with one tracked course; returns (university, course) ids.
    fn seed_university(h: &Harness) -> (i64, i64) {
        let now = Utc::now();
        let university = h
            .state
            .universities
            .insert_university("UV", LmsPlatform::Moodle, "https://aulavirtual.uv.es", now)
            .unwrap();
        h.state
            .universities
            .set_login(university.id, LoginState::Ok, Some("Ana Pérez"))
            .unwrap();
        h.secrets
            .set(
                &lms_credential_key(LmsPlatform::Moodle, university.id),
                "tok",
            )
            .unwrap();
        let remote = RemoteCourse {
            external_id: "10".into(),
            name: "Algebra".into(),
            short_name: Some("ALG".into()),
            url: None,
        };
        h.state
            .courses
            .upsert_remote_courses(university.id, &[remote], now)
            .unwrap();
        let course_id = h.state.courses.list_courses(university.id).unwrap()[0].id;
        h.state
            .courses
            .set_tracked_courses(university.id, &[course_id])
            .unwrap();
        (university.id, course_id)
    }

    fn store_items(h: &Harness, university_id: i64, course_id: i64, items: Vec<NewItem>) {
        let changes = SyncChanges {
            inserts: items,
            synced_course_ids: vec![course_id],
            ..SyncChanges::default()
        };
        h.state
            .items
            .apply_sync(university_id, &changes, Utc::now())
            .unwrap();
    }

    fn new_item(course_id: i64, remote: RemoteItem, change: Option<ItemChange>) -> NewItem {
        NewItem {
            course_id,
            remote,
            change,
        }
    }

    fn plain_item(course_id: i64, n: u32, change: Option<ItemChange>) -> NewItem {
        new_item(
            course_id,
            remote_item(
                &format!("assign:{n}"),
                &format!("Practice {n}"),
                ItemKind::Assignment,
                None,
            ),
            change,
        )
    }

    fn query() -> TrackerQueryInput {
        TrackerQueryInput {
            university_id: None,
            include_hidden: false,
            include_removed: false,
        }
    }

    #[test]
    fn tracker_returns_items_with_status_and_names() {
        let h = harness();
        let (university_id, course_id) = seed_university(&h);
        let now = Utc::now();
        store_items(
            &h,
            university_id,
            course_id,
            vec![
                new_item(
                    course_id,
                    remote_item(
                        "assign:1",
                        "Practice 1",
                        ItemKind::Assignment,
                        Some(now - Duration::days(2)),
                    ),
                    Some(ItemChange::New),
                ),
                new_item(
                    course_id,
                    remote_item(
                        "quiz:2",
                        "Quiz 1",
                        ItemKind::Quiz,
                        Some(now + Duration::days(3)),
                    ),
                    Some(ItemChange::DueChanged {
                        previous_due_at: Some(now + Duration::days(1)),
                    }),
                ),
                new_item(
                    course_id,
                    remote_item("event:3", "Office hours", ItemKind::Event, None),
                    None,
                ),
            ],
        );

        let json = serde_json::to_value(tracker(&h.state, query()).unwrap()).unwrap();
        assert_eq!(json["syncRunning"], false);
        assert_eq!(json["universities"].as_array().unwrap().len(), 1);
        let items = json["items"].as_array().unwrap();
        assert_eq!(items.len(), 3);

        let overdue = &items[0];
        assert_eq!(overdue["title"], "Practice 1");
        assert_eq!(overdue["status"], "overdue");
        assert!(overdue["dueAt"].is_string());
        assert_eq!(overdue["change"], json!({ "kind": "new" }));
        assert_eq!(overdue["universityName"], "UV");
        assert_eq!(overdue["courseName"], "Algebra");
        assert_eq!(overdue["courseShortName"], "ALG");

        let pending = &items[1];
        assert_eq!(pending["status"], "pending");
        assert_eq!(pending["change"]["kind"], "due_changed");
        assert!(pending["change"]["previousDueAt"].is_string());

        let undated = &items[2];
        assert_eq!(undated["dueAt"], json!(null));
        assert_eq!(undated["change"], json!(null));
        assert_eq!(undated["hidden"], false);
        assert_eq!(undated["note"], json!(null));
    }

    #[test]
    fn tracker_filters_by_university_and_hidden() {
        let h = harness();
        let (university_id, course_id) = seed_university(&h);
        store_items(
            &h,
            university_id,
            course_id,
            vec![
                plain_item(course_id, 1, None),
                plain_item(course_id, 2, None),
            ],
        );
        let first = tracker(&h.state, query()).unwrap().items[0].id;
        set_hidden(&h.state, first, true).unwrap();

        assert_eq!(tracker(&h.state, query()).unwrap().items.len(), 1);
        let with_hidden = TrackerQueryInput {
            include_hidden: true,
            ..query()
        };
        assert_eq!(tracker(&h.state, with_hidden).unwrap().items.len(), 2);
        let other_university = TrackerQueryInput {
            university_id: Some(university_id + 1),
            include_hidden: true,
            ..query()
        };
        assert!(tracker(&h.state, other_university)
            .unwrap()
            .items
            .is_empty());
    }

    #[test]
    fn note_validation_and_personal_setters() {
        let h = harness();
        let (university_id, course_id) = seed_university(&h);
        store_items(
            &h,
            university_id,
            course_id,
            vec![plain_item(course_id, 1, Some(ItemChange::New))],
        );
        let id = tracker(&h.state, query()).unwrap().items[0].id;

        let too_long = set_note(&h.state, id, Some("x".repeat(2001))).unwrap_err();
        assert_eq!(
            (too_long.kind, too_long.message.as_str()),
            (
                CommandErrorKind::Validation,
                "Notes can have at most 2000 characters"
            )
        );
        let limit = "ñ".repeat(2000);
        assert_eq!(
            set_note(&h.state, id, Some(format!("  {limit}  ")))
                .unwrap()
                .note,
            Some(limit)
        );
        assert_eq!(
            set_note(&h.state, id, Some("  ".into())).unwrap().note,
            None
        );
        assert_eq!(
            set_note(&h.state, id, Some(" call the tutor ".into()))
                .unwrap()
                .note
                .as_deref(),
            Some("call the tutor")
        );
        assert_eq!(set_note(&h.state, id, None).unwrap().note, None);

        let done = set_done(&h.state, id, true).unwrap();
        assert!(done.done);
        assert_eq!(done.status, EffectiveStatus::Submitted);
        assert!(!set_done(&h.state, id, false).unwrap().done);
        assert!(set_hidden(&h.state, id, true).unwrap().hidden);
        assert!(!set_hidden(&h.state, id, false).unwrap().hidden);

        assert!(done.change.is_some());
        assert!(dismiss_change(&h.state, id).unwrap().change.is_none());

        assert_eq!(
            set_done(&h.state, 9999, true).unwrap_err().kind,
            CommandErrorKind::NotFound
        );
    }

    #[test]
    fn dismiss_all_changes_returns_the_number_cleared() {
        let h = harness();
        let (university_id, course_id) = seed_university(&h);
        store_items(
            &h,
            university_id,
            course_id,
            vec![
                plain_item(course_id, 1, Some(ItemChange::New)),
                plain_item(course_id, 2, Some(ItemChange::New)),
                plain_item(course_id, 3, None),
            ],
        );
        assert_eq!(dismiss_all(&h.state, Some(university_id + 1)).unwrap(), 0);
        assert_eq!(dismiss_all(&h.state, None).unwrap(), 2);
        assert_eq!(dismiss_all(&h.state, None).unwrap(), 0);
    }

    #[test]
    fn sync_rejects_when_nothing_to_sync() {
        let h = harness();
        let error = start_sync(&h.state, None).unwrap_err();
        assert_eq!(
            (error.kind, error.message.as_str()),
            (
                CommandErrorKind::Validation,
                "Sign in to a university first"
            )
        );

        let never = h
            .state
            .universities
            .insert_university(
                "UV",
                LmsPlatform::Moodle,
                "https://aulavirtual.uv.es",
                Utc::now(),
            )
            .unwrap();
        assert_eq!(
            start_sync(&h.state, Some(never.id)).unwrap_err().kind,
            CommandErrorKind::Validation
        );
        assert!(!h.state.lms_sync.is_running());
    }

    #[test]
    fn sync_twice_conflicts() {
        let h = harness();
        let (university_id, _) = seed_university(&h);
        let held = h.state.lms_sync.prepare(Some(university_id)).unwrap();

        let error = start_sync(&h.state, Some(university_id)).unwrap_err();
        assert_eq!(
            (error.kind, error.message.as_str()),
            (CommandErrorKind::Conflict, "A sync is already running")
        );
        assert!(tracker(&h.state, query()).unwrap().sync_running);

        drop(held);
        assert!(!tracker(&h.state, query()).unwrap().sync_running);
    }

    fn one_course() -> Result<Vec<RemoteCourse>, LmsError> {
        Ok(vec![RemoteCourse {
            external_id: "10".into(),
            name: "Algebra".into(),
            short_name: None,
            url: None,
        }])
    }

    fn one_item() -> Result<Vec<RemoteItem>, LmsError> {
        Ok(vec![remote_item(
            "assign:1",
            "Practice 1",
            ItemKind::Assignment,
            None,
        )])
    }

    #[tokio::test]
    async fn sync_runs_in_the_background_and_reports_finished() {
        let h = harness_with_lms(FakeLmsBehavior {
            courses: one_course,
            items: one_item,
            ..FakeLmsBehavior::default()
        });
        let (university_id, _) = seed_university(&h);

        start_sync(&h.state, Some(university_id)).unwrap();

        for _ in 0..200 {
            if !h.lms_observer.finished.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(StdDuration::from_millis(25)).await;
        }
        let finished = h.lms_observer.finished.lock().unwrap().clone();
        assert_eq!(finished.len(), 1);
        assert_eq!(finished[0].len(), 1);
        assert_eq!(finished[0][0].outcome, SyncOutcome::Success);
        assert_eq!(
            finished[0][0].new_items, 0,
            "the first sync of a course is a baseline"
        );
        assert_eq!(tracker(&h.state, query()).unwrap().items.len(), 1);
    }

    #[tokio::test]
    async fn a_panicking_sync_still_reports_finished_with_no_results() {
        let h = harness();
        supervise_sync(
            tauri::async_runtime::spawn(async { panic!("simulated panic in a sync") }),
            h.state.lms_observer.clone(),
        )
        .await;
        assert_eq!(*h.lms_observer.finished.lock().unwrap(), vec![vec![]]);
    }

    #[tokio::test]
    async fn a_sync_that_completes_normally_is_left_alone() {
        let h = harness();
        supervise_sync(
            tauri::async_runtime::spawn(async { Vec::new() }),
            h.state.lms_observer.clone(),
        )
        .await;
        assert!(h.lms_observer.finished.lock().unwrap().is_empty());
    }
}
