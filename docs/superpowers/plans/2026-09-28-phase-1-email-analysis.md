# Phase 1 — Email analysis: implementation plan

> **For agentic workers:** execute with superpowers:subagent-driven-development. Each task below is assigned to one Sonnet sub-agent working in its own git worktree and feature branch. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> Per `AGENTS.md`, this plan defines contracts, scope and acceptance criteria; implementation code is written by the sub-agents. Contract code (types and signatures) in this plan is normative and must be reproduced exactly.

**Goal:** implement the Phase 1 design (`docs/superpowers/specs/2026-09-28-phase-1-email-analysis-design.md`, "the spec"): multi-account email sync (IMAP + Microsoft Graph), deterministic filters, structured AI analysis (jev / Ollama), SQLite persistence and the Tauri + React UI.

**Architecture:** Cargo workspace with `crates/core` (pure domain, ports, logic), `crates/infra` (adapters), `src-tauri` (composition root and commands) and a React frontend in `src/`. Contracts are fixed in Wave 0; later waves fill independent modules in parallel.

**Tech stack:** Rust 1.98 (MSVC), Tauri 2, rusqlite (bundled), tokio, reqwest (rustls), async-imap + tokio-rustls, mail-parser, keyring, React + TypeScript + Vite + Tailwind CSS v4, react-router, @tanstack/react-query, Vitest.

---

## 0. Conventions for every task

### 0.1 Environment
- Windows 11. Rust is installed under `%USERPROFILE%\.cargo\bin`; if `cargo` is not on `PATH`, prepend it: PowerShell `$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"`, bash `export PATH="$HOME/.cargo/bin:$PATH"`.
- Node 22 / npm 11. Run `npm install` in the worktree before any frontend command.

### 0.2 Git
- The sub-agent works in its own worktree. First action: `git checkout -b feature/<task-branch>` (branch name given per task). Commit there with Conventional Commit messages ending in the line `Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>`. Do not merge, rebase or push; the planner merges into `phase-1-email-analysis`.

### 0.3 Scope rules
- Only touch the files listed under "Owns". Anything else → stop and report to the planner (AGENTS.md §8.1, §9).
- Do not add dependencies to any `Cargo.toml` or `package.json` unless the task says so. Wave 0 declares every dependency needed by later waves. If a dependency is genuinely missing, add it, and report it explicitly.
- No secrets in code, tests, fixtures, logs or docs. Tests use fake values such as `test-key`.

### 0.4 Quality gates (run all that apply before reporting)
- Rust: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` (or `-p <crate>` while iterating; the full workspace at the end).
- Frontend: `npm run typecheck`, `npm run lint`, `npm run test`, `npm run build`.
- Fix all warnings you introduced. Do not silence lints with `allow` unless justified in the report.

### 0.5 Documentation
Each task writes the documentation file(s) listed under "Docs" (English, Markdown): purpose, public API, behavior and edge cases, errors, how to test, and known limitations. Documentation describes what exists, not plans.

### 0.6 Report format (final message of each sub-agent)
1. What was implemented. 2. Files created/modified. 3. Commands run and their results (pass/fail counts). 4. Pending issues. 5. Out-of-scope findings. 6. Deviations from the plan and why. 7. Branch name and last commit hash.

---

## 1. Waves and dependencies

```text
Wave 0 (parallel):  T1 Rust foundation + contracts      T2 Frontend foundation + API client
                         │                                    │
Wave 1 (parallel):  T3 normalize+filters  T4 dates  T5 analyzer  T6 SQLite  T7 AI models+secrets
                    T8 IMAP  T9 Graph  T10 UI dashboard/results/runs  T11 UI accounts/catalog/settings
                         │
Wave 2 (parallel):  T12 pipeline (needs T3,T4,T5)     T13 infra factories (needs T7,T8,T9)
                         │
Wave 3:             T14 src-tauri composition, commands, events, logging (needs all above)
                         │
