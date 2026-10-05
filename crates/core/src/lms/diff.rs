//! Pure comparison of what an LMS returned with what is stored (spec §7).

use std::collections::{HashMap, HashSet};

use chrono::{DateTime, Utc};

use crate::domain::{ItemChange, ItemKind, LmsCourse, LmsItem, RemoteItem, SyncWindow};

/// A new item to insert.
#[derive(Debug, Clone, PartialEq)]
pub struct NewItem {
    pub course_id: i64,
    pub remote: RemoteItem,
    pub change: Option<ItemChange>,
}

/// Platform fields to overwrite on an existing item, plus the resulting badge.
/// Always clears `removed_at` (the item was returned again).
#[derive(Debug, Clone, PartialEq)]
pub struct ItemUpdate {
    pub id: i64,
    pub course_id: i64,
    pub remote: RemoteItem,
    pub change: Option<ItemChange>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SyncChanges {
    pub inserts: Vec<NewItem>,
    pub updates: Vec<ItemUpdate>,
    /// Ids of items no longer returned by the platform (only those not already removed).
    pub removals: Vec<i64>,
    /// Courses whose items were fetched in this sync (their `items_synced_at` is set).
    pub synced_course_ids: Vec<i64>,
    pub new_items: u32,
    pub due_changed: u32,
    pub removed: u32,
}

/// Compares what the platform returned for `fetched_courses` with what is stored.
/// Course events are only fetched inside `window`, so a stored event outside it is
/// never reported as removed.
pub fn diff_items(
    fetched_courses: &[LmsCourse],
    stored: &[LmsItem],
    remote: &[RemoteItem],
    window: &SyncWindow,
) -> SyncChanges {
    let courses: HashMap<&str, &LmsCourse> = fetched_courses
        .iter()
        .map(|course| (course.external_id.as_str(), course))
        .collect();
    let stored_by_external: HashMap<&str, &LmsItem> = stored
        .iter()
        .map(|item| (item.external_id.as_str(), item))
        .collect();

    let mut changes = SyncChanges {
        synced_course_ids: fetched_courses.iter().map(|course| course.id).collect(),
        ..SyncChanges::default()
    };

    let mut returned: HashSet<&str> = HashSet::new();
    for item in remote {
        let Some(course) = courses.get(item.course_external_id.as_str()) else {
            continue;
        };
        if !returned.insert(item.external_id.as_str()) {
            continue;
        }
        match stored_by_external.get(item.external_id.as_str()) {
            None => insert(&mut changes, course, item),
            Some(existing) => update(&mut changes, course, existing, item),
        }
    }

    let fetched_ids: HashSet<i64> = changes.synced_course_ids.iter().copied().collect();
    for item in stored {
        if fetched_ids.contains(&item.course_id)
            && item.removed_at.is_none()
            && !returned.contains(item.external_id.as_str())
            && !is_event_outside(item, window)
        {
            changes.removals.push(item.id);
            changes.removed += 1;
        }
    }
    changes
}

/// Whether `item` is a course event the sync could not have fetched.
fn is_event_outside(item: &LmsItem, window: &SyncWindow) -> bool {
    item.kind == ItemKind::Event
        && !item
            .due_at
            .is_some_and(|due| due >= window.from && due <= window.to)
}

fn insert(changes: &mut SyncChanges, course: &LmsCourse, item: &RemoteItem) {
    let change = course.items_synced_at.map(|_| ItemChange::New);
    if change.is_some() {
        changes.new_items += 1;
    }
    changes.inserts.push(NewItem {
        course_id: course.id,
        remote: item.clone(),
        change,
    });
}

fn update(changes: &mut SyncChanges, course: &LmsCourse, existing: &LmsItem, item: &RemoteItem) {
    let due_differs = existing.due_at != item.due_at;
    let change = if due_differs {
        change_after_due_update(existing, item.due_at)
    } else {
        existing.change.clone()
    };
    if !differs(existing, course.id, item) {
        return;
    }
    if due_differs && matches!(change, Some(ItemChange::DueChanged { .. })) {
        changes.due_changed += 1;
    }
    changes.updates.push(ItemUpdate {
        id: existing.id,
        course_id: course.id,
        remote: item.clone(),
        change,
    });
}

/// The badge after the due date moved: `New` is kept; otherwise the original
/// date is remembered, and returning to it clears the badge.
fn change_after_due_update(
    existing: &LmsItem,
    new_due: Option<DateTime<Utc>>,
) -> Option<ItemChange> {
    let previous = match &existing.change {
        Some(ItemChange::New) => return Some(ItemChange::New),
        Some(ItemChange::DueChanged { previous_due_at }) => *previous_due_at,
        None => existing.due_at,
    };
    (previous != new_due).then_some(ItemChange::DueChanged {
        previous_due_at: previous,
    })
}

fn differs(existing: &LmsItem, course_id: i64, item: &RemoteItem) -> bool {
    existing.removed_at.is_some()
        || existing.course_id != course_id
        || existing.kind != item.kind
        || existing.title != item.title
        || existing.due_at != item.due_at
        || existing.url != item.url
        || existing.platform_status != item.platform_status
        || existing.grade != item.grade
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Grade, ItemKind, PlatformStatus};
    use chrono::{DateTime, TimeZone, Utc};

