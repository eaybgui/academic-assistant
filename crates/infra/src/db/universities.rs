//! `UniversityRepository` implementation.

use academic_core::domain::{LmsPlatform, LmsUniversity, LoginState, SyncOutcome, UniversityId};
use academic_core::ports::{RepoError, RepoResult, UniversityRepository};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use super::util::{
    ensure_changed, map_err, parse_datetime, parse_datetime_opt, parse_enum, to_rfc3339,
};
use super::Database;

const SELECT: &str = "SELECT id, name, platform, base_url, login_state, account_label, \
    last_sync_at, last_sync_outcome, last_sync_error, created_at FROM universities";

struct Raw {
    id: i64,
    name: String,
    platform: String,
    base_url: String,
    login_state: String,
    account_label: Option<String>,
    last_sync_at: Option<String>,
    last_sync_outcome: Option<String>,
    last_sync_error: Option<String>,
    created_at: String,
}

fn read_raw(row: &rusqlite::Row) -> rusqlite::Result<Raw> {
    Ok(Raw {
        id: row.get(0)?,
        name: row.get(1)?,
        platform: row.get(2)?,
        base_url: row.get(3)?,
        login_state: row.get(4)?,
        account_label: row.get(5)?,
        last_sync_at: row.get(6)?,
        last_sync_outcome: row.get(7)?,
        last_sync_error: row.get(8)?,
        created_at: row.get(9)?,
    })
}

fn convert(raw: Raw) -> RepoResult<LmsUniversity> {
    Ok(LmsUniversity {
        id: raw.id,
        name: raw.name,
        platform: parse_enum::<LmsPlatform>(&raw.platform)?,
        base_url: raw.base_url,
        login_state: parse_enum::<LoginState>(&raw.login_state)?,
        account_label: raw.account_label,
        last_sync_at: parse_datetime_opt(raw.last_sync_at)?,
        last_sync_outcome: raw
            .last_sync_outcome
            .as_deref()
            .map(parse_enum::<SyncOutcome>)
            .transpose()?,
        last_sync_error: raw.last_sync_error,
        created_at: parse_datetime(&raw.created_at)?,
    })
}

fn get_on(conn: &Connection, id: UniversityId) -> RepoResult<Option<LmsUniversity>> {
    conn.query_row(&format!("{SELECT} WHERE id = ?1"), [id], read_raw)
        .optional()
        .map_err(map_err)?
        .map(convert)
        .transpose()
}

fn require(conn: &Connection, id: UniversityId) -> RepoResult<LmsUniversity> {
    get_on(conn, id)?.ok_or(RepoError::NotFound)
}

impl UniversityRepository for Database {
    fn list_universities(&self) -> RepoResult<Vec<LmsUniversity>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare(&format!("{SELECT} ORDER BY name"))
            .map_err(map_err)?;
        let raws = stmt
            .query_map([], read_raw)
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;
        raws.into_iter().map(convert).collect()
    }

    fn get_university(&self, id: UniversityId) -> RepoResult<Option<LmsUniversity>> {
        let conn = self.conn()?;
        get_on(&conn, id)
    }

    fn insert_university(
        &self,
        name: &str,
        platform: LmsPlatform,
        base_url: &str,
        now: DateTime<Utc>,
    ) -> RepoResult<LmsUniversity> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO universities (name, platform, base_url, created_at) \
             VALUES (?1, ?2, ?3, ?4)",
            params![name, platform.as_str(), base_url, to_rfc3339(now)],
        )
        .map_err(map_err)?;
        require(&conn, conn.last_insert_rowid())
    }

    fn rename_university(&self, id: UniversityId, name: &str) -> RepoResult<LmsUniversity> {
        let conn = self.conn()?;
        ensure_changed(
            conn.execute(
                "UPDATE universities SET name = ?1 WHERE id = ?2",
                params![name, id],
            )
            .map_err(map_err)?,
        )?;
        require(&conn, id)
    }

    fn delete_university(&self, id: UniversityId) -> RepoResult<()> {
        let conn = self.conn()?;
        ensure_changed(
            conn.execute("DELETE FROM universities WHERE id = ?1", [id])
                .map_err(map_err)?,
        )
    }

    fn set_login(
        &self,
        id: UniversityId,
        state: LoginState,
        account_label: Option<&str>,
    ) -> RepoResult<LmsUniversity> {
        let conn = self.conn()?;
        ensure_changed(
            conn.execute(
                "UPDATE universities SET login_state = ?1, account_label = ?2 WHERE id = ?3",
                params![state.as_str(), account_label, id],
            )
            .map_err(map_err)?,
        )?;
        require(&conn, id)
    }

    fn record_sync(
        &self,
        id: UniversityId,
        at: DateTime<Utc>,
        outcome: SyncOutcome,
        error: Option<&str>,
    ) -> RepoResult<()> {
        let conn = self.conn()?;
        ensure_changed(
            conn.execute(
                "UPDATE universities SET last_sync_at = ?1, last_sync_outcome = ?2, \
                 last_sync_error = ?3 WHERE id = ?4",
                params![to_rfc3339(at), outcome.as_str(), error, id],
            )
            .map_err(map_err)?,
        )
    }
}