Wave 4:             T15 documentation consolidation + end-to-end verification
```

Branches: `feature/<id>-<slug>` from the current `phase-1-email-analysis` tip. After each wave the planner merges all branches of the wave, runs the full quality gates, and reviews before starting the next wave.

---

## 2. Wave 0

### Task T1 — Rust workspace, contracts and skeletons
**Branch:** `feature/t1-rust-foundation`
**Owns:** `Cargo.toml` (root), `Cargo.lock`, `rust-toolchain.toml`, `.gitignore`, `crates/core/**`, `crates/infra/**`, `src-tauri/**`, `docs/core/domain-and-ports.md`.
**Must not touch:** `package.json`, `src/**`, other docs.

**Objective:** a compiling workspace whose `core` crate contains the complete domain model and ports (contracts below, exact), empty module files for every later task, all dependencies declared, and a minimal Tauri app that opens a window.

- [ ] **Step 1: Workspace.** Root `Cargo.toml` with `members = ["crates/core", "crates/infra", "src-tauri"]`, `resolver = "2"`, `[workspace.package] edition = "2021"`, and `[workspace.dependencies]` with the latest stable versions of: `serde` (derive), `serde_json`, `chrono` (serde), `thiserror`, `async-trait`, `indexmap` (serde), `regex`, `sha2`, `hex`, `html2text`, `tokio` (full), `reqwest` (default-features = false; features json, rustls-tls), `rusqlite` (bundled), `keyring` (windows-native), `async-imap` (default-features = false; runtime-tokio), `tokio-rustls`, `webpki-roots`, `mail-parser`, `rand`, `base64`, `url`, `tracing`, `tracing-subscriber` (env-filter), `tracing-appender`, `tauri` (2.x), `tauri-build` (2.x), `tauri-plugin-opener` (2.x), `wiremock`, `tempfile`. `rust-toolchain.toml`: channel `stable`, components `rustfmt`, `clippy`. `.gitignore`: `target/`, `node_modules/`, `dist/`, `*.db`, `.env*`.
- [ ] **Step 2: `crates/core`** — package `academic-core`. Dependencies: serde, serde_json, chrono, thiserror, async-trait, indexmap, regex, sha2, hex, html2text. Dev-dependencies: tokio (macros, rt-multi-thread). `src/lib.rs`:
  ```rust
  pub mod analyzer;
  pub mod dates;
  pub mod domain;
  pub mod filters;
  pub mod normalize;
  pub mod pipeline;
  pub mod ports;
  ```
  `analyzer/mod.rs`, `dates.rs`, `filters.rs`, `normalize.rs`, `pipeline.rs` contain only a `//!` module doc line (filled by later tasks). `domain/` and `ports/` contain the contracts in Step 3 and Step 4, split into files by topic (`domain/{mod,account,email,analysis,catalog,run,results,settings,secrets}.rs`, `ports/{mod,email,model,secrets,repositories,runtime}.rs`), everything re-exported from `domain` and `ports`.
- [ ] **Step 3: Domain contracts (exact).** All enums serialize as snake_case (`#[serde(rename_all = "snake_case")]`); all types derive `Debug, Clone, PartialEq` (plus `Eq`, `Serialize`, `Deserialize`, `Copy` where valid). Each enum gets `as_str(&self) -> &'static str` and `FromStr` (error: `domain::ParseEnumError(String)`) using the same snake_case strings (used by the database).
  ```rust
  // account.rs
  pub type AccountId = i64;
  pub enum ProviderConfig {
      Imap { host: String, port: u16, username: String },
      MicrosoftGraph { tenant: String },
  }
  impl ProviderConfig { pub fn kind(&self) -> ProviderKind }
  pub enum ProviderKind { Imap, MicrosoftGraph }                 // "imap" | "microsoft_graph"
  pub struct Account {
      pub id: AccountId, pub name: String, pub email_address: String,
      pub provider: ProviderConfig, pub folders: Vec<String>,
      pub default_university_id: Option<i64>, pub initial_sync_days: u32, pub enabled: bool,
      pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
  }
  pub struct AccountDraft {
      pub name: String, pub email_address: String, pub provider: ProviderConfig,
      pub folders: Vec<String>, pub default_university_id: Option<i64>,
      pub initial_sync_days: u32, pub enabled: bool,
  }
  pub const DEFAULT_GRAPH_TENANT: &str = "organizations";
  pub const DEFAULT_IMAP_FOLDER: &str = "INBOX";
  pub const DEFAULT_GRAPH_FOLDER: &str = "inbox";
  pub const DEFAULT_INITIAL_SYNC_DAYS: u32 = 14;

  // email.rs
  pub struct Address { pub name: Option<String>, pub email: String }
  pub enum EmailBody { Text(String), Html(String) }
  pub struct Email {
      pub account_id: AccountId, pub folder: String, pub provider_message_id: String,
      pub message_id_header: Option<String>, pub sender: Address, pub recipients: Vec<Address>,
      pub subject: String, pub received_at: DateTime<Utc>, pub body: EmailBody,
      pub source_url: Option<String>,
  }
  pub enum SyncCursor {                                           // serde: tagged, #[serde(tag = "type")]
      Imap { uidvalidity: u32, last_uid: u32 },
      Graph { link: String },
  }
  pub struct FetchBatch { pub emails: Vec<Email>, pub next_cursor: SyncCursor }
  pub const MAX_BATCH: usize = 200;
  pub enum SyncStatus { Never, Ok, Error }
  pub struct SyncState {
      pub account_id: AccountId, pub folder: String, pub cursor: Option<SyncCursor>,
      pub last_run_at: Option<DateTime<Utc>>, pub last_success_at: Option<DateTime<Utc>>,
      pub status: SyncStatus, pub last_error: Option<String>,
  }

  // analysis.rs
  pub enum Category { Notice, Task, Exam, Event, Change, GeneralInfo }
  pub enum Priority { Low, Medium, High, Urgent }                  // derive PartialOrd, Ord (Low < Urgent)
  pub enum DateKind { Exam, Deadline, Event, Class, Other }
  pub struct ResultDate { pub date: NaiveDate, pub time: Option<NaiveTime>, pub kind: DateKind }
  pub struct DateCandidate { pub span: String, pub date: NaiveDate, pub time: Option<NaiveTime> }
  pub struct ModelDescriptor { pub provider: String, pub model: String }
  pub struct AIAnalysis {
      pub university_id: Option<i64>, pub course_id: Option<i64>,
      pub category: Category, pub priority: Priority,
      pub action_required: bool, pub possible_change: bool,
      pub dates: Vec<ResultDate>, pub confidence: Option<f32>, pub model: ModelDescriptor,
  }
  pub enum AnalysisOutcome { Irrelevant { p_relevant: f32 }, Relevant(AIAnalysis) }

  // catalog.rs
  pub struct University { pub id: i64, pub name: String }
  pub struct Course { pub id: i64, pub university_id: i64, pub name: String, pub aliases: Vec<String> }
  pub struct Catalog { pub universities: Vec<University>, pub courses: Vec<Course> }

  // run.rs
  pub type RunId = i64;
  pub enum RunTrigger { Manual }
  pub enum RunStatus { Running, Success, Partial, Failed }
  pub enum RunAccountStatus { Success, Failed }
  pub enum ErrorKind { Auth, Connection, Protocol, Ai, Internal }
  #[derive(Default)]
  pub struct Counters { pub found: u32, pub filtered: u32, pub analyzed: u32, pub failed: u32, pub results_created: u32 }
  impl Counters { pub fn add(&mut self, other: &Counters) }
  pub struct RunAccountResult {
      pub account_id: AccountId, pub status: RunAccountStatus, pub counters: Counters,
      pub error_kind: Option<ErrorKind>, pub error_message: Option<String>,
  }
  pub struct RunAccountView { pub account_name: String, pub result: RunAccountResult }
  pub struct RunRecord {
      pub id: RunId, pub trigger: RunTrigger, pub started_at: DateTime<Utc>,
      pub finished_at: Option<DateTime<Utc>>, pub status: RunStatus, pub accounts_processed: u32,
      pub counters: Counters, pub error_summary: Option<String>, pub accounts: Vec<RunAccountView>,
  }
  pub enum RunStage { Connecting, Fetching, Analyzing, Done, Failed }
  pub struct RunProgress {
      pub run_id: RunId, pub account_id: AccountId, pub account_name: String,
      pub stage: RunStage, pub processed: u32, pub total: u32,
  }
  #[derive(thiserror::Error)]
  pub enum RunError {
      #[error("A check is already running")] AlreadyRunning,
      #[error("There are no enabled accounts")] NoEnabledAccounts,
      #[error("The AI provider is not configured: {0}")] AiNotConfigured(String),
      #[error("Storage error: {0}")] Storage(String),
  }

  // results.rs
  pub enum MessageOutcome { Filtered, Irrelevant, Analyzed, Failed }
  pub enum FilterReason { Duplicate, BlockedSender, BlockedDomain, OwnMessage, Empty }
  pub struct ProcessedMessage {
      pub account_id: AccountId, pub folder: String, pub provider_message_id: String,
      pub message_id_header: Option<String>, pub content_hash: String,
      pub received_at: DateTime<Utc>, pub outcome: MessageOutcome, pub reason: Option<String>,
      pub run_id: RunId, pub processed_at: DateTime<Utc>,
  }
  pub struct NewResult {
      pub account_id: AccountId, pub run_id: RunId, pub detected_at: DateTime<Utc>,
      pub received_at: DateTime<Utc>, pub subject: String, pub sender_name: Option<String>,
      pub sender_email: String, pub source_url: Option<String>, pub analysis: AIAnalysis,
  }
  pub struct StoredResult {
      pub id: i64, pub account_id: AccountId, pub account_name: String, pub subject: String,
      pub sender_name: Option<String>, pub sender_email: String,
      pub university_name: Option<String>, pub course_name: Option<String>,
      pub category: Category, pub priority: Priority, pub action_required: bool,
      pub possible_change: bool, pub dates: Vec<ResultDate>,
      pub received_at: DateTime<Utc>, pub detected_at: DateTime<Utc>,
      pub source_url: Option<String>, pub message_id_header: Option<String>,
      pub seen: bool, pub seen_at: Option<DateTime<Utc>>,
  }
  pub enum ResultSort { Upcoming, Priority, Received }
  pub struct ResultQuery {
      pub search: Option<String>, pub seen: Option<bool>, pub category: Option<Category>,
      pub account_id: Option<AccountId>, pub course_id: Option<i64>,
      pub sort: ResultSort, pub limit: u32, pub offset: u32,
  }
  pub struct Page<T> { pub items: Vec<T>, pub total: u64 }
  #[derive(Default)]
  pub struct DashboardCounts { pub new_total: u32, pub tasks: u32, pub exams: u32, pub events: u32, pub changes_and_notices: u32 }

  // settings.rs
  pub enum AiProviderKind { Ollama, Jev }
  pub struct Settings {
      pub ai_provider: AiProviderKind, pub jev_model: String, pub ollama_base_url: String,
      pub ollama_model: String, pub microsoft_client_id: Option<String>, pub blocked_senders: Vec<String>,
  }
  impl Default for Settings  // ollama, "jev-latest", "http://localhost:11434", "qwen3:8b", None, []

  // secrets.rs
  pub const SECRET_SERVICE: &str = "academic-assistant";
  pub const JEV_API_KEY: &str = "academic-assistant/jev-api-key";
  pub fn account_password_key(id: AccountId) -> String      // "academic-assistant/account/{id}/password"
  pub fn account_refresh_token_key(id: AccountId) -> String // "academic-assistant/account/{id}/refresh-token"
  ```
- [ ] **Step 4: Port contracts (exact).**
  ```rust
  // ports/email.rs
  #[derive(Debug, thiserror::Error)]
  pub enum ProviderError {
      #[error("Authentication failed: {0}")] Auth(String),
      #[error("Connection failed: {0}")] Connection(String),
      #[error("Protocol error: {0}")] Protocol(String),
  }
  impl ProviderError { pub fn kind(&self) -> ErrorKind }
  #[async_trait]
  pub trait EmailProvider: Send + Sync {
      async fn test_connection(&self) -> Result<(), ProviderError>;
      async fn fetch_new(&self, folder: &str, cursor: Option<&SyncCursor>, since: DateTime<Utc>)
          -> Result<FetchBatch, ProviderError>;
  }
  pub trait EmailProviderFactory: Send + Sync {
      fn create(&self, account: &Account) -> Result<Box<dyn EmailProvider>, ProviderError>;
  }

  // ports/model.rs
  pub type QuestionSet = IndexMap<String, Question>;
  pub type AnswerSet = HashMap<String, Answer>;
  pub enum Question {
      Noul { instructions: String },
      Choice { instructions: String, options: IndexMap<String, String> },
      Score { instructions: String, levels: Vec<String> },
  }
  pub enum Answer {
      Noul { p_yes: f32, confidence: Option<f32> },
      Choice { choice: String, confidence: Option<f32> },
      Score { level: u8, confidence: Option<f32> },
  }
  #[derive(Debug, thiserror::Error)]
  pub enum ModelError {
      #[error("The AI provider rejected the credentials")] Auth,
      #[error("The AI provider is rate limiting requests")] RateLimited,
      #[error("The AI provider is unavailable: {0}")] Unavailable(String),
      #[error("The AI provider returned an invalid response: {0}")] InvalidResponse(String),
  }
  impl ModelError { pub fn is_transient(&self) -> bool } // true for Auth, RateLimited, Unavailable
  #[async_trait]
  pub trait StructuredModel: Send + Sync {
      fn descriptor(&self) -> ModelDescriptor;
      async fn ask(&self, state: &serde_json::Value, questions: &QuestionSet) -> Result<AnswerSet, ModelError>;
  }
  pub trait ModelProvider: Send + Sync {
      /// Builds the model selected in `settings`. Err(message) when it cannot be used
      /// (e.g. jev selected without API key); the pipeline maps it to RunError::AiNotConfigured.
      fn current(&self, settings: &Settings) -> Result<Arc<dyn StructuredModel>, String>;
  }

  // ports/secrets.rs
  #[derive(Debug, thiserror::Error)]
  #[error("Secure storage error: {0}")]
  pub struct SecretError(pub String);
  pub trait SecretStore: Send + Sync {
      fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
      fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
      fn delete(&self, key: &str) -> Result<(), SecretError>;   // Ok when missing
  }

  // ports/repositories.rs
  #[derive(Debug, thiserror::Error)]
  pub enum RepoError {
      #[error("Not found")] NotFound,
      #[error("Conflict: {0}")] Conflict(String),
      #[error("Storage error: {0}")] Storage(String),
  }
  pub type RepoResult<T> = Result<T, RepoError>;
  pub trait AccountRepository: Send + Sync {
      fn list(&self) -> RepoResult<Vec<Account>>;
      fn get(&self, id: AccountId) -> RepoResult<Option<Account>>;
      fn insert(&self, draft: &AccountDraft, now: DateTime<Utc>) -> RepoResult<Account>;
      fn update(&self, id: AccountId, draft: &AccountDraft, now: DateTime<Utc>) -> RepoResult<Account>;
      fn delete(&self, id: AccountId) -> RepoResult<()>;
  }
  pub trait CatalogRepository: Send + Sync {
      fn catalog(&self) -> RepoResult<Catalog>;
      fn add_university(&self, name: &str, now: DateTime<Utc>) -> RepoResult<University>;
      fn rename_university(&self, id: i64, name: &str) -> RepoResult<University>;
      fn delete_university(&self, id: i64) -> RepoResult<()>;
      fn add_course(&self, university_id: i64, name: &str, aliases: &[String], now: DateTime<Utc>) -> RepoResult<Course>;
      fn update_course(&self, id: i64, name: &str, aliases: &[String]) -> RepoResult<Course>;
      fn delete_course(&self, id: i64) -> RepoResult<()>;
  }
  pub trait SyncStateRepository: Send + Sync {
      fn get(&self, account_id: AccountId, folder: &str) -> RepoResult<Option<SyncState>>;
      fn list_for_account(&self, account_id: AccountId) -> RepoResult<Vec<SyncState>>;
      fn record_success(&self, account_id: AccountId, folder: &str, cursor: &SyncCursor, at: DateTime<Utc>) -> RepoResult<()>;
      fn record_failure(&self, account_id: AccountId, folder: &str, error: &str, at: DateTime<Utc>) -> RepoResult<()>;
  }
  pub trait ProcessedMessageRepository: Send + Sync {
      fn exists(&self, account_id: AccountId, folder: &str, provider_message_id: &str) -> RepoResult<bool>;
      fn is_duplicate(&self, account_id: AccountId, message_id_header: Option<&str>, content_hash: &str) -> RepoResult<bool>;
      fn insert(&self, message: &ProcessedMessage) -> RepoResult<i64>;
  }
  pub trait ResultRepository: Send + Sync {
      /// Inserts the processed message (outcome Analyzed), the result and its dates in one transaction.
      fn save_analyzed(&self, message: &ProcessedMessage, result: &NewResult) -> RepoResult<i64>;
      fn query(&self, query: &ResultQuery, today: NaiveDate) -> RepoResult<Page<StoredResult>>;
      fn get(&self, id: i64) -> RepoResult<Option<StoredResult>>;
      fn set_seen(&self, id: i64, seen: bool, at: DateTime<Utc>) -> RepoResult<StoredResult>;
      fn mark_all_seen(&self, at: DateTime<Utc>) -> RepoResult<u64>;
      fn unseen_counts(&self) -> RepoResult<DashboardCounts>;
  }
  pub trait RunRepository: Send + Sync {
      fn create(&self, trigger: RunTrigger, started_at: DateTime<Utc>) -> RepoResult<RunId>;
      fn record_account(&self, run_id: RunId, result: &RunAccountResult) -> RepoResult<()>;
      fn finish(&self, run_id: RunId, status: RunStatus, finished_at: DateTime<Utc>,
                accounts_processed: u32, counters: &Counters, error_summary: Option<&str>) -> RepoResult<()>;
      fn get(&self, run_id: RunId) -> RepoResult<Option<RunRecord>>;
      fn latest(&self) -> RepoResult<Option<RunRecord>>;
      fn history(&self, limit: u32) -> RepoResult<Vec<RunRecord>>;
      /// Sets every `running` run to `failed` with error_summary "Interrupted"; returns rows changed.
      fn mark_interrupted(&self, at: DateTime<Utc>) -> RepoResult<u64>;
  }
  pub trait SettingsRepository: Send + Sync {
      fn load(&self) -> RepoResult<Settings>;         // missing keys → Settings::default() values
      fn save(&self, settings: &Settings) -> RepoResult<()>;
  }

  // ports/runtime.rs
  pub trait Clock: Send + Sync {
      fn now(&self) -> DateTime<Utc>;
      fn local_offset(&self) -> FixedOffset;
  }
  pub struct SystemClock;                              // impl Clock using chrono::Local
  pub trait RunObserver: Send + Sync {
      fn progress(&self, progress: &RunProgress);
      fn finished(&self, run_id: RunId, status: RunStatus);
  }
  ```
  Unit tests in `domain`: every enum round-trips `as_str`/`FromStr`; `SyncCursor` JSON round-trip for both variants; `Settings::default()` values; secret key helpers; `Counters::add`; `ModelError::is_transient`; `ProviderError::kind` (Auth→Auth, Connection→Connection, Protocol→Protocol).
- [ ] **Step 5: `crates/infra`** — package `academic-infra`, depends on `academic-core` and: rusqlite, keyring, reqwest, tokio, async-imap, tokio-rustls, webpki-roots, mail-parser, rand, base64, url, sha2, serde, serde_json, chrono, thiserror, async-trait, indexmap, tracing. Dev: wiremock, tempfile, tokio. `src/lib.rs`: `pub mod db; pub mod factory; pub mod graph; pub mod imap; pub mod jev; pub mod ollama; pub mod secrets;` Directories `db/`, `graph/`, `imap/` with `mod.rs`; files `factory.rs`, `jev.rs`, `ollama.rs`, `secrets.rs`; each only a `//!` doc line. Create empty `crates/infra/migrations/` (with `.gitkeep`) and `crates/infra/tests/fixtures/` (`.gitkeep`).
- [ ] **Step 6: `src-tauri`** — package `academic-assistant`, lib name `academic_assistant_lib`, `crate-type = ["staticlib", "cdylib", "rlib"]`; depends on academic-core, academic-infra, tauri, tauri-plugin-opener, serde, serde_json, tokio, chrono, tracing, tracing-subscriber, tracing-appender; build-dependency tauri-build. `build.rs` calls `tauri_build::build()`. `src/main.rs` (windows_subsystem = "windows" in release) calls `academic_assistant_lib::run()`. `src/lib.rs` builds a Tauri app with the opener plugin and no commands yet. `tauri.conf.json`: productName `Academic Assistant`, identifier `dev.eaybar.academic-assistant`, `build.beforeDevCommand = "npm run dev"`, `build.devUrl = "http://localhost:1420"`, `build.beforeBuildCommand = "npm run build"`, `build.frontendDist = "../dist"`, one window 1200×800 titled "Academic Assistant", CSP `default-src 'self'; style-src 'self' 'unsafe-inline'; connect-src ipc: http://ipc.localhost`. `capabilities/default.json`: window `main`, permissions `core:default` and `opener:allow-open-url` restricted to `https://*`. Icons: generate the default Tauri icon set (e.g. run `npx @tauri-apps/cli@2 init --ci` in the scratch/temp dir and copy its `icons/` folder).
- [ ] **Step 7: Verify.** `cargo build --workspace`, `cargo test --workspace`, clippy and fmt pass.
- [ ] **Step 8: Docs.** `docs/core/domain-and-ports.md`: every domain type and port, their invariants and who implements/consumes them.
- [ ] **Step 9: Commit** on `feature/t1-rust-foundation`.

**Acceptance:** workspace compiles; contracts match this plan exactly; all gates green.

### Task T2 — Frontend foundation and API client
**Branch:** `feature/t2-frontend-foundation`
**Owns:** `package.json`, `package-lock.json`, `index.html`, `vite.config.ts`, `tsconfig*.json`, `eslint.config.js`, `vitest.setup.ts`, `src/**`, `docs/frontend/overview.md`.
**Must not touch:** `src-tauri/**`, `crates/**`, `Cargo.*`.

- [ ] **Step 1: Scaffold** Vite + React + TypeScript (strict) at the repo root. Dependencies: `react`, `react-dom`, `react-router` (hash router), `@tanstack/react-query`, `@tauri-apps/api` (2.x), `@tauri-apps/plugin-opener` (2.x). Dev: `vite`, `@vitejs/plugin-react`, `typescript`, `tailwindcss` v4 + `@tailwindcss/vite`, `eslint` (flat config, typescript-eslint, react-hooks), `vitest`, `jsdom`, `@testing-library/react`, `@testing-library/user-event`, `@testing-library/jest-dom`, `@tauri-apps/cli` (2.x). Scripts: `dev` (vite), `build` (`tsc -b && vite build`), `typecheck` (`tsc -b --noEmit` or equivalent), `lint` (`eslint .`), `test` (`vitest run`), `tauri` (`tauri`). Vite: `server.port = 1420`, `strictPort: true`, `clearScreen: false`, ignore `src-tauri/**` in watch.
- [ ] **Step 2: API contract (exact).** `src/lib/api/types.ts` reproduces the spec §10.1 DTO types verbatim, plus:
  ```ts
  export interface CommandError { kind: 'validation'|'not_found'|'conflict'|'auth'|'connection'|'ai'|'internal'; message: string }
  export type RunStage = 'connecting'|'fetching'|'analyzing'|'done'|'failed';
  export interface RunProgressEvent { runId: number; accountId: number; accountName: string; stage: RunStage; processed: number; total: number }
  export interface RunFinishedEvent { runId: number; status: RunStatus }
  ```
  `src/lib/api/client.ts` exports an `api` object; each method calls `invoke` with exactly these command names and argument objects:
  | method | invoke |
  |---|---|
  | `checkNow()` | `check_now` `{}` → `number` |
  | `getDashboard(sort)` | `get_dashboard` `{ sort }` |
  | `getResults(query)` | `get_results` `{ query }` |
  | `setResultSeen(id, seen)` | `set_result_seen` `{ id, seen }` |
  | `markAllSeen()` | `mark_all_seen` `{}` |
  | `getRunHistory(limit)` | `get_run_history` `{ limit }` |
  | `getAccounts()` | `get_accounts` `{}` |
  | `addAccount(input)` | `add_account` `{ input }` |
  | `updateAccount(id, input)` | `update_account` `{ id, input }` |
  | `removeAccount(id)` | `remove_account` `{ id }` |
  | `testAccount(id)` | `test_account` `{ id }` |
  | `connectMicrosoftAccount(id)` | `connect_microsoft_account` `{ id }` |
  | `getCatalog()` | `get_catalog` `{}` |
  | `addUniversity(name)` / `updateUniversity(id, name)` / `removeUniversity(id)` | `add_university` `{ name }` / `update_university` `{ id, name }` / `remove_university` `{ id }` |
  | `addCourse(universityId, name, aliases)` / `updateCourse(id, name, aliases)` / `removeCourse(id)` | `add_course` `{ universityId, name, aliases }` / `update_course` `{ id, name, aliases }` / `remove_course` `{ id }` |
  | `getSettings()` / `updateSettings(input)` | `get_settings` `{}` / `update_settings` `{ input }` |
  | `setJevApiKey(key)` / `clearJevApiKey()` | `set_jev_api_key` `{ key }` / `clear_jev_api_key` `{}` |
  | `testAiProvider()` | `test_ai_provider` `{}` |

  Rejections are normalized with `toCommandError(e: unknown): CommandError` (objects with `kind`+`message` pass through; anything else → `{ kind: 'internal', message: 'Something went wrong' }`). `src/lib/api/events.ts`: `onRunProgress(cb) → Promise<UnlistenFn>` (event `run:progress`), `onRunFinished(cb)` (event `run:finished`). `src/lib/api/queryKeys.ts`: stable React Query keys (`dashboard(sort)`, `results(query)`, `runs(limit)`, `accounts`, `catalog`, `settings`).
- [ ] **Step 3: Shell.** `src/main.tsx` (QueryClientProvider + HashRouter), `src/App.tsx` routes `/` Dashboard, `/results`, `/runs`, `/accounts`, `/catalog`, `/settings`. `src/components/Layout.tsx` (sidebar per spec §11 with the unseen badge fed by `getDashboard('upcoming').counts.newTotal`, content outlet). `src/components/Modal.tsx` (native `<dialog>`, props `{ open, title, onClose, children, footer? }`, closes on Escape and backdrop click). `src/components/ErrorMessage.tsx` (renders a `CommandError | null` inline, `role="alert"`). Placeholder pages `src/features/<area>/<Area>Page.tsx` (`DashboardPage`, `ResultsPage`, `RunsPage`, `AccountsPage`, `CatalogPage`, `SettingsPage`) each rendering its heading only. Tailwind via `src/index.css` (`@import "tailwindcss";`), neutral light theme, sentence-case copy.
- [ ] **Step 4: Tests.** `client.test.ts`: every method invokes the right command name with the right args (mock `@tauri-apps/api/core`); `toCommandError` cases. `Modal.test.tsx`: renders title/children when open, calls `onClose` on Escape. `Layout.test.tsx`: renders six nav links.
- [ ] **Step 5: Verify** typecheck, lint, test, build.
- [ ] **Step 6: Docs.** `docs/frontend/overview.md`: structure, API client, events, query keys, conventions, how to add a screen, how to test with mocked `api`.
- [ ] **Step 7: Commit.**

**Acceptance:** `npm run build` produces `dist/`; client matches the command table exactly; gates green.

---

## 3. Wave 1 (all parallel, after Wave 0 is merged)

### Task T3 — Normalization and filters (`core`)
**Branch:** `feature/t3-normalize-filters` · **Owns:** `crates/core/src/normalize.rs`, `crates/core/src/filters.rs`, `docs/core/normalization-and-filters.md`.

Contracts:
```rust
// normalize.rs
pub const MAX_BODY_CHARS: usize = 12_000;
pub fn normalize_body(body: &EmailBody) -> String;
pub fn content_hash(sender_email: &str, subject: &str, normalized_body: &str) -> String; // lowercase hex SHA-256 of lowercase(sender) + "\n" + trim(subject) + "\n" + body
// filters.rs
pub struct FilterConfig { pub blocked: Vec<String>, pub own_address: String }
pub enum FilterDecision { Keep, Discard(FilterReason) }
pub fn apply(email: &Email, normalized_body: &str, config: &FilterConfig) -> FilterDecision;
```
Behavior: spec §5.2 and §5.3. Order of filter checks: blocked sender → blocked domain → own message → empty. Domain entries (`@uv.es`) match the sender's domain exactly (not subdomains); matching is case-insensitive and trims whitespace; blank entries ignored. Truncation counts `char`s, never splits a char.

Tests (at least): HTML converted to text; `>` quoted lines removed; cut at `-----Original Message-----`, `On Mon, 1 Sep 2026, X wrote:`, `El lun, 1 sept 2026 a las 10:00, X escribió:`, and an Outlook `De:`/`Enviado:` header block; signature after `-- ` removed; whitespace collapsed (max one blank line); truncation at 12,000 chars with multibyte text; hash stable, case-insensitive on sender, sensitive to body; each filter reason; `Keep` for a normal email; blocked domain does not match `x@sub.uv.es` when `@uv.es` is blocked.

### Task T4 — Date candidate extraction (`core`)
**Branch:** `feature/t4-dates` · **Owns:** `crates/core/src/dates.rs` (may become `dates/` with submodules), `docs/core/date-extraction.md`.

Contract:
```rust
pub const MAX_CANDIDATES: usize = 20;
/// Scans subject then body. `received_at` is in the user's local offset.
pub fn extract(subject: &str, body: &str, received_at: DateTime<FixedOffset>) -> Vec<DateCandidate>;
```
Behavior: spec §6.3. `span` is the exact matched text including weekday/time when attached. Month names: Spanish (`enero`…`diciembre`, also `setiembre`) and English (full and 3-letter), case-insensitive, accents optional (`miércoles`/`miercoles`). `dd/mm` is day-first. Two-digit years → 2000+. Year inference: candidate years `received.year - 1 ..= received.year + 1`, choose the one closest to `received_at` among dates ≥ `received_at - 30 days`. Relative words resolve from `received_at.date()`: `hoy`=+0, `mañana`=+1, `pasado mañana`=+2, `próximo lunes`/`next monday` = the next such weekday strictly after today (if today is that weekday, +7). Excluded: `por la mañana`, `de la mañana`, `la mañana`, `esta mañana`. Times: `10:00`, `10.30`, `10h`, `10:00h`, `10:00 h`, `a las 10`, `a las 10:30`; hours 0–23, minutes 0–59; English `10am`/`3 pm`. Invalid dates (31/02) dropped; duplicates by (date, time) keep the first; order of appearance; cap 20.

Tests (at least 25 cases) with `received_at = 2026-09-28T09:00:00+02:00` (a Monday): each absolute format; weekday prefix kept in span; each relative form; each time form; morning exclusions; year inference across the year boundary (received 2026-12-20, "15 de enero" → 2027-01-15; received 2026-01-10, "20 de diciembre" → 2025-12-20; received 2026-09-28, "1 de septiembre" → 2026-09-01); invalid date dropped; duplicates; cap at 20; subject scanned before body.

### Task T5 — Email analyzer (`core`)
**Branch:** `feature/t5-analyzer` · **Owns:** `crates/core/src/analyzer/**`, `docs/core/email-analyzer.md`.

Contract:
```rust
pub const RELEVANCE_THRESHOLD: f32 = 0.4;
pub const FLAG_THRESHOLD: f32 = 0.5;
pub const MAX_COURSE_OPTIONS: usize = 254;
pub struct AnalysisInput<'a> {
    pub email: &'a Email, pub normalized_body: &'a str,
    pub date_candidates: &'a [DateCandidate], pub account: &'a Account, pub catalog: &'a Catalog,
}
pub struct AnalysisRequest { pub state: serde_json::Value, pub questions: QuestionSet }
pub fn build_request(input: &AnalysisInput) -> AnalysisRequest;
pub fn assemble(input: &AnalysisInput, answers: &AnswerSet, model: ModelDescriptor) -> Result<AnalysisOutcome, ModelError>;
pub async fn analyze(input: &AnalysisInput<'_>, model: &dyn StructuredModel) -> Result<AnalysisOutcome, ModelError>;
```
Behavior: spec §6.1, §6.2, §6.4 exactly (instruction texts, option keys and descriptions, levels in order `low, medium, high, urgent`, keys `u{id}`, `c{id}`, `unknown`, `none`, `date_{i}` with `i` = index in `date_candidates` truncated to `MAX_CANDIDATES`). `received_at` in state is RFC 3339. Validation: an answer missing for an asked question, of a different type, with an unknown option, or with a level out of range → `ModelError::InvalidResponse(<question id and reason>)`; unexpected extra answers are ignored. `university`/`course` questions follow the conditions in §6.2; courses limited to 254 (first 254 by catalog order).

Tests (at least): state shape with and without default university; relevant/category/action_required/possible_change/priority always present; `university` present only without default university and non-empty catalog; `course` options scoped to the default university; course description includes aliases; no `course` when no candidate courses; `date_i` per candidate; irrelevant below threshold (0.39) and relevant at 0.4; flags at 0.5; university resolution order (default → choice → course's university); `unknown`/`none` → None; `not_relevant` dates dropped and kinds mapped; confidence = minimum present / None; each invalid-answer case; `analyze` calls the model exactly once (fake `StructuredModel` in the test module).

### Task T6 — SQLite database and repositories (`infra`)
**Branch:** `feature/t6-sqlite` · **Owns:** `crates/infra/src/db/**`, `crates/infra/migrations/0001_init.sql`, `crates/infra/tests/db_*.rs`, `docs/infra/database.md`.

Contract:
```rust
pub struct Database { /* Mutex<rusqlite::Connection> */ }
impl Database {
    pub fn open(path: &std::path::Path) -> Result<Self, RepoError>;   // creates file, WAL, foreign_keys, migrations
    pub fn open_in_memory() -> Result<Self, RepoError>;
}
// Database implements AccountRepository, CatalogRepository, SyncStateRepository,
// ProcessedMessageRepository, ResultRepository, RunRepository, SettingsRepository.
```
Behavior: schema exactly as spec §9 in `0001_init.sql`, embedded with `include_str!`; a `schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT)` table tracks applied versions. JSON columns via serde_json. Settings stored per key (`ai.provider`, `jev.model`, `ollama.base_url`, `ollama.model`, `microsoft.client_id`, `filters.blocked`). `UNIQUE` violations → `RepoError::Conflict`; missing rows in update/delete/set_seen → `NotFound`. Query semantics for `ResultRepository::query`: filters combine with AND; `search` is case-insensitive substring over subject, sender_name, sender_email (escape `%`/`_`); `Upcoming` sort: results having a date ≥ `today` first ordered by their minimum such date ascending, then the rest by `received_at` desc; `Priority`: urgent→low then `received_at` desc; `Received`: `received_at` desc; `total` counts all matches ignoring limit/offset. `unseen_counts` per spec §10.2. `RunRecord.accounts` joined with account names, ordered by account id. `history` newest first.

Tests (integration, temp file or in-memory): migrations idempotent on reopen; account CRUD + JSON config round-trip for both providers; unique university name → Conflict; course aliases round-trip; deleting a university cascades courses and nulls account default; sync state success/failure upsert; `exists`/`is_duplicate` by header and by hash; `save_analyzed` atomic (result + dates + processed row); each query filter; search escaping; each sort order including mixed past/future dates; pagination and total; set_seen true/false; mark_all_seen count; unseen_counts categories; run lifecycle incl. `record_account`, `finish`, `latest`, `history`, `mark_interrupted`; deleting an account cascades its results, processed messages, sync states and run_accounts; settings defaults and round-trip.

### Task T7 — Secret store and AI model adapters (`infra`)
**Branch:** `feature/t7-ai-models` · **Owns:** `crates/infra/src/secrets.rs`, `crates/infra/src/jev.rs`, `crates/infra/src/ollama.rs`, `crates/infra/tests/jev.rs`, `crates/infra/tests/ollama.rs`, `docs/infra/ai-providers.md`, `docs/infra/secure-storage.md`.

Contracts:
```rust
// secrets.rs
pub struct KeyringSecretStore;                 // service = SECRET_SERVICE, user/target = key
impl SecretStore for KeyringSecretStore { .. } // delete of a missing entry → Ok(())
pub struct InMemorySecretStore { /* Mutex<HashMap<String,String>> */ }   // pub, Default; used by tests of other tasks
impl SecretStore for InMemorySecretStore { .. }
// jev.rs
pub const JEV_BASE_URL: &str = "https://api.typesafe.ai";
pub struct JevModel { .. }
impl JevModel { pub fn new(http: reqwest::Client, base_url: String, api_key: String, model: String) -> Self }
impl StructuredModel for JevModel { .. }       // descriptor: provider "jev", model = configured model
// ollama.rs
pub struct OllamaModel { .. }
impl OllamaModel { pub fn new(http: reqwest::Client, base_url: String, model: String) -> Self }
impl StructuredModel for OllamaModel { .. }    // descriptor: provider "ollama"
```
Behavior: spec §7.1 and §7.2 exactly (endpoint `{base_url}/v1/systemone`; retries 3 attempts with 1 s/2 s/4 s backoff — make the backoff base configurable internally so tests run with ~0 ms; timeouts via per-request `timeout`). Never log or include the API key or state content in errors. Ollama prompt lists each question as `"<id>": <instructions>` followed by options `- <key>: <description>` or levels `- <index>: <level>`.

Tests (wiremock): jev request body shape for all three types (Choice criteria object, Score criteria array, Noul without criteria) and `Authorization` header; mapping of noul/choice/score (argmax of probabilities) and confidence; 401 → Auth; 422 → InvalidResponse; 429 then 200 → success after retry; persistent 429 → RateLimited; 529 → Unavailable after retries; malformed JSON → InvalidResponse; missing answer key → passes through (validation is the analyzer's job) — i.e. adapter returns only what it received. Ollama: request contains `format` schema with boolean / string enum / integer enum and `stream:false`, `think:false`, temperature 0; mapping true/false → 1.0/0.0; connection refused → Unavailable; content not matching schema → InvalidResponse. `InMemorySecretStore` get/set/delete. `KeyringSecretStore` test marked `#[ignore]` (touches the real Credential Manager) using a random key and cleaning up.
Also an `#[ignore]` test per provider hitting the real service when `JEV_API_KEY` / `OLLAMA_MODEL` env vars are set.

