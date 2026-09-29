# Tauri composition, commands and events (`src-tauri`)

`src-tauri` is the composition root: it builds the `academic-infra` adapters, wires
them to the `academic-core` ports, and exposes the result to the React frontend as
Tauri commands and events. It contains no business logic; that lives in
`academic-core` (see `docs/core/pipeline.md`).

## Module layout

| Module | Responsibility |
|---|---|
| `lib.rs` | `run()`: Tauri builder, opener plugin, `setup`, command registration. |
| `setup.rs` | App data dir, logging, database open, `mark_interrupted`, builds `AppState`. |
| `state.rs` | `AppState` and `Collaborators`: repositories, adapters, `CheckRunner`. |
| `observer.rs` | `TauriRunObserver`: emits `run:progress` / `run:finished`. |
| `dto/` | Input/output DTOs (camelCase JSON) and conversions from the domain types. |
| `error.rs` | `CommandError` and the `From` conversions from repository/run/provider/secret errors. |
| `validation.rs` | Trimming and validation of every command input. |
| `account_status.rs` | Credentials-configured flag, last-sync aggregate and dashboard attention list. |
| `commands/{runs,results,accounts,catalog,settings}.rs` | The commands themselves. |
| `test_support.rs` | (tests only) `AppState` over an in-memory database with fake adapters. |

## Composition (`AppState`)

`AppState::new(db, Collaborators)` receives the single `Arc<Database>` (it implements
all seven repository ports) and clones it into every repository field and into the
`Repositories` bundle of the `CheckRunner`. `Collaborators` carries the secret store,
the email provider factory, the model provider, the shared `reqwest::Client`, the run
observer and the clock. In the app these are `KeyringSecretStore`,
`DefaultEmailProviderFactory`, `DefaultModelProvider`, `http_client()`,
`TauriRunObserver` and `SystemClock`; tests substitute fakes.

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
| `validation` | Invalid input; `RunError::NoEnabledAccounts` / `AiNotConfigured`; provider type change; Microsoft email mismatch. |
| `not_found` | `RepoError::NotFound` ("Item not found"), unknown account. |
| `conflict` | `RepoError::Conflict` ("An item with that name already exists"); `RunError::AlreadyRunning`. |
| `auth` | `ProviderError::Auth`. |
| `connection` | `ProviderError::Connection`. |
| `internal` | `RepoError::Storage` / `RunError::Storage` ("Something went wrong while saving data"); `SecretError` ("Something went wrong while accessing secure storage"); `ProviderError::Protocol`. |

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
| `check_now` | none | run id | `runner.prepare(Manual)`, then `runner.execute` on `tauri::async_runtime::spawn`; returns as soon as the run row exists. |
| `get_dashboard` | `sort` | `DashboardDto` | Last run, running run id, attention list, unseen counts and up to 100 unseen results (`seen=false`, given sort, offset 0). |
| `get_run_history` | `limit` | `RunDto[]` | Newest first; `limit` clamped to 1..200. |
| `get_results` | `query` | `Page<ResultDto>` | `search` trimmed (blank means none); `limit` clamped to 1..200; negative `offset` becomes 0. |
| `set_result_seen` | `id`, `seen` | `ResultDto` | Marks seen or new; unknown id is `not_found`. |
| `mark_all_seen` | none | rows updated | |
| `get_accounts` | none | `AccountDto[]` | |
| `add_account` | `input` | `AccountDto` | Insert, then store the IMAP password; if the secret cannot be stored the inserted row is deleted again. IMAP password is required. |
| `update_account` | `id`, `input` | `AccountDto` | Provider type cannot change. IMAP password replaced only when non-empty. |
| `remove_account` | `id` | nothing | Deletes both secret keys (missing is fine), then the account and, by cascade, its data. |
| `test_account` | `id` | `ConnectionTestDto` | `factory.create` then `test_connection`; `errorKind` is the provider error kind (`auth`, `connection`, `protocol`). |
| `connect_microsoft_account` | `id` | `AccountDto` | See below. |
| `get_catalog` | none | `UniversityDto[]` | Universities with nested courses. |
| `add_university` / `update_university` / `remove_university` | `name` / `id`, `name` / `id` | `UniversityDto` / `UniversityDto` / nothing | Duplicate name is `conflict`. |
| `add_course` / `update_course` / `remove_course` | `universityId`, `name`, `aliases` / `id`, `name`, `aliases` / `id` | `CourseDto` / `CourseDto` / nothing | Aliases are trimmed and blanks dropped. |
| `get_settings` | none | `SettingsDto` | `jevApiKeyConfigured` reflects the secret store; the key is never returned. |
| `update_settings` | `input` | `SettingsDto` | URLs must be `http(s)://`, model names non-empty, blocked senders trimmed, blank client id becomes `null`. |
| `set_jev_api_key` | `key` | nothing | Trimmed, non-empty. |
| `clear_jev_api_key` | none | nothing | Idempotent. |
| `test_ai_provider` | none | `ConnectionTestDto` | Builds the configured model, asks one Noul question (`greeting`: "This text is a greeting.") on state `"Hello"`; ok when an answer for it is returned. |

