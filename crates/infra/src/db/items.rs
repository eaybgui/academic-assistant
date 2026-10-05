//! `LmsItemRepository` implementation.

use academic_core::domain::{
    Grade, ItemChange, ItemKind, LmsItem, PlatformStatus, RemoteItem, TrackedItem, TrackerQuery,
    UniversityId,
};
use academic_core::lms::SyncChanges;
use academic_core::ports::{LmsItemRepository, RepoError, RepoResult};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::util::{
    ensure_changed, map_err, parse_datetime, parse_datetime_opt, parse_enum, to_rfc3339,
};
use super::Database;

const SELECT: &str = "SELECT i.id, i.university_id, i.course_id, i.external_id, i.kind, \
    i.title, i.due_at, i.url, i.platform_status, i.grade_display, i.grade_score, i.grade_max, \
    i.change_kind, i.previous_due_at, i.removed_at, i.done, i.hidden, i.note, \
    i.first_seen_at, i.updated_at, u.name, c.name, c.short_name \
    FROM lms_items i \
    JOIN courses c ON c.id = i.course_id \
    JOIN universities u ON u.id = i.university_id";

struct Raw {
    id: i64,
    university_id: i64,
    course_id: i64,
    external_id: String,
    kind: String,
    title: String,
    due_at: Option<String>,
    url: Option<String>,
    platform_status: String,
    grade_display: Option<String>,
    grade_score: Option<f64>,
    grade_max: Option<f64>,
    change_kind: Option<String>,
    previous_due_at: Option<String>,
    removed_at: Option<String>,
    done: bool,
    hidden: bool,
    note: Option<String>,
    first_seen_at: String,
    updated_at: String,
    university_name: String,
    course_name: String,
    course_short_name: Option<String>,
}

fn read_raw(row: &rusqlite::Row) -> rusqlite::Result<Raw> {
    Ok(Raw {
        id: row.get(0)?,
        university_id: row.get(1)?,
        course_id: row.get(2)?,
        external_id: row.get(3)?,
        kind: row.get(4)?,
        title: row.get(5)?,
        due_at: row.get(6)?,
        url: row.get(7)?,
        platform_status: row.get(8)?,
        grade_display: row.get(9)?,
        grade_score: row.get(10)?,
        grade_max: row.get(11)?,
        change_kind: row.get(12)?,
        previous_due_at: row.get(13)?,
        removed_at: row.get(14)?,
        done: row.get(15)?,
        hidden: row.get(16)?,
        note: row.get(17)?,
        first_seen_at: row.get(18)?,
        updated_at: row.get(19)?,
        university_name: row.get(20)?,
        course_name: row.get(21)?,
        course_short_name: row.get(22)?,
    })
}

fn convert_change(
    kind: Option<String>,
    previous_due_at: Option<String>,
) -> RepoResult<Option<ItemChange>> {
    match kind.as_deref() {
        None => Ok(None),
        Some("new") => Ok(Some(ItemChange::New)),
        Some("due_changed") => Ok(Some(ItemChange::DueChanged {
            previous_due_at: parse_datetime_opt(previous_due_at)?,
        })),
        Some(other) => Err(RepoError::Storage(format!("invalid change kind '{other}'"))),
    }
}

fn convert(raw: Raw) -> RepoResult<TrackedItem> {
    let grade = raw.grade_display.map(|display| Grade {
        display,
        score: raw.grade_score,
        max: raw.grade_max,
    });
    Ok(TrackedItem {
        item: LmsItem {
            id: raw.id,
            university_id: raw.university_id,
            course_id: raw.course_id,
            external_id: raw.external_id,
            kind: parse_enum::<ItemKind>(&raw.kind)?,
            title: raw.title,
            due_at: parse_datetime_opt(raw.due_at)?,
            url: raw.url,
            platform_status: parse_enum::<PlatformStatus>(&raw.platform_status)?,
            grade,
            change: convert_change(raw.change_kind, raw.previous_due_at)?,
            removed_at: parse_datetime_opt(raw.removed_at)?,
            done: raw.done,
            hidden: raw.hidden,
            note: raw.note,
            first_seen_at: parse_datetime(&raw.first_seen_at)?,
            updated_at: parse_datetime(&raw.updated_at)?,
        },
        university_name: raw.university_name,
        course_name: raw.course_name,
        course_short_name: raw.course_short_name,
    })
}

