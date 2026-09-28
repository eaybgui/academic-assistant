# `academic-core`: domain model and ports

`academic-core` (package `academic-core`) is the pure domain crate for the Academic
Assistant email analysis pipeline. It has **no I/O dependencies** — only `serde`,
`serde_json`, `chrono`, `thiserror`, `async-trait`, `indexmap`, `regex`, `sha2`, `hex`
and `html2text`. All persistence, network and OS-credential-store access lives in
`academic-infra`, which depends on `academic-core` and implements the traits
described below. `src-tauri` is the composition root that wires `academic-infra`
adapters into `academic-core`'s ports and exposes Tauri commands.

Module layout (`crates/core/src/`):

```
lib.rs          re-exports: analyzer, dates, domain, filters, normalize, pipeline, ports
domain/         domain model (this document, "Domain" section)
ports/          traits + port-level error types (this document, "Ports" section)
normalize.rs    HTML→text, quote/signature stripping, content hash        (Task T3)
filters.rs      deterministic pre-filters                                  (Task T3)
dates.rs        date candidate extraction                                  (Task T4)
analyzer/       question building + answer assembly (EmailAnalyzer)        (Task T5)
pipeline.rs     CheckRunner orchestration                                  (Task T12)
```

`analyzer/mod.rs`, `dates.rs`, `filters.rs`, `normalize.rs` and `pipeline.rs` currently
contain only a module doc comment; their behavior is implemented by later tasks (T3–T5,
T12) against the domain/port contracts fixed here.

## Conventions

- Every domain enum is fieldless (a plain C-like enum) unless noted otherwise, derives
  `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize` and serializes as
  `snake_case` (`#[serde(rename_all = "snake_case")]`).
