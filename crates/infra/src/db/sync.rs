//! `SyncStateRepository` implementation.

use academic_core::domain::{AccountId, SyncCursor, SyncState, SyncStatus};
use academic_core::ports::{RepoResult, SyncStateRepository};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};

use super::util::{map_err, parse_datetime_opt, parse_enum, parse_json, to_json, to_rfc3339};
use super::Database;

struct SyncStateRow {
    account_id: i64,
    folder: String,
    cursor: Option<String>,
    last_run_at: Option<String>,
    last_success_at: Option<String>,
    status: String,
    last_error: Option<String>,
}

fn read_row(row: &Row) -> rusqlite::Result<SyncStateRow> {
    Ok(SyncStateRow {
        account_id: row.get(0)?,
        folder: row.get(1)?,
        cursor: row.get(2)?,
        last_run_at: row.get(3)?,
        last_success_at: row.get(4)?,
        status: row.get(5)?,
        last_error: row.get(6)?,
    })
}

fn into_sync_state(row: SyncStateRow) -> RepoResult<SyncState> {
    Ok(SyncState {
        account_id: row.account_id,
        folder: row.folder,
        cursor: row.cursor.map(|c| parse_json(&c)).transpose()?,
        last_run_at: parse_datetime_opt(row.last_run_at)?,
        last_success_at: parse_datetime_opt(row.last_success_at)?,
        status: parse_enum::<SyncStatus>(&row.status)?,
        last_error: row.last_error,
    })
}

const SELECT_COLUMNS: &str =
    "account_id, folder, cursor, last_run_at, last_success_at, status, last_error";

impl SyncStateRepository for Database {
    fn get(&self, account_id: AccountId, folder: &str) -> RepoResult<Option<SyncState>> {
        let conn = self.conn()?;
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM sync_states WHERE account_id = ?1 AND folder = ?2"
        );
        let row = conn
            .query_row(&sql, params![account_id, folder], read_row)
            .optional()
            .map_err(map_err)?;
        row.map(into_sync_state).transpose()
    }

    fn list_for_account(&self, account_id: AccountId) -> RepoResult<Vec<SyncState>> {
        let conn = self.conn()?;
        let sql = format!(
            "SELECT {SELECT_COLUMNS} FROM sync_states WHERE account_id = ?1 ORDER BY folder"
        );
        let mut stmt = conn.prepare(&sql).map_err(map_err)?;
        let rows = stmt
            .query_map(params![account_id], read_row)
            .map_err(map_err)?;
        let mut states = Vec::new();
        for row in rows {
            states.push(into_sync_state(row.map_err(map_err)?)?);
        }
        Ok(states)
    }

    fn record_success(
        &self,
        account_id: AccountId,
        folder: &str,
        cursor: &SyncCursor,
        at: DateTime<Utc>,
    ) -> RepoResult<()> {
        let cursor_json = to_json(cursor)?;
        let at_str = to_rfc3339(at);
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO sync_states (account_id, folder, cursor, last_run_at, last_success_at, status, last_error) \
             VALUES (?1, ?2, ?3, ?4, ?4, 'ok', NULL) \
             ON CONFLICT(account_id, folder) DO UPDATE SET \
                cursor = excluded.cursor, last_run_at = excluded.last_run_at, \
                last_success_at = excluded.last_success_at, status = 'ok', last_error = NULL",
            params![account_id, folder, cursor_json, at_str],
        )
        .map_err(map_err)?;
        Ok(())
    }

    fn record_failure(
        &self,
        account_id: AccountId,
        folder: &str,
        error: &str,
        at: DateTime<Utc>,
    ) -> RepoResult<()> {
        let at_str = to_rfc3339(at);
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO sync_states (account_id, folder, cursor, last_run_at, last_success_at, status, last_error) \
             VALUES (?1, ?2, NULL, ?3, NULL, 'error', ?4) \
             ON CONFLICT(account_id, folder) DO UPDATE SET \
                last_run_at = excluded.last_run_at, status = 'error', last_error = excluded.last_error",
            params![account_id, folder, at_str, error],
        )
        .map_err(map_err)?;
        Ok(())
    }
}
