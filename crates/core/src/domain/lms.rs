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
}
