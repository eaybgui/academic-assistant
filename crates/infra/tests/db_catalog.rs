//! `CatalogRepository` behavior: universities, courses, aliases and cascades.

#[path = "support/mod.rs"]
mod support;

use academic_core::ports::{AccountRepository, CatalogRepository, RepoError};
use academic_infra::db::Database;
use chrono::Utc;
use support::imap_draft;

#[test]
fn add_university_then_catalog_lists_it() {
    let db = Database::open_in_memory().unwrap();
    let uni = db
        .add_university("Universitat de Valencia", Utc::now())
        .unwrap();

    let catalog = db.catalog().unwrap();
    assert_eq!(catalog.universities, vec![uni]);
}

#[test]
fn duplicate_university_name_is_conflict() {
    let db = Database::open_in_memory().unwrap();
    db.add_university("UV", Utc::now()).unwrap();

    let err = db.add_university("UV", Utc::now()).unwrap_err();
    assert!(matches!(err, RepoError::Conflict(_)));
}

#[test]
fn rename_university_updates_its_name() {
    let db = Database::open_in_memory().unwrap();
    let uni = db.add_university("UV", Utc::now()).unwrap();

    let renamed = db
        .rename_university(uni.id, "Universitat de Valencia")
        .unwrap();
    assert_eq!(renamed.name, "Universitat de Valencia");
}

#[test]
fn rename_missing_university_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db.rename_university(999, "x").unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn delete_missing_university_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db.delete_university(999).unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn add_course_on_missing_university_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db
        .add_course(999, "Programacion", &[], Utc::now())
        .unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn course_aliases_round_trip() {
    let db = Database::open_in_memory().unwrap();
    let uni = db.add_university("UV", Utc::now()).unwrap();
    let aliases = vec!["PROG101".to_string(), "Programacion I".to_string()];

    let course = db
        .add_course(uni.id, "Programacion", &aliases, Utc::now())
        .unwrap();
    assert_eq!(course.aliases, aliases);

    let catalog = db.catalog().unwrap();
    assert_eq!(catalog.courses[0].aliases, aliases);
}

#[test]
fn duplicate_course_name_within_university_is_conflict() {
    let db = Database::open_in_memory().unwrap();
    let uni = db.add_university("UV", Utc::now()).unwrap();
    db.add_course(uni.id, "Programacion", &[], Utc::now())
        .unwrap();

    let err = db
        .add_course(uni.id, "Programacion", &[], Utc::now())
        .unwrap_err();
    assert!(matches!(err, RepoError::Conflict(_)));
}

#[test]
fn same_course_name_is_allowed_in_different_universities() {
    let db = Database::open_in_memory().unwrap();
    let uv = db.add_university("UV", Utc::now()).unwrap();
    let uah = db.add_university("UAH", Utc::now()).unwrap();

    db.add_course(uv.id, "Calculo I", &[], Utc::now()).unwrap();
    let course = db.add_course(uah.id, "Calculo I", &[], Utc::now()).unwrap();
    assert_eq!(course.name, "Calculo I");
}

#[test]
fn update_course_replaces_name_and_aliases() {
    let db = Database::open_in_memory().unwrap();
    let uni = db.add_university("UV", Utc::now()).unwrap();
    let course = db
        .add_course(uni.id, "Programacion", &[], Utc::now())
        .unwrap();

    let updated = db
        .update_course(course.id, "Programacion I", &["PROG1".to_string()])
        .unwrap();
    assert_eq!(updated.name, "Programacion I");
    assert_eq!(updated.aliases, vec!["PROG1".to_string()]);
    assert_eq!(updated.university_id, uni.id);
}

#[test]
fn update_missing_course_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db.update_course(999, "x", &[]).unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn delete_course_removes_it() {
    let db = Database::open_in_memory().unwrap();
    let uni = db.add_university("UV", Utc::now()).unwrap();
    let course = db
        .add_course(uni.id, "Programacion", &[], Utc::now())
        .unwrap();

    db.delete_course(course.id).unwrap();

    let catalog = db.catalog().unwrap();
    assert!(catalog.courses.is_empty());
}

#[test]
fn delete_missing_course_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db.delete_course(999).unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn deleting_university_cascades_courses_and_nulls_account_default() {
    let db = Database::open_in_memory().unwrap();
    let uni = db.add_university("UV", Utc::now()).unwrap();
    db.add_course(uni.id, "Programacion", &[], Utc::now())
        .unwrap();

    let mut draft = imap_draft("acc");
    draft.default_university_id = Some(uni.id);
    let account = db.insert(&draft, Utc::now()).unwrap();
    assert_eq!(account.default_university_id, Some(uni.id));

    db.delete_university(uni.id).unwrap();

    let catalog = db.catalog().unwrap();
    assert!(catalog.universities.is_empty());
    assert!(catalog.courses.is_empty());

    let refreshed = db.get(account.id).unwrap().unwrap();
    assert_eq!(refreshed.default_university_id, None);
}
