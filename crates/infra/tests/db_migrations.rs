//! Connection setup and migration behavior.

use academic_core::ports::CatalogRepository;
use academic_infra::db::Database;

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
