//! Run DTOs: counters, per-account results and the run record.

use academic_core::domain::{
    Counters, ErrorKind, RunAccountStatus, RunAccountView, RunRecord, RunStatus, RunTrigger,
};
use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CountersDto {
    pub found: u32,
    pub filtered: u32,
    pub analyzed: u32,
    pub failed: u32,
    pub results_created: u32,
}

impl From<&Counters> for CountersDto {
    fn from(counters: &Counters) -> Self {
        Self {
            found: counters.found,
            filtered: counters.filtered,
            analyzed: counters.analyzed,
            failed: counters.failed,
            results_created: counters.results_created,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunAccountDto {
    pub account_id: i64,
    pub account_name: String,
    pub status: RunAccountStatus,
    pub counters: CountersDto,
    pub error_kind: Option<ErrorKind>,
    pub error_message: Option<String>,
}

impl From<&RunAccountView> for RunAccountDto {
    fn from(view: &RunAccountView) -> Self {
        Self {
            account_id: view.result.account_id,
            account_name: view.account_name.clone(),
            status: view.result.status,
            counters: (&view.result.counters).into(),
            error_kind: view.result.error_kind,
            error_message: view.result.error_message.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunDto {
    pub id: i64,
    pub trigger: RunTrigger,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: RunStatus,
    pub accounts_processed: u32,
    pub counters: CountersDto,
    pub error_summary: Option<String>,
    pub accounts: Vec<RunAccountDto>,
}

impl From<&RunRecord> for RunDto {
    fn from(run: &RunRecord) -> Self {
        Self {
            id: run.id,
            trigger: run.trigger,
            started_at: run.started_at,
            finished_at: run.finished_at,
            status: run.status,
            accounts_processed: run.accounts_processed,
            counters: (&run.counters).into(),
            error_summary: run.error_summary.clone(),
            accounts: run.accounts.iter().map(RunAccountDto::from).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use academic_core::domain::RunAccountResult;
    use chrono::TimeZone;
    use serde_json::json;

    fn sample_run() -> RunRecord {
        RunRecord {
            id: 7,
            trigger: RunTrigger::Manual,
            started_at: Utc.with_ymd_and_hms(2026, 9, 28, 9, 0, 0).unwrap(),
            finished_at: Some(Utc.with_ymd_and_hms(2026, 9, 28, 9, 1, 0).unwrap()),
            status: RunStatus::Partial,
            accounts_processed: 2,
            counters: Counters {
                found: 5,
                filtered: 1,
                analyzed: 3,
                failed: 1,
                results_created: 2,
            },
            error_summary: Some("1 of 2 accounts failed".into()),
            accounts: vec![RunAccountView {
                account_name: "UV IMAP".into(),
                result: RunAccountResult {
                    account_id: 3,
                    status: RunAccountStatus::Failed,
                    counters: Counters::default(),
                    error_kind: Some(ErrorKind::Auth),
                    error_message: Some("Authentication failed: bad".into()),
                },
            }],
        }
    }

    #[test]
    fn run_dto_matches_the_typescript_shape() {
        let dto = RunDto::from(&sample_run());
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            json!({
                "id": 7,
                "trigger": "manual",
                "startedAt": "2026-09-28T09:00:00Z",
                "finishedAt": "2026-09-28T09:01:00Z",
                "status": "partial",
                "accountsProcessed": 2,
                "counters": {
                    "found": 5, "filtered": 1, "analyzed": 3, "failed": 1, "resultsCreated": 2
                },
                "errorSummary": "1 of 2 accounts failed",
                "accounts": [{
                    "accountId": 3,
                    "accountName": "UV IMAP",
                    "status": "failed",
                    "counters": {
                        "found": 0, "filtered": 0, "analyzed": 0, "failed": 0, "resultsCreated": 0
                    },
                    "errorKind": "auth",
                    "errorMessage": "Authentication failed: bad"
                }]
            })
        );
    }

    #[test]
    fn unfinished_run_serializes_nulls() {
        let mut run = sample_run();
        run.finished_at = None;
        run.error_summary = None;
        run.accounts.clear();
        let value = serde_json::to_value(RunDto::from(&run)).unwrap();
        assert_eq!(value["finishedAt"], serde_json::Value::Null);
        assert_eq!(value["errorSummary"], serde_json::Value::Null);
        assert_eq!(value["accounts"], json!([]));
    }
}
