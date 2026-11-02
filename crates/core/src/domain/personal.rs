use chrono::{DateTime, NaiveDate, Utc};

/// A task the user created themselves for a calendar day. Not tied to any platform item.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonalTask {
    pub id: i64,
    pub title: String,
    /// The day the task belongs to: a local calendar date, never an instant.
    pub date: NaiveDate,
    pub note: Option<String>,
    pub done: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// The user-editable fields of a [`PersonalTask`].
#[derive(Debug, Clone, PartialEq)]
pub struct PersonalTaskDraft {
    pub title: String,
    pub date: NaiveDate,
    pub note: Option<String>,
}