### Task T8 — IMAP provider (`infra`)
**Branch:** `feature/t8-imap` · **Owns:** `crates/infra/src/imap/**`, `crates/infra/tests/imap_*.rs`, `crates/infra/tests/fixtures/eml/**`, `docs/infra/imap.md`.

Contract:
```rust
pub struct ImapEmailProvider { .. }
impl ImapEmailProvider {
    pub fn new(account_id: AccountId, host: String, port: u16, username: String, password: String) -> Self;
}
impl EmailProvider for ImapEmailProvider { .. }
/// Pure parsing, used by the provider and tested with fixtures.
pub fn parse_message(account_id: AccountId, folder: &str, uid: u32, raw: &[u8],
                     internal_date: Option<DateTime<Utc>>) -> Result<Email, ProviderError>;
/// Pure cursor planning, tested without a server.
pub fn select_uids(found: &[u32], cursor_last_uid: Option<u32>, limit: usize) -> Vec<u32>; // filters ≤ last_uid, sorts asc, takes `limit`
```
Behavior: spec §8.1. TLS via tokio-rustls with `webpki-roots`. `parse_message`: sender from `From` (name + address; missing → Protocol error), recipients from `To`+`Cc`, subject (empty if missing), `received_at` from `internal_date` (IMAP `INTERNALDATE`, fetched together with the body) and, when it is None, from the `Date` header; when both are missing → Protocol error, body `Text` if a text/plain part exists else `Html`, `message_id_header` without angle brackets, `provider_message_id = uid.to_string()`, `source_url = None`. Connection errors mapped per spec. Password never logged.

