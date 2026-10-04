use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::account::AccountId;
use super::analysis::{AIAnalysis, Category, Priority, ResultDate};
use super::enum_str;
use super::run::RunId;

/// What happened to a fetched email during pipeline processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageOutcome {
    Filtered,
    Irrelevant,
    Analyzed,
    Failed,
}

enum_str!(MessageOutcome {
    Filtered => "filtered",
    Irrelevant => "irrelevant",
    Analyzed => "analyzed",
    Failed => "failed",
});

/// Why a deterministic filter discarded an email before analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilterReason {
    Duplicate,
    BlockedSender,
    BlockedDomain,
    OwnMessage,
    Empty,
}

enum_str!(FilterReason {
    Duplicate => "duplicate",
    BlockedSender => "blocked_sender",
    BlockedDomain => "blocked_domain",
    OwnMessage => "own_message",
    Empty => "empty",
});

/// Dedup/audit record written for every email the pipeline looks at, regardless of
/// outcome (used to skip already-seen messages on the next sync).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessedMessage {
    pub account_id: AccountId,
    pub folder: String,
    pub provider_message_id: String,
    pub message_id_header: Option<String>,
    pub content_hash: String,
    pub received_at: DateTime<Utc>,
    pub outcome: MessageOutcome,
    pub reason: Option<String>,
    pub run_id: RunId,
    pub processed_at: DateTime<Utc>,
}

/// A newly analyzed, relevant email ready to be persisted as a [`StoredResult`].
#[derive(Debug, Clone, PartialEq)]
pub struct NewResult {
    pub account_id: AccountId,
    pub run_id: RunId,
    pub detected_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub subject: String,
    pub sender_name: Option<String>,
    pub sender_email: String,
    pub source_url: Option<String>,
    pub analysis: AIAnalysis,
}

/// A persisted, analyzed result as read back from storage for display.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredResult {
    pub id: i64,
    pub account_id: AccountId,
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
    pub dates: Vec<ResultDate>,
    pub received_at: DateTime<Utc>,
    pub detected_at: DateTime<Utc>,
    pub source_url: Option<String>,
    pub message_id_header: Option<String>,
    pub seen: bool,
    pub seen_at: Option<DateTime<Utc>>,
}

/// Ordering applied to a [`ResultQuery`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultSort {
    Upcoming,
    Priority,
    Received,
}

enum_str!(ResultSort {
    Upcoming => "upcoming",
    Priority => "priority",
    Received => "received",
});

/// Filter and pagination parameters for [`crate::ports::ResultRepository::query`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultQuery {
    pub search: Option<String>,
    pub seen: Option<bool>,
    pub category: Option<Category>,
    pub account_id: Option<AccountId>,
    pub course_id: Option<i64>,
    pub sort: ResultSort,
    pub limit: u32,
    pub offset: u32,
}

/// A page of items plus the total count of matches (ignoring `limit`/`offset`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: u64,
}

/// Unseen-result counts by category, shown on the dashboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DashboardCounts {
    pub new_total: u32,
    pub tasks: u32,
    pub exams: u32,
    pub events: u32,
    pub changes_and_notices: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn message_outcome_round_trips() {
        for o in [
            MessageOutcome::Filtered,
            MessageOutcome::Irrelevant,
            MessageOutcome::Analyzed,
            MessageOutcome::Failed,
        ] {
            assert_eq!(MessageOutcome::from_str(o.as_str()).unwrap(), o);
        }
    }

    #[test]
    fn filter_reason_round_trips() {
        for r in [
            FilterReason::Duplicate,
            FilterReason::BlockedSender,
            FilterReason::BlockedDomain,
            FilterReason::OwnMessage,
            FilterReason::Empty,
        ] {
            assert_eq!(FilterReason::from_str(r.as_str()).unwrap(), r);
        }
    }

    #[test]
    fn result_sort_round_trips() {
        for s in [
            ResultSort::Upcoming,
            ResultSort::Priority,
            ResultSort::Received,
        ] {
            assert_eq!(ResultSort::from_str(s.as_str()).unwrap(), s);
        }
    }

    #[test]
    fn dashboard_counts_default_is_zero() {
        assert_eq!(
            DashboardCounts::default(),
            DashboardCounts {
                new_total: 0,
                tasks: 0,
                exams: 0,
                events: 0,
                changes_and_notices: 0
            }
        );
    }
}
