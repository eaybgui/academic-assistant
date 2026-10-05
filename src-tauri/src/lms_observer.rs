//! `TauriLmsSyncObserver`: forwards LMS sync progress to the frontend as Tauri
//! events (`lms-sync:progress` / `lms-sync:finished`).
//!
//! Logs only university ids, stages and counts, never item titles or notes.

use academic_core::domain::UniversityId;
use academic_core::ports::{LmsSyncObserver, LmsSyncProgress, LmsSyncResult, LmsSyncStage};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::dto::LmsSyncResultDto;

pub const EVENT_LMS_SYNC_PROGRESS: &str = "lms-sync:progress";
pub const EVENT_LMS_SYNC_FINISHED: &str = "lms-sync:finished";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsSyncProgressPayload {
    pub university_id: UniversityId,
    pub university_name: String,
    pub stage: LmsSyncStage,
}

impl From<&LmsSyncProgress> for LmsSyncProgressPayload {
    fn from(progress: &LmsSyncProgress) -> Self {
        Self {
            university_id: progress.university_id,
            university_name: progress.university_name.clone(),
            stage: progress.stage,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LmsSyncFinishedPayload {
    pub results: Vec<LmsSyncResultDto>,
}

impl From<&[LmsSyncResult]> for LmsSyncFinishedPayload {
    fn from(results: &[LmsSyncResult]) -> Self {
        Self {
            results: results.iter().map(LmsSyncResultDto::from).collect(),
        }
    }
}

pub struct TauriLmsSyncObserver {
    app: AppHandle,
}

impl TauriLmsSyncObserver {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn emit<P: Serialize + Clone>(&self, event: &str, payload: P) {
        if let Err(error) = self.app.emit(event, payload) {
            tracing::warn!(event, %error, "failed to emit event");
        }
    }
}

impl LmsSyncObserver for TauriLmsSyncObserver {
    fn progress(&self, progress: &LmsSyncProgress) {
        if matches!(
            progress.stage,
            LmsSyncStage::Done | LmsSyncStage::Failed | LmsSyncStage::LoginRequired
        ) {
            tracing::info!(
                university_id = progress.university_id,
                stage = ?progress.stage,
                "university synced"
            );
        }
        self.emit(
            EVENT_LMS_SYNC_PROGRESS,
            LmsSyncProgressPayload::from(progress),
        );
    }

    fn finished(&self, results: &[LmsSyncResult]) {
        tracing::info!(universities = results.len(), "lms sync finished");
        self.emit(
            EVENT_LMS_SYNC_FINISHED,
            LmsSyncFinishedPayload::from(results),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use academic_core::domain::SyncOutcome;
    use serde_json::json;

    #[test]
    fn progress_payload_uses_camel_case_and_snake_case_stage() {
        let payload = LmsSyncProgressPayload::from(&LmsSyncProgress {
            university_id: 3,
            university_name: "UV".into(),
            stage: LmsSyncStage::LoginRequired,
        });
        assert_eq!(
            serde_json::to_value(payload).unwrap(),
            json!({
                "universityId": 3,
                "universityName": "UV",
                "stage": "login_required"
            })
        );
    }

    #[test]
    fn finished_payload_wraps_the_result_dtos() {
        let results = [LmsSyncResult {
            university_id: 3,
            university_name: "UV".into(),
            outcome: SyncOutcome::Success,
            new_items: 4,
            due_changed: 1,
            removed: 0,
            error: None,
        }];
        assert_eq!(
            serde_json::to_value(LmsSyncFinishedPayload::from(&results[..])).unwrap(),
            json!({
                "results": [{
                    "universityId": 3,
                    "universityName": "UV",
                    "outcome": "success",
                    "newItems": 4,
                    "dueChanged": 1,
                    "removed": 0,
                    "error": null
                }]
            })
        );
        assert_eq!(
            serde_json::to_value(LmsSyncFinishedPayload::from(&[][..])).unwrap(),
            json!({ "results": [] })
        );
    }
}
