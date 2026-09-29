//! `TauriRunObserver`: forwards pipeline progress to the frontend as Tauri events
//! (`run:progress` / `run:finished`, spec §10.3).

use academic_core::domain::{RunId, RunProgress, RunStage, RunStatus};
use academic_core::ports::RunObserver;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

pub const EVENT_RUN_PROGRESS: &str = "run:progress";
pub const EVENT_RUN_FINISHED: &str = "run:finished";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunProgressPayload {
    pub run_id: RunId,
    pub account_id: i64,
    pub account_name: String,
    pub stage: RunStage,
    pub processed: u32,
    pub total: u32,
}

impl From<&RunProgress> for RunProgressPayload {
    fn from(progress: &RunProgress) -> Self {
        Self {
            run_id: progress.run_id,
            account_id: progress.account_id,
            account_name: progress.account_name.clone(),
            stage: progress.stage,
            processed: progress.processed,
            total: progress.total,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFinishedPayload {
    pub run_id: RunId,
    pub status: RunStatus,
}

pub struct TauriRunObserver {
    app: AppHandle,
}

impl TauriRunObserver {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }

    fn emit<P: Serialize + Clone>(&self, event: &str, payload: P) {
        if let Err(error) = self.app.emit(event, payload) {
            tracing::warn!(event, %error, "failed to emit event");
        }
    }
}

impl RunObserver for TauriRunObserver {
    fn progress(&self, progress: &RunProgress) {
        if matches!(progress.stage, RunStage::Done | RunStage::Failed) {
            tracing::info!(
                run_id = progress.run_id,
                account_id = progress.account_id,
                stage = ?progress.stage,
                "account processed"
            );
        }
        self.emit(EVENT_RUN_PROGRESS, RunProgressPayload::from(progress));
    }

    fn finished(&self, run_id: RunId, status: RunStatus) {
        tracing::info!(run_id, status = ?status, "check finished");
        self.emit(EVENT_RUN_FINISHED, RunFinishedPayload { run_id, status });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn progress_payload_uses_camel_case_and_snake_case_stage() {
        let payload = RunProgressPayload::from(&RunProgress {
            run_id: 4,
            account_id: 2,
            account_name: "UV IMAP".into(),
            stage: RunStage::Analyzing,
            processed: 12,
            total: 40,
        });
        assert_eq!(
            serde_json::to_value(payload).unwrap(),
            json!({
                "runId": 4,
                "accountId": 2,
                "accountName": "UV IMAP",
                "stage": "analyzing",
                "processed": 12,
                "total": 40
            })
        );
    }

    #[test]
    fn finished_payload_serializes_run_id_and_status() {
        let payload = RunFinishedPayload {
            run_id: 4,
            status: RunStatus::Partial,
        };
        assert_eq!(
            serde_json::to_value(payload).unwrap(),
            json!({ "runId": 4, "status": "partial" })
        );
    }
}
