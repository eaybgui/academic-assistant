# SQLite database (`crates/infra/src/db`)

This module implements the application's SQLite persistence: connection setup,
migrations, and one `impl <Trait> for Database` per repository trait defined in
`academic_core::ports` (`AccountRepository`, `CatalogRepository`,
`SyncStateRepository`, `ProcessedMessageRepository`, `ResultRepository`,
`RunRepository`, `SettingsRepository`).

## Public API

```rust
pub struct Database { /* Mutex<rusqlite::Connection> */ }

impl Database {
    pub fn open(path: &Path) -> Result<Self, RepoError>;
    pub fn open_in_memory() -> Result<Self, RepoError>;
}
```

- `open` opens (creating if missing) the SQLite file at `path`, enables `journal_mode
  = WAL` and `foreign_keys = ON`, and applies any pending migrations.
- `open_in_memory` opens a private `:memory:` database with `foreign_keys = ON` and
  the same migrations applied (no WAL: SQLite does not support WAL journaling for
  in-memory databases). Used by the integration test suite.
- `Database` holds the connection behind a `std::sync::Mutex`; every repository
  method locks it for the duration of the call. A poisoned mutex (a prior panic
  while the lock was held) is surfaced as `RepoError::Storage` rather than
  propagating the panic to every subsequent caller.

## Module layout

| File | Responsibility |
|---|---|
| `mod.rs` | `Database`, `open`/`open_in_memory`, migration bookkeeping |
| `util.rs` | Shared helpers: rusqlite → `RepoError` mapping, JSON (de)serialization, RFC 3339 / `YYYY-MM-DD` / `HH:MM` timestamp (de)serialization, enum parsing, `LIKE` escaping |
| `accounts.rs` | `impl AccountRepository for Database` |
| `catalog.rs` | `impl CatalogRepository for Database` |
| `sync.rs` | `impl SyncStateRepository for Database` |
| `processed.rs` | `impl ProcessedMessageRepository for Database` |
| `results.rs` | `impl ResultRepository for Database` |
| `runs.rs` | `impl RunRepository for Database` |
| `settings.rs` | `impl SettingsRepository for Database` |

## Migrations

`crates/infra/migrations/0001_init.sql` is embedded at compile time with
`include_str!` and reproduces the schema in the design spec (§9) exactly. A
`schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT)` table (created
directly in `mod.rs`, not part of the migration file itself) tracks which
migrations have run. `Database::open`/`open_in_memory` check whether version `1` is
recorded; if not, they run `0001_init.sql` and record it, both inside one
`rusqlite` transaction (`Connection::unchecked_transaction`, safe here since this runs
before the connection is shared behind its `Mutex`) — a failure partway through never
leaves the schema applied without the version recorded, or vice versa. Reopening an
already-migrated file is therefore a no-op beyond that check — safe to call on
every app startup.

`Database::open`/`open_in_memory` also register a `unicode_lower(text)` SQL scalar
function on the connection (see "Behavior notes" below), independently of migrations —
this requires the `rusqlite` crate's `functions` Cargo feature, enabled on the workspace
dependency in the root `Cargo.toml`.

## Column encoding

- **Timestamps** (`created_at`, `received_at`, `started_at`, ...): RFC 3339 UTC text
  (`DateTime<Utc>::to_rfc3339()` / `DateTime::parse_from_rfc3339`).
- **Result dates** (`result_dates.date` / `.time`): local `YYYY-MM-DD` /
  `HH:MM` text, matching the design spec and the DTOs the frontend consumes.
- **Domain enums** (`provider_type`, `status`, `category`, `priority`, `kind`,
  `outcome`, `trigger`, `error_kind`, ...): stored as their `as_str()` snake_case
  text and parsed back with `FromStr`, via the shared `util::parse_enum` helper.
- **`accounts.id`**: `INTEGER PRIMARY KEY AUTOINCREMENT`, so the id of a deleted
  account is never handed to a later one. Secrets are keyed by account id in the
  credential store; reusing an id could give a new account the refresh token or password
  left behind by a removed one. (`db_accounts.rs` checks that deleting the highest id and
  inserting again yields a greater id.)
- **`accounts.config`**: JSON *without* the type tag — `{"host","port","username"}`
  for IMAP, `{"tenant"}` for Microsoft Graph — because the tag is already stored
  separately in `provider_type`. `accounts.folders`, `courses.aliases` and every
  `settings.value` are plain `serde_json` JSON.
- **`sync_states.cursor`**: `SyncCursor` serialized as-is via serde (it carries its
  own `#[serde(tag = "type")]`), nullable.
- **Booleans** (`enabled`, `action_required`, `possible_change`, `seen`): SQLite
  `INTEGER` 0/1, read/written through `rusqlite`'s built-in `bool` support.

## Error mapping

`util::map_err` is the single place that turns a `rusqlite::Error` into a
`RepoError`:

