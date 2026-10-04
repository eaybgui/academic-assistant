# Testing

The default test commands never touch real services, the Windows Credential Manager or the
network: everything external is replaced by fakes, fixtures or a local mock server. Tests that
need a real service are marked `#[ignore]` and run only when you ask for them.

## Suites at a glance

| Suite | Location | Run |
|---|---|---|
| Core unit tests | `#[cfg(test)]` modules in `crates/core/src/` | `cargo test -p academic-core --lib` |
| Core pipeline integration | `crates/core/tests/pipeline.rs` (+ `tests/support/` fakes) | `cargo test -p academic-core --test pipeline` |
| Infra unit tests | `#[cfg(test)]` modules in `crates/infra/src/` | `cargo test -p academic-infra --lib` |
| Infra integration | `crates/infra/tests/*.rs` (+ `fixtures/`, `support/`) | `cargo test -p academic-infra` |
| Tauri crate | unit tests in `src-tauri/src/` (DTO JSON shape, error mapping, validation, commands over the `test_support` harness) | `cargo test -p academic-assistant` |
| Frontend | `src/**/*.test.ts(x)` (Vitest + Testing Library + jsdom) | `npm run test` |
| Everything (Rust) | | `cargo test --workspace` |

## Core (`crates/core`)

**Unit tests** live next to the code and use no I/O:

- `domain/`: every enum's `as_str`/`FromStr` round trip, `SyncCursor` JSON shape, `Settings`
  defaults, secret key helpers, `Counters`.
- `ports/`: `ProviderError::kind`, `ModelError::is_transient`.
- `normalize.rs` and `filters.rs`: HTML to text, quote and signature stripping, truncation, hash
  stability, every filter reason and precedence.
- `dates/` (`dates/tests.rs` plus module tests): Spanish and English absolute formats, relative
  dates, the `mañana` "morning" exclusion, time attachment, year resolution, deduplication.
- `analyzer/`: conditional question building (`university`, `course`, `date_*`), answer assembly
  and validation.
- `pipeline/`: the run lock and message reason sanitizing.

