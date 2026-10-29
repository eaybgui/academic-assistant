//! SQLite connection, migrations and repository implementations.
//!
//! [`Database`] wraps a single `rusqlite::Connection` behind a `Mutex` (SQLite
//! connections are not `Sync`) and implements every repository trait from
//! `academic_core::ports`, one `impl <Trait> for Database` per module in this
//! directory. See `docs/infra/database.md` for the schema and behavior notes.

mod accounts;
mod catalog;
mod courses;
mod items;
mod processed;
mod results;
mod runs;
mod settings;
mod sync;
mod universities;
pub(crate) mod util;

use std::path::Path;
use std::sync::Mutex;

use academic_core::ports::RepoError;
use chrono::Utc;
use rusqlite::functions::FunctionFlags;
use rusqlite::Connection;

use util::map_err;

/// Every migration, in order. Embedded at compile time so the binary never depends
/// on files next to it at runtime. Versions are never edited once released.
const MIGRATIONS: [(i64, &str); 3] = [
    (1, include_str!("../../migrations/0001_init.sql")),
    (2, include_str!("../../migrations/0002_lms.sql")),
    (3, include_str!("../../migrations/0003_lms_items.sql")),
];

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
        register_unicode_lower(&conn)?;
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

/// Registers `unicode_lower(text)` on `conn`, used by `ResultRepository::query`'s
/// `search` filter so it matches case-insensitively beyond ASCII (SQLite's built-in
/// `LIKE` only case-folds `A`-`Z`). Deterministic: same input always yields the same
/// output, letting SQLite cache/optimize calls. Passes a SQL `NULL` argument straight
/// through as `NULL` (e.g. `ai_results.sender_name` is nullable) rather than erroring.
fn register_unicode_lower(conn: &Connection) -> Result<(), RepoError> {
    conn.create_scalar_function(
        "unicode_lower",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let text = ctx.get::<Option<String>>(0)?;
            Ok(text.map(|value| value.to_lowercase()))
        },
    )
    .map_err(map_err)
}

/// Tracks which migrations have been applied, so reopening the same database file
/// is idempotent. Each pending migration and its `schema_migrations` row are applied
/// in their own transaction, so a failure partway through never leaves the schema
/// updated without the version recorded (or vice versa).
fn apply_migrations(conn: &Connection) -> Result<(), RepoError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (\
            version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);",
    )
    .map_err(map_err)?;

    for (version, sql) in MIGRATIONS {
        let applied: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
                [version],
                |row| row.get(0),
            )
            .map_err(map_err)?;
        if applied {
            continue;
        }
        // `unchecked_transaction` is safe here: `from_connection` runs before the
        // connection is shared behind its `Mutex`, so nothing else can be using it
        // concurrently.
        let tx = conn.unchecked_transaction().map_err(map_err)?;
        tx.execute_batch(sql).map_err(map_err)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            rusqlite::params![version, Utc::now().to_rfc3339()],
        )
        .map_err(map_err)?;
        tx.commit().map_err(map_err)?;
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
        assert_eq!(version_count, MIGRATIONS.len() as i64);
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
