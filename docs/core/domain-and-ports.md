# `academic-core`: domain model and ports

`academic-core` (package `academic-core`) is the pure domain crate for the Academic
Assistant: the Phase 1 email analysis pipeline and the Phase 2 LMS tracker. It has **no I/O dependencies** — only `serde`,
`serde_json`, `chrono`, `thiserror`, `async-trait`, `indexmap`, `regex`, `sha2`, `hex`
and `html2text`. All persistence, network and OS-credential-store access lives in
`academic-infra`, which depends on `academic-core` and implements the traits
described below. `src-tauri` is the composition root that wires `academic-infra`
adapters into `academic-core`'s ports and exposes Tauri commands.

Module layout (`crates/core/src/`):

```
lib.rs          public modules: analyzer, dates, domain, filters, lms, normalize, pipeline, ports
domain/         domain model (this document, "Domain" section)
ports/          traits + port-level error types (this document, "Ports" section)
normalize.rs    HTML→text, quote/signature stripping, content hash    (normalization-and-filters.md)
filters.rs      deterministic pre-filters                             (normalization-and-filters.md)
dates/          date candidate extraction                             (date-extraction.md)
analyzer/       question building + answer assembly                    (email-analyzer.md)
pipeline/       CheckRunner orchestration                             (pipeline.md)
lms/            URL normalization, session check, item diff, LmsSyncRunner (../infra/lms.md)
```

The behavior of the non-contract modules is documented in their own files:
[normalization-and-filters.md](normalization-and-filters.md),
[date-extraction.md](date-extraction.md), [email-analyzer.md](email-analyzer.md) and
[pipeline.md](pipeline.md). This document covers the domain types and ports they are built
on.

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
  ~20 enums below; it has no runtime behavior of its own.
- Structs derive `Debug, Clone, PartialEq` plus `Eq`/`Serialize`/`Deserialize`/`Copy`
  whenever every field supports them. Types holding an `f32` (`AIAnalysis`,
  `AnalysisOutcome`, the AI-provider `Answer` enum) cannot derive `Eq`.
- Two data-carrying enums use serde's internal tagging instead of `as_str`/`FromStr`:
  `SyncCursor` and `ProviderConfig`, both `#[serde(tag = "type", rename_all = "snake_case")]`.
  They are stored in SQLite JSON columns: `SyncCursor` in `sync_states.cursor` with its
  `type` tag; `ProviderConfig` in `accounts.config` **without** the tag, because the
  variant is stored separately in the `provider_type` column (see
  [../infra/database.md](../infra/database.md)).

## Domain (`crate::domain`)

Re-exported flat from `crate::domain`; internally split by topic into
`account.rs`, `email.rs`, `analysis.rs`, `catalog.rs`, `lms.rs`, `run.rs`, `results.rs`,
`settings.rs`, `secrets.rs`.

### `ParseEnumError`

`pub struct ParseEnumError(pub String)` — returned by every enum's `FromStr` when the
input string doesn't match any known snake_case variant. The wrapped `String` is the
invalid input, for error messages.

### Accounts (`account.rs`)

- **`AccountId = i64`**.
- **`ProviderConfig`** — `Imap { host, port, username }` | `MicrosoftGraph { tenant }`.
  When serialized on its own it is JSON-tagged (`type`: `"imap"` | `"microsoft_graph"`),
  but the database stores only the fields (`accounts.config`) and keeps the variant in
  `accounts.provider_type`. `.kind()` returns the matching `ProviderKind`. Note: the IMAP variant intentionally has no password field — the
  password lives in the OS credential store under `account_password_key(id)`, never in
  the domain struct or the database.
- **`ProviderKind`** — `Imap` (`"imap"`) | `MicrosoftGraph` (`"microsoft_graph"`).
  Stored in its own column (`accounts.provider_type`) next to the JSON config, which
  omits the type tag; the kind selects how the config is parsed.