- Every fieldless domain enum also implements `as_str(&self) -> &'static str` and
  `FromStr` (error type [`domain::ParseEnumError`](#parseenumerror)), both backed by the
  same snake_case string table. This is what `academic-infra`'s SQLite repositories use
  to store/read enum columns as `TEXT`. The mapping is generated once, per enum, by the
  private `domain::enum_str!` macro (`crates/core/src/domain/mod.rs`) — it exists purely
  to avoid writing the same "match to string" / "match from string" pair by hand for the
  ~14 enums below; it has no runtime behavior of its own.
- Structs derive `Debug, Clone, PartialEq` plus `Eq`/`Serialize`/`Deserialize`/`Copy`
  whenever every field supports them. Types holding an `f32` (`AIAnalysis`,
  `AnalysisOutcome`, the AI-provider `Answer` enum) cannot derive `Eq`.
- Two data-carrying enums use external tagging instead of `as_str`/`FromStr`:
  `SyncCursor` and `ProviderConfig`, both `#[serde(tag = "type", rename_all = "snake_case")]`.
  Their JSON shape is what gets stored in the corresponding SQLite JSON columns
  (`sync_state.cursor`, `accounts.provider_config`, owned by Task T6).

## Domain (`crate::domain`)

Re-exported flat from `crate::domain`; internally split by topic into
`account.rs`, `email.rs`, `analysis.rs`, `catalog.rs`, `run.rs`, `results.rs`,
`settings.rs`, `secrets.rs`.

### `ParseEnumError`

`pub struct ParseEnumError(pub String)` — returned by every enum's `FromStr` when the
input string doesn't match any known snake_case variant. The wrapped `String` is the
invalid input, for error messages.

### Accounts (`account.rs`)

- **`AccountId = i64`**.
- **`ProviderConfig`** — `Imap { host, port, username }` | `MicrosoftGraph { tenant }`.
  JSON-tagged (`type`: `"imap"` | `"microsoft_graph"`). `.kind()` returns the matching
  `ProviderKind`. Note: the IMAP variant intentionally has no password field — the
  password lives in the OS credential store under `account_password_key(id)`, never in
  the domain struct or the database.
- **`ProviderKind`** — `Imap` (`"imap"`) | `MicrosoftGraph` (`"microsoft_graph"`).
  Denormalized alongside `ProviderConfig` in storage so accounts can be filtered by
  provider kind without deserializing the JSON config.
- **`Account`** — a configured, persisted mailbox: id, display name, email address,
  provider config, folders to sync, optional default university (used by the analyzer
  to skip asking "which university" for single-university accounts), initial sync
  window in days, enabled flag, timestamps. Owned/produced by
  `AccountRepository` (Task T6); consumed by the pipeline (Task T12) and
  `EmailProviderFactory` (Task T13).
- **`AccountDraft`** — the same fields minus `id`/timestamps; the repository assigns
  those on `insert`/`update`.
- Constants: `DEFAULT_GRAPH_TENANT = "organizations"`, `DEFAULT_IMAP_FOLDER = "INBOX"`,
  `DEFAULT_GRAPH_FOLDER = "inbox"`, `DEFAULT_INITIAL_SYNC_DAYS = 14`.

### Email (`email.rs`)

- **`Address`** — `{ name: Option<String>, email: String }`.
- **`EmailBody`** — `Text(String) | Html(String)`, the normalized-format body kept for
  provider-agnostic downstream processing (`normalize::normalize_body` converts this to
  plain text for filtering/analysis).
- **`Email`** — a fetched-but-not-yet-processed message: account/folder, provider's own
  message id (`provider_message_id`, used for dedup together with `content_hash`),
  optional RFC 5322 `Message-ID` header (angle brackets stripped), sender, recipients,
  subject, `received_at` (UTC), body, and an optional `source_url` (Graph deep link;
  `None` for IMAP). Produced by `EmailProvider::fetch_new` implementations (Task T8/T9).
- **`SyncCursor`** — `Imap { uidvalidity, last_uid } | Graph { link }`, JSON-tagged
  (`type`: `"imap"` | `"graph"`). Opaque outside the matching provider; stored as JSON in
  `sync_state.cursor` (Task T6) and round-tripped by value.
- **`FetchBatch`** — `{ emails, next_cursor }`, the return value of
  `EmailProvider::fetch_new`.
- **`MAX_BATCH: usize = 200`** — the largest batch a single `fetch_new` call may return;
  providers are responsible for paging internally to stay under this.
- **`SyncStatus`** — `Never` (`"never"`, no sync attempted yet) | `Ok` (`"ok"`) |
  `Error` (`"error"`).
- **`SyncState`** — per-account, per-folder sync bookkeeping: cursor, last run/success
  timestamps, status, last error message (sanitized — never includes credentials).
  Owned by `SyncStateRepository` (Task T6).

### Analysis (`analysis.rs`)

- **`Category`** — `Notice | Task | Exam | Event | Change | GeneralInfo`
  (`"general_info"`), the topic classification assigned to a relevant email.
- **`Priority`** — `Low | Medium | High | Urgent`. Declared in ascending order and
  derives `PartialOrd, Ord`, so `Priority::Low < Priority::Urgent` holds via the
  derived, declaration-order comparison — no manual `Ord` impl needed.
- **`DateKind`** — `Exam | Deadline | Event | Class | Other`, what a resolved date
  represents.
- **`ResultDate`** — `{ date: NaiveDate, time: Option<NaiveTime>, kind: DateKind }`, a
  date the AI model confirmed and classified, attached to a `StoredResult`.
- **`DateCandidate`** — `{ span: String, date: NaiveDate, time: Option<NaiveTime> }`, a
  date mention found in the raw text by `dates::extract` (Task T4) and offered to the
  analyzer (Task T5) as something to confirm/classify. Not `Serialize`/`Deserialize`
  (ephemeral, never persisted directly — only the model's confirmed subset becomes
  `ResultDate`s).
- **`ModelDescriptor`** — `{ provider: String, model: String }`, identifies which AI
  provider/model produced an analysis (e.g. `("ollama", "qwen3:8b")`).
- **`AIAnalysis`** — the structured result of analyzing a relevant email: resolved
  university/course ids, category, priority, `action_required`, `possible_change`,
  confirmed dates, optional confidence (`None` when the model reported no confidence
  for any answer that fed the analysis), and the `ModelDescriptor`. Not `Eq`/`Serialize`
  (contains `f32`).
- **`AnalysisOutcome`** — `Irrelevant { p_relevant: f32 } | Relevant(AIAnalysis)`, the
  return value of `analyzer::assemble`/`analyzer::analyze` (Task T5).

### Catalog (`catalog.rs`)

- **`University`** — `{ id, name }`.
- **`Course`** — `{ id, university_id, name, aliases }`; `aliases` are alternate
  names/codes the analyzer should also recognize for the same course.
- **`Catalog`** — `{ universities, courses }`, the full configured catalog as read by
  `CatalogRepository::catalog` and passed into `analyzer::AnalysisInput`.

### Runs (`run.rs`)

- **`RunId = i64`**.
- **`RunTrigger`** — `Manual` (`"manual"`) only in Phase 1 (no scheduled runs yet).
- **`RunStatus`** — `Running | Success | Partial | Failed`, the run's overall outcome.
- **`RunAccountStatus`** — `Success | Failed`, one account's outcome within a run.
- **`ErrorKind`** — `Auth | Connection | Protocol | Ai | Internal`. Shared coarse
  classification used by `ProviderError::kind()`, and stored on `RunAccountResult` /
  surfaced to the UI as `CommandError.kind` (via `src-tauri`'s DTO mapping, Task T14).
- **`Counters`** — `{ found, filtered, analyzed, failed, results_created }` (all `u32`),
  `Default` (all zero), `Copy`. `.add(&mut self, other: &Counters)` sums each field
  in place; used to fold per-account counters into the run total.
- **`RunAccountResult`** — one account's outcome for a run: id, status, counters,
  optional `error_kind`/`error_message` (present only when `status == Failed`).
- **`RunAccountView`** — `RunAccountResult` plus the account's display name, for
  read-side reporting (`RunRepository::get`/`history`/`latest` join account names in,
  ordered by account id).
- **`RunRecord`** — a full run: id, trigger, start/finish timestamps, overall status,
  `accounts_processed`, summed `counters`, optional `error_summary`, and the per-account
  `accounts` views.
- **`RunStage`** — `Connecting | Fetching | Analyzing | Done | Failed`, the stage a
  single account is in *right now*, used only for live progress (`RunProgress`), not
  persisted.
- **`RunProgress`** — a live progress event: run/account id, account name, stage,
  `processed`/`total` message counts. Pushed to the UI as the `run:progress` Tauri event
  (Task T14) via `RunObserver::progress`.
- **`RunError`** — errors that can prevent a run from starting/completing:
  `AlreadyRunning`, `NoEnabledAccounts`, `AiNotConfigured(String)`, `Storage(String)`.
  Derives `Debug, Clone, PartialEq, Eq, thiserror::Error` (the plan's contract listing
  showed only `#[derive(thiserror::Error)]`; `Debug` is added because
  `std::error::Error` — which the `thiserror` derive implements — requires it, and
  `Clone`/`PartialEq`/`Eq` follow the same "derive what's valid" rule used everywhere
  else in `domain`, matching the sibling port error types below).

### Results (`results.rs`)

- **`MessageOutcome`** — `Filtered | Irrelevant | Analyzed | Failed`, what happened to
  one fetched email during pipeline processing.
- **`FilterReason`** — `Duplicate | BlockedSender | BlockedDomain | OwnMessage | Empty`,
  why a deterministic filter (Task T3) discarded an email before analysis.
- **`ProcessedMessage`** — the dedup/audit row written for *every* email the pipeline
  looks at regardless of outcome: account/folder/provider id, optional `Message-ID`
  header, `content_hash` (from `normalize::content_hash`), `received_at`, `outcome`,
  optional human-readable `reason`, the `run_id` that processed it, `processed_at`.
  Used by `ProcessedMessageRepository::exists`/`is_duplicate` to skip messages already
  seen on a later sync.
- **`NewResult`** — a newly analyzed, relevant email ready to persist: account/run id,
  `detected_at` (when the pipeline analyzed it) vs `received_at` (email timestamp),
  subject, sender name/email, optional `source_url`, and the full `AIAnalysis`. Not
  `Eq`/`Serialize` (via `AIAnalysis`'s `f32`).
- **`StoredResult`** — a persisted, analyzed result as read back for display: adds `id`,
  denormalized `account_name`/`university_name`/`course_name` (joined at read time by
  `ResultRepository`, Task T6), resolved `category`/`priority`/flags/`dates`
  (post-analysis, no longer wrapped in `AIAnalysis`/`ModelDescriptor`), and `seen`/
  `seen_at`.
- **`ResultSort`** — `Upcoming | Priority | Received`, ordering for `ResultQuery`; exact
  tie-break semantics are `ResultRepository::query`'s responsibility (Task T6).
- **`ResultQuery`** — filter + pagination for `ResultRepository::query`: free-text
  `search` (case-insensitive substring over subject/sender), `seen`, `category`,
  `account_id`, `course_id`, `sort`, `limit`, `offset`. All filters combine with AND.
- **`Page<T>`** — `{ items: Vec<T>, total: u64 }`; `total` counts all matches ignoring
  `limit`/`offset`.
- **`DashboardCounts`** — `{ new_total, tasks, exams, events, changes_and_notices }`
  (all `u32`), `Default`/`Copy`; unseen-result counts by category for the dashboard,
  from `ResultRepository::unseen_counts`.

### Settings (`settings.rs`)

- **`AiProviderKind`** — `Ollama | Jev`, which AI provider is currently selected.
- **`Settings`** — `{ ai_provider, jev_model, ollama_base_url, ollama_model,
  microsoft_client_id, blocked_senders }`. `Default` values:
  `ai_provider: Ollama`, `jev_model: "jev-latest"`,
  `ollama_base_url: "http://localhost:11434"`, `ollama_model: "qwen3:8b"`,
  `microsoft_client_id: None`, `blocked_senders: []`. Note: no API key field —
  the jev API key is a secret (see below), never stored in `Settings`/SQLite.
  Owned by `SettingsRepository` (Task T6): `load()` falls back to these defaults for
  any missing key.

### Secrets (`secrets.rs`)

Key-naming helpers for the OS credential store (never storage itself — see
`ports::SecretStore` below).

- **`SECRET_SERVICE = "academic-assistant"`** — the credential-store service name every
  secret is stored under.
- **`JEV_API_KEY = "academic-assistant/jev-api-key"`** — fixed key for the jev API key.
- **`account_password_key(id) -> String`** — `"academic-assistant/account/{id}/password"`,
  an IMAP account's password.
- **`account_refresh_token_key(id) -> String`** —
  `"academic-assistant/account/{id}/refresh-token"`, a Microsoft Graph account's OAuth
  refresh token.

## Ports (`crate::ports`)

Traits and port-level error types only — no implementations. `academic-infra` provides
the real adapters; `crates/core/src/ports/secrets.rs`'s sibling `InMemorySecretStore`
(a test fake, owned by Task T7) and any other fakes used by `core`'s own unit tests are
the only implementations that live outside `academic-infra`.

Split into `email.rs`, `model.rs`, `secrets.rs`, `repositories.rs`, `runtime.rs`, all
re-exported flat from `crate::ports`.

### Email providers (`email.rs`)

- **`ProviderError`** — `Auth(String) | Connection(String) | Protocol(String)`.
  `.kind()` maps `Auth → ErrorKind::Auth`, `Connection → ErrorKind::Connection`,
  `Protocol → ErrorKind::Protocol`.
- **`EmailProvider`** (`Send + Sync`, `#[async_trait]`) — a read-only connection to one
  account's mailbox: `test_connection()`, `fetch_new(folder, cursor, since)`. Implemented
  by `ImapEmailProvider` (Task T8) and `MicrosoftGraphEmailProvider` (Task T9).
- **`EmailProviderFactory`** (`Send + Sync`) — `create(&self, account: &Account) ->
  Result<Box<dyn EmailProvider>, ProviderError>`; builds the right provider for an
  account's `ProviderConfig`. Implemented in `academic-infra::factory` (Task T13) and
  consumed by the pipeline (Task T12) / `src-tauri` (Task T14).

### AI models (`model.rs`)

- **`QuestionSet = IndexMap<String, Question>`** — order-preserving; question order
  matters for prompt building.
- **`AnswerSet = HashMap<String, Answer>`**.
- **`Question`** — `Noul { instructions } | Choice { instructions, options:
  IndexMap<String,String> } | Score { instructions, levels: Vec<String> }`. "Noul" is a
  yes/no probability question.
- **`Answer`** — `Noul { p_yes, confidence } | Choice { choice, confidence } | Score {
  level, confidence }`, all with `Option<f32> confidence`.
- **`ModelError`** — `Auth | RateLimited | Unavailable(String) | InvalidResponse(String)`.
  `.is_transient()` is `true` for `Auth`, `RateLimited`, `Unavailable`; `false` for
  `InvalidResponse` (a malformed reply, not a transient provider issue — retrying
  won't help).
- **`StructuredModel`** (`Send + Sync`, `#[async_trait]`) — `descriptor()`,
  `ask(state, questions) -> Result<AnswerSet, ModelError>`. Implemented by `JevModel`
  and `OllamaModel` (Task T7).
- **`ModelProvider`** (`Send + Sync`) — `current(&self, settings: &Settings) ->
  Result<Arc<dyn StructuredModel>, String>`; builds the model selected in `settings`,
  `Err(message)` when unusable (e.g. jev selected without an API key set) — the pipeline
  (Task T12) maps that `Err` to `RunError::AiNotConfigured`. Implemented in
  `academic-infra::factory` (Task T13).

### Secrets (`secrets.rs`)

- **`SecretError(pub String)`** — `"Secure storage error: {0}"`.
- **`SecretStore`** (`Send + Sync`) — `get(key)`, `set(key, value)`, `delete(key)`
  (`Ok(())` even when the key doesn't exist). Implemented by
  `academic-infra::secrets::KeyringSecretStore` (Windows Credential Manager) and
  `InMemorySecretStore` (in-memory fake, also usable by other tasks' tests), both
  Task T7.

### Repositories (`repositories.rs`)

- **`RepoError`** — `NotFound | Conflict(String) | Storage(String)`; `RepoResult<T> =
  Result<T, RepoError>`.
- **`AccountRepository`** — CRUD for `Account`/`AccountDraft`.
- **`CatalogRepository`** — read the full `Catalog`; add/rename/delete universities;
  add/update/delete courses (with aliases).
- **`SyncStateRepository`** — read one/all `SyncState`(s) for an account; record
  success (advances the cursor) or failure (records the error) per account+folder.
- **`ProcessedMessageRepository`** — `exists` (by provider message id, for cheap
  re-fetch skipping), `is_duplicate` (by `Message-ID` header or content hash, for
  cross-account/cross-folder dedup), `insert`.
- **`ResultRepository`** — `save_analyzed` (atomically inserts the `ProcessedMessage`
  with outcome `Analyzed`, the result and its dates — one transaction); `query`
  (filtered/sorted/paginated `StoredResult`s); `get`; `set_seen`; `mark_all_seen`;
  `unseen_counts`.
- **`RunRepository`** — `create` (starts a run, returns its id); `record_account`
  (appends a `RunAccountResult`); `finish` (sets final status/counters/summary);
  `get`/`latest`/`history`; `mark_interrupted` (on startup, flips any run left
  `Running` — e.g. from a crash — to `Failed` with `error_summary: "Interrupted"`,
  returns the row count changed).
- **`SettingsRepository`** — `load` (missing keys fall back to `Settings::default()`
  values) / `save`.

All seven traits are implemented by a single `academic_infra::db::Database` (Task T6),
backed by one SQLite connection guarded by a mutex.

### Runtime (`runtime.rs`)

- **`Clock`** (`Send + Sync`) — `now() -> DateTime<Utc>`, `local_offset() ->
  FixedOffset`. Exists so date resolution (relative dates like "mañana", "next monday")
  and timestamps are testable without depending on the real system clock.
- **`SystemClock`** — the real `Clock`, backed by `chrono::Utc`/`chrono::Local`.
- **`RunObserver`** (`Send + Sync`) — `progress(&RunProgress)`, `finished(run_id,
  status)`; how the pipeline (Task T12) reports live progress to `src-tauri` (Task T14),
  which forwards it as `run:progress`/`run:finished` Tauri events.

## Testing

`crates/core/src/domain/*.rs` and `crates/core/src/ports/{email,model}.rs` each carry a
`#[cfg(test)] mod tests` covering: every fieldless enum's `as_str`/`FromStr` round trip
(plus one "rejects an unknown value" case); `SyncCursor`'s JSON shape and round trip for
both variants; `Settings::default()`; the secret key helpers; `Counters::add` and its
`Default`; `DashboardCounts::default`; `ProviderConfig::kind()`; `ProviderError::kind()`
(all three variants); `ModelError::is_transient()` (all four variants). Run with
`cargo test -p academic-core` (or `--workspace` for the whole tree).

## Known limitations

- `academic-core` compiles and its contract types/traits are exercised by unit tests,
  but `analyzer`, `dates`, `filters`, `normalize` and `pipeline` have no behavior yet —
  that's Tasks T3–T5 and T12.
- `academic-infra` (Task T1, this task) only declares its module skeleton and
  dependencies; no adapter implements any port yet (Tasks T6–T9, T13).
- `src-tauri` (Task T1) only opens a window via the `tauri-plugin-opener` plugin; no
  commands, DTOs or composition of the ports above exist yet (Task T14).
