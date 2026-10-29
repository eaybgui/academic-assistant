//! LMS DTOs (spec §9.1, §18).

use academic_core::domain::{
    effective_status, EffectiveStatus, Grade, ItemChange, ItemKind, LmsCourse, LmsPlatform,
    LmsUniversity, LoginState, SyncOutcome, TrackedItem, TrackerQuery,
};
use academic_core::ports::LmsSyncResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsLastSyncDto {
    pub at: Option<DateTime<Utc>>,
    pub outcome: Option<SyncOutcome>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsCourseCountsDto {
    pub tracked: usize,
    pub total: usize,
    pub unreviewed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsUniversityDto {
    pub id: i64,
    pub name: String,
    pub platform: LmsPlatform,
    pub base_url: String,
    pub login_state: LoginState,
    pub account_label: Option<String>,
    pub last_sync: LmsLastSyncDto,
    pub courses: LmsCourseCountsDto,
}

impl LmsUniversityDto {
    pub fn new(university: &LmsUniversity, courses: &[LmsCourse]) -> Self {
        Self {
            id: university.id,
            name: university.name.clone(),
            platform: university.platform,
            base_url: university.base_url.clone(),
            login_state: university.login_state,
            account_label: university.account_label.clone(),
            last_sync: LmsLastSyncDto {
                at: university.last_sync_at,
                outcome: university.last_sync_outcome,
                error: university.last_sync_error.clone(),
            },
            courses: LmsCourseCountsDto {
                tracked: courses.iter().filter(|c| c.tracked).count(),
                total: courses.len(),
                unreviewed: courses.iter().filter(|c| !c.reviewed).count(),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsCourseDto {
    pub id: i64,
    pub university_id: i64,
    pub name: String,
    pub short_name: Option<String>,
    pub url: Option<String>,
    pub tracked: bool,
    pub available: bool,
    /// The user has not reviewed the course selection since the course appeared.
    pub is_new: bool,
}

impl From<&LmsCourse> for LmsCourseDto {
    fn from(course: &LmsCourse) -> Self {
        Self {
            id: course.id,
            university_id: course.university_id,
            name: course.name.clone(),
            short_name: course.short_name.clone(),
            url: course.url.clone(),
            tracked: course.tracked,
            available: course.available,
            is_new: !course.reviewed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GradeDto {
    pub display: String,
    pub score: Option<f64>,
    pub max: Option<f64>,
}

impl From<&Grade> for GradeDto {
    fn from(grade: &Grade) -> Self {
        Self {
            display: grade.display.clone(),
            score: grade.score,
            max: grade.max,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ItemChangeDto {
    New,
    DueChanged {
        #[serde(rename = "previousDueAt")]
        previous_due_at: Option<DateTime<Utc>>,
    },
}

impl From<&ItemChange> for ItemChangeDto {
    fn from(change: &ItemChange) -> Self {
        match change {
            ItemChange::New => Self::New,
            ItemChange::DueChanged { previous_due_at } => Self::DueChanged {
                previous_due_at: *previous_due_at,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsItemDto {
    pub id: i64,
    pub university_id: i64,
    pub university_name: String,
    pub course_id: i64,
    pub course_name: String,
    pub course_short_name: Option<String>,
    pub external_id: String,
    pub kind: ItemKind,
    pub title: String,
    pub due_at: Option<DateTime<Utc>>,
    pub url: Option<String>,
    pub status: EffectiveStatus,
    pub grade: Option<GradeDto>,
    pub change: Option<ItemChangeDto>,
    pub removed_at: Option<DateTime<Utc>>,
    pub done: bool,
    pub hidden: bool,
    pub note: Option<String>,
}

impl LmsItemDto {
    pub fn new(tracked: &TrackedItem, now: DateTime<Utc>) -> Self {
        let item = &tracked.item;
        Self {
            id: item.id,
            university_id: item.university_id,
            university_name: tracked.university_name.clone(),
            course_id: item.course_id,
            course_name: tracked.course_name.clone(),
            course_short_name: tracked.course_short_name.clone(),
            external_id: item.external_id.clone(),
            kind: item.kind,
            title: item.title.clone(),
            due_at: item.due_at,
            url: item.url.clone(),
            status: effective_status(item, now),
            grade: item.grade.as_ref().map(GradeDto::from),
            change: item.change.as_ref().map(ItemChangeDto::from),
            removed_at: item.removed_at,
            done: item.done,
            hidden: item.hidden,
            note: item.note.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackerDto {
    pub items: Vec<LmsItemDto>,
    pub universities: Vec<LmsUniversityDto>,
    pub sync_running: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackerQueryInput {
    pub university_id: Option<i64>,
    pub include_hidden: bool,
    pub include_removed: bool,
}

impl From<TrackerQueryInput> for TrackerQuery {
    fn from(input: TrackerQueryInput) -> Self {
        Self {
            university_id: input.university_id,
            include_hidden: input.include_hidden,
            include_removed: input.include_removed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsSyncResultDto {
    pub university_id: i64,
    pub university_name: String,
    pub outcome: SyncOutcome,
    pub new_items: u32,
    pub due_changed: u32,
    pub removed: u32,
    pub error: Option<String>,
}

impl From<&LmsSyncResult> for LmsSyncResultDto {
    fn from(result: &LmsSyncResult) -> Self {
        Self {
            university_id: result.university_id,
            university_name: result.university_name.clone(),
            outcome: result.outcome,
            new_items: result.new_items,
            due_changed: result.due_changed,
            removed: result.removed,
            error: result.error.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsUniversityInput {
    pub name: String,
    pub platform: LmsPlatform,
    pub base_url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeDto {
    pub base_url: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use academic_core::domain::{ItemKind, LmsItem, PlatformStatus};
    use chrono::TimeZone;
    use serde_json::json;

    fn university() -> LmsUniversity {
        LmsUniversity {
            id: 3,
            name: "UV".into(),
            platform: LmsPlatform::Moodle,
            base_url: "https://aulavirtual.uv.es".into(),
            login_state: LoginState::Ok,
            account_label: Some("Ana Pérez".into()),
            last_sync_at: Some(Utc.with_ymd_and_hms(2026, 10, 4, 8, 30, 0).unwrap()),
            last_sync_outcome: Some(SyncOutcome::LoginRequired),
            last_sync_error: None,
            created_at: Utc.with_ymd_and_hms(2026, 10, 1, 8, 0, 0).unwrap(),
        }
    }

    fn course(id: i64, tracked: bool, reviewed: bool) -> LmsCourse {
        LmsCourse {
            id,
            university_id: 3,
            external_id: id.to_string(),
            name: format!("Course {id}"),
            short_name: None,
            url: None,
            tracked,
            available: true,
            reviewed,
            items_synced_at: None,
        }
    }

    #[test]
    fn university_serializes_in_the_frontend_shape() {
        let courses = [
            course(1, true, true),
            course(2, false, true),
            course(3, false, false),
        ];
        let dto = LmsUniversityDto::new(&university(), &courses);
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            json!({
                "id": 3,
                "name": "UV",
                "platform": "moodle",
                "baseUrl": "https://aulavirtual.uv.es",
                "loginState": "ok",
                "accountLabel": "Ana Pérez",
                "lastSync": {
                    "at": "2026-10-04T08:30:00Z",
                    "outcome": "login_required",
                    "error": null
                },
                "courses": { "tracked": 1, "total": 3, "unreviewed": 1 }
            })
        );
    }

    fn tracked_item(change: Option<ItemChange>) -> TrackedItem {
        TrackedItem {
            item: LmsItem {
                id: 9,
                university_id: 3,
                course_id: 5,
                external_id: "assign:42".into(),
                kind: ItemKind::Assignment,
                title: "Practice 1".into(),
                due_at: Some(Utc.with_ymd_and_hms(2026, 10, 20, 22, 0, 0).unwrap()),
                url: Some("https://aulavirtual.uv.es/mod/assign/view.php?id=42".into()),
                platform_status: PlatformStatus::NotSubmitted,
                grade: Some(Grade {
                    display: "8 / 10".into(),
                    score: Some(8.0),
                    max: Some(10.0),
                }),
                change,
                removed_at: None,
                done: false,
                hidden: true,
                note: Some("Ask for an extension".into()),
                first_seen_at: Utc.with_ymd_and_hms(2026, 10, 1, 8, 0, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2026, 10, 1, 8, 0, 0).unwrap(),
            },
            university_name: "UV".into(),
            course_name: "Algebra".into(),
            course_short_name: Some("ALG".into()),
        }
    }

    fn now() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 5, 12, 0, 0).unwrap()
    }

    fn item_json(change: serde_json::Value) -> serde_json::Value {
        json!({
            "id": 9,
            "universityId": 3,
            "universityName": "UV",
            "courseId": 5,
            "courseName": "Algebra",
            "courseShortName": "ALG",
            "externalId": "assign:42",
            "kind": "assignment",
            "title": "Practice 1",
            "dueAt": "2026-10-20T22:00:00Z",
            "url": "https://aulavirtual.uv.es/mod/assign/view.php?id=42",
            "status": "graded",
            "grade": { "display": "8 / 10", "score": 8.0, "max": 10.0 },
            "change": change,
            "removedAt": null,
            "done": false,
            "hidden": true,
            "note": "Ask for an extension"
        })
    }

    #[test]
    fn item_serializes_in_the_frontend_shape_for_every_change_variant() {
        let dto = |change| serde_json::to_value(LmsItemDto::new(&tracked_item(change), now()));
        assert_eq!(
            dto(Some(ItemChange::New)).unwrap(),
            item_json(json!({ "kind": "new" }))
        );
        assert_eq!(
            dto(Some(ItemChange::DueChanged {
                previous_due_at: Some(Utc.with_ymd_and_hms(2026, 10, 18, 22, 0, 0).unwrap()),
            }))
            .unwrap(),
            item_json(json!({ "kind": "due_changed", "previousDueAt": "2026-10-18T22:00:00Z" }))
        );
        assert_eq!(
            dto(Some(ItemChange::DueChanged {
                previous_due_at: None,
            }))
            .unwrap(),
            item_json(json!({ "kind": "due_changed", "previousDueAt": null }))
        );
        assert_eq!(dto(None).unwrap(), item_json(json!(null)));
    }

    #[test]
    fn item_status_is_derived_from_the_clock() {
        let mut tracked = tracked_item(None);
        tracked.item.grade = None;
        let status =
            |now| serde_json::to_value(LmsItemDto::new(&tracked, now)).unwrap()["status"].clone();
        assert_eq!(status(now()), "pending");
        assert_eq!(
            status(Utc.with_ymd_and_hms(2026, 10, 21, 0, 0, 0).unwrap()),
            "overdue"
        );
    }

    #[test]
    fn course_serializes_in_the_frontend_shape() {
        let mut unreviewed = course(7, true, false);
        unreviewed.short_name = Some("ALG".into());
        unreviewed.url = Some("https://aulavirtual.uv.es/course/view.php?id=7".into());
        assert_eq!(
            serde_json::to_value(LmsCourseDto::from(&unreviewed)).unwrap(),
            json!({
                "id": 7,
                "universityId": 3,
                "name": "Course 7",
                "shortName": "ALG",
                "url": "https://aulavirtual.uv.es/course/view.php?id=7",
                "tracked": true,
                "available": true,
                "isNew": true
            })
        );
        let reviewed = serde_json::to_value(LmsCourseDto::from(&course(8, false, true))).unwrap();
        assert_eq!(reviewed["isNew"], false);
        assert_eq!(reviewed["shortName"], json!(null));
    }

    #[test]
    fn sync_result_serializes_in_the_frontend_shape() {
        let result = LmsSyncResult {
            university_id: 3,
            university_name: "UV".into(),
            outcome: SyncOutcome::LoginRequired,
            new_items: 2,
            due_changed: 1,
            removed: 0,
            error: Some("UV needs you to sign in again".into()),
        };
        assert_eq!(
            serde_json::to_value(LmsSyncResultDto::from(&result)).unwrap(),
            json!({
                "universityId": 3,
                "universityName": "UV",
                "outcome": "login_required",
                "newItems": 2,
                "dueChanged": 1,
                "removed": 0,
                "error": "UV needs you to sign in again"
            })
        );
    }

    #[test]
    fn tracker_query_input_deserializes_camel_case() {
        let input: TrackerQueryInput = serde_json::from_value(json!({
            "universityId": 3,
            "includeHidden": true,
            "includeRemoved": false
        }))
        .unwrap();
        assert_eq!(
            TrackerQuery::from(input),
            TrackerQuery {
                university_id: Some(3),
                include_hidden: true,
                include_removed: false,
            }
        );
    }
}