Tests: `.eml` fixtures (plain text Spanish with accents in quoted-printable, HTML-only, multipart alternative, encoded-word subject `=?UTF-8?B?...?=`, missing Message-ID, Cc recipients); `select_uids` cases (no cursor, cursor filtering the `*` quirk, limit 200). An `#[ignore]` live test reading `IMAP_HOST`, `IMAP_USER`, `IMAP_PASSWORD` env vars that runs `test_connection` and a `fetch_new` of the last 2 days.

### Task T9 — Microsoft Graph provider and OAuth (`infra`)
**Branch:** `feature/t9-graph` · **Owns:** `crates/infra/src/graph/**`, `crates/infra/tests/graph_*.rs`, `crates/infra/tests/fixtures/graph/**`, `docs/infra/microsoft-graph.md`.

Contracts:
```rust
pub const LOGIN_BASE_URL: &str = "https://login.microsoftonline.com";
pub const GRAPH_BASE_URL: &str = "https://graph.microsoft.com/v1.0";
pub const SCOPES: &str = "offline_access Mail.Read User.Read";
pub struct OAuthConfig { pub client_id: String, pub tenant: String, pub login_base_url: String }
pub struct TokenSet { pub access_token: String, pub refresh_token: String, pub expires_at: DateTime<Utc> }
pub mod oauth {
    pub struct Pkce { pub verifier: String, pub challenge: String }       // S256
    pub fn new_pkce() -> Pkce;
    pub fn authorize_url(config: &OAuthConfig, redirect_uri: &str, pkce: &Pkce, state: &str) -> String;
    /// Binds 127.0.0.1:0, calls `open_browser(authorize_url)`, waits (timeout) for the redirect,
    /// checks `state`, exchanges the code. Returns tokens.
    pub async fn sign_in(http: &reqwest::Client, config: &OAuthConfig,
                         open_browser: &(dyn Fn(&str) -> Result<(), String> + Send + Sync),
                         timeout: std::time::Duration) -> Result<TokenSet, ProviderError>;
    pub async fn refresh(http: &reqwest::Client, config: &OAuthConfig, refresh_token: &str) -> Result<TokenSet, ProviderError>;
}
/// GET /me → returns mail or userPrincipalName.
pub struct SignedInIdentity { pub mail: Option<String>, pub user_principal_name: Option<String> }
/// GET /me → both identifiers; the account email must match either (case-insensitive).
pub async fn signed_in_identity(http: &reqwest::Client, graph_base_url: &str, access_token: &str) -> Result<SignedInIdentity, ProviderError>;
pub struct MicrosoftGraphEmailProvider { .. }
impl MicrosoftGraphEmailProvider {
    pub fn new(account_id: AccountId, http: reqwest::Client, oauth: OAuthConfig, graph_base_url: String,
               secrets: Arc<dyn SecretStore>) -> Self;
}
impl EmailProvider for MicrosoftGraphEmailProvider { .. }
```
Behavior: spec §8.2. The provider reads the refresh token from `account_refresh_token_key(account_id)`; missing → `Auth("Sign in to this account")`; on refresh stores the rotated refresh token; `invalid_grant` and AADSTS consent errors mapped as specified. The loopback listener serves a minimal HTML page ("You can close this window") and handles only `GET /?code=…&state=…` or `?error=…`. `test_connection` = refresh + `GET /me`. Tokens never logged. Folder name is used as the well-known folder id in the URL (URL-encoded). Graph message JSON → `Email` (`from.emailAddress`, `toRecipients`+`ccRecipients`, `receivedDateTime`, `body.contentType` text/html, `internetMessageId` without angle brackets, `webLink`).