/// Column values of an `ItemChange`: `(change_kind, previous_due_at)`.
fn change_columns(change: &Option<ItemChange>) -> (Option<&'static str>, Option<String>) {
    match change {
        None => (None, None),
        Some(ItemChange::New) => (Some("new"), None),
        Some(ItemChange::DueChanged { previous_due_at }) => {
            (Some("due_changed"), previous_due_at.map(to_rfc3339))
        }
    }
}

/// The platform-owned columns shared by inserts and updates.
struct PlatformColumns {
    kind: &'static str,
    status: &'static str,
    due_at: Option<String>,
    grade_display: Option<String>,
    grade_score: Option<f64>,
    grade_max: Option<f64>,
    change_kind: Option<&'static str>,
    previous_due_at: Option<String>,
}

fn platform_columns(remote: &RemoteItem, change: &Option<ItemChange>) -> PlatformColumns {
    let (change_kind, previous_due_at) = change_columns(change);
    PlatformColumns {
        kind: remote.kind.as_str(),
        status: remote.platform_status.as_str(),
        due_at: remote.due_at.map(to_rfc3339),
        grade_display: remote.grade.as_ref().map(|g| g.display.clone()),
        grade_score: remote.grade.as_ref().and_then(|g| g.score),
        grade_max: remote.grade.as_ref().and_then(|g| g.max),
        change_kind,
        previous_due_at,
    }
}

fn query_tracked(
    conn: &Connection,
    filter: &str,
    args: &[&dyn rusqlite::ToSql],
) -> RepoResult<Vec<TrackedItem>> {
    let mut stmt = conn
        .prepare(&format!("{SELECT} {filter}"))
        .map_err(map_err)?;
    let raws = stmt
        .query_map(args, read_raw)
        .map_err(map_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_err)?;
    raws.into_iter().map(convert).collect()
}

fn get_on(conn: &Connection, id: i64) -> RepoResult<Option<TrackedItem>> {
    conn.query_row(&format!("{SELECT} WHERE i.id = ?1"), [id], read_raw)
        .optional()
        .map_err(map_err)?
        .map(convert)
        .transpose()
}

/// Runs a single-row `UPDATE lms_items SET <set> WHERE id = ?1` and returns the re-read item.
fn update_item(
    conn: &Connection,
    id: i64,
    set: &str,
    args: &[&dyn rusqlite::ToSql],
) -> RepoResult<TrackedItem> {
    let mut all: Vec<&dyn rusqlite::ToSql> = vec![&id];
    all.extend_from_slice(args);
    let changed = conn
        .execute(&format!("UPDATE lms_items SET {set} WHERE id = ?1"), &*all)
        .map_err(map_err)?;
    ensure_changed(changed)?;
    get_on(conn, id)?.ok_or(RepoError::NotFound)
}

impl LmsItemRepository for Database {
    fn list_items_for_university(&self, university_id: UniversityId) -> RepoResult<Vec<LmsItem>> {
        let conn = self.conn()?;
        let items = query_tracked(
            &conn,
            "WHERE i.university_id = ?1 ORDER BY i.id",
            &[&university_id],
        )?;
        Ok(items.into_iter().map(|tracked| tracked.item).collect())
    }

