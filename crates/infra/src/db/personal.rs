//! `PersonalTaskRepository` implementation.

use academic_core::domain::{PersonalTask, PersonalTaskDraft};
use academic_core::ports::{PersonalTaskRepository, RepoResult};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use super::util::{ensure_changed, format_date, map_err, parse_date, parse_datetime, to_rfc3339};
use super::Database;

const SELECT: &str =
    "SELECT id, title, date, note, done, created_at, updated_at FROM personal_tasks";

struct Raw {
    id: i64,
    title: String,
    date: String,
    note: Option<String>,
    done: bool,
    created_at: String,
    updated_at: String,
}

fn read_raw(row: &rusqlite::Row) -> rusqlite::Result<Raw> {
    Ok(Raw {
        id: row.get(0)?,
        title: row.get(1)?,
        date: row.get(2)?,
        note: row.get(3)?,
        done: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn convert(raw: Raw) -> RepoResult<PersonalTask> {
    Ok(PersonalTask {
        id: raw.id,
        title: raw.title,
        date: parse_date(&raw.date)?,
        note: raw.note,
        done: raw.done,
        created_at: parse_datetime(&raw.created_at)?,
        updated_at: parse_datetime(&raw.updated_at)?,
    })
}

/// Loads one task; a missing row maps to `NotFound`.
fn get_task(conn: &Connection, id: i64) -> RepoResult<PersonalTask> {
    let raw = conn
        .query_row(&format!("{SELECT} WHERE id = ?1"), [id], read_raw)
        .map_err(map_err)?;
    convert(raw)
}

impl PersonalTaskRepository for Database {
    fn list_personal_tasks(&self) -> RepoResult<Vec<PersonalTask>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(&format!("{SELECT} ORDER BY date, id"))
            .map_err(map_err)?;
        let rows = stmt.query_map([], read_raw).map_err(map_err)?;
        let mut tasks = Vec::new();
        for raw in rows {
            tasks.push(convert(raw.map_err(map_err)?)?);
        }
        Ok(tasks)
    }

    fn insert_personal_task(
        &self,
        draft: &PersonalTaskDraft,
        now: DateTime<Utc>,
    ) -> RepoResult<PersonalTask> {
        let conn = self.conn()?;
        let stamp = to_rfc3339(now);
        conn.execute(
            "INSERT INTO personal_tasks (title, date, note, done, created_at, updated_at) \
             VALUES (?1, ?2, ?3, 0, ?4, ?4)",
            params![draft.title, format_date(draft.date), draft.note, stamp],
        )
        .map_err(map_err)?;
        get_task(&conn, conn.last_insert_rowid())
    }

    fn update_personal_task(
        &self,
        id: i64,
        draft: &PersonalTaskDraft,
        now: DateTime<Utc>,
    ) -> RepoResult<PersonalTask> {
        let conn = self.conn()?;
        let changed = conn
            .execute(
                "UPDATE personal_tasks SET title = ?2, date = ?3, note = ?4, updated_at = ?5 \
                 WHERE id = ?1",
                params![
                    id,
                    draft.title,
                    format_date(draft.date),
                    draft.note,
                    to_rfc3339(now)
                ],
            )
            .map_err(map_err)?;
        ensure_changed(changed)?;
        get_task(&conn, id)
    }

    fn set_personal_task_done(
        &self,
        id: i64,
        done: bool,
        now: DateTime<Utc>,
    ) -> RepoResult<PersonalTask> {
        let conn = self.conn()?;
        let changed = conn
            .execute(
                "UPDATE personal_tasks SET done = ?2, updated_at = ?3 WHERE id = ?1",
                params![id, done, to_rfc3339(now)],
            )
            .map_err(map_err)?;
        ensure_changed(changed)?;
        get_task(&conn, id)
    }

    fn delete_personal_task(&self, id: i64) -> RepoResult<()> {
        let conn = self.conn()?;
        let changed = conn
            .execute("DELETE FROM personal_tasks WHERE id = ?1", [id])
            .map_err(map_err)?;
        ensure_changed(changed)
    }
}