Tests (wiremock + JSON fixtures): PKCE challenge = base64url(SHA-256(verifier)) without padding; authorize URL contains client_id, response_type=code, redirect_uri, scope, code_challenge_method=S256, state; token exchange form fields; refresh stores rotated token in `InMemorySecretStore`; `invalid_grant` → Auth; AADSTS65001 → Auth with the admin-approval message; delta first request has `$filter` and `Prefer` header; pagination follows `nextLink` until `deltaLink`; stops at 200 and returns the `nextLink` as cursor; `@removed` ignored; 400 on filtered request → retried without `$filter` and older messages discarded; message mapping. `sign_in` tested end-to-end by calling the loopback URL from the test via `open_browser` callback that performs a GET to the redirect with a code and state against a wiremock token endpoint; also state mismatch → Auth error.

### Task T10 — UI: Dashboard, Results, Runs
**Branch:** `feature/t10-ui-dashboard-results-runs` · **Owns:** `src/features/dashboard/**`, `src/features/results/**`, `src/features/runs/**`, `src/components/results/**` (ResultCard, PriorityBadge, CategoryLabel, ResultDates, SortSelect, format helpers), `docs/frontend/dashboard-results-runs.md`.
**Must not touch:** `src/lib/api/**`, `src/components/{Layout,Modal,ErrorMessage}.tsx`, other features.