    /// The days of March 2026 from the 10th to the 25th.
    fn window() -> SyncWindow {
        SyncWindow {
            from: at(10),
            to: at(25),
        }
    }

    fn diff(
        fetched_courses: &[LmsCourse],
        stored: &[LmsItem],
        remote: &[RemoteItem],
    ) -> SyncChanges {
        diff_items(fetched_courses, stored, remote, &window())
    }

    fn at(day: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, day, 12, 0, 0).unwrap()
    }

    fn course(id: i64, external: &str, synced: bool) -> LmsCourse {
        LmsCourse {
            id,
            university_id: 1,
            external_id: external.into(),
            name: format!("Course {external}"),
            short_name: None,
            url: None,
            tracked: true,
            available: true,
            reviewed: true,
            items_synced_at: synced.then(|| at(1)),
        }
    }

    fn stored(
        id: i64,
        course_id: i64,
        ext: &str,
        due: Option<DateTime<Utc>>,
        change: Option<ItemChange>,
        removed: bool,
    ) -> LmsItem {
        LmsItem {
            id,
            university_id: 1,
            course_id,
            external_id: ext.into(),
            kind: ItemKind::Assignment,
            title: "Essay".into(),
            due_at: due,
            url: None,
            platform_status: PlatformStatus::NotSubmitted,
            grade: None,
            change,
            removed_at: removed.then(|| at(2)),
            done: false,
            hidden: false,
            note: None,
            first_seen_at: at(1),
            updated_at: at(1),
        }
    }

    fn remote(course_ext: &str, ext: &str, due: Option<DateTime<Utc>>) -> RemoteItem {
        RemoteItem {
            course_external_id: course_ext.into(),
            external_id: ext.into(),
            kind: ItemKind::Assignment,
            title: "Essay".into(),
            due_at: due,
            url: None,
            platform_status: PlatformStatus::NotSubmitted,
            grade: None,
        }
    }

    fn due_changed(previous: Option<DateTime<Utc>>) -> Option<ItemChange> {
        Some(ItemChange::DueChanged {
            previous_due_at: previous,
        })
    }

    #[test]
    fn first_sync_of_a_course_inserts_without_badge() {
        let courses = [course(10, "c1", false)];
        let r = remote("c1", "a1", Some(at(5)));

        let changes = diff(&courses, &[], std::slice::from_ref(&r));

        assert_eq!(
            changes.inserts,
            vec![NewItem {
                course_id: 10,
                remote: r,
                change: None
            }]
        );
        assert_eq!(changes.new_items, 0);
        assert_eq!(changes.synced_course_ids, vec![10]);
    }

    #[test]
    fn later_sync_marks_new_items() {
        let courses = [course(10, "c1", true)];
        let r = remote("c1", "a1", None);

        let changes = diff(&courses, &[], std::slice::from_ref(&r));

        assert_eq!(
            changes.inserts,
            vec![NewItem {
                course_id: 10,
                remote: r,
                change: Some(ItemChange::New)
            }]
        );
        assert_eq!(changes.new_items, 1);
    }

    #[test]
    fn due_date_change_records_the_original_previous_date() {
        let courses = [course(10, "c1", true)];

        let first = diff(
            &courses,
            &[stored(1, 10, "a1", Some(at(5)), None, false)],
            &[remote("c1", "a1", Some(at(6)))],
        );
        assert_eq!(first.updates.len(), 1);
        assert_eq!(first.updates[0].change, due_changed(Some(at(5))));
        assert_eq!(first.due_changed, 1);

        let second = diff(
            &courses,
            &[stored(
                1,
                10,
                "a1",
                Some(at(6)),
                due_changed(Some(at(5))),
                false,
            )],
            &[remote("c1", "a1", Some(at(7)))],
        );
        assert_eq!(second.updates[0].change, due_changed(Some(at(5))));
        assert_eq!(second.due_changed, 1);
    }

    #[test]
    fn returning_to_the_original_date_clears_the_badge() {
        let courses = [course(10, "c1", true)];

        let changes = diff(
            &courses,
            &[stored(
                1,
                10,
                "a1",
                Some(at(6)),
                due_changed(Some(at(5))),
                false,
            )],
            &[remote("c1", "a1", Some(at(5)))],
        );

        assert_eq!(changes.updates.len(), 1);
        assert_eq!(changes.updates[0].change, None);
        assert_eq!(changes.due_changed, 0);
    }

    #[test]
    fn due_date_appearing_or_disappearing_is_a_change() {
        let courses = [course(10, "c1", true)];

        let appearing = diff(
            &courses,
            &[stored(1, 10, "a1", None, None, false)],
            &[remote("c1", "a1", Some(at(5)))],
        );
        assert_eq!(appearing.updates[0].change, due_changed(None));
        assert_eq!(appearing.due_changed, 1);

        let disappearing = diff(
            &courses,
            &[stored(1, 10, "a1", Some(at(5)), None, false)],
            &[remote("c1", "a1", None)],
        );
        assert_eq!(disappearing.updates[0].change, due_changed(Some(at(5))));
        assert_eq!(disappearing.due_changed, 1);
    }

    #[test]
    fn new_badge_survives_a_due_change() {
        let courses = [course(10, "c1", true)];

        let changes = diff(
            &courses,
            &[stored(
                1,
                10,
                "a1",
                Some(at(5)),
                Some(ItemChange::New),
                false,
            )],
            &[remote("c1", "a1", Some(at(6)))],
        );

        assert_eq!(changes.updates.len(), 1);
        assert_eq!(changes.updates[0].change, Some(ItemChange::New));
        assert_eq!(changes.due_changed, 0);
    }

    #[test]
    fn unchanged_items_produce_no_update() {
        let courses = [course(10, "c1", true)];

        let changes = diff(
            &courses,
            &[stored(1, 10, "a1", Some(at(5)), None, false)],
            &[remote("c1", "a1", Some(at(5)))],
        );

        assert!(changes.updates.is_empty());
        assert!(changes.inserts.is_empty());
        assert!(changes.removals.is_empty());
        assert_eq!(
            (changes.new_items, changes.due_changed, changes.removed),
            (0, 0, 0)
        );
    }

    #[test]
    fn other_field_changes_update_silently() {
        let courses = [course(10, "c1", true)];
        let kept = due_changed(Some(at(3)));
        let base = stored(1, 10, "a1", Some(at(5)), kept.clone(), false);

        let mut variants = Vec::new();
        let mut r = remote("c1", "a1", Some(at(5)));
        r.title = "Final essay".into();
        variants.push(r);
        let mut r = remote("c1", "a1", Some(at(5)));
        r.kind = ItemKind::Quiz;
        variants.push(r);
        let mut r = remote("c1", "a1", Some(at(5)));
        r.url = Some("https://lms.example.edu/a1".into());
        variants.push(r);
        let mut r = remote("c1", "a1", Some(at(5)));
        r.platform_status = PlatformStatus::Submitted;
        variants.push(r);
        let mut r = remote("c1", "a1", Some(at(5)));
        r.grade = Some(Grade {
            display: "8/10".into(),
            score: Some(8.0),
            max: Some(10.0),
        });
        variants.push(r);

        for r in variants {
            let changes = diff(
                &courses,
                std::slice::from_ref(&base),
                std::slice::from_ref(&r),
            );
            assert_eq!(changes.updates.len(), 1, "{r:?}");
            assert_eq!(changes.updates[0].change, kept);
            assert_eq!(changes.updates[0].remote, r);
            assert_eq!(
                (changes.new_items, changes.due_changed, changes.removed),
                (0, 0, 0)
            );
        }
    }

    #[test]
    fn course_change_updates_the_course() {
        let courses = [course(10, "c1", true), course(11, "c2", true)];

        let changes = diff(
            &courses,
            &[stored(1, 10, "a1", Some(at(5)), None, false)],
            &[remote("c2", "a1", Some(at(5)))],
        );

        assert_eq!(changes.updates.len(), 1);
        assert_eq!(changes.updates[0].id, 1);
        assert_eq!(changes.updates[0].course_id, 11);
        assert!(changes.removals.is_empty());
        assert!(changes.inserts.is_empty());
    }

    #[test]
    fn missing_items_are_removed_once() {
        let courses = [course(10, "c1", true)];

        let changes = diff(
            &courses,
            &[
                stored(1, 10, "a1", None, None, false),
                stored(2, 10, "a2", None, None, true),
            ],
            &[],
        );

        assert_eq!(changes.removals, vec![1]);
        assert_eq!(changes.removed, 1);
        assert!(changes.updates.is_empty());
    }

    #[test]
    fn reappearing_items_are_restored() {
        let courses = [course(10, "c1", true)];

        let changes = diff(
            &courses,
            &[stored(1, 10, "a1", Some(at(5)), None, true)],
            &[remote("c1", "a1", Some(at(5)))],
        );

        assert_eq!(changes.updates.len(), 1);
        assert_eq!(changes.updates[0].id, 1);
        assert_eq!(changes.updates[0].change, None);
        assert!(changes.removals.is_empty());
        assert_eq!(
            (changes.new_items, changes.due_changed, changes.removed),
            (0, 0, 0)
        );
    }

    #[test]
    fn items_of_unfetched_courses_are_untouched() {
        let courses = [course(10, "c1", true)];

        let changes = diff(
            &courses,
            &[stored(1, 99, "a1", Some(at(5)), None, false)],
            &[],
        );

        assert!(changes.removals.is_empty());
        assert!(changes.updates.is_empty());
        assert_eq!(changes.removed, 0);
    }

    #[test]
    fn remote_duplicates_and_unknown_courses_are_ignored() {
        let courses = [course(10, "c1", true)];
        let first = remote("c1", "a1", Some(at(5)));
        let mut second = remote("c1", "a1", Some(at(9)));
        second.title = "Duplicate".into();
        let unknown = remote("zzz", "a2", None);

        let changes = diff(&courses, &[], &[first.clone(), second, unknown]);

        assert_eq!(changes.inserts.len(), 1);
        assert_eq!(changes.inserts[0].remote, first);
        assert_eq!(changes.new_items, 1);
    }

    fn stored_event(id: i64, due: Option<DateTime<Utc>>) -> LmsItem {
        let mut event = stored(id, 10, &format!("e{id}"), due, None, false);
        event.kind = ItemKind::Event;
        event
    }

    #[test]
    fn events_outside_the_sync_window_are_not_removed() {
        let courses = [course(10, "c1", true)];

        let changes = diff(
            &courses,
            &[
                stored_event(1, Some(at(5))),
                stored_event(2, Some(at(28))),
                stored_event(3, None),
            ],
            &[],
        );

        assert!(changes.removals.is_empty());
        assert_eq!(changes.removed, 0);
    }

    #[test]
    fn events_inside_the_sync_window_that_vanish_are_removed() {
        let courses = [course(10, "c1", true)];

        let changes = diff(&courses, &[stored_event(1, Some(at(15)))], &[]);

        assert_eq!(changes.removals, vec![1]);
        assert_eq!(changes.removed, 1);
    }

    #[test]
    fn assignments_before_the_window_that_vanish_are_still_removed() {
        let courses = [course(10, "c1", true)];

        let changes = diff(
            &courses,
            &[stored(1, 10, "a1", Some(at(5)), None, false)],
            &[],
        );

        assert_eq!(changes.removals, vec![1]);
    }
}
