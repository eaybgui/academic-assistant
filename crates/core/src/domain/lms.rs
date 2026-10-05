use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::enum_str;

pub type UniversityId = i64;

/// Learning platform a university runs. Stored as `universities.platform`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LmsPlatform {
    Moodle,
    Blackboard,
}

enum_str!(LmsPlatform {
    Moodle => "moodle",
    Blackboard => "blackboard",
});

/// Whether the app holds a working credential for a university.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoginState {
    Never,
    Ok,
    Expired,
    Error,
}

enum_str!(LoginState {
    Never => "never",
    Ok => "ok",
    Expired => "expired",
    Error => "error",
});

/// Outcome of the last sync of a university (written from M3 on).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncOutcome {
    Success,
    Failed,
    LoginRequired,
}

enum_str!(SyncOutcome {
    Success => "success",
    Failed => "failed",
    LoginRequired => "login_required",
});

/// A university the user follows through its learning platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LmsUniversity {
    pub id: UniversityId,
    pub name: String,
    pub platform: LmsPlatform,
    /// Normalized `https` origin without trailing slash, e.g. `https://aulavirtual.uv.es`.
    pub base_url: String,
    pub login_state: LoginState,
    /// Name the platform reports for the signed-in user ("Connected as …").
    pub account_label: Option<String>,
    pub last_sync_at: Option<DateTime<Utc>>,
    pub last_sync_outcome: Option<SyncOutcome>,
    pub last_sync_error: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A course discovered on a university's platform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LmsCourse {
    pub id: i64,
    pub university_id: UniversityId,
    /// Platform course id: Moodle `"1234"`, Blackboard `"_10_1"`.
    pub external_id: String,
    pub name: String,
    pub short_name: Option<String>,
    pub url: Option<String>,
    pub tracked: bool,
    /// False when the platform stopped returning the course.
    pub available: bool,
    /// False until the user saves a course selection that includes it.
    pub reviewed: bool,
    /// None until the course's items are synced for the first time (M3).
    pub items_synced_at: Option<DateTime<Utc>>,
}

/// The signed-in user as reported by the platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LmsIdentity {
    pub user_id: String,
    pub display_name: String,
}

/// A course as returned by the platform, before it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCourse {
    pub external_id: String,
    pub name: String,
    pub short_name: Option<String>,
    pub url: Option<String>,
}

/// Kind of stored item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ItemKind {
    Assignment,
    Quiz,
    Test,
    Event,
    Other,
}

enum_str!(ItemKind {
    Assignment => "assignment",
    Quiz => "quiz",
    Test => "test",
    Event => "event",
    Other => "other",
});

/// Submission state as the platform reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlatformStatus {
    NotSubmitted,
    Submitted,
    Graded,
    NotApplicable,
}

enum_str!(PlatformStatus {
    NotSubmitted => "not_submitted",
    Submitted => "submitted",
    Graded => "graded",
    NotApplicable => "not_applicable",
});

/// A grade as the platform shows it, plus the numbers when available.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grade {
    pub display: String,
    pub score: Option<f64>,
    pub max: Option<f64>,
}

/// Badge shown until the user dismisses it (spec §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ItemChange {
    New,
    DueChanged {
        previous_due_at: Option<DateTime<Utc>>,
    },
}

/// A stored deliverable, exam or course event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LmsItem {
    pub id: i64,
    pub university_id: UniversityId,
    pub course_id: i64,
    pub external_id: String,
    pub kind: ItemKind,
    pub title: String,
    pub due_at: Option<DateTime<Utc>>,
    pub url: Option<String>,
    pub platform_status: PlatformStatus,
    pub grade: Option<Grade>,
    pub change: Option<ItemChange>,
    pub removed_at: Option<DateTime<Utc>>,
    pub done: bool,
    pub hidden: bool,
    pub note: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectiveStatus {
    Pending,
    Overdue,
    Submitted,
    Graded,
    PastEvent,
}

/// Derived status (spec §4.1): graded › submitted/done › past event › overdue › pending.
pub fn effective_status(item: &LmsItem, now: DateTime<Utc>) -> EffectiveStatus {
    let past = item.due_at.is_some_and(|due| due < now);
    if item.grade.is_some() || item.platform_status == PlatformStatus::Graded {
        EffectiveStatus::Graded
    } else if item.platform_status == PlatformStatus::Submitted || item.done {
        EffectiveStatus::Submitted
    } else if item.kind == ItemKind::Event && past {
        EffectiveStatus::PastEvent
    } else if past {
        EffectiveStatus::Overdue
    } else {
        EffectiveStatus::Pending
    }
}

/// Date range used for course events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyncWindow {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
}

impl SyncWindow {
    pub fn around(now: DateTime<Utc>) -> Self {
        Self {
            from: now - chrono::Duration::days(180),
            to: now + chrono::Duration::days(365),
        }
    }
}

/// A tracked course to fetch items for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCourseRef {
    pub external_id: String,
}

