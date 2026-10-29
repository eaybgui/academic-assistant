# Tauri composition, commands and events (`src-tauri`)

`src-tauri` is the composition root: it builds the `academic-infra` adapters, wires
them to the `academic-core` ports, and exposes the result to the React frontend as
Tauri commands and events. It contains no business logic; that lives in
`academic-core` (see `docs/core/pipeline.md` for email and `docs/infra/lms.md` for the LMS
tracker). The one piece of platform glue it owns is the LMS sign-in window (`lms_auth/`).

## Module layout

| Module | Responsibility |
|---|---|
| `lib.rs` | `run()`: Tauri builder, opener plugin, `setup`, command registration. |
| `setup.rs` | App data dir, logging, database open, `mark_interrupted`, builds `AppState`. |
| `state.rs` | `AppState` and `Collaborators`: repositories, adapters, `CheckRunner`. |
| `observer.rs` | `TauriRunObserver`: emits `run:progress` / `run:finished`. |
| `lms_observer.rs` | `TauriLmsSyncObserver`: emits `lms-sync:progress` / `lms-sync:finished`. Logs only university ids, stages and counts. |
| `lms_auth/` | Embedded sign-in windows: credential capture (`capture.rs` pure helpers, `window.rs` window driver) and `TauriLmsAuthenticator` (silent renewal). See [../infra/lms.md](../infra/lms.md). |
| `dto/` | Input/output DTOs (camelCase JSON) and conversions from the domain types (`accounts`, `catalog`, `lms`, `results`, `runs`, `settings`). |
| `error.rs` | `CommandError` and the `From` conversions from repository/run/provider/secret errors. |
| `validation.rs` | Trimming and validation of every command input. |
| `account_status.rs` | Credentials-configured flag, last-sync aggregate and dashboard attention list. |
| `commands/{runs,results,accounts,catalog,settings,universities,tracker}.rs` | The commands themselves. `universities` holds the sign-in, session and course commands; `tracker` holds sync, the tracker list and the personal item fields. |
| `test_support.rs` | (tests only) `AppState` over an in-memory database with fake adapters. |

## Composition (`AppState`)

`AppState::new(db, Collaborators)` receives the single `Arc<Database>` (it implements
all ten repository ports) and clones it into every repository field, into the
`Repositories` bundle of the `CheckRunner` and into the `LmsRepositories` bundle of the
`LmsSyncRunner`. `Collaborators` carries the secret store, the email provider factory, the model
provider, the shared `reqwest::Client`, the run observer, the clock, and the LMS client factory,
authenticator and sync observer. In the app these are `KeyringSecretStore`,
`DefaultEmailProviderFactory`, `DefaultModelProvider`, `http_client()`,
`TauriRunObserver`, `SystemClock`, `DefaultLmsClientFactory`, `TauriLmsAuthenticator` and
`TauriLmsSyncObserver`; tests substitute fakes. The state owns two runners: the `CheckRunner`
(email) and the `LmsSyncRunner` (platform sync), each with its own single-run lock.

`AppState` is registered with `app.manage(...)` and injected into commands as
`State<'_, AppState>`. It also owns the `tracing` file-writer guard.

Every command is an `async fn` wrapper that calls a plain function taking `&AppState`
and passes the result through `logged(name, result)`, which logs a failure at `warn`.
The plain functions are what the unit tests exercise.

## Errors

Every command returns `Result<T, CommandError>`; the frontend receives
`{ kind, message }`:

| `kind` | Produced by |
|---|---|
| `validation` | Invalid input; `RunError::NoEnabledAccounts` / `AiNotConfigured`; no tracked course for "Check now"; `LmsSyncError::NothingToSync`; an invalid platform URL or one that is not the chosen platform; provider type change; Microsoft email mismatch. |
| `not_found` | `RepoError::NotFound` ("Item not found"), unknown account; unknown university ("University not found"). |
| `conflict` | `RepoError::Conflict` ("An item with that name already exists"); a platform URL that is already added ("This platform is already added"); `RunError::AlreadyRunning`; `LmsSyncError::AlreadyRunning` ("A sync is already running"). |
| `auth` | `ProviderError::Auth`; `LmsError::Auth`; a university that needs a new sign-in ("{name} needs you to sign in again", "Sign in to {name} first"); a cancelled or timed-out sign-in window. |
| `connection` | `ProviderError::Connection`; `LmsError::Connection`. |
| `internal` | `RepoError::Storage` / `RunError::Storage` / `LmsSyncError::Storage` ("Something went wrong while saving data"); `SecretError` ("Something went wrong while accessing secure storage"); `ProviderError::Protocol`; `LmsError::Protocol`. |

