//! `RunRepository` implementation.

use academic_core::domain::{
    Counters, ErrorKind, RunAccountResult, RunAccountStatus, RunAccountView, RunId, RunRecord,
    RunStatus, RunTrigger,
};
use academic_core::ports::{RepoError, RepoResult, RunRepository};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};

use super::util::{map_err, parse_datetime, parse_datetime_opt, parse_enum, to_rfc3339};
use super::Database;

const RUN_COLUMNS: &str = "id, trigger, started_at, finished_at, status, accounts_processed, \
    messages_found, messages_filtered, messages_analyzed, messages_failed, results_created, \
    error_summary";

struct RunRow {
    id: i64,
    trigger: String,
    started_at: String,
    finished_at: Option<String>,
    status: String,
    accounts_processed: i64,
    counters: Counters,
    error_summary: Option<String>,
}

fn read_run_row(row: &Row) -> rusqlite::Result<RunRow> {
    Ok(RunRow {
        id: row.get(0)?,
        trigger: row.get(1)?,
        started_at: row.get(2)?,
        finished_at: row.get(3)?,
        status: row.get(4)?,
        accounts_processed: row.get(5)?,
        counters: Counters {
            found: row.get::<_, i64>(6)? as u32,
            filtered: row.get::<_, i64>(7)? as u32,
            analyzed: row.get::<_, i64>(8)? as u32,
            failed: row.get::<_, i64>(9)? as u32,
            results_created: row.get::<_, i64>(10)? as u32,
        },
        error_summary: row.get(11)?,
    })
}

struct RunAccountRow {
    account_id: i64,
    account_name: String,
    status: String,
    counters: Counters,
    error_kind: Option<String>,
    error_message: Option<String>,
}

fn read_run_account_row(row: &Row) -> rusqlite::Result<RunAccountRow> {
    Ok(RunAccountRow {
        account_id: row.get(0)?,
        account_name: row.get(1)?,
        status: row.get(2)?,
        counters: Counters {
            found: row.get::<_, i64>(3)? as u32,
            filtered: row.get::<_, i64>(4)? as u32,
            analyzed: row.get::<_, i64>(5)? as u32,
            failed: row.get::<_, i64>(6)? as u32,
            results_created: row.get::<_, i64>(7)? as u32,
        },
        error_kind: row.get(8)?,
        error_message: row.get(9)?,
    })
}

fn into_run_account_view(row: RunAccountRow) -> RepoResult<RunAccountView> {
    let error_kind = row
        .error_kind
        .map(|k| parse_enum::<ErrorKind>(&k))
        .transpose()?;
    Ok(RunAccountView {
        account_name: row.account_name,
        result: RunAccountResult {
            account_id: row.account_id,
            status: parse_enum::<RunAccountStatus>(&row.status)?,
            counters: row.counters,
            error_kind,
            error_message: row.error_message,
        },
    })
}

fn into_run_record(row: RunRow, accounts: Vec<RunAccountView>) -> RepoResult<RunRecord> {
    Ok(RunRecord {
        id: row.id,
        trigger: parse_enum::<RunTrigger>(&row.trigger)?,
        started_at: parse_datetime(&row.started_at)?,
        finished_at: parse_datetime_opt(row.finished_at)?,
        status: parse_enum::<RunStatus>(&row.status)?,
        accounts_processed: row.accounts_processed as u32,
        counters: row.counters,
        error_summary: row.error_summary,
        accounts,
    })
}

fn fetch_run(conn: &Connection, run_id: RunId) -> RepoResult<Option<RunRecord>> {
    let sql = format!("SELECT {RUN_COLUMNS} FROM runs WHERE id = ?1");
    let row = conn
        .query_row(&sql, params![run_id], read_run_row)
        .optional()
        .map_err(map_err)?;
    let Some(row) = row else {
        return Ok(None);
    };

    let mut stmt = conn
        .prepare(
            "SELECT run_accounts.account_id, accounts.name, run_accounts.status, \
             run_accounts.messages_found, run_accounts.messages_filtered, \
             run_accounts.messages_analyzed, run_accounts.messages_failed, \
             run_accounts.results_created, run_accounts.error_kind, run_accounts.error_message \
             FROM run_accounts JOIN accounts ON accounts.id = run_accounts.account_id \
             WHERE run_accounts.run_id = ?1 ORDER BY run_accounts.account_id",
        )
        .map_err(map_err)?;
    let rows = stmt
        .query_map(params![run_id], read_run_account_row)
        .map_err(map_err)?;
    let mut accounts = Vec::new();
    for row in rows {
        accounts.push(into_run_account_view(row.map_err(map_err)?)?);
    }

    Ok(Some(into_run_record(row, accounts)?))
}