- `rusqlite::Error::SqliteFailure` with `ErrorCode::ConstraintViolation` and extended
  code `SQLITE_CONSTRAINT_FOREIGNKEY` (a `FOREIGN KEY` violation, e.g. `add_course` on a
  `university_id` that doesn't exist) → `RepoError::NotFound`, since it means the row
  the new row references is missing.
- Any other `ConstraintViolation` (`UNIQUE` or `CHECK`) → `RepoError::Conflict(<sqlite
  message>)`. The SQLite message never contains secrets (there are none in this
  database), so it is safe to surface directly.
- `rusqlite::Error::QueryReturnedNoRows` → `RepoError::NotFound`.
- Everything else → `RepoError::Storage(<message>)`.

Repository methods that update or delete a row by id (`AccountRepository::update`/
`delete`, `CatalogRepository::rename_university`/`delete_university`/
`update_course`/`delete_course`, `RunRepository::finish`, `ResultRepository::set_seen`)
additionally check the affected-row count and return `RepoError::NotFound` when it
is zero, since a `rusqlite::execute` with no matching row does not itself error.

## Behavior notes

### `unicode_lower` SQL function

`Database::open`/`open_in_memory` register a deterministic scalar function
`unicode_lower(text)` on the connection (`db::register_unicode_lower`, in `mod.rs`),
implemented with Rust's `str::to_lowercase`. It passes a SQL `NULL` argument straight
through as `NULL` rather than erroring, since it's applied to nullable columns
(`ai_results.sender_name`) in `ResultRepository::query`'s `search` filter (see below).

### Cascades

Foreign keys are enforced (`PRAGMA foreign_keys = ON`), so the schema's
`ON DELETE CASCADE` / `ON DELETE SET NULL` clauses do the cascading:

- Deleting a **university** cascades its courses and sets
  `accounts.default_university_id` to `NULL` on any account that referenced it.
- Deleting an **account** cascades its `sync_states`, `processed_messages` (and
  transitively their `ai_results`/`result_dates`, since `ai_results.processed_message_id`
  references `processed_messages(id) ON DELETE CASCADE`) and `run_accounts` rows.

### `ProcessedMessageRepository::is_duplicate`

A message is a duplicate for an account when either its `Message-ID` header
matches a previously processed message for that account, or its content hash does
— matching the `idx_processed_msgid`/`idx_processed_hash` indexes, both scoped by
`account_id`. When the new message has no `Message-ID` header, the check falls back
to the content hash alone: two headerless emails are not considered duplicates of
each other purely by both lacking a header.

### `ResultRepository::query`

- Filters (`search`, `seen`, `category`, `account_id`, `course_id`) combine with
  `AND`; only the filters actually set in `ResultQuery` are applied.
- `search` is a case-insensitive substring match over `subject`, `sender_name` and
  `sender_email`, using `unicode_lower(column) LIKE unicode_lower(?) ESCAPE '\'` (both
  sides lowercased through the connection-registered `unicode_lower` scalar function —
  see below) with the user's `%`, `_` and `\` escaped first, so literal wildcard
  characters in the search term cannot expand the match. SQLite's built-in `LIKE` only
  case-folds ASCII `A`-`Z`; `unicode_lower` uses Rust's `str::to_lowercase`, so e.g.
  `"álgebra"` matches `"ÁLGEBRA"`.
- `total` in the returned `Page` counts every match ignoring `limit`/`offset`.
- Sort semantics:
  - `Upcoming`: a `LEFT JOIN` subquery computes each result's minimum
    `result_dates.date >= today` (the `today` parameter). Results with such a date
    sort first, ordered by that date ascending; the rest sort by `received_at`
    descending. Every branch ends with `ai_results.id` as a stable tiebreak.
  - `Priority`: `urgent` → `low`, ties broken by `received_at` descending.
  - `Received`: `received_at` descending.
- `save_analyzed` runs as a single `rusqlite` transaction: it inserts the
  `processed_messages` row (outcome as given, expected to be `Analyzed`), the
  `ai_results` row, and one `result_dates` row per date, committing only if every
  insert succeeds.

### `ResultRepository::unseen_counts`

Counts only unseen (`seen = 0`) results: `new_total` is every unseen result,
`tasks`/`exams`/`events` are unseen results in that single category, and
`changes_and_notices` is unseen results in `change` **or** `notice` (per the design
spec §10.2). A result whose category is `general_info` counts toward `new_total`
only, matching no per-category bucket.

### `RunRepository`

- `history`/`latest` order by `started_at` descending, `id` descending as a
  tiebreak.
- `get`/`latest`/`history` all join `run_accounts` with `accounts` to populate
  `RunAccountView.account_name`, ordered by `account_id`.
- `mark_interrupted` sets every run still in `status = 'running'` to `failed` with
  `error_summary = "Interrupted"` and the given `finished_at`, returning the number
  of rows changed.

## Testing

Integration tests live in `crates/infra/tests/db_*.rs`, one file per repository
area (`db_accounts.rs`, `db_catalog.rs`, `db_sync.rs`, `db_processed.rs`,
`db_results.rs`, `db_runs.rs`, `db_settings.rs`, `db_migrations.rs`), each using
`Database::open_in_memory()`. `crates/infra/tests/support/mod.rs` holds shared
fixtures (building an `AccountDraft`, inserting a throwaway account/run, building a
minimal `ProcessedMessage`/`NewResult`) and is included per test binary via
`#[path = "support/mod.rs"] mod support;` — a `tests/` subdirectory is not
auto-discovered by cargo as its own test target, so this file does not itself run
as a test binary.

Run the suite with:

```sh
cargo test -p academic-infra
```

## Known limitations

- `Database` does not create missing parent directories for the path passed to
  `open`; the caller (composition root, Task T14) is expected to ensure the app
  data directory exists first.
- There is currently only one migration (`0001_init.sql`). The `schema_migrations`
  tracking table is ready for future migrations to be added the same way (check the
  next version number, apply it, record it), but no upgrade path beyond version 1
  exists yet.