The `kind` value `ai` in the frontend's `CommandError` type is not produced by any
command: AI failures during `test_ai_provider` are reported inside
`ConnectionTestDto` (`errorKind: "ai"`), not as command errors.

Storage and secret errors never expose their detail to the frontend; the detail is
logged (see `logging-and-data.md`). Messages are sentence case with no trailing period.

## Commands

Argument names are the camelCase keys the frontend sends (`src/lib/api/client.ts`);
Tauri maps them to the snake_case Rust parameters.

| Command | Arguments | Result | Behavior |
|---|---|---|---|
| `check_now` | none | run id | Refused with `validation` "Track at least one course before checking email" while no course is tracked (the email analyzer uses the tracked courses as its catalog). Then `runner.prepare(Manual)` and `runner.execute` on `tauri::async_runtime::spawn` (supervised, see below); returns as soon as the run row exists. Logs `check started` with the run id. |
| `get_dashboard` | `sort` | `DashboardDto` | Email only (the Announcements header and the sidebar badge). Last run, running run id, attention list, unseen counts and up to 100 unseen results (`seen=false`, given sort, offset 0). |
| `get_run_history` | `limit` | `RunDto[]` | Newest first; `limit` clamped to 1..200. |
| `get_results` | `query` | `Page<ResultDto>` | `search` trimmed (blank means none); `limit` clamped to 1..200; negative `offset` becomes 0. |
| `set_result_seen` | `id`, `seen` | `ResultDto` | Marks seen or new; unknown id is `not_found`. |
| `mark_all_seen` | none | rows updated | |
| `get_accounts` | none | `AccountDto[]` | |
| `add_account` | `input` | `AccountDto` | Insert, delete both secret keys for the new id (defensive: a new account never inherits a leftover secret), then store the IMAP password; if the secrets cannot be prepared the inserted row is deleted again. IMAP password is required. A Microsoft tenant must match `^[A-Za-z0-9.-]+$` ("Enter a valid tenant"). |
| `update_account` | `id`, `input` | `AccountDto` | Provider type cannot change. IMAP password replaced only when non-empty. For a Microsoft account, changing the email (case-insensitive) or the tenant deletes the stored refresh token after the update, so the account shows "Sign in to this account". Tenant validated as in `add_account`. |
| `remove_account` | `id` | nothing | Deletes both secret keys (missing is fine), then the account and, by cascade, its data. |
| `test_account` | `id` | `ConnectionTestDto` | `factory.create` then `test_connection`; `errorKind` is the provider error kind (`auth`, `connection`, `protocol`). |
| `connect_microsoft_account` | `id` | `AccountDto` | See below. |
| `get_catalog` | none | `UniversityDto[]` | Read-only projection for the email screens: every configured university with its tracked, available courses nested (`aliases` holds the short name when it adds information). The manual catalog and its CRUD commands no longer exist. |
| `lms_probe` | `url`, `platform` | `ProbeDto` | Normalizes the URL to an `https` origin and checks, without credentials, that it hosts the platform; returns `{ baseUrl }`. Mismatch is `validation`, network failure `connection`. |
| `get_universities` | none | `LmsUniversityDto[]` | Ordered by name, each with its login state, last sync and course counts (tracked, total, unreviewed). |
| `add_university` | `input` (`name`, `platform`, `baseUrl`) | `LmsUniversityDto` | Probes the URL again on the server, then inserts. Duplicate platform URL is `conflict` "This platform is already added"; duplicate name is `conflict`. |
| `rename_university` | `id`, `name` | `LmsUniversityDto` | The platform and URL are immutable. |
| `remove_university` | `id` | nothing | Deletes the stored credential first, then the university and, by cascade, its courses and items (notes included); closes any open sign-in window. |
| `lms_sign_in` | `id` | `LmsUniversityDto` | Opens the visible sign-in window (up to 10 minutes) and resolves after capture and verification. Cancelled or timed out is `auth`. A rejected credential marks the university `error`. |
| `lms_sign_out` | `id` | `LmsUniversityDto` | Deletes the credential and sets the state to `never`. |
| `lms_check_session` | `id` | `LmsUniversityDto` | `ensure_session`: verify, one silent renewal if needed, update the state. `auth` "{name} needs you to sign in again" when the session cannot be renewed. |
| `lms_fetch_courses` | `id` | `LmsCourseDto[]` | Requires login state `ok` (else `auth` "Sign in to {name} first"). Lists the platform's courses and stores them: new ones untracked and unreviewed, missing ones unavailable. A credential the platform rejects marks the university `expired`. |
| `get_courses` | `universityId` | `LmsCourseDto[]` | The stored courses, without contacting the platform. |
| `set_tracked_courses` | `universityId`, `courseIds` | `LmsCourseDto[]` | Exactly these courses become tracked and every course of the university becomes reviewed, in one transaction; an id that does not belong to the university is `not_found` and nothing changes. |
| `lms_sync` | `universityId` (number or `null` for all) | nothing | Takes the sync lock and selects the signed-in universities, then runs the sync in the background (supervised, see below). `conflict` if a sync is running, `validation` "Sign in to a university first" if there is nothing to sync. Progress and results arrive as `lms-sync:*` events. Logs `lms sync started`. |
| `get_tracker` | `query` (`universityId`, `includeHidden`, `includeRemoved`) | `TrackerDto` | Items of tracked, available courses (hidden and removed ones only when asked), ordered by due date (undated last), with the effective status computed now, plus the universities and whether a sync is running. |
| `set_item_done` / `set_item_hidden` | `id`, `value` | `LmsItemDto` | Personal flags; never overwritten by a sync. Unknown id is `not_found`. |
| `set_item_note` | `id`, `note` (string or `null`) | `LmsItemDto` | Trimmed; blank becomes none; more than 2000 characters is `validation` "Notes can have at most 2000 characters". |
| `dismiss_item_change` | `id` | `LmsItemDto` | Clears the "new" / "date changed" badge. |
| `dismiss_all_changes` | `universityId` (number or `null`) | rows updated | Clears every badge of the university, or of all. |
| `get_settings` | none | `SettingsDto` | `jevApiKeyConfigured` reflects the secret store; the key is never returned. |
| `update_settings` | `input` | `SettingsDto` | URLs must be `http(s)://`, model names non-empty, blocked senders trimmed, blank client id becomes `null`. |
| `set_jev_api_key` | `key` | nothing | Trimmed, non-empty. |
| `clear_jev_api_key` | none | nothing | Idempotent. |
| `test_ai_provider` | none | `ConnectionTestDto` | Builds the configured model, asks one Noul question (`greeting`: "This text is a greeting.") on state `"Hello"`; ok when an answer for it is returned. |