- **`Account`** — a configured, persisted mailbox: id, display name, email address,
  provider config, folders to sync, optional default university (used by the analyzer
  to skip asking "which university" for single-university accounts), initial sync
  window in days, enabled flag, timestamps. Produced by `AccountRepository`; consumed by
  the pipeline ([pipeline.md](pipeline.md)) and `EmailProviderFactory`
  ([../infra/factories.md](../infra/factories.md)).
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
  `None` for IMAP). Produced by `EmailProvider::fetch_new` implementations
  ([IMAP](../infra/imap.md), [Microsoft Graph](../infra/microsoft-graph.md)).
- **`SyncCursor`** — `Imap { uidvalidity, last_uid } | Graph { link }`, JSON-tagged
  (`type`: `"imap"` | `"graph"`). Opaque outside the matching provider; stored as JSON in
  `sync_states.cursor` and round-tripped by value.
- **`FetchBatch`** — `{ emails, next_cursor }`, the return value of
  `EmailProvider::fetch_new`.
- **`MAX_BATCH: usize = 200`** — the largest batch a single `fetch_new` call may return;
  providers are responsible for paging internally to stay under this.
- **`SyncStatus`** — `Never` (`"never"`, no sync attempted yet) | `Ok` (`"ok"`) |
  `Error` (`"error"`).
- **`SyncState`** — per-account, per-folder sync bookkeeping: cursor, last run/success
  timestamps, status, last error message (sanitized — never includes credentials).
  Owned by `SyncStateRepository`.

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
  date mention found in the raw text by `dates::extract` ([date-extraction.md](date-extraction.md))
  and offered to the analyzer ([email-analyzer.md](email-analyzer.md)) as something to confirm/classify. Not `Serialize`/`Deserialize`
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
  return value of `analyzer::assemble`/`analyzer::analyze`.

### Catalog (`catalog.rs`)

The analyzer's read-only projection of the LMS data (there is no manual catalog any more).

- **`University`** — `{ id, name }`.
- **`Course`** — `{ id, university_id, name, aliases }`; `aliases` are alternate
  names/codes the analyzer should also recognize for the same course (here, the course's short
  name when it differs from its name).
- **`Catalog`** — `{ universities, courses }`: the configured universities and their **tracked,
  available** courses, as read by `CatalogRepository::catalog` and passed into
  `analyzer::AnalysisInput`.

### LMS (`lms.rs`)

- **`UniversityId = i64`**.
- **`LmsPlatform`** — `Moodle` (`"moodle"`) | `Blackboard` (`"blackboard"`).
- **`LoginState`** — `Never | Ok | Expired | Error`: whether the app holds a working credential.
- **`SyncOutcome`** — `Success | Failed | LoginRequired` (`"login_required"`): result of a
  university's last sync.
- **`LmsUniversity`** — id, name, platform, `base_url` (normalized `https` origin, e.g.
  `https://campus.demo.example`), `login_state`, `account_label` ("Connected as ..."), last sync
  time/outcome/error, `created_at`.