Behavior: spec §11 Dashboard, Results and Runs, using `api` and React Query only.
- Dashboard: `getDashboard(sort)`; "Check now" → `checkNow()`, then listens to `onRunProgress`/`onRunFinished` to show per-account progress ("UV IMAP: analyzing 12/40") and invalidates dashboard/results/runs queries on finish; if `runningRunId` is set on load, show the running state. Attention banners. Five counters. Sort selector (labels "Nearest upcoming date", "Priority", "Received date"). "Mark all as seen". Result cards with "Open email" (via `@tauri-apps/plugin-opener` `openUrl`) or "Copy Message-ID" (clipboard), and "Seen".
- Results: search input (debounced 300 ms) + "Filters and sort" button with active-filter count opening `Modal` with state, category, account (from `getAccounts`), course (from `getCatalog`) and sort; Apply/Reset. Paged table (25 per page, previous/next). Row expands inline with details and "Mark as seen"/"Mark as new".
- Runs: `getRunHistory(50)`; rows with status, start time, duration, counters; expandable per-account detail with error kind and message.
- Dates formatted with `Intl.DateTimeFormat('en-GB')`; empty states per spec copy rules.

Tests (Vitest + Testing Library, `api` mocked with `vi.mock('../../lib/api/client')`): dashboard renders counters and cards; sort change refetches with the new sort; Check now calls `checkNow` and shows progress from a mocked event; Seen calls `setResultSeen(id, true)`; Results filter modal applies filters to the query (category + seen) and shows the active count; Mark as new calls `setResultSeen(id, false)`; pagination offsets; Runs expands a failed account with its message.

