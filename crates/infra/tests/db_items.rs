use academic_core::domain::{
    Grade, ItemChange, ItemKind, LmsItem, LmsPlatform, PlatformStatus, RemoteCourse, RemoteItem,
    TrackerQuery,
};
use academic_core::lms::{ItemUpdate, NewItem, SyncChanges};
use academic_core::ports::{CourseRepository, LmsItemRepository, RepoError, UniversityRepository};
use academic_infra::db::Database;
use chrono::{DateTime, Duration, TimeZone, Utc};

struct Fixture {
    db: Database,
    university_id: i64,
    /// Tracked course.
    tracked: i64,
    /// Course that is available but not tracked.
    untracked: i64,
}

fn remote_course(id: &str, name: &str, short: Option<&str>) -> RemoteCourse {
    RemoteCourse {
        external_id: id.into(),
        name: name.into(),
        short_name: short.map(str::to_string),
        url: None,
    }
}

fn base_time() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 3, 1, 10, 0, 0).unwrap()
}

fn fixture() -> Fixture {
    let db = Database::open_in_memory().unwrap();
    let university_id = db
        .insert_university(
            "Universidad Demo",
            LmsPlatform::Moodle,
            "https://aula.demo.test",
            base_time(),
        )
        .unwrap()
        .id;
    db.upsert_remote_courses(
        university_id,
        &[
            remote_course("c1", "Álgebra Lineal", Some("ALG")),
            remote_course("c2", "Historia del Arte", None),
        ],
        base_time(),
    )
    .unwrap();
    let courses = db.list_courses(university_id).unwrap();
    let id_of = |external: &str| {
        courses
            .iter()
            .find(|c| c.external_id == external)
            .unwrap()
            .id
    };
    let (tracked, untracked) = (id_of("c1"), id_of("c2"));
    db.set_tracked_courses(university_id, &[tracked]).unwrap();
    Fixture {
        db,
        university_id,
        tracked,
        untracked,
    }
}

fn remote(external_id: &str, title: &str, due_at: Option<DateTime<Utc>>) -> RemoteItem {
    RemoteItem {
        course_external_id: "c1".into(),
        external_id: external_id.into(),
        kind: ItemKind::Assignment,
        title: title.into(),
        due_at,
        url: Some(format!("https://aula.demo.test/item/{external_id}")),
        platform_status: PlatformStatus::NotSubmitted,
        grade: None,
    }
}

fn insert(course_id: i64, remote: RemoteItem, change: Option<ItemChange>) -> NewItem {
    NewItem {
        course_id,
        remote,
        change,
    }
}

fn sync_inserts(f: &Fixture, inserts: Vec<NewItem>, now: DateTime<Utc>) {
    f.db.apply_sync(
        f.university_id,
        &SyncChanges {
            inserts,
            ..SyncChanges::default()
        },
        now,
    )
    .unwrap();
}

fn find(items: &[LmsItem], external_id: &str) -> LmsItem {
    items
        .iter()
        .find(|i| i.external_id == external_id)
        .unwrap()
        .clone()
}