### Dashboard attention

One entry per enabled account, first reason wins: a sync state with status `error`
(its `last_error`), else a missing credential ("The password is missing. Edit the
account to set it" for IMAP, "Sign in to this account" for Microsoft).

### `AccountDto` derived fields

- `credentialsConfigured`: the IMAP password or the Microsoft refresh token exists in the
  secret store.
- `lastSync`: `at` is the latest `last_run_at`; `status` is `error` if any folder errored,
  else `ok` if any folder succeeded, else `never`; `error` is the first folder error.

### `connect_microsoft_account`

1. The account must be a Microsoft account and `microsoft_client_id` must be set
   (otherwise a validation error).
2. `oauth::sign_in` with a 5-minute timeout; the authorize URL is opened in the system
   browser through the opener plugin.
3. `signed_in_email` must equal the account email, compared case-insensitively;
   otherwise a validation error "Signed in as <x>, expected <y>" and nothing is stored.
4. The refresh token is stored under `account_refresh_token_key(id)`; the updated
   `AccountDto` is returned. The access token is never kept.

## DTO conventions

JSON mirrors `src/lib/api/types.ts`: camelCase fields, snake_case enum strings, dates
`YYYY-MM-DD`, times `HH:MM`, timestamps RFC 3339 UTC. `ProviderDto` and the input
provider are tagged with `type` (`"imap"` / `"microsoft_graph"`). Optional input fields
(`search`, `seen`, `category`, `accountId`, `courseId`, `password`) may be absent.
Numeric input fields that have a valid range (`port`, `initialSyncDays`, `limit`,
`offset`) are deserialized as signed integers so out-of-range values become clear
validation errors instead of opaque deserialization failures.

## Events

| Event | Payload |
|---|---|
| `run:progress` | `{ runId, accountId, accountName, stage: "connecting"\|"fetching"\|"analyzing"\|"done"\|"failed", processed, total }` |
| `run:finished` | `{ runId, status: "running"\|"success"\|"partial"\|"failed" }` |

Emitted by `TauriRunObserver` for every `RunObserver` callback of the pipeline.

## Capabilities

`src-tauri/capabilities/default.json` (window `main`): `core:default` (includes the
event `listen`/`unlisten` permissions the frontend needs) and `opener:allow-open-url`
restricted to `https://*`. App commands defined with `#[tauri::command]` are allowed by
default in Tauri 2, so no per-command permission is listed.

## Testing

```sh
cargo test -p academic-assistant
```

Unit tests cover the DTO JSON shapes (compared with literals equal to what the TypeScript
client sends or expects), input deserialization with absent optional fields, error
mapping, every validation helper, and each command's logic over an in-memory database,
an in-memory secret store and fake providers/models.

## Known limitations

- `connect_microsoft_account` and the real `check_now` execution need the running app
  and live services; they are covered by the manual acceptance checklist in the spec.
- If storing the IMAP password fails during `update_account`, the account row has
  already been updated (the previous password stays in place).
