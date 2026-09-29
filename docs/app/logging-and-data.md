# Logging and application data

## Data location

Everything the app stores lives in Tauri's app data directory
(`app.path().app_data_dir()`, on Windows `%APPDATA%\dev.eaybar.academic-assistant`):

| Path | Content |
|---|---|
| `academic-assistant.db` (+ `-wal`, `-shm`) | SQLite database (accounts, catalog, sync state, runs, processed-message metadata, results, settings). Email bodies are never stored. |
| `logs/academic-assistant.log.<YYYY-MM-DD>` | Daily rolling application log. |

Secrets (IMAP passwords, Microsoft refresh tokens, the jev API key) are stored only in
the Windows Credential Manager, never in the database or the logs
(`docs/infra/secure-storage.md`).

## Startup sequence (`setup.rs`)

1. Resolve and create the app data directory.
2. Initialize logging (below). The returned writer guard is kept in `AppState`.
3. Open the database (creates the file, enables WAL and foreign keys, applies migrations).
4. `mark_interrupted(now)`: runs still marked `running` (the app was closed or crashed
   mid-run) become `failed` with error summary "Interrupted"; a warning with the count is
   logged when any were changed.
5. Build the adapters and `AppState`, and register it as managed state.

A failure in any step aborts startup: the `setup` hook returns the error and the app exits.

## Logging

- `tracing` with an `EnvFilter`: `RUST_LOG` when set, otherwise `info`.
- File output through `tracing-appender` with daily rotation into `{app_data}/logs`,
  written by a non-blocking worker (the guard must stay alive so buffered lines are
  flushed on exit).
- In debug builds the same events are also written to stderr.

### What is logged

- Startup, interrupted-run recovery, event emission failures.
- Every failed command at `warn`, with the command name, the error `kind`, the
  user-facing `message` and, for internal errors, the internal `detail` (for example the
  SQLite error text). Command arguments are never logged.

### What is never logged

Bodies, subjects, tokens, passwords and API keys. The only free text that can reach the
log is an error message produced by the adapters, which are written to exclude secrets
and message content. `CommandError`s returned to the frontend for storage or secret
failures carry a generic message; the detail exists only in the log.

## Known limitations

- Old log files are not deleted automatically.
- `RUST_LOG` can raise verbosity (for example to `debug`); adapters do not log secrets at
  any level, but the log directory should be treated as private.