**Pipeline integration tests** (`crates/core/tests/pipeline.rs`) drive the full `CheckRunner`
against in-memory fakes in `crates/core/tests/support/`: `TestStore` (all seven repository
traits on one shared store), a fake provider factory with queued `fetch_new` responses, a fake
`StructuredModel` and `ModelProvider`, a recording `RunObserver` and a fixed `Clock`. They cover
deduplication, each filter reason, irrelevant and invalid-response handling, transient model
errors not advancing the cursor, per-account error isolation, run status aggregation, the
single-run lock and preconditions, and the progress event sequence. See
[core/pipeline.md](core/pipeline.md#how-to-test).

```powershell
cargo test -p academic-core                       # unit + pipeline integration
cargo test -p academic-core --lib normalize       # a subset by name filter
```

## Infra (`crates/infra`)

- **Database** (`db_accounts`, `db_catalog`, `db_migrations`, `db_processed`, `db_results`,
  `db_runs`, `db_settings`, `db_sync`): repositories against `Database::open_in_memory()`,
  including sort semantics, unicode search, cascade deletes and migration idempotence. The shared
  helpers in `tests/support/mod.rs` are pulled in with `#[path = "support/mod.rs"] mod support;`.
- **IMAP** (`imap_parse`, `imap_cursor`): message parsing from `.eml` fixtures and UID batch
  planning, both without a server. Fixtures in `crates/infra/tests/fixtures/eml/` are fictional
  Spanish messages (plain text with accents, HTML only, multipart, encoded-word subject, missing
  `Message-ID`, Cc recipients).
- **Microsoft Graph** (`graph_provider`, `graph_oauth`): provider and OAuth flow against
  `wiremock`, using JSON fixtures in `crates/infra/tests/fixtures/graph/`. The sign-in test
  performs the browser redirect itself against the local loopback listener.
- **AI providers** (`jev`, `ollama`): request shape, response mapping, retries and every error
  path against `wiremock`.
- **Factories** (`factory`): all provider-selection branches with `InMemorySecretStore` and an
  in-memory database.
- **Secrets** (`secrets`): the in-memory store; the keyring tests are ignored (see below).

```powershell
cargo test -p academic-infra                      # all of the above
cargo test -p academic-infra --test graph_provider
```

## Frontend

Vitest runs in jsdom with Testing Library and jest-dom (`vitest.setup.ts` also polyfills the
`<dialog>` methods jsdom lacks). Tests mock `src/lib/api/client` (or `@tauri-apps/api/core`'s
`invoke` for the client's own tests), `src/lib/api/events` and `@tauri-apps/plugin-opener`, so no
backend is needed. They cover the API client (exact command names and argument objects), the
check-run provider, the layout and modal, result card rendering and actions, formatting helpers,
and each screen's behavior (filters and sort, account form validation and payloads, catalog
forms, settings including the write-only jev key).

```powershell
npm run test                                       # vitest run
npx vitest run src/features/results               # a subset
npm run typecheck                                  # tsc -b --noEmit
npm run lint                                       # eslint .
npm run build                                      # type-check and production build
```

## Live tests (`#[ignore]`)

These hit real services or the real credential store. They are skipped by `cargo test` and run
only with `-- --ignored`. Never commit real values; pass them through the environment of your
shell session only.

| Test | Needs | Command |
|---|---|---|
| `jev.rs::live_jev_answers_a_simple_noul_question` | `JEV_API_KEY` (no-op if unset). Uses model `jev-latest`. | `$env:JEV_API_KEY = "<key>"; cargo test -p academic-infra --test jev -- --ignored` |
| `ollama.rs::live_ollama_answers_a_simple_noul_question` | `OLLAMA_MODEL` (required, no-op if unset), `OLLAMA_BASE_URL` (optional, default `http://localhost:11434`); Ollama running with the model pulled | `$env:OLLAMA_MODEL = "qwen3:8b"; cargo test -p academic-infra --test ollama -- --ignored` |
| `imap_live.rs` (`test_connection` + `fetch_new` over the last 2 days) | `IMAP_HOST`, `IMAP_USER`, `IMAP_PASSWORD` (required, the test panics if missing), `IMAP_PORT` (optional, default 993). Read-only, but use a test mailbox or an app password. | `$env:IMAP_HOST = "imap.example.com"; $env:IMAP_USER = "me@example.com"; $env:IMAP_PASSWORD = "<app password>"; cargo test -p academic-infra --test imap_live -- --ignored` |
| `secrets.rs::keyring_store_round_trips_a_value`, `keyring_store_round_trips_a_large_value` | Windows Credential Manager. Use random keys under `academic-assistant/test/` and clean up after themselves. | `cargo test -p academic-infra --test secrets -- --ignored` |

Clear the environment variables afterwards (`Remove-Item Env:JEV_API_KEY`, and so on). There
are no live tests for Microsoft Graph; verify it through the manual checklist.

## Test conventions

- Use fake secrets (`test-key`, `test-value`). Fixtures are fictional; do not add real emails.
- Do not weaken a test to make an incorrect implementation pass; update it only when the
  contract changes.
- Prefer fakes at the port boundary over mocking internals. New adapters get `wiremock` or
  fixture-based tests plus, where useful, an `#[ignore]` live test driven by environment
  variables.

## Manual acceptance checklist (end of Phase 1)

Run with a real Ollama and, separately, with jev (see [setup.md](setup.md) for prerequisites).

1. Launch with `npm run tauri dev`. In **Settings**, choose Ollama, then **Test provider**: it
   must report success.
2. Add an **IMAP** account (for example UV) with host, port, username and app password.
   Press **Test**: it must succeed. Add a **Microsoft 365** account (for example UAH): enter
   the Entra client ID in Settings, save the account, **Sign in with Microsoft**, then **Test**.
   If your tenant needs admin approval, confirm the message "Your organization requires admin
   approval for this app" is shown.
3. In **University catalog**, add the universities and a few courses with aliases.
4. On the **Dashboard**, press **Check now**. Progress appears per account. When it finishes,
   the last check shows its status and new results appear with category, priority, course and
   dates.
5. Verify results against the real emails: exam date change flagged as a possible change,
   deadlines as tasks, newsletters and administrative notices classified sensibly, irrelevant mail
   absent.
6. Confirm the mailboxes were not modified (nothing became read, moved or deleted).
7. Mark a result as seen and confirm it leaves the Dashboard list; on **Results**, mark it as
   new again. Try **Mark all as seen**.
8. On **Results**, test search, the **Filters and sort** modal (state, category, account, course,
   sort) and row expansion with **Open email** / **Copy Message-ID**.
9. Press **Check now** again: nothing is duplicated and only new messages are processed.
10. Open **Runs** and expand the last run: counters and per-account detail are correct. Provoke
    an error (wrong IMAP password, Ollama stopped) and confirm the account fails with a clear
    message while other accounts still succeed (status Partial), and that the Dashboard shows
    the attention banner.
11. Switch to jev in Settings, replace the key, **Test provider**, run **Check now** on new mail
    and compare quality, including Spanish emails.
12. Confirm no secrets or message bodies appear in the database or in the log files under the
    app data directory ([app/logging-and-data.md](app/logging-and-data.md)).
