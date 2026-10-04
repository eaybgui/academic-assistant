//! LMS DTOs (spec §9.1, §18).

use academic_core::domain::{LmsCourse, LmsPlatform, LmsUniversity, LoginState, SyncOutcome};
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
}