#[test]
fn apply_sync_inserts_updates_removes_and_marks_courses_synced() {
    let f = fixture();
    let t0 = base_time();
    let due = t0 + Duration::days(7);
    sync_inserts(
        &f,
        vec![
            insert(f.tracked, remote("a", "Entrega 1", Some(due)), None),
            insert(f.tracked, remote("b", "Entrega 2", None), None),
        ],
        t0,
    );
    let items = f.db.list_items_for_university(f.university_id).unwrap();
    assert_eq!(items.len(), 2);
    let a = find(&items, "a");
    assert_eq!((a.first_seen_at, a.updated_at), (t0, t0));
    assert_eq!(a.removed_at, None);
    assert_eq!(a.course_id, f.tracked);

    let t1 = t0 + Duration::hours(2);
    let new_due = due + Duration::days(1);
    let b = find(&items, "b");
    f.db.apply_sync(
        f.university_id,
        &SyncChanges {
            updates: vec![ItemUpdate {
                id: a.id,
                course_id: f.tracked,
                remote: remote("a", "Entrega 1 (rev.)", Some(new_due)),
                change: Some(ItemChange::DueChanged {
                    previous_due_at: Some(due),
                }),
            }],
            removals: vec![b.id],
            synced_course_ids: vec![f.tracked],
            ..SyncChanges::default()
        },
        t1,
    )
    .unwrap();

    let items = f.db.list_items_for_university(f.university_id).unwrap();
    let a = find(&items, "a");
    assert_eq!(a.title, "Entrega 1 (rev.)");
    assert_eq!(a.due_at, Some(new_due));
    assert_eq!(
        a.change,
        Some(ItemChange::DueChanged {
            previous_due_at: Some(due)
        })
    );
    assert_eq!((a.first_seen_at, a.updated_at), (t0, t1));
    let b = find(&items, "b");
    assert_eq!(b.removed_at, Some(t1));

    let courses = f.db.list_courses(f.university_id).unwrap();
    let synced = |id: i64| courses.iter().find(|c| c.id == id).unwrap().items_synced_at;
    assert_eq!(synced(f.tracked), Some(t1));
    assert_eq!(synced(f.untracked), None);
}

#[test]
fn personal_fields_survive_sync_updates() {
    let f = fixture();
    let t0 = base_time();
    sync_inserts(
        &f,
        vec![insert(f.tracked, remote("a", "Entrega 1", None), None)],
        t0,
    );
    let id = find(
        &f.db.list_items_for_university(f.university_id).unwrap(),
        "a",
    )
    .id;
    f.db.set_item_done(id, true).unwrap();
    f.db.set_item_hidden(id, true).unwrap();
    f.db.set_item_note(id, Some("revisar apuntes")).unwrap();
    // Removed by one sync, returned by the next.
    let t1 = t0 + Duration::hours(1);
    f.db.apply_sync(
        f.university_id,
        &SyncChanges {
            removals: vec![id],
            ..SyncChanges::default()
        },
        t1,
    )
    .unwrap();
    assert_eq!(
        f.db.get_item(id).unwrap().unwrap().item.removed_at,
        Some(t1)
    );

    let t2 = t1 + Duration::hours(1);
    f.db.apply_sync(
        f.university_id,
        &SyncChanges {
            updates: vec![ItemUpdate {
                id,
                course_id: f.tracked,
                remote: remote("a", "Entrega 1 v2", None),
                change: None,
            }],
            ..SyncChanges::default()
        },
        t2,
    )
    .unwrap();

    let item = f.db.get_item(id).unwrap().unwrap().item;
    assert_eq!(item.title, "Entrega 1 v2");
    assert!(item.done);
    assert!(item.hidden);
    assert_eq!(item.note.as_deref(), Some("revisar apuntes"));
    assert_eq!(item.removed_at, None);
    assert_eq!(item.updated_at, t2);
}