/// An item as returned by the platform, before it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteItem {
    pub course_external_id: String,
    pub external_id: String,
    pub kind: ItemKind,
    pub title: String,
    pub due_at: Option<DateTime<Utc>>,
    pub url: Option<String>,
    pub platform_status: PlatformStatus,
    pub grade: Option<Grade>,
}

/// An item with the names the tracker shows next to it.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedItem {
    pub item: LmsItem,
    pub university_name: String,
    pub course_name: String,
    pub course_short_name: Option<String>,
}

/// Filter for the tracker list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TrackerQuery {
    pub university_id: Option<UniversityId>,
    pub include_hidden: bool,
    pub include_removed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn enums_round_trip_through_their_strings() {
        for p in [LmsPlatform::Moodle, LmsPlatform::Blackboard] {
            assert_eq!(LmsPlatform::from_str(p.as_str()).unwrap(), p);
        }
        for s in [
            LoginState::Never,
            LoginState::Ok,
            LoginState::Expired,
            LoginState::Error,
        ] {
            assert_eq!(LoginState::from_str(s.as_str()).unwrap(), s);
        }
        for o in [
            SyncOutcome::Success,
            SyncOutcome::Failed,
            SyncOutcome::LoginRequired,
        ] {
            assert_eq!(SyncOutcome::from_str(o.as_str()).unwrap(), o);
        }
        assert_eq!(SyncOutcome::LoginRequired.as_str(), "login_required");
    }

    #[test]
    fn platform_serializes_snake_case() {
        assert_eq!(
            serde_json::to_value(LmsPlatform::Blackboard).unwrap(),
            "blackboard"
        );
    }

    fn item(kind: ItemKind, status: PlatformStatus, due: Option<DateTime<Utc>>) -> LmsItem {
        LmsItem {
            id: 1,
            university_id: 1,
            course_id: 1,
            external_id: "assign:1".into(),
            kind,
            title: "t".into(),
            due_at: due,
            url: None,
            platform_status: status,
            grade: None,
            change: None,
            removed_at: None,
            done: false,
            hidden: false,
            note: None,
            first_seen_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn effective_status_follows_the_spec_order() {
        let now = Utc::now();
        let past = Some(now - chrono::Duration::days(1));
        let future = Some(now + chrono::Duration::days(1));
        let mut graded = item(ItemKind::Assignment, PlatformStatus::Submitted, past);
        graded.grade = Some(Grade {
            display: "8 / 10".into(),
            score: Some(8.0),
            max: Some(10.0),
        });
        assert_eq!(effective_status(&graded, now), EffectiveStatus::Graded);
        assert_eq!(
            effective_status(&item(ItemKind::Quiz, PlatformStatus::Graded, None), now),
            EffectiveStatus::Graded
        );
        assert_eq!(
            effective_status(
                &item(ItemKind::Assignment, PlatformStatus::Submitted, past),
                now
            ),
            EffectiveStatus::Submitted
        );
        let mut done = item(ItemKind::Assignment, PlatformStatus::NotSubmitted, past);
        done.done = true;
        assert_eq!(effective_status(&done, now), EffectiveStatus::Submitted);
        assert_eq!(
            effective_status(
                &item(ItemKind::Event, PlatformStatus::NotApplicable, past),
                now
            ),
            EffectiveStatus::PastEvent
        );
        assert_eq!(
            effective_status(
                &item(ItemKind::Event, PlatformStatus::NotApplicable, future),
                now
            ),
            EffectiveStatus::Pending
        );
        assert_eq!(
            effective_status(
                &item(ItemKind::Assignment, PlatformStatus::NotSubmitted, past),
                now
            ),
            EffectiveStatus::Overdue
        );
        assert_eq!(
            effective_status(
                &item(ItemKind::Assignment, PlatformStatus::NotSubmitted, future),
                now
            ),
            EffectiveStatus::Pending
        );
        assert_eq!(
            effective_status(
                &item(ItemKind::Assignment, PlatformStatus::NotSubmitted, None),
                now
            ),
            EffectiveStatus::Pending
        );
    }

    #[test]
    fn sync_window_spans_180_days_back_and_365_ahead() {
        let now = Utc::now();
        let w = SyncWindow::around(now);
        assert_eq!(w.from, now - chrono::Duration::days(180));
        assert_eq!(w.to, now + chrono::Duration::days(365));
    }

    #[test]
    fn item_enums_round_trip() {
        for k in [
            ItemKind::Assignment,
            ItemKind::Quiz,
            ItemKind::Test,
            ItemKind::Event,
            ItemKind::Other,
        ] {
            assert_eq!(ItemKind::from_str(k.as_str()).unwrap(), k);
        }
        for s in [
            PlatformStatus::NotSubmitted,
            PlatformStatus::Submitted,
            PlatformStatus::Graded,
            PlatformStatus::NotApplicable,
        ] {
            assert_eq!(PlatformStatus::from_str(s.as_str()).unwrap(), s);
        }
        assert_eq!(PlatformStatus::NotApplicable.as_str(), "not_applicable");
        assert_eq!(
            serde_json::to_value(EffectiveStatus::PastEvent).unwrap(),
            "past_event"
        );
    }
}