### Task T11 — UI: Accounts, Catalog, Settings
**Branch:** `feature/t11-ui-config` · **Owns:** `src/features/accounts/**`, `src/features/catalog/**`, `src/features/settings/**`, `src/components/forms/**` (Field, TextInput, Select, Toggle helpers), `docs/frontend/accounts-catalog-settings.md`.
**Must not touch:** `src/lib/api/**`, `src/components/{Layout,Modal,ErrorMessage}.tsx`, other features.

Behavior: spec §11 Accounts, Catalog, Settings.
- Accounts: list with provider, email, default university name, folders, last sync status; actions Test (shows result inline), Edit, Reconnect/Sign in (Graph → `connectMicrosoftAccount`, shows "Waiting for sign-in in your browser…"), enable toggle (`updateAccount` with `enabled` flipped), Delete with confirmation modal (copy from spec). Add/edit modal with IMAP / Microsoft 365 switch (switch disabled when editing); validation: name, email (contains `@`), host, port 1–65535, username, password required on create for IMAP, folders comma-separated non-empty (defaults `INBOX` / `inbox`), initial sync days 1–365; tenant defaults to `organizations` (not shown). Microsoft accounts: after Save, offer "Sign in with Microsoft".
- Catalog: universities with nested courses; add/rename/delete university; add/edit/delete course with aliases (comma-separated input → trimmed array). Delete confirmations.
- Settings: form bound to `getSettings`/`updateSettings`; provider radio cards with the privacy hints; jev key status + Replace (modal with password input → `setJevApiKey`) + Remove (`clearJevApiKey`); Test provider (`testAiProvider`, shows result); Microsoft client ID; blocked senders textarea (one per line ↔ array, blanks dropped).
- Errors from commands shown with `ErrorMessage` next to the control.

Tests: IMAP add form validation errors and successful `addAccount` payload (exact object); Microsoft add payload with tenant `organizations` and folders `['inbox']`; enable toggle calls `updateAccount` with `enabled: false`; delete requires confirmation; catalog add course parses aliases; settings save sends blocked senders array; jev key replace calls `setJevApiKey` and never renders the key afterwards; Test provider shows the error message from `ConnectionTestDto`.

---

## 4. Wave 2 (after Wave 1 is merged)

### Task T12 — Check pipeline (`core`)
**Branch:** `feature/t12-pipeline` · **Owns:** `crates/core/src/pipeline.rs` (may become `pipeline/`), `crates/core/tests/pipeline.rs` (+ fakes under `crates/core/tests/support/`), `docs/core/pipeline.md`.

Contract:
```rust
pub struct Repositories {
    pub accounts: Arc<dyn AccountRepository>, pub catalog: Arc<dyn CatalogRepository>,
    pub sync: Arc<dyn SyncStateRepository>, pub processed: Arc<dyn ProcessedMessageRepository>,
    pub results: Arc<dyn ResultRepository>, pub runs: Arc<dyn RunRepository>,
    pub settings: Arc<dyn SettingsRepository>,
}
pub struct CheckRunner { .. }
impl CheckRunner {
    pub fn new(repos: Repositories, providers: Arc<dyn EmailProviderFactory>,
               models: Arc<dyn ModelProvider>, observer: Arc<dyn RunObserver>, clock: Arc<dyn Clock>) -> Self;
    /// Checks preconditions, takes the single-run lock, creates the run row. Does no I/O on mail.
    pub fn prepare(&self, trigger: RunTrigger) -> Result<PreparedRun, RunError>;
    /// Executes a prepared run to completion (never panics on provider/model errors) and releases the lock.
    pub async fn execute(&self, run: PreparedRun) -> RunStatus;
    pub fn running_run(&self) -> Option<RunId>;
}
pub struct PreparedRun { pub run_id: RunId, /* model + settings snapshot, private */ }
```
Behavior: spec §5 and §5.1 exactly, using `normalize`, `filters`, `dates::extract` (with `received_at` converted using `clock.local_offset()`), `analyzer::analyze`. Details:
- `prepare`: lock busy → `AlreadyRunning`; `settings.load`; no enabled accounts → `NoEnabledAccounts`; `models.current(&settings)` Err(msg) → `AiNotConfigured(msg)`; repo errors → `Storage`. Lock released if `prepare` fails after taking it.
- Per account: `factory.create` error → account Failed with `error.kind()`; for each folder: `sync.get` → cursor; `since = now − initial_sync_days`; `fetch_new` error → `record_failure`, account Failed, continue with next folder; messages processed per §5.1 table; `found` counts messages not already `exists`; a transient `ModelError` stops the folder and the account (no `record_success`, `record_failure` with the message, error kind `Ai`); after a folder completes → `record_success(next_cursor)`. An account is Failed if any folder failed.
- `FilterConfig.own_address` = account email; `blocked` = settings.blocked_senders.
- Progress events: `Connecting` per account, `Fetching` per folder, `Analyzing` with processed/total every 10 messages and at the end of the batch, then `Done` or `Failed` per account. `observer.finished` once.
- Final status and counters per spec; `error_summary` = `"<n> of <m> accounts failed"` when partial/failed, else None.
- Repository errors during execution mark the current account Failed with kind `Internal`.