### Dashboard attention

One entry per enabled account, first reason wins: a sync state with status `error`
(its `last_error`), else a missing credential ("The password is missing. Edit the
account to set it" for IMAP, "Sign in to this account" for Microsoft). Only sync states
of folders still in the account's `folders` count; the state of a removed folder is
ignored (also for `lastSync`).

### Sync supervision

`lms_sync` prepares the sync (taking the lock) and spawns the execution plus a supervisor task
that awaits it. If the execution task panics, the observer would never hear `finished`, so the
supervisor logs the failure at `error` and calls `finished` with no results; the UI shows "The
sync was interrupted" and stops waiting. The runner releases the lock before it notifies the
observer, so a tracker refetch after `lms-sync:finished` never sees the sync as running. Saving
tracked courses in the UI starts a sync of that university immediately; that is done by the
frontend calling `lms_sync` after `set_tracked_courses`, not by the command itself.

### Run supervision

`check_now` spawns the execution and a supervisor task that awaits it. If the execution
task panics (or is cancelled) the run would stay `running`, so the supervisor logs the
failure at `error`, marks the run `failed` with error summary "Interrupted" (unless it
had already finished) and calls the observer's `finished`, which emits `run:finished`
with status `failed` so the UI recovers. The pipeline releases the single-run lock
before it notifies the observer, so a dashboard refetch after `run:finished` never sees
the run as running.

### `AccountDto` derived fields

- `credentialsConfigured`: the IMAP password or the Microsoft refresh token exists in the
  secret store.
- `lastSync`: `at` is the latest `last_run_at`; `status` is `error` if any folder errored,
  else `ok` if any folder succeeded, else `never`; `error` is the first folder error.
  Only the account's current `folders` are considered.

### `connect_microsoft_account`

1. The account must be a Microsoft account and `microsoft_client_id` must be set
   (otherwise a validation error).
2. `oauth::sign_in` with a 5-minute timeout; the authorize URL is opened in the system
   browser through the opener plugin.
3. `signed_in_identity` (`GET /me`) returns `mail` and `userPrincipalName`. After the
   OAuth wait the account is loaded again: if it was removed meanwhile the command
   returns `not_found` and stores nothing.
4. The account email must equal `mail` or `userPrincipalName`, compared case-insensitively
   and trimmed; otherwise a validation error "Signed in as <x>, expected <y>" and nothing
   is stored. The addresses appear only in that user-facing message; the log line says
   just "signed-in account mismatch".
5. The refresh token is stored under `account_refresh_token_key(id)`; the updated
   `AccountDto` is returned. The access token is never kept.

Account ids are never reused (`accounts.id` is `AUTOINCREMENT`, see
`docs/infra/database.md`), so a refresh token orphaned by a removed account cannot be
picked up by a later account.

## DTO conventions

JSON mirrors `src/lib/api/types.ts`: camelCase fields, snake_case enum strings, dates
`YYYY-MM-DD`, times `HH:MM`, timestamps RFC 3339 UTC. `ProviderDto` and the input
provider are tagged with `type` (`"imap"` / `"microsoft_graph"`). Optional input fields
(`search`, `seen`, `category`, `accountId`, `courseId`, `password`) may be absent.

LMS DTOs (`dto/lms.rs`): `LmsUniversityDto` (`id`, `name`, `platform`, `baseUrl`, `loginState`,
`accountLabel`, `lastSync { at, outcome, error }`, `courses { tracked, total, unreviewed }`),
`LmsCourseDto` (`id`, `universityId`, `name`, `shortName`, `url`, `tracked`, `available`, `isNew`
= not yet reviewed), `LmsItemDto` (ids, `universityName`, `courseName`, `courseShortName`,
`externalId`, `kind`, `title`, `dueAt`, `url`, `status`, `grade { display, score, max }`, `change`,
`removedAt`, `done`, `hidden`, `note`) and `TrackerDto` (`items`, `universities`, `syncRunning`).
`change` is tagged with `kind`: `{ "kind": "new" }` or
`{ "kind": "due_changed", "previousDueAt": ... }`. `status` is one of `pending`, `overdue`,
`submitted`, `graded`, `past_event`. The stored credential (token or cookies) never appears in
any DTO.
Numeric input fields that have a valid range (`port`, `initialSyncDays`, `limit`,
`offset`) are deserialized as signed integers so out-of-range values become clear
validation errors instead of opaque deserialization failures.

## Events

| Event | Payload |
|---|---|
| `run:progress` | `{ runId, accountId, accountName, stage: "connecting"\|"fetching"\|"analyzing"\|"done"\|"failed", processed, total }` |
| `run:finished` | `{ runId, status: "running"\|"success"\|"partial"\|"failed" }` |
| `lms-sync:progress` | `{ universityId, universityName, stage: "courses"\|"items"\|"saving"\|"done"\|"failed"\|"login_required" }` |
| `lms-sync:finished` | `{ results: [{ universityId, universityName, outcome: "success"\|"failed"\|"login_required", newItems, dueChanged, removed, error }] }` |

`run:*` events are emitted by `TauriRunObserver` for every `RunObserver` callback of the
pipeline. `lms-sync:*` events are emitted by `TauriLmsSyncObserver`: `progress` at each stage of
each university, `finished` exactly once per executed sync. Neither logs item titles, notes or
grades.

## Capabilities

`src-tauri/capabilities/default.json` (window `main`): `core:event:default` and
`opener:allow-open-url` restricted to `https://*`.

Least privilege: the frontend only calls the app's own commands (`invoke`), listens to
the `run:*` and `lms-sync:*` events and opens result and platform links, so it needs the event
permissions (`listen`/`unlisten`, part of `core:event:default`) and nothing else. The capability
targets only the `main` window: the LMS sign-in windows (created by the backend, labelled
`lms-login-{id}`) get no IPC access at all. `core:default`
would additionally grant the path, window, webview, app, image, resources, menu and tray
APIs, which the app does not use. App commands defined with `#[tauri::command]` are
allowed by default in Tauri 2, so no per-command permission is listed.

## Testing

```sh
cargo test -p academic-assistant
```

Unit tests cover the DTO JSON shapes (compared with literals equal to what the TypeScript
client sends or expects), input deserialization with absent optional fields, error
mapping, every validation helper, and each command's logic over an in-memory database,
an in-memory secret store and fake providers/models.

## Known limitations

- `connect_microsoft_account`, the sign-in windows (`lms_sign_in`, silent renewal), the real
  `check_now` execution and a real `lms_sync` need the running app and live services; they are
  covered by the manual acceptance checklists in [../testing.md](../testing.md).
- If storing the IMAP password fails during `update_account`, the account row has
  already been updated (the previous password stays in place).