- **`LmsCourse`** — id, `university_id`, `external_id` (platform id: Moodle `"1234"`, Blackboard
  `"_10_1"`), name, short name, URL, `tracked`, `available` (false when the platform stops
  returning it), `reviewed` (false until the user saves a selection) and `items_synced_at`
  (empty until the course's first item sync, which suppresses the "new" badge).
- **`ItemKind`** — `Assignment | Quiz | Test | Event | Other`.
- **`PlatformStatus`** — `NotSubmitted | Submitted | Graded | NotApplicable` (`"not_applicable"`).
- **`Grade`** — `{ display, score: Option<f64>, max: Option<f64> }`.
- **`ItemChange`** — `New | DueChanged { previous_due_at }`; the badge shown until dismissed.
- **`LmsItem`** — a stored deliverable or event: ids, `external_id` (stable per university:
  `assign:812`, `quiz:77`, `event:9031`, `column:_20_1`, `calendar:abc`), kind, title, due date,
  URL, platform status, grade, change, `removed_at`, the personal fields `done`, `hidden`, `note`
  (at most 2000 characters) and timestamps.
- **`EffectiveStatus`** — `Pending | Overdue | Submitted | Graded | PastEvent`, derived by
  `effective_status(item, now)`: graded, then submitted or done, then past event (a past event, or
  any past item with nothing to submit), then overdue, else pending.
- **`SyncWindow`** — `{ from, to }`; `SyncWindow::around(now)` is now - 180 days to now + 365 days.
  Limits course events only.
- **`LmsIdentity`**, **`RemoteCourse`**, **`RemoteCourseRef`**, **`RemoteItem`** — what a platform
  returns before it is stored.
- **`TrackedItem`** — an item with its university and course names; **`TrackerQuery`** —
  `{ university_id, include_hidden, include_removed }`.

### Runs (`run.rs`)

- **`RunId = i64`**.
- **`RunTrigger`** — `Manual` (`"manual"`) only in Phase 1 (no scheduled runs yet).
- **`RunStatus`** — `Running | Success | Partial | Failed`, the run's overall outcome.
- **`RunAccountStatus`** — `Success | Failed`, one account's outcome within a run.
- **`ErrorKind`** — `Auth | Connection | Protocol | Ai | Internal`. Shared coarse
  classification used by `ProviderError::kind()`, and stored on `RunAccountResult` /
  surfaced to the UI as the error kind of a failed account (via `src-tauri`'s DTO mapping,
  see [../app/tauri-commands.md](../app/tauri-commands.md)).
- **`Counters`** — `{ found, filtered, analyzed, failed, results_created }` (all `u32`),
  `Default` (all zero), `Copy`. `.add(&mut self, other: &Counters)` sums each field
  in place; used to fold per-account counters into the run total.
- **`RunAccountResult`** — one account's outcome for a run: id, status, counters,
  optional `error_kind`/`error_message` (present only when `status == Failed`).
- **`RunAccountView`** — `{ account_name, result: RunAccountResult }`, the result plus the account's display name, for
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
  via `RunObserver::progress`.
- **`RunError`** — errors that can prevent a run from starting/completing:
  `AlreadyRunning`, `NoEnabledAccounts`, `AiNotConfigured(String)`, `Storage(String)`.
  Derives `Debug, Clone, PartialEq, Eq, thiserror::Error`; its `Display` texts are the
  user-facing messages ("A check is already running", "There are no enabled accounts",
  "The AI provider is not configured: {0}", "Storage error: {0}"). The "track at least one
  course" precondition of "Check now" is checked by the `check_now` command, not here
  ([../app/tauri-commands.md](../app/tauri-commands.md)).

### Results (`results.rs`)

- **`MessageOutcome`** — `Filtered | Irrelevant | Analyzed | Failed`, what happened to
  one fetched email during pipeline processing.
- **`FilterReason`** — `Duplicate | BlockedSender | BlockedDomain | OwnMessage | Empty`,
  why a deterministic filter ([normalization-and-filters.md](normalization-and-filters.md))
  discarded an email before analysis.
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
  `ResultRepository`), the `Message-ID` header (`message_id_header`), resolved `category`/`priority`/flags/`dates`
  (post-analysis, no longer wrapped in `AIAnalysis`/`ModelDescriptor`), and `seen`/
  `seen_at`.
- **`ResultSort`** — `Upcoming | Priority | Received`, ordering for `ResultQuery`; exact
  tie-break semantics are `ResultRepository::query`'s responsibility.
- **`ResultQuery`** — filter + pagination for `ResultRepository::query`: free-text
  `search` (case-insensitive substring over subject/sender), `seen`, `category`,
  `account_id`, `course_id`, `sort`, `limit`, `offset`. All filters combine with AND.
- **`Page<T>`** — `{ items: Vec<T>, total: u64 }`; `total` counts all matches ignoring
  `limit`/`offset`.
- **`DashboardCounts`** — `{ new_total, tasks, exams, events, changes_and_notices }`
  (all `u32`), `Default`/`Copy`; unseen-result counts by category for the Announcements header,
  from `ResultRepository::unseen_counts`.

### Settings (`settings.rs`)

