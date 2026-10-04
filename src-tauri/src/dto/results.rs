//! Result, query, page and dashboard DTOs.

use academic_core::domain::{
    Category, DashboardCounts, DateKind, Priority, ResultDate, ResultSort, StoredResult,
};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};

use super::runs::RunDto;

const TIME_FORMAT: &str = "%H:%M";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultDateDto {
    pub date: NaiveDate,
    pub time: Option<String>,
    pub kind: DateKind,
}

impl From<&ResultDate> for ResultDateDto {
    fn from(date: &ResultDate) -> Self {
        Self {
            date: date.date,
            time: date.time.map(|time| time.format(TIME_FORMAT).to_string()),
            kind: date.kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultDto {
    pub id: i64,
    pub account_id: i64,
    pub account_name: String,
    pub subject: String,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub university_name: Option<String>,
    pub course_name: Option<String>,
    pub category: Category,
    pub priority: Priority,
    pub action_required: bool,
    pub possible_change: bool,
    pub dates: Vec<ResultDateDto>,
    pub received_at: DateTime<Utc>,
    pub detected_at: DateTime<Utc>,
    pub source_url: Option<String>,
    pub message_id_header: Option<String>,
    pub seen: bool,
    pub seen_at: Option<DateTime<Utc>>,
}

impl From<&StoredResult> for ResultDto {
    fn from(result: &StoredResult) -> Self {
        Self {
            id: result.id,
            account_id: result.account_id,
            account_name: result.account_name.clone(),
            subject: result.subject.clone(),
            sender_name: result.sender_name.clone(),
            sender_email: result.sender_email.clone(),
            university_name: result.university_name.clone(),
            course_name: result.course_name.clone(),
            category: result.category,
            priority: result.priority,
            action_required: result.action_required,
            possible_change: result.possible_change,
            dates: result.dates.iter().map(ResultDateDto::from).collect(),
            received_at: result.received_at,
            detected_at: result.detected_at,
            source_url: result.source_url.clone(),
            message_id_header: result.message_id_header.clone(),
            seen: result.seen,
            seen_at: result.seen_at,
        }
    }
}

/// `get_results` argument. `limit`/`offset` are signed so that out-of-range values
/// reach [`crate::validation::result_query`] (which clamps them) instead of failing
/// deserialization with an opaque error.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultQueryInput {
    pub search: Option<String>,
    pub seen: Option<bool>,
    pub category: Option<Category>,
    pub account_id: Option<i64>,
    pub course_id: Option<i64>,
    pub sort: ResultSort,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageDto<T> {
    pub items: Vec<T>,
    pub total: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardCountsDto {
    pub new_total: u32,
    pub tasks: u32,
    pub exams: u32,
    pub events: u32,
    pub changes_and_notices: u32,
}

impl From<&DashboardCounts> for DashboardCountsDto {
    fn from(counts: &DashboardCounts) -> Self {
        Self {
            new_total: counts.new_total,
            tasks: counts.tasks,
            exams: counts.exams,
            events: counts.events,
            changes_and_notices: counts.changes_and_notices,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttentionDto {
    pub account_id: i64,
    pub account_name: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardDto {
    pub last_run: Option<RunDto>,
    pub running_run_id: Option<i64>,
    pub attention: Vec<AttentionDto>,
    pub counts: DashboardCountsDto,
    pub new_results: Vec<ResultDto>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveTime, TimeZone};
    use serde_json::json;

    fn sample_result() -> StoredResult {
        StoredResult {
            id: 11,
            account_id: 2,
            account_name: "UV IMAP".into(),
            subject: "Exam moved".into(),
            sender_name: Some("Prof. Ruiz".into()),
            sender_email: "ruiz@uv.es".into(),
            university_name: Some("UV".into()),
            course_name: None,
            category: Category::GeneralInfo,
            priority: Priority::Urgent,
            action_required: true,
            possible_change: false,
            dates: vec![
                ResultDate {
                    date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
                    time: Some(NaiveTime::from_hms_opt(9, 30, 0).unwrap()),
                    kind: DateKind::Exam,
                },
                ResultDate {
                    date: NaiveDate::from_ymd_opt(2026, 10, 6).unwrap(),
                    time: None,
                    kind: DateKind::Deadline,
                },
            ],
            received_at: Utc.with_ymd_and_hms(2026, 9, 28, 8, 0, 0).unwrap(),
            detected_at: Utc.with_ymd_and_hms(2026, 9, 28, 9, 0, 0).unwrap(),
            source_url: None,
            message_id_header: Some("abc@uv.es".into()),
            seen: false,
            seen_at: None,
        }
    }

    #[test]
    fn result_dto_uses_camel_case_keys_and_formats_dates() {
        let dto = ResultDto::from(&sample_result());
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            json!({
                "id": 11,
                "accountId": 2,
                "accountName": "UV IMAP",
                "subject": "Exam moved",
                "senderName": "Prof. Ruiz",
                "senderEmail": "ruiz@uv.es",
                "universityName": "UV",
                "courseName": null,
                "category": "general_info",
                "priority": "urgent",
                "actionRequired": true,
                "possibleChange": false,
                "dates": [
                    { "date": "2026-10-05", "time": "09:30", "kind": "exam" },
                    { "date": "2026-10-06", "time": null, "kind": "deadline" }
                ],
                "receivedAt": "2026-09-28T08:00:00Z",
                "detectedAt": "2026-09-28T09:00:00Z",
                "sourceUrl": null,
                "messageIdHeader": "abc@uv.es",
                "seen": false,
                "seenAt": null
            })
        );
    }

    #[test]
    fn query_input_accepts_absent_optional_fields() {
        let input: ResultQueryInput =
            serde_json::from_value(json!({ "sort": "upcoming", "limit": 25, "offset": 0 }))
                .unwrap();
        assert_eq!(
            input,
            ResultQueryInput {
                search: None,
                seen: None,
                category: None,
                account_id: None,
                course_id: None,
                sort: ResultSort::Upcoming,
                limit: 25,
                offset: 0,
            }
        );
    }

    #[test]
    fn query_input_parses_every_field() {
        let input: ResultQueryInput = serde_json::from_value(json!({
            "search": "exam",
            "seen": false,
            "category": "general_info",
            "accountId": 3,
            "courseId": 4,
            "sort": "priority",
            "limit": 50,
            "offset": 100
        }))
        .unwrap();
        assert_eq!(input.search.as_deref(), Some("exam"));
        assert_eq!(input.seen, Some(false));
        assert_eq!(input.category, Some(Category::GeneralInfo));
        assert_eq!(input.account_id, Some(3));
        assert_eq!(input.course_id, Some(4));
        assert_eq!(input.sort, ResultSort::Priority);
        assert_eq!(input.offset, 100);
    }

    #[test]
    fn dashboard_dto_matches_the_typescript_shape() {
        let dto = DashboardDto {
            last_run: None,
            running_run_id: Some(9),
            attention: vec![AttentionDto {
                account_id: 1,
                account_name: "UAH".into(),
                message: "Sign in to this account".into(),
            }],
            counts: DashboardCountsDto::from(&DashboardCounts {
                new_total: 4,
                tasks: 1,
                exams: 1,
                events: 1,
                changes_and_notices: 1,
            }),
            new_results: vec![],
        };
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            json!({
                "lastRun": null,
                "runningRunId": 9,
                "attention": [
                    { "accountId": 1, "accountName": "UAH", "message": "Sign in to this account" }
                ],
                "counts": {
                    "newTotal": 4, "tasks": 1, "exams": 1, "events": 1, "changesAndNotices": 1
                },
                "newResults": []
            })
        );
    }

    #[test]
    fn page_dto_serializes_items_and_total() {
        let page = PageDto {
            items: vec![1, 2],
            total: 10,
        };
        assert_eq!(
            serde_json::to_value(&page).unwrap(),
            json!({ "items": [1, 2], "total": 10 })
        );
    }
}
