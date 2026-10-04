//! Connection setup and migration behavior.

use academic_core::ports::{
    AccountRepository, CatalogRepository, ResultRepository, UniversityRepository,
};
use academic_infra::db::Database;
use rusqlite::Connection;

const MIGRATION_0001: &str = include_str!("../migrations/0001_init.sql");

/// A database as Phase 1 left it: schema version 1, a manual university and course,
/// an account defaulting to that university and a result linked to both.
fn phase_one_database(path: &std::path::Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(MIGRATION_0001).unwrap();
    conn.execute_batch(
        "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
         INSERT INTO schema_migrations VALUES (1, '2026-09-28T00:00:00Z');
         INSERT INTO universities (id, name, created_at) VALUES (1, 'UV', '2026-09-28T00:00:00Z');
         INSERT INTO courses (id, university_id, name, aliases, created_at)
           VALUES (1, 1, 'Algebra', '[]', '2026-09-28T00:00:00Z');
         INSERT INTO accounts (id, name, email_address, provider_type, config, folders,
           default_university_id, initial_sync_days, enabled, created_at, updated_at)
           VALUES (1, 'UV mail', 'me@uv.es', 'imap',
             '{\"host\":\"imap.uv.es\",\"port\":993,\"username\":\"me\"}', '[\"INBOX\"]',
             1, 14, 1, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z');
         INSERT INTO runs (id, trigger, started_at, status) VALUES (1, 'manual', '2026-09-28T00:00:00Z', 'success');
         INSERT INTO processed_messages (id, account_id, folder, provider_message_id, content_hash,
           received_at, outcome, run_id, processed_at)
           VALUES (1, 1, 'INBOX', '42', 'h', '2026-09-28T00:00:00Z', 'analyzed', 1, '2026-09-28T00:00:00Z');
         INSERT INTO ai_results (id, account_id, processed_message_id, run_id, detected_at, received_at,
           subject, sender_email, university_id, course_id, category, priority, action_required,
           possible_change, model_provider, model_name)
           VALUES (1, 1, 1, 1, '2026-09-28T00:00:00Z', '2026-09-28T00:00:00Z', 'Exam', 'p@uv.es',
             1, 1, 'exam', 'high', 1, 0, 'ollama', 'qwen3:8b');",
    )
    .unwrap();
}

#[test]
fn migrations_are_idempotent_on_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");

    {
        let db = Database::open(&path).unwrap();
        drop(db);
    }
    // Reopening an already-migrated database file must succeed without error...
    let db = Database::open(&path).unwrap();
    drop(db);
    // ...as many times as the app is restarted.
    let db = Database::open(&path).unwrap();
    drop(db);
}

#[test]
fn open_in_memory_starts_with_an_empty_catalog() {
    let db = Database::open_in_memory().unwrap();
    let catalog = db.catalog().unwrap();
    assert!(catalog.universities.is_empty());
    assert!(catalog.courses.is_empty());
}

#[test]
fn upgrading_a_phase_one_database_discards_the_manual_catalog_but_keeps_mail_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    phase_one_database(&path);

    let db = Database::open(&path).unwrap();

    assert!(db.list_universities().unwrap().is_empty());
    assert!(db.catalog().unwrap().courses.is_empty());
    let account = AccountRepository::get(&db, 1).unwrap().unwrap();
    assert_eq!(account.default_university_id, None);
    let result = ResultRepository::get(&db, 1).unwrap().unwrap();
    assert_eq!(result.university_name, None);
    assert_eq!(result.course_name, None);
}