- **`AiProviderKind`** — `Ollama | Jev`, which AI provider is currently selected.
- **`Settings`** — `{ ai_provider, jev_model, ollama_base_url, ollama_model,
  microsoft_client_id, blocked_senders }`. `Default` values:
  `ai_provider: Ollama`, `jev_model: "jev-latest"`,
  `ollama_base_url: "http://localhost:11434"`, `ollama_model: "qwen3:8b"`,
  `microsoft_client_id: None`, `blocked_senders: []`. Note: no API key field —
  the jev API key is a secret (see below), never stored in `Settings`/SQLite.
  Owned by `SettingsRepository`: `load()` falls back to these defaults for
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
- **`lms_credential_key(platform, id) -> String`** —
  `"academic-assistant/university/{id}/moodle-token"` (Moodle web-service token) or
  `"academic-assistant/university/{id}/blackboard-session"` (Blackboard cookies as JSON).

## Ports (`crate::ports`)

Traits and port-level error types only — no implementations. `academic-infra` provides
the real adapters. `InMemorySecretStore` (a test fake) lives in `academic_infra::secrets`,
not in `core`; `core`'s own tests use the fakes in `crates/core/tests/support/` and
test-only modules.

Split into `email.rs`, `lms.rs`, `model.rs`, `secrets.rs`, `repositories.rs`, `runtime.rs`, all
re-exported flat from `crate::ports`.

### Email providers (`email.rs`)

- **`ProviderError`** — `Auth(String) | Connection(String) | Protocol(String)`.
  `.kind()` maps `Auth → ErrorKind::Auth`, `Connection → ErrorKind::Connection`,
  `Protocol → ErrorKind::Protocol`.
- **`EmailProvider`** (`Send + Sync`, `#[async_trait]`) — a read-only connection to one
  account's mailbox: `test_connection()`, `fetch_new(folder, cursor, since)`. Implemented
  by `ImapEmailProvider` ([imap.md](../infra/imap.md)) and `MicrosoftGraphEmailProvider`
  ([microsoft-graph.md](../infra/microsoft-graph.md)).
- **`EmailProviderFactory`** (`Send + Sync`) — `create(&self, account: &Account) ->
  Result<Box<dyn EmailProvider>, ProviderError>`; builds the right provider for an
  account's `ProviderConfig`. Implemented in `academic-infra::factory`
  ([factories.md](../infra/factories.md)) and consumed by the pipeline and `src-tauri`.

### LMS platforms (`lms.rs`)

- **`LmsError`** — `Auth | Connection | Protocol`, each with a user-facing message that never
  contains a token, cookie or login-window URL. `.kind()` maps to `ErrorKind`. `Auth` means a
  missing, unreadable, expired or rejected credential: sign-in is needed.
- **`LmsClient`** (`Send + Sync`, `#[async_trait]`) — read-only access to one university as the
  signed-in user: `verify()` (who the credential belongs to; also checks the platform exposes what
  is needed), `list_courses()` and `fetch_items(courses, window)` (all or nothing). Implemented by
  `MoodleClient` and `BlackboardClient` ([../infra/lms.md](../infra/lms.md)).
- **`LmsClientFactory`** (`Send + Sync`) — `create(university)` builds a client from the stored
  credential (a missing one is `Auth`); `create_with_credential(university, credential)` builds one
  from a credential that is not stored yet, to verify it first.
- **`LmsAuthenticator`** (`Send + Sync`, `#[async_trait]`) — `renew_silently(university)`: a hidden
  sign-in attempt; on success the new credential is already verified and stored; `Err(Auth)` when
  the user has to sign in interactively. Implemented by `TauriLmsAuthenticator` in `src-tauri`.
- **`LmsSyncObserver`** (`Send + Sync`) — `progress(&LmsSyncProgress)` with the stage (`courses`,
  `items`, `saving`, `done`, `failed`, `login_required`) and `finished(&[LmsSyncResult])`, called
  exactly once per executed sync. Forwarded as `lms-sync:*` events.

### AI models (`model.rs`)

- **`QuestionSet = IndexMap<String, Question>`** — order-preserving; question order
  matters for prompt building.
