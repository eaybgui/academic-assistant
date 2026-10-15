# Testing

The default test commands never touch real services, the Windows Credential Manager or the
network: everything external is replaced by fakes, fixtures or a local mock server. Tests that
need a real service are marked `#[ignore]` and run only when you ask for them.

## Suites at a glance

| Suite | Location | Run |
|---|---|---|
| Core unit tests | `#[cfg(test)]` modules in `crates/core/src/` | `cargo test -p academic-core --lib` |
| Core pipeline integration | `crates/core/tests/pipeline.rs` (+ `tests/support/` fakes) | `cargo test -p academic-core --test pipeline` |
| Core LMS sync integration | `crates/core/tests/lms_sync.rs` (fake LMS client, factory, authenticator and repositories) | `cargo test -p academic-core --test lms_sync` |
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
- `lms/`: platform URL normalization, `ensure_session` (verify, silent renewal, expiry),
  `effective_status` (including past events and items with nothing to submit) and `diff_items`
  (first-sync badge suppression, due date change and revert, `New` kept on a due change, removal
  and reappearance, untracked and unavailable courses, events outside the sync window, personal
  fields untouched).

**Pipeline integration tests** (`crates/core/tests/pipeline.rs`) drive the full `CheckRunner`
against in-memory fakes in `crates/core/tests/support/`: `TestStore` (all seven repository
traits on one shared store), a fake provider factory with queued `fetch_new` responses, a fake
`StructuredModel` and `ModelProvider`, a recording `RunObserver` and a fixed `Clock`. They cover
deduplication, each filter reason, irrelevant and invalid-response handling, transient model
errors not advancing the cursor, per-account error isolation, run status aggregation, the
single-run lock and preconditions, and the progress event sequence. See
[core/pipeline.md](core/pipeline.md#how-to-test).

**LMS sync integration tests** (`crates/core/tests/lms_sync.rs`) drive `LmsSyncRunner` against
the fakes in `crates/core/tests/support/lms.rs`: all-or-nothing fetches, silent renewal success and
failure, per-university isolation, the single-run lock, `NothingToSync`, and the progress and
finished notifications (exactly one `finished` per sync). See
[infra/lms.md](infra/lms.md#sync-lmssyncrunner).

```powershell
cargo test -p academic-core                       # unit + pipeline and sync integration
cargo test -p academic-core --lib normalize       # a subset by name filter
```

## Infra (`crates/infra`)

- **Database** (`db_accounts`, `db_catalog`, `db_migrations`, `db_processed`, `db_results`,
  `db_runs`, `db_settings`, `db_sync`, `db_lms`, `db_items`): repositories against
  `Database::open_in_memory()`, including sort semantics, unicode search, cascade deletes,
  migration idempotence and the upgrade of a Phase 1 database by migration 2 (results and
  accounts survive, their university and course links become empty), the university, course and
  item repositories (tracked/available filtering, transactional course selection and sync). The shared
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
- **Learning platforms** (`lms_probe`, `lms_moodle`, `lms_moodle_items`, `lms_blackboard`,
  `lms_blackboard_items`, `lms_factory`): the probe, the Moodle launch-token decoding and
  signature check, and both clients against `wiremock` with invented JSON (paging, calendar
  chunking, error mapping, skipped attendance and calculated columns, event filtering, 403/404
  course gradebooks, credential readability). `lms_moodle_items` and `lms_blackboard_items` also
  contain tests that parse real captures; they read the git-ignored `tests/fixtures/lms-local/`
  folder and **skip silently when it does not exist**, so a fresh clone passes without them.

```powershell
cargo test -p academic-infra                      # all of the above
cargo test -p academic-infra --test graph_provider
```

## Frontend

Vitest runs in jsdom with Testing Library and jest-dom (`vitest.setup.ts` also polyfills the
`<dialog>` methods jsdom lacks). Tests mock `src/lib/api/client` (or `@tauri-apps/api/core`'s
`invoke` for the client's own tests), `src/lib/api/events` and `@tauri-apps/plugin-opener`, so no
backend is needed. They cover the API client (exact command names and argument objects), the
check-run and sync providers, the layout (sidebar, disabled Announcements link) and modal, result
row rendering and actions, formatting helpers, and each screen's behavior: the tracker dashboard
(states, tabs, persistence, time and status grouping around week boundaries, item rows, details and
note editor, banners), the Universities page (add wizard, course selection, Edit courses with its
fallback, card actions), Announcements (header, filters and sort), account form validation and
payloads, and settings including the write-only jev key.

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
| `lms_live.rs` (`live_verify_and_list_courses`, `live_fetch_items`) | A university already added and signed in through the app, and its credential in Credential Manager; `LMS_LIVE_UNIVERSITY_ID`, `LMS_LIVE_PLATFORM`, `LMS_LIVE_BASE_URL`. See below. | see "Live LMS tests and the capture tool" |

Clear the environment variables afterwards (`Remove-Item Env:JEV_API_KEY`, and so on). There
are no live tests for Microsoft Graph; verify it through the manual checklist.

## Live LMS tests and the capture tool

Both use the credential the app stored when you signed in, read-only, and print only counts and
the display name: never the credential, course names or item titles. Pass the values through your
shell session only and never commit them.

**Live tests** (`crates/infra/tests/lms_live.rs`, `#[ignore]`). `LMS_LIVE_UNIVERSITY_ID` is the
university's id (1 for the first one added, 2 for the second, and so on),
`LMS_LIVE_PLATFORM` is `moodle` or `blackboard` and `LMS_LIVE_BASE_URL` is the platform address as
shown on its Universities card. Run once per university, from PowerShell:

```powershell
$env:LMS_LIVE_UNIVERSITY_ID = "1"
$env:LMS_LIVE_PLATFORM = "moodle"            # or "blackboard"
$env:LMS_LIVE_BASE_URL = "https://campus.demo.example"
cargo test -p academic-infra --test lms_live -- --ignored --nocapture
```

`live_verify_and_list_courses` prints who the credential belongs to and the number of courses
(which must be above zero); `live_fetch_items` queries up to three courses and prints the item
count per kind and per platform status and how many items have a due date or a grade.

**Capture tool** (`crates/infra/examples/lms_capture.rs`). Calls the raw platform endpoints with
the stored credential, redacts personal text (names, titles, free text, grades) while keeping the
JSON shape, ids, enum-like values, timestamps and URLs, and writes one JSON file per endpoint. Use
it when a platform answers differently from what the clients expect and you need a sample to build
a test from:

```powershell
$env:LMS_CAPTURE_UNIVERSITY_ID = "1"
$env:LMS_CAPTURE_PLATFORM = "blackboard"     # or "moodle"
$env:LMS_CAPTURE_BASE_URL = "https://campus.demo.example"
$env:LMS_CAPTURE_MAX_COURSES = "3"           # optional, default 3
cargo run -p academic-infra --example lms_capture
```

**Captures stay local.** The tool writes to `crates/infra/tests/fixtures/lms-local/<platform>/`,
which is git-ignored because it still derives from your real accounts. Never copy a capture into
a committed fixture, a test, a doc or an issue; a committed test must use invented data
(`Universidad Demo`, `Álgebra Lineal`, ids such as `_10_1`). The tests that read `lms-local` skip
when it is absent. If a capture ever appears in `git status`, do not commit it.

## Test conventions

- Use fake secrets (`test-key`, `test-value`). Fixtures are fictional; do not add real emails.
- Do not weaken a test to make an incorrect implementation pass; update it only when the
  contract changes.
- Prefer fakes at the port boundary over mocking internals. New adapters get `wiremock` or
  fixture-based tests plus, where useful, an `#[ignore]` live test driven by environment
  variables.

## Manual acceptance checklist (end of Phase 1, email)

Run with a real Ollama and, separately, with jev (see [setup.md](setup.md) for prerequisites).
Since Phase 2 the email screens live under **Announcements**, **Email accounts**, **Email runs**
and **Settings**, and "Check now" needs a tracked course (do the Phase 2 checklist's university
steps first); the steps below use the current names.

1. Launch with `npm run tauri dev`. In **Settings**, choose Ollama, then **Test provider**: it
   must report success.
2. Add an **IMAP** account (for example UV) with host, port, username and app password.
   Press **Test**: it must succeed. Add a **Microsoft 365** account (for example UAH): enter
   the Entra client ID in Settings, save the account, **Sign in with Microsoft**, then **Test**.
   If your tenant needs admin approval, confirm the message "Your organization requires admin
   approval for this app" is shown.
3. On **Universities**, add a university and track its courses (the analyzer uses the tracked
   courses as its catalog).
4. On **Announcements**, press **Check now**. Progress appears per account. When it finishes,
   the last check shows its status and new results appear with category, priority, course and
   dates.
5. Verify results against the real emails: exam date change flagged as a possible change,
   deadlines as tasks, newsletters and administrative notices classified sensibly, irrelevant mail
   absent.
6. Confirm the mailboxes were not modified (nothing became read, moved or deleted).
7. Mark a result as seen and confirm the unseen counters and the sidebar badge drop; mark it as
   new again. Try **Mark all as seen**.
8. On **Announcements**, test search, the **Filters and sort** modal (state, category, account, course,
   sort) and row expansion with **Open email** / **Copy Message-ID**.
9. Press **Check now** again: nothing is duplicated and only new messages are processed.
10. Open **Email runs** and expand the last run: counters and per-account detail are correct. Provoke
    an error (wrong IMAP password, Ollama stopped) and confirm the account fails with a clear
    message while other accounts still succeed (status Partial), and that Announcements shows
    the attention banner.
11. Switch to jev in Settings, replace the key, **Test provider**, run **Check now** on new mail
    and compare quality, including Spanish emails.
12. Confirm no secrets or message bodies appear in the database or in the log files under the
    app data directory ([app/logging-and-data.md](app/logging-and-data.md)).

## Phase 2 acceptance checklist (manual, real accounts)

For the user, with one Moodle university and one Blackboard Learn university (the examples below
call them "the Moodle university" and "the Blackboard university"). Run `npm run tauri dev` and
record date, result and observations for each line. Before starting, close the app and copy
`academic-assistant.db` (and the `-wal` / `-shm` files if present) from the app data directory
somewhere safe: migration 2 removes the Phase 1 manual catalog and the Phase 1 build cannot open the
migrated database.

1. **Empty state.** With no universities the Dashboard shows "Track your deliverables" and **Add
   your first university**, and the sidebar's **Announcements** link is greyed out with the hint
   "Add a university and track courses first".
2. **Add both universities.** On Universities, add the Moodle university (name, platform, address):
   Continue shows the address; **Sign in** opens the platform's sign-in in a separate window, which
   closes by itself, and the dialog shows "Connected as ...". Repeat with the Blackboard university.
   A wrong platform ("This URL is not a Blackboard Learn site") and a repeated address ("This
   platform is already added") are refused.
3. **Choose courses.** **Choose courses** lists every course, all ticked. Untick a few, press
   **Save and sync**: the app goes to the Dashboard and a sync of that university starts at once
   (the button shows its progress).
4. **First sync.** The Dashboard shows items from the tracked courses of both universities and no
   "New" badges (a course's first sync is never badged). "Last synced" appears; the cards on
   Universities show "N of M courses tracked" and the sync time.
5. **Tabs and grouping.** The **All** tab shows both universities with their name on each row;
   each university tab shows only its own. **Group by Time** shows Overdue, Today, This week, Next
   week, Later and No date as applicable, with finished items in a collapsed **Completed**
   section; **Group by Status** shows Pending (overdue first), Submitted and Graded, with past
   events collapsed. Reload the app: the tab and grouping are remembered. **Show hidden** and
   **Show removed** are off by default.
6. **Statuses and grades.** Submitted work shows **Submitted**, graded work shows **Graded** with
   the grade, work past its date and not handed in shows **Overdue**, and past course events show
   **Past** (not Overdue).
7. **Done, hide, note.** Tick the done checkbox of an assignment: it moves to Completed / Submitted.
   Open an item's details, **Hide** it: it disappears, and reappears with **Show hidden** (and
   **Show again** restores it). Save a note: a pencil mark appears on the row; saving an empty note
   removes it. A note of more than 2000 characters shows an error and keeps your text.
8. **Open in platform.** In the details, **Open in platform** opens the right page in your
   browser (for both platforms).
9. **A second sync shows no badges.** Press **Sync** again without anything changing on the
   platform: the summary says "0 new, 0 date changed, 0 removed" and no badge appears.
10. **Badges on change.** If something new appears or a due date moves on a platform, the next
    sync shows **New** or **Date changed: A -> B** on that item and counts it in the summary. The
    x removes one badge; **Dismiss all changes** removes the rest. (Skip if nothing changes in your
    courses during the test.)
11. **Edit courses, then sync.** On a university card press **Edit courses**: the list is
    refreshed from the platform. Tick a course that was not tracked and save: a sync starts and the
    course's items appear without "New" badges. Untick a course: its items leave the Dashboard.
12. **Edit courses without a connection.** With a signed-in university and the network off, press
    **Edit courses**: the stored course list is shown together with an error message, and saving
    still works. After **Sign out**, the card no longer offers **Sync** or **Edit courses**.
13. **Sign-in banner after an expired session.** Make a session unusable (for Moodle reset the
    "Moodle mobile web service" security key in your Moodle preferences; for Blackboard sign out of
    the platform in a browser or wait for the session to expire), then press **Sync**. If the
    quiet renewal succeeds, the sync simply works. If it cannot, the Dashboard shows "<name> needs
    you to sign in again" with a **Sign in** button; pressing it signs in and syncs that university.
    The university never stays "Connected" with a revoked credential.
14. **Announcements gating.** Before any course was tracked the **Announcements** link was greyed
    out (step 1). After tracking a course it is enabled. As a check, untrack every course of every
    university: the link greys out again, and re-tracking one enables it.
15. **Check now works.** With an email account configured ([setup.md](setup.md)), open
    **Announcements** and press **Check now**: progress appears and results arrive as in the Phase 1
    checklist. With no tracked course the command refuses with "Track at least one course before
    checking email".
16. **Delete.** Delete a university: its card, items and Dashboard tab disappear, and its
    credential is gone from `cmdkey /list`.
17. **Logs.** Open `%APPDATA%\dev.eaybar.academic-assistant\logs`: only university ids, stages,
    counts and error kinds; no tokens, cookie values, item titles, notes, grades or sign-in window
    addresses.

When this checklist passes, `phase-2-lms-tracker` is merged into `main`. Run the live tests and
the capture tool (above) only if a platform behaves unexpectedly.

## Phase 2 · M1 sign-in gate (manual, real accounts; historical)

This was the first gate of Phase 2 (sign-in only). Its lines are kept as a reference for the login
window behaviour; the Phase 2 checklist above supersedes it for the finished product. Run
`npm run tauri dev`. Record date, result and observations for each line in the PR/merge notes.

0. Before starting: close the app and copy `%APPDATA%\dev.eaybar.academic-assistant\academic-assistant.db` (and the `-wal`/`-shm` files if present) somewhere safe. Migration 0002 removes the manual catalog and the Phase 1 build on `main` cannot open the migrated database; restore the copy to go back to `main`.
1. Universities → Add university → "UV", Moodle, `aulavirtual.uv.es` → Continue shows `https://aulavirtual.uv.es`.
2. Sign in → the UV login opens in a separate window → after login the window closes by itself and the dialog shows "Connected as …".
3. Repeat with "UAH", Blackboard, `uah.blackboard.com` (Microsoft login). Same result.
4. Wrong platform: add `aulavirtual.uv.es` as Blackboard → "This URL is not a Blackboard Learn site". Add `aulavirtual.uv.es` again as Moodle → "This platform is already added".
5. Close the sign-in window before finishing → "Sign-in cancelled"; signing in again works.
6. Restart the app → both universities still "Connected"; "Check connection" on each succeeds.
7. Live test for each university (see `crates/infra/tests/lms_live.rs`) prints the display name and a course count > 0. The id is 1 for the first university added, 2 for the second. Run from PowerShell, once per university:

   ```powershell
   $env:LMS_LIVE_UNIVERSITY_ID = "1"
   $env:LMS_LIVE_PLATFORM = "moodle"   # or "blackboard"
   $env:LMS_LIVE_BASE_URL = "https://aulavirtual.uv.es"
   cargo test -p academic-infra --test lms_live -- --ignored --nocapture
   ```
8. Silent renewal: delete the stored credential (`cmdkey /list | findstr university` then `cmdkey /delete:<target>`), press "Check connection": the session is renewed without a visible window (no login prompt) and the card shows "Connected". If the IdP requires interaction, the card shows "Session expired" and "Sign in" works — record which happened for UV and UAH.
9. Leave the app for several hours (or overnight), then "Check connection" on UAH: record whether the Blackboard session had expired and whether renewal was silent. Record how long the Moodle token kept working.
10. Sign out → card shows "Not signed in"; the credential is gone from `cmdkey /list`. Delete → the university disappears.
11. Logs (`%APPDATA%/dev.eaybar.academic-assistant/logs`): no tokens, cookie values or login-window URLs; only university ids, stages and error kinds.
12. Credential size: step 3 succeeded without "too large to store securely" (if it failed, note the cookie names logged).
13. UAH: Sign out, then Sign in again — the login window must close by itself without typing the password (the Microsoft session is reused). Record whether it did.
14. Invalid Moodle token: in UV Moodle → Preferences → Security keys, reset the 'Moodle mobile web service' key; then Check connection on UV → it renews silently or shows 'Session expired' with a working Sign in; it must never stay 'Connected' with the revoked token.

### Login window behaviour (runtime only)

These cannot be covered by automated tests; check them while doing the steps above.

1. **Moodle redirect.** After the UV login there is no Windows "open app"/"choose an app" prompt and no error page for `moodlemobile://`. The window closes by itself.
2. **Moodle site address.** Moodle sign-in succeeds, which also confirms the configured URL equals Moodle's own site address. If it fails with "The Moodle sign-in could not be verified", note it.
3. **Hidden windows.** A silent renewal (main-list item 8) shows no window flash or taskbar entry.
4. **Blackboard landing.** Blackboard sign-in only completes after the real login. The window never closes on an intermediate page.
5. **Closing the login window.** It never closes the main app.
6. **Second sign-in.** Starting a second sign-in for the same university while its window is open focuses the existing window and shows "A sign-in window for this university is already open".
7. **Two universities.** Two different universities can have sign-in windows open at the same time.
8. **Saved session.** After restarting the app, signing in again to an already-signed-in IdP may complete without typing the password. Record whether it did. The `lms-auth` profile folder in the app data directory exists.
9. **No app access from the login window.** In the login window, the developer console (if available in dev builds) shows `window.__TAURI_INTERNALS__` is undefined or invoking a command fails.
10. **Interactive timeout.** Leave an interactive sign-in window idle for 10 minutes. It closes and the dialog shows "Sign-in timed out" (optional, record if done).
