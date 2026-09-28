//! `ResultRepository` implementation.

use std::collections::HashMap;

use academic_core::domain::{
    Category, DashboardCounts, DateKind, NewResult, Page, Priority, ProcessedMessage, ResultDate,
    ResultQuery, ResultSort, StoredResult,
};
use academic_core::ports::{RepoError, RepoResult, ResultRepository};
use chrono::{DateTime, NaiveDate, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row, ToSql};

use super::util::{
    escape_like, format_date, format_time, map_err, parse_date, parse_datetime, parse_datetime_opt,
    parse_enum, parse_time, to_rfc3339,
};
use super::Database;

type SqlParam = Box<dyn ToSql>;

const RESULT_SELECT: &str = "SELECT ai_results.id, ai_results.account_id, accounts.name, \
    ai_results.subject, ai_results.sender_name, ai_results.sender_email, universities.name, \
    courses.name, ai_results.category, ai_results.priority, ai_results.action_required, \
    ai_results.possible_change, ai_results.received_at, ai_results.detected_at, \
    ai_results.source_url, processed_messages.message_id_header, ai_results.seen, \
    ai_results.seen_at \
    FROM ai_results \
    JOIN accounts ON accounts.id = ai_results.account_id \
    JOIN processed_messages ON processed_messages.id = ai_results.processed_message_id \
    LEFT JOIN universities ON universities.id = ai_results.university_id \
    LEFT JOIN courses ON courses.id = ai_results.course_id";

struct RawResultRow {
    id: i64,
    account_id: i64,
    account_name: String,
    subject: String,
    sender_name: Option<String>,
    sender_email: String,
    university_name: Option<String>,
    course_name: Option<String>,
    category: String,
    priority: String,
    action_required: bool,
    possible_change: bool,
    received_at: String,
    detected_at: String,
    source_url: Option<String>,
    message_id_header: Option<String>,
    seen: bool,
    seen_at: Option<String>,
}

fn read_result_row(row: &Row) -> rusqlite::Result<RawResultRow> {
    Ok(RawResultRow {
        id: row.get(0)?,
        account_id: row.get(1)?,
        account_name: row.get(2)?,
        subject: row.get(3)?,
        sender_name: row.get(4)?,
        sender_email: row.get(5)?,
        university_name: row.get(6)?,
        course_name: row.get(7)?,
        category: row.get(8)?,
        priority: row.get(9)?,
        action_required: row.get(10)?,
        possible_change: row.get(11)?,
        received_at: row.get(12)?,
        detected_at: row.get(13)?,
        source_url: row.get(14)?,
        message_id_header: row.get(15)?,
        seen: row.get(16)?,
        seen_at: row.get(17)?,
    })
}

fn into_stored_result(raw: RawResultRow, dates: Vec<ResultDate>) -> RepoResult<StoredResult> {
    Ok(StoredResult {
        id: raw.id,
        account_id: raw.account_id,
        account_name: raw.account_name,
        subject: raw.subject,
        sender_name: raw.sender_name,
        sender_email: raw.sender_email,
        university_name: raw.university_name,
        course_name: raw.course_name,
        category: parse_enum::<Category>(&raw.category)?,
        priority: parse_enum::<Priority>(&raw.priority)?,
        action_required: raw.action_required,
        possible_change: raw.possible_change,
        dates,
        received_at: parse_datetime(&raw.received_at)?,
        detected_at: parse_datetime(&raw.detected_at)?,
        source_url: raw.source_url,
        message_id_header: raw.message_id_header,
        seen: raw.seen,
        seen_at: parse_datetime_opt(raw.seen_at)?,
    })
}

/// Builds the `WHERE` clause shared by the count and page queries: every set filter
/// combines with `AND`. `search` is a case-insensitive substring match (SQLite's
/// default `LIKE` collation) over subject, sender name and sender email, with the
/// user's input escaped so literal `%`/`_` are not treated as wildcards.
fn build_where(query: &ResultQuery) -> (String, Vec<SqlParam>) {
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<SqlParam> = Vec::new();

    if let Some(search) = &query.search {
        let pattern = format!("%{}%", escape_like(search));
        clauses.push(
            "(ai_results.subject LIKE ? ESCAPE '\\' \
              OR ai_results.sender_name LIKE ? ESCAPE '\\' \
              OR ai_results.sender_email LIKE ? ESCAPE '\\')"
                .to_string(),
        );
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern.clone()));
        params.push(Box::new(pattern));
    }
    if let Some(seen) = query.seen {
        clauses.push("ai_results.seen = ?".to_string());
        params.push(Box::new(seen));
    }
    if let Some(category) = query.category {
        clauses.push("ai_results.category = ?".to_string());
        params.push(Box::new(category.as_str()));
    }
    if let Some(account_id) = query.account_id {
        clauses.push("ai_results.account_id = ?".to_string());
        params.push(Box::new(account_id));
    }
    if let Some(course_id) = query.course_id {
        clauses.push("ai_results.course_id = ?".to_string());
        params.push(Box::new(course_id));
    }

    let where_sql = if clauses.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", clauses.join(" AND "))
    };
    (where_sql, params)
}

