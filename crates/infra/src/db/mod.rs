//! SQLite connection, migrations and repository implementations.
//!
//! [`Database`] wraps a single `rusqlite::Connection` behind a `Mutex` (SQLite
//! connections are not `Sync`) and implements every repository trait from
//! `academic_core::ports`, one `impl <Trait> for Database` per module in this
//! directory. See `docs/infra/database.md` for the schema and behavior notes.

mod accounts;
mod catalog;
mod processed;
mod results;
mod runs;
mod settings;
mod sync;
pub(crate) mod util;

use std::path::Path;
use std::sync::Mutex;

use academic_core::ports::RepoError;
use chrono::Utc;
use rusqlite::Connection;

use util::map_err;

/// SQL for the one migration this app currently ships. Embedded at compile time so
/// the binary never depends on a file existing next to it at runtime.
const MIGRATION_0001: &str = include_str!("../../migrations/0001_init.sql");

/// Handle to the application's SQLite database. Cheap to clone-share via `Arc`
/// (construct once, wrap in `Arc<Database>`); every method takes `&self` and
/// serializes access through an internal mutex.
pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    /// Opens (creating if necessary) the database file at `path`, enables WAL
    /// journaling and foreign key enforcement, and applies any pending migrations.
    pub fn open(path: &Path) -> Result<Self, RepoError> {
        let conn = Connection::open(path).map_err(map_err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(map_err)?;
        Self::from_connection(conn)
    }

    /// Opens a private, in-memory database (no WAL: SQLite does not support it for
    /// `:memory:` databases). Used by tests.
    pub fn open_in_memory() -> Result<Self, RepoError> {
        let conn = Connection::open_in_memory().map_err(map_err)?;
        Self::from_connection(conn)
    }

    fn from_connection(conn: Connection) -> Result<Self, RepoError> {
        conn.pragma_update(None, "foreign_keys", "ON")
            .map_err(map_err)?;
        apply_migrations(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Locks the underlying connection. A poisoned mutex (a prior panic while
    /// holding the lock) is surfaced as a storage error rather than propagating the
    /// panic, since a single misbehaving caller should not permanently wedge every
    /// other repository call.
    fn conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>, RepoError> {
        self.conn
            .lock()
            .map_err(|_| RepoError::Storage("database connection lock poisoned".into()))
    }
}

/// Tracks which migrations have been applied, so reopening the same database file
/// is idempotent.
fn apply_migrations(conn: &Connection) -> Result<(), RepoError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\
            version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);",
    )
    .map_err(map_err)?;

    let already_applied: bool = conn
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = 1)",
            [],
            |row| row.get(0),
        )
        .map_err(map_err)?;

    if !already_applied {
        conn.execute_batch(MIGRATION_0001).map_err(map_err)?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?1)",
            [Utc::now().to_rfc3339()],
        )
        .map_err(map_err)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_applies_migrations() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'accounts'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn reopening_the_same_file_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        {
            let _db = Database::open(&path).unwrap();
        }
        // Reopening must not fail even though the schema already exists.
        let db = Database::open(&path).unwrap();
        let conn = db.conn().unwrap();
        let version_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(version_count, 1);
    }

    #[test]
    fn open_enables_foreign_keys() {
        let db = Database::open_in_memory().unwrap();
        let conn = db.conn().unwrap();
        let enabled: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(enabled, 1);
    }
}
