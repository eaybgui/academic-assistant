use chrono::{NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};

use super::enum_str;

/// Topic classification assigned to a relevant email.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Notice,
    Task,
    Exam,
    Event,
    Change,
    GeneralInfo,
}

enum_str!(Category {
    Notice => "notice",
    Task => "task",
    Exam => "exam",
    Event => "event",
    Change => "change",
    GeneralInfo => "general_info",
});

/// Urgency assigned to a relevant email. Ordered `Low < Medium < High < Urgent`
/// (declaration order), so `Priority` comparisons and sorting work via `derive(Ord)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

enum_str!(Priority {
    Low => "low",
    Medium => "medium",
    High => "high",
    Urgent => "urgent",
});

/// What a [`ResultDate`]/[`DateCandidate`] represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateKind {
    Exam,
    Deadline,
    Event,
    Class,
    Other,
}

enum_str!(DateKind {
    Exam => "exam",
    Deadline => "deadline",
    Event => "event",
    Class => "class",
    Other => "other",
});

/// A date attached to an analyzed result, after the AI model has confirmed and
/// classified one of the extracted [`DateCandidate`]s.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResultDate {
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
    pub kind: DateKind,
}

/// A date mention found in an email's text by [`crate::dates::extract`], offered to
/// the AI model as a candidate to confirm and classify.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateCandidate {
    pub span: String,
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
}

/// Identifies which AI provider and model produced an [`AIAnalysis`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelDescriptor {
    pub provider: String,
    pub model: String,
}

/// The structured outcome of analyzing a relevant email.
#[derive(Debug, Clone, PartialEq)]
pub struct AIAnalysis {
    pub university_id: Option<i64>,
    pub course_id: Option<i64>,
    pub category: Category,
    pub priority: Priority,
    pub action_required: bool,
    pub possible_change: bool,
    pub dates: Vec<ResultDate>,
    pub confidence: Option<f32>,
    pub model: ModelDescriptor,
}

/// Result of running the analyzer on an email: either judged irrelevant (below the
/// relevance threshold) or fully analyzed.
#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisOutcome {
    Irrelevant { p_relevant: f32 },
    Relevant(AIAnalysis),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn category_round_trips() {
        for c in [
            Category::Notice,
            Category::Task,
            Category::Exam,
            Category::Event,
            Category::Change,
            Category::GeneralInfo,
        ] {
            assert_eq!(Category::from_str(c.as_str()).unwrap(), c);
        }
        assert_eq!(Category::GeneralInfo.as_str(), "general_info");
    }

    #[test]
    fn priority_round_trips_and_orders_low_to_urgent() {
        for p in [
            Priority::Low,
            Priority::Medium,
            Priority::High,
            Priority::Urgent,
        ] {
            assert_eq!(Priority::from_str(p.as_str()).unwrap(), p);
        }
        assert!(Priority::Low < Priority::Urgent);
        assert!(Priority::Medium < Priority::High);
    }

    #[test]
    fn date_kind_round_trips() {
        for k in [
            DateKind::Exam,
            DateKind::Deadline,
            DateKind::Event,
            DateKind::Class,
            DateKind::Other,
        ] {
            assert_eq!(DateKind::from_str(k.as_str()).unwrap(), k);
        }
    }
}