fn count(conn: &Connection, query: &ResultQuery) -> RepoResult<u64> {
    let (where_sql, params) = build_where(query);
    let sql = format!("SELECT COUNT(*) FROM ai_results {where_sql}");
    let param_refs: Vec<&dyn ToSql> = params.iter().map(|p| p.as_ref()).collect();
    let total: i64 = conn
        .query_row(&sql, param_refs.as_slice(), |row| row.get(0))
        .map_err(map_err)?;
    Ok(total as u64)
}

/// Fetches one page of results in the order `query.sort` specifies. `Upcoming`
/// needs a per-result "next date on or after today" computed via a correlated
/// subquery (`nd`), joined in only for that sort; every sort ends with a stable
/// tiebreak on `ai_results.id` so pagination never reorders ties across pages.
fn fetch_page(
    conn: &Connection,
    query: &ResultQuery,
    today: NaiveDate,
) -> RepoResult<Vec<StoredResult>> {
    let (where_sql, where_params) = build_where(query);

    let (join_sql, order_sql, today_param): (&str, &str, Option<String>) = match query.sort {
        ResultSort::Upcoming => (
            "LEFT JOIN (SELECT result_id, MIN(date) AS next_date FROM result_dates \
             WHERE date >= ? GROUP BY result_id) nd ON nd.result_id = ai_results.id",
            "ORDER BY (nd.next_date IS NULL) ASC, nd.next_date ASC, \
             ai_results.received_at DESC, ai_results.id ASC",
            Some(format_date(today)),
        ),
        ResultSort::Priority => (
            "",
            "ORDER BY CASE ai_results.priority \
                WHEN 'urgent' THEN 0 WHEN 'high' THEN 1 WHEN 'medium' THEN 2 WHEN 'low' THEN 3 \
                ELSE 4 END ASC, \
             ai_results.received_at DESC, ai_results.id DESC",
            None,
        ),
        ResultSort::Received => (
            "",
            "ORDER BY ai_results.received_at DESC, ai_results.id DESC",
            None,
        ),
    };

    let sql = format!("{RESULT_SELECT} {join_sql} {where_sql} {order_sql} LIMIT ? OFFSET ?");

    let mut all_params: Vec<SqlParam> = Vec::new();
    if let Some(today_str) = today_param {
        all_params.push(Box::new(today_str));
    }
    all_params.extend(where_params);
    all_params.push(Box::new(query.limit as i64));
    all_params.push(Box::new(query.offset as i64));
    let param_refs: Vec<&dyn ToSql> = all_params.iter().map(|p| p.as_ref()).collect();

    let mut stmt = conn.prepare(&sql).map_err(map_err)?;
    let rows = stmt
        .query_map(param_refs.as_slice(), read_result_row)
        .map_err(map_err)?;
    let mut raw_rows = Vec::new();
    for row in rows {
        raw_rows.push(row.map_err(map_err)?);
    }

    let ids: Vec<i64> = raw_rows.iter().map(|r| r.id).collect();
    let mut dates_by_id = fetch_dates_for(conn, &ids)?;

    let mut results = Vec::with_capacity(raw_rows.len());
    for raw in raw_rows {
        let dates = dates_by_id.remove(&raw.id).unwrap_or_default();
        results.push(into_stored_result(raw, dates)?);
    }
    Ok(results)
}