Tests (fakes for every port, in-memory): happy path creates results and advances cursor; already-exists messages skipped and not counted; duplicate by header/hash → filtered; each filter → filtered with reason string (`blocked_sender`, …); irrelevant stored as `irrelevant`; `InvalidResponse` → `failed` and continues, cursor advanced; transient AI error → stops account, cursor not advanced, `record_failure` called, other accounts still processed; provider creation error → Failed with Auth; run statuses success/partial/failed and summary; `AlreadyRunning` while a run is executing; `NoEnabledAccounts`; `AiNotConfigured`; lock released after execute and after failed prepare; progress sequence contains Connecting/Fetching/Analyzing/Done; dates use local offset (candidate from "mañana" resolved relative to local date).

### Task T13 — Infra factories (`infra`)
**Branch:** `feature/t13-factories` · **Owns:** `crates/infra/src/factory.rs`, `crates/infra/tests/factory.rs`, `docs/infra/factories.md`.

Contracts:
```rust
pub struct DefaultEmailProviderFactory { .. }
impl DefaultEmailProviderFactory {
    pub fn new(secrets: Arc<dyn SecretStore>, settings: Arc<dyn SettingsRepository>, http: reqwest::Client) -> Self;
}
impl EmailProviderFactory for DefaultEmailProviderFactory { .. }
// IMAP: password from account_password_key(id); missing → ProviderError::Auth("The password is missing. Edit the account to set it").
// Graph: requires settings.microsoft_client_id (missing → Auth("Set the Microsoft client ID in Settings")),
//        OAuthConfig{client_id, tenant from account, LOGIN_BASE_URL}, GRAPH_BASE_URL, secrets.
pub struct DefaultModelProvider { .. }
impl DefaultModelProvider { pub fn new(secrets: Arc<dyn SecretStore>, http: reqwest::Client) -> Self; }
impl ModelProvider for DefaultModelProvider { .. }
// Ollama → OllamaModel(settings.ollama_base_url, settings.ollama_model).
// Jev → key from JEV_API_KEY; missing → Err("Add your jev API key in Settings"); JevModel(JEV_BASE_URL, key, settings.jev_model).
pub fn http_client() -> reqwest::Client;   // rustls, user agent "academic-assistant/<version>", connect timeout 10 s
```
Tests: each branch above using `InMemorySecretStore` and `Database::open_in_memory()` for settings; created model descriptors (`provider`, `model`).

---

## 5. Wave 3

### Task T14 — Tauri composition, commands and events
**Branch:** `feature/t14-tauri-commands` · **Owns:** `src-tauri/src/**`, `src-tauri/capabilities/**`, `src-tauri/tauri.conf.json` (only if needed), `docs/app/tauri-commands.md`, `docs/app/logging-and-data.md`.

Behavior: spec §10 and §12.
- `lib.rs` setup: resolve app data dir; init `tracing` (daily rolling file in `{app_data}/logs`, `info`, plus stderr in debug builds); `Database::open({app_data}/academic-assistant.db)`; `mark_interrupted(now)`; build `KeyringSecretStore`, `http_client()`, factories, `SystemClock`, a `TauriRunObserver` (emits `run:progress` / `run:finished` with camelCase payloads), `CheckRunner`; manage `AppState { db: Arc<Database>, secrets, runner: Arc<CheckRunner>, http, clock }`. Register the opener plugin and all commands.
- `dto.rs`: DTOs from spec §10.1 with `#[serde(rename_all = "camelCase")]`, conversions from/to domain; `ProviderDto`/`AccountInput.provider` tagged with `#[serde(tag = "type", rename_all = "snake_case")]` so JSON uses `"imap"` / `"microsoft_graph"`.
- `error.rs`: `CommandError { kind, message }` (Serialize) with `From<RepoError>` (NotFound→not_found, Conflict→conflict, Storage→internal with generic message "Something went wrong while saving data" and the detail logged), `From<RunError>` (AlreadyRunning→conflict, NoEnabledAccounts/AiNotConfigured→validation, Storage→internal), `From<ProviderError>` (auth/connection/internal for protocol), `From<SecretError>` (internal).
- `commands/{runs,results,accounts,catalog,settings}.rs` implementing every command in spec §10.2 with exactly the argument names of the T2 table (Tauri maps camelCase JS keys to snake_case params). Validation in commands: non-empty trimmed names; email contains `@`; port 1–65535; initial_sync_days 1–365; folders non-empty; `search` trimmed (empty → None); `limit` clamped 1–200; settings URLs start with `http://` or `https://`; model names non-empty.
- `check_now`: `runner.prepare(Manual)?`, spawn `runner.execute` with `tauri::async_runtime::spawn`, return run id.
- `get_dashboard`: `latest` run, `running_run`, `unseen_counts`, `query(seen=false, sort, limit 100)`, attention list (enabled accounts with any sync state `error` → its last_error; IMAP without password / Graph without refresh token → "Sign in to this account" / "The password is missing").
- `add_account`: insert, then store the IMAP password (on secret failure delete the inserted account and return internal). `update_account`: provider kind change → validation; password stored only when provided. `remove_account`: delete both secret keys, then the account. `test_account`: factory create + `test_connection` → `ConnectionTestDto`. `connect_microsoft_account`: client id required; `oauth::sign_in` with `open_browser` using `tauri_plugin_opener::open_url`, 5-minute timeout; verify the account email equals `mail` or `userPrincipalName` from `signed_in_identity` (case-insensitive) else validation error "Signed in as <x>, expected <y>" and nothing stored; store refresh token.
- `test_ai_provider`: `ModelProvider::current` then one Noul question (`"This text is a greeting."` on state `"Hello"`), ok when an answer is returned.
- Commands never return or log secrets, bodies or subjects.

Tests: unit tests for DTO conversions (serde JSON shape of `AccountInput` for both providers, `ResultDto` camelCase keys), error mapping, and validation helpers. `cargo build -p academic-assistant` and full gates. Then run `npm run build` and `npx tauri build --debug --no-bundle` to verify the app compiles end-to-end (report duration and outcome).

---

## 6. Wave 4

### Task T15 — Documentation consolidation and verification
**Branch:** `feature/t15-docs` · **Owns:** `README.md`, `docs/README.md`, `docs/architecture.md`, `docs/setup.md`, `docs/user-guide.md`, `docs/development.md`, `docs/testing.md`.
- `README.md`: what the app does, status (Phase 1), quick start, links to docs.
- `docs/README.md`: index of every doc file (including the per-component files written by T1–T14).
- `docs/architecture.md`: crates, dependency rule, data flow of a check, diagrams in Mermaid, extension points (new email provider, new AI provider, future sources).
- `docs/setup.md`: prerequisites (Rust, MSVC Build Tools, Node, WebView2, Ollama + model, jev key), Microsoft Entra registration step by step (spec §8.2), running in dev (`npm install`, `npm run tauri dev`), building.
- `docs/user-guide.md`: every screen and action, privacy notes (what is stored, what is sent to jev), troubleshooting (admin consent, IMAP login, Ollama not running).
- `docs/development.md`: repo layout, conventions, quality gates, branch workflow (phase/feature branches), how to add a migration/command/screen.
- `docs/testing.md`: test suites, fixtures, `#[ignore]` live tests and their env vars.
- Verify every command in the docs by running it (except live-service ones) and that all links resolve.

---

## 7. Planner checkpoints
After each wave: merge feature branches into `phase-1-email-analysis` (`git merge --no-ff`), run all gates on the merged tree, review diffs against contracts and spec, dispatch fix tasks when needed. At the end: code review over the whole phase branch, final gates, `npx tauri build --debug --no-bundle`, and a summary for the user with the manual acceptance checklist (spec §13).