    fn apply_sync(
        &self,
        university_id: UniversityId,
        changes: &SyncChanges,
        now: DateTime<Utc>,
    ) -> RepoResult<()> {
        let now = to_rfc3339(now);
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_err)?;
        for new in &changes.inserts {
            let cols = platform_columns(&new.remote, &new.change);
            tx.execute(
                "INSERT INTO lms_items (university_id, course_id, external_id, kind, title, \
                 due_at, url, platform_status, grade_display, grade_score, grade_max, \
                 change_kind, previous_due_at, first_seen_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)",
                params![
                    university_id,
                    new.course_id,
                    new.remote.external_id,
                    cols.kind,
                    new.remote.title,
                    cols.due_at,
                    new.remote.url,
                    cols.status,
                    cols.grade_display,
                    cols.grade_score,
                    cols.grade_max,
                    cols.change_kind,
                    cols.previous_due_at,
                    now
                ],
            )
            .map_err(map_err)?;
        }
        for update in &changes.updates {
            let cols = platform_columns(&update.remote, &update.change);
            let changed = tx
                .execute(
                    "UPDATE lms_items SET course_id = ?3, kind = ?4, title = ?5, due_at = ?6, \
                     url = ?7, platform_status = ?8, grade_display = ?9, grade_score = ?10, \
                     grade_max = ?11, change_kind = ?12, previous_due_at = ?13, \
                     removed_at = NULL, updated_at = ?14 \
                     WHERE id = ?1 AND university_id = ?2",
                    params![
                        update.id,
                        university_id,
                        update.course_id,
                        cols.kind,
                        update.remote.title,
                        cols.due_at,
                        update.remote.url,
                        cols.status,
                        cols.grade_display,
                        cols.grade_score,
                        cols.grade_max,
                        cols.change_kind,
                        cols.previous_due_at,
                        now
                    ],
                )
                .map_err(map_err)?;
            ensure_changed(changed)?;
        }
        for id in &changes.removals {
            let changed = tx
                .execute(
                    "UPDATE lms_items SET removed_at = ?3 WHERE id = ?1 AND university_id = ?2",
                    params![id, university_id, now],
                )
                .map_err(map_err)?;
            ensure_changed(changed)?;
        }
        for id in &changes.synced_course_ids {
            let changed = tx
                .execute(
                    "UPDATE courses SET items_synced_at = ?3 WHERE id = ?1 AND university_id = ?2",
                    params![id, university_id, now],
                )
                .map_err(map_err)?;
            ensure_changed(changed)?;
        }
        tx.commit().map_err(map_err)
    }

    fn query_items(&self, query: &TrackerQuery) -> RepoResult<Vec<TrackedItem>> {
        let conn = self.conn()?;
        query_tracked(
            &conn,
            "WHERE c.tracked = 1 AND c.available = 1 \
             AND (?1 IS NULL OR i.university_id = ?1) \
             AND (?2 OR i.hidden = 0) AND (?3 OR i.removed_at IS NULL) \
             ORDER BY i.due_at IS NULL, i.due_at, i.title, i.id",
            &[
                &query.university_id,
                &query.include_hidden,
                &query.include_removed,
            ],
        )
    }

    fn get_item(&self, id: i64) -> RepoResult<Option<TrackedItem>> {
        let conn = self.conn()?;
        get_on(&conn, id)
    }

    fn set_item_done(&self, id: i64, done: bool) -> RepoResult<TrackedItem> {
        let conn = self.conn()?;
        update_item(&conn, id, "done = ?2", &[&done])
    }

    fn set_item_hidden(&self, id: i64, hidden: bool) -> RepoResult<TrackedItem> {
        let conn = self.conn()?;
        update_item(&conn, id, "hidden = ?2", &[&hidden])
    }

    fn set_item_note(&self, id: i64, note: Option<&str>) -> RepoResult<TrackedItem> {
        let conn = self.conn()?;
        update_item(&conn, id, "note = ?2", &[&note])
    }

    fn dismiss_item_change(&self, id: i64) -> RepoResult<TrackedItem> {
        let conn = self.conn()?;
        update_item(&conn, id, "change_kind = NULL, previous_due_at = NULL", &[])
    }

    fn dismiss_all_changes(&self, university_id: Option<UniversityId>) -> RepoResult<u64> {
        let conn = self.conn()?;
        let changed = conn
            .execute(
                "UPDATE lms_items SET change_kind = NULL, previous_due_at = NULL \
                 WHERE change_kind IS NOT NULL AND (?1 IS NULL OR university_id = ?1)",
                [university_id],
            )
            .map_err(map_err)?;
        Ok(changed as u64)
    }
}