#[test]
fn query_items_filters_and_orders() {
    let f = fixture();
    let t0 = base_time();
    let day = |n: i64| Some(t0 + Duration::days(n));
    sync_inserts(
        &f,
        vec![
            insert(f.tracked, remote("late", "Zeta", day(10)), None),
            insert(f.tracked, remote("soon", "Beta", day(2)), None),
            insert(f.tracked, remote("tie", "Alfa", day(2)), None),
            insert(f.tracked, remote("undated", "Sin fecha", None), None),
            insert(f.tracked, remote("hidden", "Oculta", day(3)), None),
            insert(f.tracked, remote("gone", "Retirada", day(4)), None),
            insert(f.untracked, remote("other", "Otra", day(1)), None),
        ],
        t0,
    );
    let items = f.db.list_items_for_university(f.university_id).unwrap();
    let id = |ext: &str| find(&items, ext).id;
    f.db.set_item_hidden(id("hidden"), true).unwrap();
    f.db.apply_sync(
        f.university_id,
        &SyncChanges {
            removals: vec![id("gone")],
            ..SyncChanges::default()
        },
        t0 + Duration::hours(1),
    )
    .unwrap();

    let external = |query: &TrackerQuery| -> Vec<String> {
        f.db.query_items(query)
            .unwrap()
            .into_iter()
            .map(|t| t.item.external_id)
            .collect()
    };
    assert_eq!(
        external(&TrackerQuery::default()),
        ["tie", "soon", "late", "undated"]
    );
    assert_eq!(
        external(&TrackerQuery {
            include_hidden: true,
            ..TrackerQuery::default()
        }),
        ["tie", "soon", "hidden", "late", "undated"]
    );
    assert_eq!(
        external(&TrackerQuery {
            include_removed: true,
            ..TrackerQuery::default()
        }),
        ["tie", "soon", "gone", "late", "undated"]
    );
    assert_eq!(
        external(&TrackerQuery {
            include_hidden: true,
            include_removed: true,
            ..TrackerQuery::default()
        }),
        ["tie", "soon", "hidden", "gone", "late", "undated"]
    );
    assert_eq!(
        external(&TrackerQuery {
            university_id: Some(f.university_id),
            ..TrackerQuery::default()
        }),
        ["tie", "soon", "late", "undated"]
    );
    assert!(external(&TrackerQuery {
        university_id: Some(f.university_id + 100),
        ..TrackerQuery::default()
    })
    .is_empty());

    let first =
        f.db.query_items(&TrackerQuery::default())
            .unwrap()
            .remove(0);
    assert_eq!(first.university_name, "Universidad Demo");
    assert_eq!(first.course_name, "Álgebra Lineal");
    assert_eq!(first.course_short_name.as_deref(), Some("ALG"));
}

#[test]
fn unavailable_courses_are_excluded() {
    let f = fixture();
    sync_inserts(
        &f,
        vec![insert(f.tracked, remote("a", "Entrega 1", None), None)],
        base_time(),
    );
    assert_eq!(f.db.query_items(&TrackerQuery::default()).unwrap().len(), 1);
    // The platform no longer lists the tracked course.
    f.db.upsert_remote_courses(
        f.university_id,
        &[remote_course("c2", "Historia del Arte", None)],
        base_time(),
    )
    .unwrap();
    assert!(f
        .db
        .query_items(&TrackerQuery::default())
        .unwrap()
        .is_empty());
}

#[test]
fn personal_setters_and_dismiss() {
    let f = fixture();
    let t0 = base_time();
    let new = Some(ItemChange::New);
    sync_inserts(
        &f,
        vec![
            insert(f.tracked, remote("a", "Entrega 1", None), new.clone()),
            insert(f.tracked, remote("b", "Entrega 2", None), new.clone()),
        ],
        t0,
    );
    let other_university =
        f.db.insert_university(
            "Otra Universidad",
            LmsPlatform::Blackboard,
            "https://bb.demo.test",
            t0,
        )
        .unwrap()
        .id;
    f.db.upsert_remote_courses(other_university, &[remote_course("x", "Física", None)], t0)
        .unwrap();
    let other_course = f.db.list_courses(other_university).unwrap()[0].id;
    f.db.set_tracked_courses(other_university, &[other_course])
        .unwrap();
    f.db.apply_sync(
        other_university,
        &SyncChanges {
            inserts: vec![insert(other_course, remote("z", "Práctica", None), new)],
            ..SyncChanges::default()
        },
        t0,
    )
    .unwrap();

    let items = f.db.list_items_for_university(f.university_id).unwrap();
    let (a, b) = (find(&items, "a").id, find(&items, "b").id);

    let item = f.db.set_item_done(a, true).unwrap();
    assert!(item.item.done);
    assert_eq!(item.course_name, "Álgebra Lineal");
    assert!(!f.db.set_item_done(a, false).unwrap().item.done);
    assert!(f.db.set_item_hidden(a, true).unwrap().item.hidden);
    assert!(!f.db.set_item_hidden(a, false).unwrap().item.hidden);
    let noted = f.db.set_item_note(a, Some("llevar calculadora")).unwrap();
    assert_eq!(noted.item.note.as_deref(), Some("llevar calculadora"));
    assert_eq!(f.db.set_item_note(a, None).unwrap().item.note, None);

    assert_eq!(f.db.dismiss_item_change(a).unwrap().item.change, None);
    assert_eq!(
        f.db.get_item(b).unwrap().unwrap().item.change,
        Some(ItemChange::New)
    );

    assert_eq!(f.db.dismiss_all_changes(Some(f.university_id)).unwrap(), 1);
    assert_eq!(f.db.get_item(b).unwrap().unwrap().item.change, None);
    let z = f.db.list_items_for_university(other_university).unwrap();
    assert_eq!(z[0].change, Some(ItemChange::New));
    assert_eq!(f.db.dismiss_all_changes(None).unwrap(), 1);
    assert_eq!(f.db.dismiss_all_changes(None).unwrap(), 0);

    let missing = 9_999;
    assert!(f.db.get_item(missing).unwrap().is_none());
    assert!(matches!(
        f.db.set_item_done(missing, true),
        Err(RepoError::NotFound)
    ));
    assert!(matches!(
        f.db.set_item_hidden(missing, true),
        Err(RepoError::NotFound)
    ));
    assert!(matches!(
        f.db.set_item_note(missing, None),
        Err(RepoError::NotFound)
    ));
    assert!(matches!(
        f.db.dismiss_item_change(missing),
        Err(RepoError::NotFound)
    ));
}

