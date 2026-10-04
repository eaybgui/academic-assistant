//! Shared fixture builders for the analyzer's unit tests (`questions`, `assemble` and
//! `analyzer` itself). Only compiled under `#[cfg(test)]` (see the `mod` declaration in
//! `analyzer/mod.rs`).

use chrono::{NaiveDate, TimeZone, Utc};

use crate::domain::{
    Account, Address, Catalog, Course, DateCandidate, Email, EmailBody, ProviderConfig, University,
    DEFAULT_IMAP_FOLDER,
};

pub fn email() -> Email {
    Email {
        account_id: 1,
        folder: DEFAULT_IMAP_FOLDER.to_string(),
        provider_message_id: "1".to_string(),
        message_id_header: Some("<msg-1@example.edu>".to_string()),
        sender: Address {
            name: Some("Prof. Ada Lovelace".to_string()),
            email: "ada@example.edu".to_string(),
        },
        recipients: vec![],
        subject: "Reminder: exam next week".to_string(),
        received_at: Utc.with_ymd_and_hms(2026, 9, 28, 9, 14, 0).unwrap(),
        body: EmailBody::Text("raw body".to_string()),
        source_url: None,
    }
}

pub fn account(default_university_id: Option<i64>) -> Account {
    Account {
        id: 1,
        name: "Personal".to_string(),
        email_address: "student@example.edu".to_string(),
        provider: ProviderConfig::Imap {
            host: "imap.example.edu".to_string(),
            port: 993,
            username: "student@example.edu".to_string(),
        },
        folders: vec![DEFAULT_IMAP_FOLDER.to_string()],
        default_university_id,
        initial_sync_days: 14,
        enabled: true,
        created_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
        updated_at: Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap(),
    }
}

pub fn catalog(universities: Vec<University>, courses: Vec<Course>) -> Catalog {
    Catalog {
        universities,
        courses,
    }
}

pub fn university(id: i64, name: &str) -> University {
    University {
        id,
        name: name.to_string(),
    }
}

pub fn course(id: i64, university_id: i64, name: &str, aliases: Vec<&str>) -> Course {
    Course {
        id,
        university_id,
        name: name.to_string(),
        aliases: aliases.into_iter().map(str::to_string).collect(),
    }
}

pub fn date_candidate(span: &str, date: NaiveDate) -> DateCandidate {
    DateCandidate {
        span: span.to_string(),
        date,
        time: None,
    }
}
