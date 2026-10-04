use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::account::AccountId;
use super::enum_str;

pub type RunId = i64;

/// How a check run was started. Phase 1 only supports manual runs from the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunTrigger {
    Manual,
}

enum_str!(RunTrigger {
    Manual => "manual",
});

/// Overall outcome of a check run across every processed account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Success,
    Partial,
    Failed,
}

enum_str!(RunStatus {
    Running => "running",
    Success => "success",
    Partial => "partial",
    Failed => "failed",
});

/// Outcome of a check run for a single account.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunAccountStatus {
    Success,
    Failed,
}

enum_str!(RunAccountStatus {
    Success => "success",
    Failed => "failed",
});

/// Coarse-grained classification of an error, shared by provider, AI and run errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Auth,
    Connection,
    Protocol,
    Ai,
    Internal,
}

enum_str!(ErrorKind {
    Auth => "auth",
    Connection => "connection",
    Protocol => "protocol",
    Ai => "ai",
    Internal => "internal",
});

/// Message-level counters accumulated while processing one account or an entire run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Counters {
    pub found: u32,
    pub filtered: u32,
    pub analyzed: u32,
    pub failed: u32,
    pub results_created: u32,
}

impl Counters {
    /// Adds `other`'s counts into `self`, field by field.
    pub fn add(&mut self, other: &Counters) {
        self.found += other.found;
        self.filtered += other.filtered;
        self.analyzed += other.analyzed;
        self.failed += other.failed;
        self.results_created += other.results_created;
    }
}

/// Per-account result of a check run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunAccountResult {
    pub account_id: AccountId,
    pub status: RunAccountStatus,
    pub counters: Counters,
    pub error_kind: Option<ErrorKind>,
    pub error_message: Option<String>,
}

/// A [`RunAccountResult`] together with the account's display name, for reporting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunAccountView {
    pub account_name: String,
    pub result: RunAccountResult,
}

/// A full check run record: when it ran, its overall status and the per-account detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunRecord {
    pub id: RunId,
    pub trigger: RunTrigger,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
    pub status: RunStatus,
    pub accounts_processed: u32,
    pub counters: Counters,
    pub error_summary: Option<String>,
    pub accounts: Vec<RunAccountView>,
}

/// Stage a single account's processing is currently in, reported via [`RunProgress`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStage {
    Connecting,
    Fetching,
    Analyzing,
    Done,
    Failed,
}

enum_str!(RunStage {
    Connecting => "connecting",
    Fetching => "fetching",
    Analyzing => "analyzing",
    Done => "done",
    Failed => "failed",
});

/// A live progress update for one account within a running check, emitted to the UI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunProgress {
    pub run_id: RunId,
    pub account_id: AccountId,
    pub account_name: String,
    pub stage: RunStage,
    pub processed: u32,
    pub total: u32,
}

/// Errors that can prevent a check run from starting or completing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RunError {
    #[error("A check is already running")]
    AlreadyRunning,
    #[error("There are no enabled accounts")]
    NoEnabledAccounts,
    #[error("The AI provider is not configured: {0}")]
    AiNotConfigured(String),
    #[error("Storage error: {0}")]
    Storage(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn run_trigger_round_trips() {
        assert_eq!(
            RunTrigger::from_str(RunTrigger::Manual.as_str()).unwrap(),
            RunTrigger::Manual
        );
    }

    #[test]
    fn run_status_round_trips() {
        for s in [
            RunStatus::Running,
            RunStatus::Success,
            RunStatus::Partial,
            RunStatus::Failed,
        ] {
            assert_eq!(RunStatus::from_str(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn run_account_status_round_trips() {
        for s in [RunAccountStatus::Success, RunAccountStatus::Failed] {
            assert_eq!(RunAccountStatus::from_str(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn error_kind_round_trips() {
        for k in [
            ErrorKind::Auth,
            ErrorKind::Connection,
            ErrorKind::Protocol,
            ErrorKind::Ai,
            ErrorKind::Internal,
        ] {
            assert_eq!(ErrorKind::from_str(k.as_str()).unwrap(), k);
        }
    }

    #[test]
    fn run_stage_round_trips() {
        for s in [
            RunStage::Connecting,
            RunStage::Fetching,
            RunStage::Analyzing,
            RunStage::Done,
            RunStage::Failed,
        ] {
            assert_eq!(RunStage::from_str(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn counters_add_sums_each_field() {
        let mut total = Counters {
            found: 1,
            filtered: 2,
            analyzed: 3,
            failed: 4,
            results_created: 5,
        };
        let other = Counters {
            found: 10,
            filtered: 20,
            analyzed: 30,
            failed: 40,
            results_created: 50,
        };
        total.add(&other);
        assert_eq!(
            total,
            Counters {
                found: 11,
                filtered: 22,
                analyzed: 33,
                failed: 44,
                results_created: 55,
            }
        );
    }

    #[test]
    fn counters_default_is_all_zero() {
        assert_eq!(
            Counters::default(),
            Counters {
                found: 0,
                filtered: 0,
                analyzed: 0,
                failed: 0,
                results_created: 0
            }
        );
    }
}