#[test]
fn grade_and_change_round_trip() {
    let f = fixture();
    let mut graded = remote("g", "Examen", None);
    graded.platform_status = PlatformStatus::Graded;
    graded.kind = ItemKind::Test;
    graded.grade = Some(Grade {
        display: "7,5".into(),
        score: Some(7.5),
        max: None,
    });
    sync_inserts(
        &f,
        vec![
            insert(
                f.tracked,
                graded.clone(),
                Some(ItemChange::DueChanged {
                    previous_due_at: None,
                }),
            ),
            insert(f.tracked, remote("p", "Plain", None), None),
        ],
        base_time(),
    );
    let items = f.db.list_items_for_university(f.university_id).unwrap();
    let g = find(&items, "g");
    assert_eq!(g.grade, graded.grade);
    assert_eq!(g.kind, ItemKind::Test);
    assert_eq!(g.platform_status, PlatformStatus::Graded);
    assert_eq!(
        g.change,
        Some(ItemChange::DueChanged {
            previous_due_at: None
        })
    );
    let p = find(&items, "p");
    assert_eq!((p.grade, p.change), (None, None));
}

#[test]
fn deleting_a_university_cascades_to_items() {
    let f = fixture();
    sync_inserts(
        &f,
        vec![insert(f.tracked, remote("a", "Entrega 1", None), None)],
        base_time(),
    );
    f.db.delete_university(f.university_id).unwrap();
    assert!(f
        .db
        .query_items(&TrackerQuery::default())
        .unwrap()
        .is_empty());
    assert!(f
        .db
        .list_items_for_university(f.university_id)
        .unwrap()
        .is_empty());
}

#[test]
fn apply_sync_is_atomic() {
    let f = fixture();
    let duplicated = vec![
        insert(f.tracked, remote("dup", "Primera", None), None),
        insert(f.tracked, remote("dup", "Segunda", None), None),
    ];
    let result = f.db.apply_sync(
        f.university_id,
        &SyncChanges {
            inserts: duplicated,
            synced_course_ids: vec![f.tracked],
            ..SyncChanges::default()
        },
        base_time(),
    );
    assert!(matches!(result, Err(RepoError::Conflict(_))));
    assert!(f
        .db
        .list_items_for_university(f.university_id)
        .unwrap()
        .is_empty());
    let courses = f.db.list_courses(f.university_id).unwrap();
    assert!(courses.iter().all(|c| c.items_synced_at.is_none()));
}
