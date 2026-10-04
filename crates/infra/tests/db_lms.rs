//! `UniversityRepository`, `CourseRepository` and the catalog projection.

use academic_core::domain::{LmsPlatform, LoginState, RemoteCourse, SyncOutcome};
use academic_core::ports::{CatalogRepository, CourseRepository, RepoError, UniversityRepository};
use academic_infra::db::Database;
use chrono::Utc;

fn remote(id: &str, name: &str, short: Option<&str>) -> RemoteCourse {
    RemoteCourse {
        external_id: id.into(),
        name: name.into(),
        short_name: short.map(str::to_string),
        url: Some(format!("https://x.es/course/{id}")),
    }
}

fn uv(db: &Database) -> i64 {
    db.insert_university(
        "UV",
        LmsPlatform::Moodle,
        "https://aulavirtual.uv.es",
        Utc::now(),
    )
    .unwrap()
    .id
}

#[test]
fn inserted_university_starts_never_signed_in() {
    let db = Database::open_in_memory().unwrap();
    let u = db
        .insert_university(
            "UV",
            LmsPlatform::Moodle,
            "https://aulavirtual.uv.es",
            Utc::now(),
        )
        .unwrap();
    assert_eq!(u.login_state, LoginState::Never);
    assert_eq!(u.account_label, None);
    assert_eq!(db.list_universities().unwrap(), vec![u.clone()]);
    assert_eq!(db.get_university(u.id).unwrap(), Some(u));
}

#[test]
fn duplicate_name_or_url_is_a_conflict() {
    let db = Database::open_in_memory().unwrap();
    uv(&db);
    let same_name = db.insert_university(
        "UV",
        LmsPlatform::Blackboard,
        "https://other.es",
        Utc::now(),
    );
    assert!(matches!(same_name, Err(RepoError::Conflict(_))));
    let same_url = db.insert_university(
        "Valencia",
        LmsPlatform::Moodle,
        "https://aulavirtual.uv.es",
        Utc::now(),
    );
    assert!(matches!(same_url, Err(RepoError::Conflict(_))));
}

#[test]
fn set_login_and_record_sync_update_the_row() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    let u = db.set_login(id, LoginState::Ok, Some("Ana Pérez")).unwrap();
    assert_eq!(
        (u.login_state, u.account_label.as_deref()),
        (LoginState::Ok, Some("Ana Pérez"))
    );
    let at = Utc::now();
    db.record_sync(id, at, SyncOutcome::Failed, Some("offline"))
        .unwrap();
    let u = db.get_university(id).unwrap().unwrap();
    assert_eq!(u.last_sync_outcome, Some(SyncOutcome::Failed));
    assert_eq!(u.last_sync_error.as_deref(), Some("offline"));
    assert!(u.last_sync_at.is_some());
}

#[test]
fn missing_university_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    assert!(matches!(
        db.rename_university(9, "x"),
        Err(RepoError::NotFound)
    ));
    assert!(matches!(db.delete_university(9), Err(RepoError::NotFound)));
    assert!(matches!(
        db.set_login(9, LoginState::Ok, None),
        Err(RepoError::NotFound)
    ));
    assert!(matches!(
        db.upsert_remote_courses(9, &[remote("1", "A", None)], Utc::now()),
        Err(RepoError::NotFound)
    ));
}

#[test]
fn upsert_inserts_updates_and_flags_missing_courses() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    db.upsert_remote_courses(
        id,
        &[
            remote("1", "Algebra", Some("ALG")),
            remote("2", "Calculus", None),
        ],
        Utc::now(),
    )
    .unwrap();
    let courses = db.list_courses(id).unwrap();
    assert_eq!(courses.len(), 2);
    assert!(courses
        .iter()
        .all(|c| !c.tracked && !c.reviewed && c.available));

    db.upsert_remote_courses(id, &[remote("1", "Algebra II", Some("ALG2"))], Utc::now())
        .unwrap();
    let courses = db.list_courses(id).unwrap();
    let algebra = courses.iter().find(|c| c.external_id == "1").unwrap();
    assert_eq!(
        (algebra.name.as_str(), algebra.short_name.as_deref()),
        ("Algebra II", Some("ALG2"))
    );
    assert!(algebra.available);
    assert!(
        !courses
            .iter()
            .find(|c| c.external_id == "2")
            .unwrap()
            .available
    );

    db.upsert_remote_courses(
        id,
        &[
            remote("1", "Algebra II", None),
            remote("2", "Calculus", None),
        ],
        Utc::now(),
    )
    .unwrap();
    assert!(db.list_courses(id).unwrap().iter().all(|c| c.available));
}

#[test]
fn set_tracked_courses_tracks_exactly_the_given_ids_and_reviews_all() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    db.upsert_remote_courses(
        id,
        &[remote("1", "Algebra", None), remote("2", "Calculus", None)],
        Utc::now(),
    )
    .unwrap();
    let ids: Vec<i64> = db.list_courses(id).unwrap().iter().map(|c| c.id).collect();

    let courses = db.set_tracked_courses(id, &[ids[0]]).unwrap();
    assert_eq!(courses.iter().filter(|c| c.tracked).count(), 1);
    assert!(courses.iter().all(|c| c.reviewed));

    let other = db
        .insert_university(
            "UAH",
            LmsPlatform::Blackboard,
            "https://uah.blackboard.com",
            Utc::now(),
        )
        .unwrap();
    assert!(matches!(
        db.set_tracked_courses(other.id, &[ids[1]]),
        Err(RepoError::NotFound)
    ));
    assert_eq!(
        db.list_courses(id)
            .unwrap()
            .iter()
            .filter(|c| c.tracked)
            .count(),
        1,
        "no partial change"
    );
}

#[test]
fn catalog_lists_tracked_available_courses_with_short_name_alias() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    db.upsert_remote_courses(
        id,
        &[
            remote("1", "Algebra", Some("ALG")),
            remote("2", "Calculus", Some("Calculus")),
            remote("3", "Physics", None),
        ],
        Utc::now(),
    )
    .unwrap();
    let ids: Vec<i64> = db.list_courses(id).unwrap().iter().map(|c| c.id).collect();
    db.set_tracked_courses(id, &ids[..2]).unwrap();

    let catalog = db.catalog().unwrap();
    assert_eq!(catalog.universities.len(), 1);
    assert_eq!(catalog.courses.len(), 2);
    let algebra = catalog
        .courses
        .iter()
        .find(|c| c.name == "Algebra")
        .unwrap();
    assert_eq!(algebra.aliases, vec!["ALG"]);
    let calculus = catalog
        .courses
        .iter()
        .find(|c| c.name == "Calculus")
        .unwrap();
    assert!(calculus.aliases.is_empty(), "same as the name adds nothing");
}

#[test]
fn deleting_a_university_removes_its_courses() {
    let db = Database::open_in_memory().unwrap();
    let id = uv(&db);
    db.upsert_remote_courses(id, &[remote("1", "Algebra", None)], Utc::now())
        .unwrap();
    db.delete_university(id).unwrap();
    assert!(db.list_universities().unwrap().is_empty());
    assert!(db.list_courses(id).unwrap().is_empty());
}