- **`AnswerSet = HashMap<String, Answer>`**.
- **`Question`** — `Noul { instructions } | Choice { instructions, options:
  IndexMap<String,String> } | Score { instructions, levels: Vec<String> }`. "Noul" is a
  yes/no probability question.
- **`Answer`** — `Noul { p_yes, confidence } | Choice { choice, confidence } | Score {
  level, confidence }`, all with `Option<f32> confidence`. `Score.level` is a `u8` index
  into the question's `levels`.
- **`ModelError`** — `Auth | RateLimited | Unavailable(String) | InvalidResponse(String)`.
  `.is_transient()` is `true` for `Auth`, `RateLimited`, `Unavailable`; `false` for
  `InvalidResponse` (a malformed reply, not a transient provider issue — retrying
  won't help).
- **`StructuredModel`** (`Send + Sync`, `#[async_trait]`) — `descriptor()`,
  `ask(state, questions) -> Result<AnswerSet, ModelError>`. Implemented by `JevModel`
  and `OllamaModel` ([ai-providers.md](../infra/ai-providers.md)).
- **`ModelProvider`** (`Send + Sync`) — `current(&self, settings: &Settings) ->
  Result<Arc<dyn StructuredModel>, String>`; builds the model selected in `settings`,
  `Err(message)` when unusable (e.g. jev selected without an API key set) — the pipeline
  maps that `Err` to `RunError::AiNotConfigured`. Implemented in
  `academic-infra::factory`.

### Secrets (`secrets.rs`)

- **`SecretError(pub String)`** — `"Secure storage error: {0}"`.
- **`SecretStore`** (`Send + Sync`) — `get(key)`, `set(key, value)`, `delete(key)`
  (`Ok(())` even when the key doesn't exist). Implemented by
  `academic_infra::secrets::KeyringSecretStore` (Windows Credential Manager) and
  `academic_infra::secrets::InMemorySecretStore` (an in-memory fake for tests; see
  [../infra/secure-storage.md](../infra/secure-storage.md)).

### Repositories (`repositories.rs`)

- **`RepoError`** — `NotFound | Conflict(String) | Storage(String)`; `RepoResult<T> =
  Result<T, RepoError>`.
- **`AccountRepository`** — CRUD for `Account`/`AccountDraft`.
- **`CatalogRepository`** — read-only: `catalog()` returns the `Catalog` (universities and their
  tracked, available courses). The Phase 1 add/rename/delete methods for universities and
  courses were removed with the manual catalog.
- **`UniversityRepository`** — `list_universities`, `get_university`, `insert_university(name,
  platform, base_url, now)`, `rename_university`, `delete_university` (cascades courses and
  items), `set_login(id, state, account_label)` and `record_sync(id, at, outcome, error)`.
- **`CourseRepository`** — `list_courses`, `upsert_remote_courses` (new courses untracked and
  unreviewed, missing ones unavailable) and `set_tracked_courses` (exactly these become tracked and
  every course of the university becomes reviewed).
- **`LmsItemRepository`** — `list_items_for_university` (for diffing), `apply_sync` (one
  transaction), `query_items(&TrackerQuery)`, `get_item`, `set_item_done`, `set_item_hidden`,
  `set_item_note`, `dismiss_item_change` and `dismiss_all_changes`.
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

All ten traits are implemented by a single `academic_infra::db::Database`,
backed by one SQLite connection guarded by a mutex.

### Runtime (`runtime.rs`)

- **`Clock`** (`Send + Sync`) — `now() -> DateTime<Utc>`, `local_offset() ->
  FixedOffset`. Exists so date resolution (relative dates like "mañana", "next monday")
  and timestamps are testable without depending on the real system clock.
- **`SystemClock`** — the real `Clock`, backed by `chrono::Utc`/`chrono::Local`.
- **`RunObserver`** (`Send + Sync`) — `progress(&RunProgress)`, `finished(run_id,
  status)`; how the pipeline reports live progress to `src-tauri`,
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

- `academic-core` only defines the contracts and the pure logic built on them; every port
  is implemented outside it (`academic-infra`, plus the in-memory fakes used by tests).
  See [../architecture.md](../architecture.md) for how the crates fit together.