fn fetch_dates_for(conn: &Connection, ids: &[i64]) -> RepoResult<HashMap<i64, Vec<ResultDate>>> {
    let mut map: HashMap<i64, Vec<ResultDate>> = HashMap::new();
    if ids.is_empty() {
        return Ok(map);
    }
    let placeholders = ids.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
    let sql = format!(
        "SELECT result_id, date, time, kind FROM result_dates \
         WHERE result_id IN ({placeholders}) ORDER BY result_id, id"
    );
    let param_refs: Vec<&dyn ToSql> = ids.iter().map(|id| id as &dyn ToSql).collect();
    let mut stmt = conn.prepare(&sql).map_err(map_err)?;
    let rows = stmt
        .query_map(param_refs.as_slice(), |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(map_err)?;
    for row in rows {
        let (result_id, date_str, time_str, kind_str) = row.map_err(map_err)?;
        let date = parse_date(&date_str)?;
        let time = time_str.map(|t| parse_time(&t)).transpose()?;
        let kind = parse_enum::<DateKind>(&kind_str)?;
        map.entry(result_id)
            .or_default()
            .push(ResultDate { date, time, kind });
    }
    Ok(map)
}

impl ResultRepository for Database {
    fn save_analyzed(&self, message: &ProcessedMessage, result: &NewResult) -> RepoResult<i64> {
        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_err)?;

        tx.execute(
            "INSERT INTO processed_messages (account_id, folder, provider_message_id, \
             message_id_header, content_hash, received_at, outcome, reason, run_id, processed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                message.account_id,
                message.folder,
                message.provider_message_id,
                message.message_id_header,
                message.content_hash,
                to_rfc3339(message.received_at),
                message.outcome.as_str(),
                message.reason,
                message.run_id,
                to_rfc3339(message.processed_at),
            ],
        )
        .map_err(map_err)?;
        let processed_message_id = tx.last_insert_rowid();

        let analysis = &result.analysis;
        tx.execute(
            "INSERT INTO ai_results (account_id, processed_message_id, run_id, detected_at, \
             received_at, subject, sender_name, sender_email, university_id, course_id, \
             category, priority, action_required, possible_change, confidence, model_provider, \
             model_name, source_url, seen, seen_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, 0, NULL)",
            params![
                result.account_id,
                processed_message_id,
                result.run_id,
                to_rfc3339(result.detected_at),
                to_rfc3339(result.received_at),
                result.subject,
                result.sender_name,
                result.sender_email,
                analysis.university_id,
                analysis.course_id,
                analysis.category.as_str(),
                analysis.priority.as_str(),
                analysis.action_required,
                analysis.possible_change,
                analysis.confidence.map(|c| c as f64),
                analysis.model.provider,
                analysis.model.model,
                result.source_url,
            ],
        )
        .map_err(map_err)?;
        let result_id = tx.last_insert_rowid();

        for date in &analysis.dates {
            tx.execute(
                "INSERT INTO result_dates (result_id, date, time, kind) VALUES (?1, ?2, ?3, ?4)",
                params![
                    result_id,
                    format_date(date.date),
                    date.time.map(format_time),
                    date.kind.as_str(),
                ],
            )
            .map_err(map_err)?;
        }

        tx.commit().map_err(map_err)?;
        Ok(result_id)
    }

    fn query(&self, query: &ResultQuery, today: NaiveDate) -> RepoResult<Page<StoredResult>> {
        let conn = self.conn()?;
        let total = count(&conn, query)?;
        let items = fetch_page(&conn, query, today)?;
        Ok(Page { items, total })
    }

    fn get(&self, id: i64) -> RepoResult<Option<StoredResult>> {
        let conn = self.conn()?;
        let sql = format!("{RESULT_SELECT} WHERE ai_results.id = ?1");
        let raw = conn
            .query_row(&sql, params![id], read_result_row)
            .optional()
            .map_err(map_err)?;
        let Some(raw) = raw else {
            return Ok(None);
        };
        let mut dates_by_id = fetch_dates_for(&conn, &[raw.id])?;
        let dates = dates_by_id.remove(&raw.id).unwrap_or_default();
        Ok(Some(into_stored_result(raw, dates)?))
    }

    fn set_seen(&self, id: i64, seen: bool, at: DateTime<Utc>) -> RepoResult<StoredResult> {
        let conn = self.conn()?;
        let seen_at = if seen { Some(to_rfc3339(at)) } else { None };
        let changed = conn
            .execute(
                "UPDATE ai_results SET seen = ?1, seen_at = ?2 WHERE id = ?3",
                params![seen, seen_at, id],
            )
            .map_err(map_err)?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        let sql = format!("{RESULT_SELECT} WHERE ai_results.id = ?1");
        let raw = conn
            .query_row(&sql, params![id], read_result_row)
            .map_err(map_err)?;
        let mut dates_by_id = fetch_dates_for(&conn, &[id])?;
        let dates = dates_by_id.remove(&id).unwrap_or_default();
        into_stored_result(raw, dates)
    }

    fn mark_all_seen(&self, at: DateTime<Utc>) -> RepoResult<u64> {
        let conn = self.conn()?;
        let changed = conn
            .execute(
                "UPDATE ai_results SET seen = 1, seen_at = ?1 WHERE seen = 0",
                params![to_rfc3339(at)],
            )
            .map_err(map_err)?;
        Ok(changed as u64)
    }

    fn unseen_counts(&self) -> RepoResult<DashboardCounts> {
        let conn = self.conn()?;
        let (new_total, tasks, exams, events, changes_and_notices): (i64, i64, i64, i64, i64) =
            conn.query_row(
                "SELECT \
                    COALESCE(SUM(CASE WHEN seen = 0 THEN 1 ELSE 0 END), 0), \
                    COALESCE(SUM(CASE WHEN seen = 0 AND category = 'task' THEN 1 ELSE 0 END), 0), \
                    COALESCE(SUM(CASE WHEN seen = 0 AND category = 'exam' THEN 1 ELSE 0 END), 0), \
                    COALESCE(SUM(CASE WHEN seen = 0 AND category = 'event' THEN 1 ELSE 0 END), 0), \
                    COALESCE(SUM(CASE WHEN seen = 0 AND category IN ('change', 'notice') \
                        THEN 1 ELSE 0 END), 0) \
                 FROM ai_results",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .map_err(map_err)?;
        Ok(DashboardCounts {
            new_total: new_total as u32,
            tasks: tasks as u32,
            exams: exams as u32,
            events: events as u32,
            changes_and_notices: changes_and_notices as u32,
        })
    }
}
