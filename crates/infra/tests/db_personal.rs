use academic_core::domain::PersonalTaskDraft;
use academic_core::ports::{PersonalTaskRepository, RepoError};
use academic_infra::db::Database;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use rusqlite::Connection;

fn at(day: u32, hour: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, day, hour, 0, 0).unwrap()
}

fn day(d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, d).unwrap()
}

fn draft(title: &str, date: NaiveDate, note: Option<&str>) -> PersonalTaskDraft {
    PersonalTaskDraft {
        title: title.into(),
        date,
        note: note.map(str::to_string),
    }
}

#[test]
fn insert_then_list_orders_by_date_then_id() {
    let db = Database::open_in_memory().unwrap();
    let now = at(3, 9);
    let late = db
        .insert_personal_task(&draft("Pay the rent", day(10), None), now)
        .unwrap();
    let first = db
        .insert_personal_task(
            &draft("Go to the doctor", day(5), Some("Bring the card")),
            now,
        )
        .unwrap();
    let second = db
        .insert_personal_task(&draft("Buy a notebook", day(5), None), now)
        .unwrap();

    for task in [&late, &first, &second] {
        assert!(!task.done);
        assert_eq!(task.created_at, now);
        assert_eq!(task.updated_at, now);
    }
    assert_eq!(first.note.as_deref(), Some("Bring the card"));

    let ids: Vec<i64> = db
        .list_personal_tasks()
        .unwrap()
        .iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(ids, vec![first.id, second.id, late.id]);
}

#[test]
fn update_replaces_fields_and_keeps_done() {
    let db = Database::open_in_memory().unwrap();
    let (t0, t1, t2) = (at(3, 9), at(3, 10), at(3, 11));
    let task = db
        .insert_personal_task(
            &draft("Go to the doctor", day(5), Some("Bring the card")),
            t0,
        )
        .unwrap();
    db.set_personal_task_done(task.id, true, t1).unwrap();

    let updated = db
        .update_personal_task(task.id, &draft("Call the dentist", day(8), None), t2)
        .unwrap();

    assert_eq!(updated.title, "Call the dentist");
    assert_eq!(updated.date, day(8));
    assert_eq!(updated.note, None);
    assert!(updated.done);
    assert_eq!(updated.updated_at, t2);
    assert_eq!(updated.created_at, t0);
    assert_eq!(db.list_personal_tasks().unwrap(), vec![updated]);
}

#[test]
fn set_done_toggles_and_stamps_updated_at() {
    let db = Database::open_in_memory().unwrap();
    let (t0, t1, t2) = (at(3, 9), at(3, 10), at(3, 11));
    let task = db
        .insert_personal_task(&draft("Go to the doctor", day(5), None), t0)
        .unwrap();

    let done = db.set_personal_task_done(task.id, true, t1).unwrap();
    assert!(done.done);
    assert_eq!(done.updated_at, t1);
    assert_eq!(done.created_at, t0);

    let undone = db.set_personal_task_done(task.id, false, t2).unwrap();
    assert!(!undone.done);
    assert_eq!(undone.updated_at, t2);
}

#[test]
fn accented_title_round_trips() {
    let db = Database::open_in_memory().unwrap();
    for title in ["Recoger el DNI — Ñandú", "Lavar el coche — Álvaro"] {
        db.insert_personal_task(&draft(title, day(5), Some("Nota — ñ")), at(3, 9))
            .unwrap();
    }

    let tasks = db.list_personal_tasks().unwrap();
    assert_eq!(tasks[0].title, "Recoger el DNI — Ñandú");
    assert_eq!(tasks[1].title, "Lavar el coche — Álvaro");
    assert_eq!(tasks[0].note.as_deref(), Some("Nota — ñ"));
}

#[test]
fn missing_id_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    let now = at(3, 9);

    assert!(matches!(
        db.update_personal_task(999, &draft("Go to the doctor", day(5), None), now),
        Err(RepoError::NotFound)
    ));
    assert!(matches!(
        db.set_personal_task_done(999, true, now),
        Err(RepoError::NotFound)
    ));
    assert!(matches!(
        db.delete_personal_task(999),
        Err(RepoError::NotFound)
    ));
}

#[test]
fn delete_removes_the_task() {
    let db = Database::open_in_memory().unwrap();
    let task = db
        .insert_personal_task(&draft("Go to the doctor", day(5), None), at(3, 9))
        .unwrap();

    db.delete_personal_task(task.id).unwrap();

    assert!(db.list_personal_tasks().unwrap().is_empty());
}

#[test]
fn migration_applies_on_existing_database() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("app.db");
    {
        // A database as Phase 2 left it: migrations 1-3 applied, no personal tasks table.
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations (version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);",
        )
        .unwrap();
        for (version, sql) in [
            (1, include_str!("../migrations/0001_init.sql")),
            (2, include_str!("../migrations/0002_lms.sql")),
            (3, include_str!("../migrations/0003_lms_items.sql")),
        ] {
            conn.execute_batch(sql).unwrap();
            conn.execute(
                "INSERT INTO schema_migrations VALUES (?1, '2026-10-01T00:00:00Z')",
                [version],
            )
            .unwrap();
        }
    }

    let db = Database::open(&path).unwrap();

    assert_eq!(db.list_personal_tasks().unwrap(), vec![]);
}
