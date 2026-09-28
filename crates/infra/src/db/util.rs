//! Shared row-mapping and timestamp helpers used by every repository implementation
//! in this module, plus the rusqlite -> [`RepoError`] mapping.

use academic_core::ports::RepoError;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};

/// Maps a `rusqlite` error to the shared [`RepoError`] taxonomy: a constraint
/// violation (`UNIQUE`, `FOREIGN KEY`, `CHECK`, ...) becomes `Conflict`, a query that
/// unexpectedly returned no rows becomes `NotFound`, everything else becomes
/// `Storage`. The underlying SQLite message never contains secrets (there are none
/// in this database), so it is safe to surface directly.
pub(crate) fn map_err(err: rusqlite::Error) -> RepoError {
    match &err {
        rusqlite::Error::QueryReturnedNoRows => RepoError::NotFound,
        rusqlite::Error::SqliteFailure(sqlite_err, _)
            if sqlite_err.code == rusqlite::ErrorCode::ConstraintViolation =>
        {
            RepoError::Conflict(err.to_string())
        }
        _ => RepoError::Storage(err.to_string()),
    }
}

/// Parses a value out of a JSON column, mapping a malformed payload to `Storage`
/// (this only happens if the database was corrupted or written by a future version).
pub(crate) fn parse_json<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, RepoError> {
    serde_json::from_str(value).map_err(|e| RepoError::Storage(format!("invalid JSON: {e}")))
}

/// Serializes a value to its JSON column representation.
pub(crate) fn to_json<T: serde::Serialize + ?Sized>(value: &T) -> Result<String, RepoError> {
    serde_json::to_string(value).map_err(|e| RepoError::Storage(format!("invalid JSON: {e}")))
}

/// Formats a UTC timestamp as RFC 3339 for storage.
pub(crate) fn to_rfc3339(dt: DateTime<Utc>) -> String {
    dt.to_rfc3339()
}

/// Parses an RFC 3339 timestamp column back into a UTC timestamp.
pub(crate) fn parse_datetime(value: &str) -> Result<DateTime<Utc>, RepoError> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| RepoError::Storage(format!("invalid timestamp '{value}': {e}")))
}

/// Parses an optional RFC 3339 timestamp column.
pub(crate) fn parse_datetime_opt(
    value: Option<String>,
) -> Result<Option<DateTime<Utc>>, RepoError> {
    value.map(|v| parse_datetime(&v)).transpose()
}

/// Formats a local result date as `YYYY-MM-DD`.
pub(crate) fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Formats a local result time as `HH:MM`.
pub(crate) fn format_time(time: NaiveTime) -> String {
    time.format("%H:%M").to_string()
}

/// Parses a `YYYY-MM-DD` date column.
pub(crate) fn parse_date(value: &str) -> Result<NaiveDate, RepoError> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|e| RepoError::Storage(format!("invalid date '{value}': {e}")))
}

/// Parses an `HH:MM` time column.
pub(crate) fn parse_time(value: &str) -> Result<NaiveTime, RepoError> {
    NaiveTime::parse_from_str(value, "%H:%M")
        .map_err(|e| RepoError::Storage(format!("invalid time '{value}': {e}")))
}

/// Parses a domain enum stored as its `as_str()` text back from a column.
pub(crate) fn parse_enum<T>(value: &str) -> Result<T, RepoError>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    value
        .parse::<T>()
        .map_err(|e| RepoError::Storage(format!("invalid enum value '{value}': {e}")))
}

/// Escapes `%`, `_` and `\` in a user-supplied search term so it can be embedded in
/// a `LIKE ... ESCAPE '\'` pattern without the user's own text being interpreted as
/// wildcards.
pub(crate) fn escape_like(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}