impl RunRepository for Database {
    fn create(&self, trigger: RunTrigger, started_at: DateTime<Utc>) -> RepoResult<RunId> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO runs (trigger, started_at, status, accounts_processed, \
             messages_found, messages_filtered, messages_analyzed, messages_failed, results_created) \
             VALUES (?1, ?2, 'running', 0, 0, 0, 0, 0, 0)",
            params![trigger.as_str(), to_rfc3339(started_at)],
        )
        .map_err(map_err)?;
        Ok(conn.last_insert_rowid())
    }

    fn record_account(&self, run_id: RunId, result: &RunAccountResult) -> RepoResult<()> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO run_accounts (run_id, account_id, status, messages_found, \
             messages_filtered, messages_analyzed, messages_failed, results_created, \
             error_kind, error_message) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                run_id,
                result.account_id,
                result.status.as_str(),
                result.counters.found as i64,
                result.counters.filtered as i64,
                result.counters.analyzed as i64,
                result.counters.failed as i64,
                result.counters.results_created as i64,
                result.error_kind.map(|k| k.as_str()),
                result.error_message,
            ],
        )
        .map_err(map_err)?;
        Ok(())
    }

    fn finish(
        &self,
        run_id: RunId,
        status: RunStatus,
        finished_at: DateTime<Utc>,
        accounts_processed: u32,
        counters: &Counters,
        error_summary: Option<&str>,
    ) -> RepoResult<()> {
        let conn = self.conn()?;
        let changed = conn
            .execute(
                "UPDATE runs SET finished_at = ?1, status = ?2, accounts_processed = ?3, \
                 messages_found = ?4, messages_filtered = ?5, messages_analyzed = ?6, \
                 messages_failed = ?7, results_created = ?8, error_summary = ?9 WHERE id = ?10",
                params![
                    to_rfc3339(finished_at),
                    status.as_str(),
                    accounts_processed as i64,
                    counters.found as i64,
                    counters.filtered as i64,
                    counters.analyzed as i64,
                    counters.failed as i64,
                    counters.results_created as i64,
                    error_summary,
                    run_id,
                ],
            )
            .map_err(map_err)?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }

    fn get(&self, run_id: RunId) -> RepoResult<Option<RunRecord>> {
        let conn = self.conn()?;
        fetch_run(&conn, run_id)
    }

    fn latest(&self) -> RepoResult<Option<RunRecord>> {
        let conn = self.conn()?;
        let id: Option<i64> = conn
            .query_row(
                "SELECT id FROM runs ORDER BY started_at DESC, id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(map_err)?;
        match id {
            Some(id) => fetch_run(&conn, id),
            None => Ok(None),
        }
    }

    fn history(&self, limit: u32) -> RepoResult<Vec<RunRecord>> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT id FROM runs ORDER BY started_at DESC, id DESC LIMIT ?1")
            .map_err(map_err)?;
        let ids = stmt
            .query_map(params![limit as i64], |row| row.get::<_, i64>(0))
            .map_err(map_err)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(map_err)?;

        let mut records = Vec::with_capacity(ids.len());
        for id in ids {
            if let Some(record) = fetch_run(&conn, id)? {
                records.push(record);
            }
        }
        Ok(records)
    }

    fn mark_interrupted(&self, at: DateTime<Utc>) -> RepoResult<u64> {
        let conn = self.conn()?;
        let changed = conn
            .execute(
                "UPDATE runs SET status = 'failed', error_summary = 'Interrupted', \
                 finished_at = ?1 WHERE status = 'running'",
                params![to_rfc3339(at)],
            )
            .map_err(map_err)?;
        Ok(changed as u64)
    }
}
