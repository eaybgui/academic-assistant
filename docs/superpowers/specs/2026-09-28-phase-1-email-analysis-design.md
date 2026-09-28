# Phase 1 — Email analysis: design

Status: approved design (2026-09-28). Branch: `phase-1-email-analysis`.
Related: `requirements.md`, `technical-specification.md`.

## 1. Goal and scope

Deliver a vertical slice: the user adds several email accounts (IMAP and Microsoft 365), runs a manual check, and sees structured academic results (category, priority, course, dates) produced by an interchangeable structured AI provider (jev or Ollama).

### In scope
- Accounts: IMAP (username + password) and Microsoft 365 (Microsoft Graph, OAuth2). Add, edit, enable/disable, delete, test, reconnect.
- University and course catalog (manual CRUD).
- Manual "Check now" over all enabled accounts, with per-account error isolation.
- Incremental sync; first sync covers the last N days (per account, default 14); configured folders only (default inbox).
- Deduplication and conservative deterministic filters.
- AI analysis through `StructuredModel` (jev, Ollama) and a provider-agnostic `EmailAnalyzer`.
- Persistence of results (never the body), new/seen status, undo seen.
- UI: Dashboard, Results, Runs, Accounts, Catalog, Settings.
- Run log and application log.

### Out of scope (later phases)
Scheduler/background execution, Windows notifications, relating results to each other, free-text summaries, scraping and other sources, attachments, final visual design.

## 2. Prerequisites (user)
1. Rust via rustup (stable, `x86_64-pc-windows-msvc`) and Visual Studio Build Tools with the C++ workload. WebView2 ships with Windows 11.
2. A Microsoft Entra app registration (see §8.2) and its client ID, entered in Settings.
3. An Ollama model: `ollama pull qwen3:8b` (default; configurable).
4. A jev API key, entered in Settings (never in code, chat or files).

## 3. Repository layout

```text
Cargo.toml                      workspace: crates/core, crates/infra, src-tauri
crates/core/src/
  lib.rs
  domain/                       Account, Email, AIAnalysis, Run, catalog types, enums
  ports/                        EmailProvider(+Factory), StructuredModel, SecretStore, repositories
  normalize.rs                  HTML→text, quote/signature stripping, content hash
  filters.rs                    deterministic filters
  dates.rs                      date candidate extraction and resolution
  analyzer/                     question building + answer assembly (EmailAnalyzer)
  pipeline.rs                   CheckRunner
crates/infra/src/
  lib.rs
  db/                           connection, migrations (migrations/0001_init.sql), repositories
  secrets.rs                    Windows Credential Manager (keyring)
  imap/                         ImapEmailProvider
  graph/                        MicrosoftGraphEmailProvider + OAuth (PKCE, loopback)
  jev/                          JevModel
  ollama/                       OllamaModel
  factory.rs                    EmailProviderFactory, StructuredModel selection from settings
src-tauri/src/
  main.rs, lib.rs               composition root, app state, startup recovery
  commands/                     one module per area (runs, results, accounts, catalog, settings)
  dto.rs                        serde DTOs (camelCase)
  events.rs                     run:progress, run:finished
src/                            React app
  lib/api/                      types.ts (DTO mirror), client.ts (invoke wrappers), events.ts
  components/                   Layout/Sidebar, Modal (<dialog>), ResultCard, badges, SortSelect
  features/                     dashboard, results, runs, accounts, catalog, settings
```

Dependency rule: `src-tauri → infra → core`. `core` has no I/O dependencies (pure crates such as `serde`, `chrono`, `regex`, `sha2`, `html2text`, `async-trait`, `thiserror`, `indexmap` are allowed).

## 4. Core contracts (`crates/core`)

### 4.1 Domain

```rust
pub type AccountId = i64;

pub enum ProviderConfig {
    Imap { host: String, port: u16, username: String },     // implicit TLS
    MicrosoftGraph { tenant: String },                       // default "organizations"
}

pub struct Account {
    pub id: AccountId, pub name: String, pub email_address: String,
    pub provider: ProviderConfig, pub folders: Vec<String>,  // IMAP default ["INBOX"], Graph default ["inbox"]
    pub default_university_id: Option<i64>, pub initial_sync_days: u32, pub enabled: bool,
    pub created_at: DateTime<Utc>, pub updated_at: DateTime<Utc>,
}

pub struct Address { pub name: Option<String>, pub email: String }
pub enum EmailBody { Text(String), Html(String) }

pub struct Email {
    pub account_id: AccountId, pub folder: String,
    pub provider_message_id: String,          // IMAP: UID as string · Graph: message id
    pub message_id_header: Option<String>,
    pub sender: Address, pub recipients: Vec<Address>,
    pub subject: String, pub received_at: DateTime<Utc>,
    pub body: EmailBody, pub source_url: Option<String>,
}

pub enum SyncCursor {
    Imap { uidvalidity: u32, last_uid: u32 },
    Graph { link: String },                   // @odata.nextLink or @odata.deltaLink
}
pub struct FetchBatch { pub emails: Vec<Email>, pub next_cursor: SyncCursor }

pub enum Category { Notice, Task, Exam, Event, Change, GeneralInfo }
pub enum Priority { Low, Medium, High, Urgent }
pub enum DateKind { Exam, Deadline, Event, Class, Other }
pub struct ResultDate { pub date: NaiveDate, pub time: Option<NaiveTime>, pub kind: DateKind }

pub struct AIAnalysis {
    pub university_id: Option<i64>, pub course_id: Option<i64>,
    pub category: Category, pub priority: Priority,
    pub action_required: bool, pub possible_change: bool,
    pub dates: Vec<ResultDate>, pub confidence: Option<f32>,
    pub model: ModelDescriptor,
}
pub enum AnalysisOutcome { Irrelevant { p_relevant: f32 }, Relevant(AIAnalysis) }
```

Serialized enum values (DB and DTOs) are snake_case: `notice`, `task`, `exam`, `event`, `change`, `general_info`; `low`…`urgent`; `exam`, `deadline`, `event`, `class`, `other`.

### 4.2 Ports

```rust
#[async_trait]
pub trait EmailProvider: Send + Sync {
    async fn test_connection(&self) -> Result<(), ProviderError>;
    /// Returns at most `MAX_BATCH` (200) emails, oldest first, and the cursor
    /// that resumes right after them. `since` only applies when `cursor` is None.
    async fn fetch_new(&self, folder: &str, cursor: Option<&SyncCursor>, since: DateTime<Utc>)
        -> Result<FetchBatch, ProviderError>;
}
pub trait EmailProviderFactory: Send + Sync {
    fn create(&self, account: &Account) -> Result<Box<dyn EmailProvider>, ProviderError>;
}
#[derive(thiserror::Error)]
pub enum ProviderError { Auth(String), Connection(String), Protocol(String) }

#[async_trait]
pub trait StructuredModel: Send + Sync {
    fn descriptor(&self) -> ModelDescriptor;          // { provider: "jev"|"ollama", model: String }
    async fn ask(&self, state: &serde_json::Value, questions: &QuestionSet)
        -> Result<AnswerSet, ModelError>;
}
pub type QuestionSet = IndexMap<String, Question>;   // question id → question
pub type AnswerSet = HashMap<String, Answer>;
pub enum Question {
    Noul { instructions: String },
    Choice { instructions: String, options: IndexMap<String, String> },  // key → description, 2..=255
    Score { instructions: String, levels: Vec<String> },                 // 2..=10, ordered low→high
}
pub enum Answer {
    Noul { p_yes: f32, confidence: Option<f32> },
    Choice { choice: String, confidence: Option<f32> },
    Score { level: u8, confidence: Option<f32> },     // index into `levels`
}
#[derive(thiserror::Error)]
pub enum ModelError {
    Auth,                      // bad/missing key
    RateLimited,               // after retries
    Unavailable(String),       // network, 5xx, 529, Ollama down
    InvalidResponse(String),   // unparseable, missing/extra answer, wrong type, unknown option
}

pub trait SecretStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError>;
    fn set(&self, key: &str, value: &str) -> Result<(), SecretError>;
    fn delete(&self, key: &str) -> Result<(), SecretError>;   // idempotent
}
```

Secret keys: `academic-assistant/account/{id}/password` (IMAP), `academic-assistant/account/{id}/refresh-token` (Graph), `academic-assistant/jev-api-key`.

Repository traits are synchronous (SQLite is local; one connection behind a mutex in WAL mode). One trait per aggregate: `AccountRepository`, `CatalogRepository`, `SyncStateRepository`, `ProcessedMessageRepository`, `ResultRepository`, `RunRepository`, `SettingsRepository`. `ResultRepository::save_analyzed(processed_message, analysis, email_meta)` persists the processed message, the result and its dates in one transaction.

## 5. Pipeline (`core::pipeline::CheckRunner`)

`CheckRunner::start(trigger) -> Result<RunId, RunError>`:
1. Preconditions (return `RunError`, no run created): another run in progress → `AlreadyRunning`; no enabled accounts → `NoEnabledAccounts`; AI provider not usable (jev selected without key) → `AiNotConfigured`.
2. Create `runs` row (`running`) and spawn the run; return its id.
3. For each enabled account (sequential), for each folder:
   1. `factory.create(account)`; errors → account `failed` with the mapped `error_kind`; continue with next account.
   2. `fetch_new(folder, cursor, now − initial_sync_days)`.
   3. For each email (sequential): normalize → dedup → filters → analyze → persist (see 5.1).
   4. When the whole batch is persisted: save `next_cursor`, `last_success_at`, status `ok`.
   5. If the account/folder stops early (transient error), the cursor is **not** advanced; `sync_states.status = error`, `last_error` set.
4. Emit `run:progress` after each stage change and every 10 messages.
5. Finish: `success` (all accounts succeeded), `partial` (some failed), `failed` (all failed); set counters and `error_summary`; emit `run:finished`.

Startup recovery: any run still `running` when the app starts is set to `failed` with `error_summary = "Interrupted"`.

### 5.1 Per-message processing
| Step | Result |
|---|---|
| `(account, folder, provider_message_id)` already in `processed_messages` | skip silently (not counted as found) |
| same `message_id_header` or `content_hash` already processed for the account | outcome `filtered`, reason `duplicate` |
| filter match | outcome `filtered`, reason `blocked_sender` \| `blocked_domain` \| `own_message` \| `empty` |
| analyzer → `Irrelevant` | outcome `irrelevant` |
| analyzer → `Relevant` | outcome `analyzed` + `ai_results` + `result_dates` (one transaction) |
| `ModelError::InvalidResponse` | outcome `failed`, reason = sanitized message; continue |
| `ModelError::{Auth, RateLimited, Unavailable}` | stop this account (no cursor advance), `error_kind = ai` |

Counters per account and run: `found`, `filtered`, `analyzed` (sent to AI), `failed`, `results_created`.

### 5.2 Normalization (`core::normalize`)
- HTML → text (`html2text`), whitespace collapsed.
- Remove quoted replies: lines starting with `>`, and everything after `-----Original Message-----`, `On … wrote:`, `El … escribió:`, `De: … Enviado:` blocks.
- Remove signature after a line equal to `-- `.
- Truncate to 12,000 characters.
- `content_hash` = SHA-256 hex of `lowercase(sender.email) + "\n" + trim(subject) + "\n" + normalized_body`.

### 5.3 Filters (`core::filters`)
`FilterConfig { blocked: Vec<String>, own_address: String }`. Entries starting with `@` block a domain; others block an exact address; case-insensitive. Also discards mail whose sender is the account address and mail with empty subject and empty normalized body. Pure function: `fn apply(email, normalized_body, cfg) -> FilterDecision { Keep, Discard(FilterReason) }`.

## 6. AI analysis (`core::analyzer::EmailAnalyzer`)

`EmailAnalyzer::analyze(email, normalized_body, account, catalog, model) -> Result<AnalysisOutcome, ModelError>`: builds one `QuestionSet`, calls `model.ask` once, validates and assembles.

### 6.1 State

```json
{ "email": { "subject": "...", "from": { "name": "...", "email": "..." },
             "received_at": "2026-09-28T09:14:00+02:00", "body": "<normalized body>" },
  "context": { "account_university": "Universidad de Alcalá" } }
```
`context.account_university` is omitted when the account has no default university.

### 6.2 Question set

| Id | Type | Instructions / options |
|---|---|---|
| `relevant` | Noul | "This email is an academic communication relevant to a university student: courses, classes, exams, assignments, grades, schedules, tutoring or academic administrative procedures." |
| `category` | Choice | `notice`: general announcement or reminder for students · `task`: assignment, submission, exercise or practical work to complete · `exam`: exam, test or assessment information · `event`: talk, seminar, session or activity at a specific time · `change`: modifies or cancels previously communicated information · `general_info`: administrative or informational content with no specific action |
| `action_required` | Noul | "The email asks the student to do something: submit, register, reply, attend, prepare, pay or bring something." |
| `possible_change` | Noul | "The email changes information communicated before: a new date or time, a new room, a cancellation or new instructions." |
| `priority` | Score | levels: `low` "Informational; no action or dates are far away" · `medium` "Action or relevant date within the next weeks" · `high` "Action, exam or deadline within the next few days" · `urgent` "Immediate action needed or something happens within 48 hours" |
| `university` | Choice | only when the account has no default university and the catalog has ≥1 university. Options `u{id}` → university name, plus `unknown` → "None of these universities". |
| `course` | Choice | only when candidate courses exist (the default university's courses, or all courses when there is none). Options `c{id}` → `"{name} ({university})" + " — also known as: {aliases}"` when aliases exist, plus `none` → "The email is not about any of these courses". Capped at 254 courses. |
| `date_{i}` | Choice | one per date candidate (max 20): "In this email, what does the date written as «{span}» refer to?" Options: `exam`, `deadline`, `event`, `class`, `other`, `not_relevant` (with short descriptions). |

### 6.3 Date candidates (`core::dates`)
Pure function `extract(body, subject, received_at_local) -> Vec<DateCandidate { span, date, time }>`:
- Absolute: `dd/mm`, `dd/mm/yy(yy)`, `dd-mm-yyyy`, `yyyy-mm-dd`, `d de <mes>( de yyyy)`, optional leading weekday (`lunes 27 de octubre`), English `<Month> d(, yyyy)` and `d <Month>( yyyy)`.
- Relative (Spanish and English): `hoy`/`today`, `mañana`/`tomorrow` (not when preceded by `la`/`por la`/`de la`), `pasado mañana`, `(el) próximo|próxima <weekday>`/`next <weekday>`.
- Time: attached when within 20 characters after the date: `10:00`, `10.30`, `10h`, `10:00h`, `a las 10`.
- Missing year: the year that places the date closest to `received_at`, never earlier than 30 days before it. Relative dates resolve against `received_at` in the local time zone.
- Invalid dates are dropped; duplicates (same date and time) keep the first span; at most 20, in order of appearance.

The model only classifies candidates; it never computes dates.

### 6.4 Assembly and validation
- Every asked question must have an answer of the matching type with a valid option/level; otherwise `ModelError::InvalidResponse`.
- `relevant.p_yes < 0.4` → `Irrelevant`. Threshold constant `RELEVANCE_THRESHOLD = 0.4` (conservative).
- `action_required`, `possible_change`: `p_yes ≥ 0.5`.
- `university_id`: account default → else `university` choice (`unknown` → None) → else the chosen course's university.
- `course_id`: `course` choice (`none` → None).
- Dates: candidates whose answer is `not_relevant` are dropped.
- `confidence`: minimum of all present answer confidences; `None` when none are present.
- Professor is the sender (`sender_name`, `sender_email`), not an AI field.

## 7. AI providers (`crates/infra`)

### 7.1 JevModel
- `POST https://api.typesafe.ai/v1/systemone`, header `Authorization: Bearer <key>` (key from `SecretStore`), JSON `{ "model": <setting, default "jev-latest">, "state": ..., "questions": { id: { "type", "instructions", "criteria" } } }`. Choice criteria = options object; Score criteria = levels array; Noul has no criteria.
- Response mapping: noul → `answers[id].noul` as `p_yes`; choice → `answers[id].choice`; score → level = argmax of `answers[id].probabilities` (keys are string indices); `confidence` when present.
- Errors: 401 → `Auth`; 422 → `InvalidResponse`; 429 → retry, then `RateLimited`; 529/5xx/network → retry, then `Unavailable`. 3 attempts, exponential backoff (1 s, 2 s, 4 s). Timeout 60 s.

### 7.2 OllamaModel
- `POST {base_url}/api/chat` (default `http://localhost:11434`), `stream: false`, `think: false`, `options.temperature: 0`, `format`: JSON schema object with one required property per question — Noul → `boolean`, Choice → string `enum` of option keys, Score → integer `enum` `[0..n-1]`.
- Messages: a system message ("You answer typed questions about the provided state. Reply only with JSON matching the schema.") and a user message containing the state and each question's instructions with option/level descriptions.
- Mapping: Noul `true` → `p_yes = 1.0`, `false` → `0.0`; confidence always `None`.
- Errors: connection refused/timeouts/5xx → `Unavailable`; unparseable or schema-violating content → `InvalidResponse`. Timeout 120 s.

### 7.3 Provider selection
Setting `ai.provider` ∈ {`ollama`, `jev`}; default `ollama` (local first). The composition root builds the selected `StructuredModel` at the start of each run.

## 8. Email providers (`crates/infra`)

### 8.1 ImapEmailProvider
- Implicit TLS (default port 993) with `async-imap` + rustls; `LOGIN` with the password from `SecretStore`.
- `SELECT` folder; if `UIDVALIDITY` differs from the cursor, treat as no cursor (dedup prevents duplicates).
- No cursor: `UID SEARCH SINCE <dd-Mon-yyyy>`. With cursor: `UID SEARCH UID {last_uid+1}:*`, discarding UIDs ≤ `last_uid` (the `*` quirk).
- Take the lowest 200 UIDs, `UID FETCH … (UID BODY.PEEK[])` (never sets `\Seen`), parse with `mail-parser`; prefer `text/plain`, fall back to HTML.
- `next_cursor` = `{uidvalidity, last_uid: highest fetched UID}`. When nothing is found: with a cursor, return it unchanged; without a cursor, return `{uidvalidity, last_uid: UIDNEXT − 1}`.
- `source_url = None`.
- Errors: login rejected → `Auth`; DNS/TCP/TLS → `Connection`; unexpected responses → `Protocol`.

### 8.2 MicrosoftGraphEmailProvider
- App registration (user, documented in README): Entra ID → App registrations → New; supported account types "Accounts in any organizational directory"; platform "Mobile and desktop applications" with redirect URI `http://localhost`; API permissions (delegated) `Mail.Read`, `User.Read`, `offline_access`; "Allow public client flows" enabled. No client secret.
- Sign-in (`connect_microsoft_account`): authorization code + PKCE against `https://login.microsoftonline.com/{tenant}/oauth2/v2.0/authorize`, loopback listener on `127.0.0.1:{random port}` with redirect `http://localhost:{port}`, `state` checked, system browser opened, 5-minute timeout. Tokens from `/token`; the refresh token is stored in `SecretStore`; the access token lives in memory only. After sign-in, `GET /me` must match the account's email address (`mail` or `userPrincipalName`, case-insensitive), otherwise the token is discarded and a validation error is returned.
- Token refresh on each run; a rotated refresh token replaces the stored one. `invalid_grant` → `ProviderError::Auth` (UI shows "Reconnect").
- Fetch: no cursor → `GET https://graph.microsoft.com/v1.0/me/mailFolders/{folder}/messages/delta?$select=id,internetMessageId,subject,from,toRecipients,ccRecipients,receivedDateTime,body,webLink&$filter=receivedDateTime ge {since}`, header `Prefer: outlook.body-content-type="text", odata.maxpagesize=50`. With cursor → `GET {link}`. Follow `@odata.nextLink` until 200 emails are collected or `@odata.deltaLink` appears; `next_cursor` = the last link obtained. Items with `@removed` are ignored. If the first (filtered) request returns 400, repeat it without `$filter` and discard messages with `receivedDateTime < since` client-side.
- `source_url = webLink`.
- Errors: 401/403 or token errors → `Auth`; network → `Connection`; other → `Protocol`. Admin-consent errors (`AADSTS65001`/`AADSTS90094`) surface as `Auth` with the message "Your organization requires admin approval for this app".

## 9. SQLite (`crates/infra/db`, migration `0001_init.sql`)

Timestamps are RFC 3339 UTC text; result dates are local `YYYY-MM-DD` / `HH:MM`. `PRAGMA foreign_keys = ON; journal_mode = WAL`. Database file `academic-assistant.db` in the Tauri app data directory. Migrations are embedded and applied at startup.

```sql
CREATE TABLE universities (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, created_at TEXT NOT NULL);
CREATE TABLE courses (
  id INTEGER PRIMARY KEY,
  university_id INTEGER NOT NULL REFERENCES universities(id) ON DELETE CASCADE,
  name TEXT NOT NULL, aliases TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL,
  UNIQUE (university_id, name));
CREATE TABLE accounts (
  id INTEGER PRIMARY KEY, name TEXT NOT NULL, email_address TEXT NOT NULL,
  provider_type TEXT NOT NULL CHECK (provider_type IN ('imap','microsoft_graph')),
  config TEXT NOT NULL, folders TEXT NOT NULL,
  default_university_id INTEGER REFERENCES universities(id) ON DELETE SET NULL,
  initial_sync_days INTEGER NOT NULL DEFAULT 14 CHECK (initial_sync_days BETWEEN 1 AND 365),
  enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE sync_states (
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  folder TEXT NOT NULL, cursor TEXT, last_run_at TEXT, last_success_at TEXT,
  status TEXT NOT NULL DEFAULT 'never' CHECK (status IN ('never','ok','error')),
  last_error TEXT, PRIMARY KEY (account_id, folder));
CREATE TABLE runs (
  id INTEGER PRIMARY KEY, trigger TEXT NOT NULL CHECK (trigger IN ('manual')),
  started_at TEXT NOT NULL, finished_at TEXT,
  status TEXT NOT NULL CHECK (status IN ('running','success','partial','failed')),
  accounts_processed INTEGER NOT NULL DEFAULT 0, messages_found INTEGER NOT NULL DEFAULT 0,
  messages_filtered INTEGER NOT NULL DEFAULT 0, messages_analyzed INTEGER NOT NULL DEFAULT 0,
  messages_failed INTEGER NOT NULL DEFAULT 0, results_created INTEGER NOT NULL DEFAULT 0,
  error_summary TEXT);
CREATE TABLE run_accounts (
  run_id INTEGER NOT NULL REFERENCES runs(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('success','failed')),
  messages_found INTEGER NOT NULL DEFAULT 0, messages_filtered INTEGER NOT NULL DEFAULT 0,
  messages_analyzed INTEGER NOT NULL DEFAULT 0, messages_failed INTEGER NOT NULL DEFAULT 0,
  results_created INTEGER NOT NULL DEFAULT 0,
  error_kind TEXT CHECK (error_kind IN ('auth','connection','protocol','ai','internal')),
  error_message TEXT, PRIMARY KEY (run_id, account_id));
CREATE TABLE processed_messages (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  folder TEXT NOT NULL, provider_message_id TEXT NOT NULL, message_id_header TEXT,
  content_hash TEXT NOT NULL, received_at TEXT NOT NULL,
  outcome TEXT NOT NULL CHECK (outcome IN ('filtered','irrelevant','analyzed','failed')),
  reason TEXT, run_id INTEGER NOT NULL REFERENCES runs(id), processed_at TEXT NOT NULL,
  UNIQUE (account_id, folder, provider_message_id));
CREATE INDEX idx_processed_msgid ON processed_messages (account_id, message_id_header);
CREATE INDEX idx_processed_hash ON processed_messages (account_id, content_hash);
CREATE TABLE ai_results (
  id INTEGER PRIMARY KEY,
  account_id INTEGER NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
  processed_message_id INTEGER NOT NULL UNIQUE REFERENCES processed_messages(id) ON DELETE CASCADE,
  run_id INTEGER NOT NULL REFERENCES runs(id),
  detected_at TEXT NOT NULL, received_at TEXT NOT NULL,
  subject TEXT NOT NULL, sender_name TEXT, sender_email TEXT NOT NULL,
  university_id INTEGER REFERENCES universities(id) ON DELETE SET NULL,
  course_id INTEGER REFERENCES courses(id) ON DELETE SET NULL,
  category TEXT NOT NULL CHECK (category IN ('notice','task','exam','event','change','general_info')),
  priority TEXT NOT NULL CHECK (priority IN ('low','medium','high','urgent')),
  action_required INTEGER NOT NULL, possible_change INTEGER NOT NULL, confidence REAL,
  model_provider TEXT NOT NULL, model_name TEXT NOT NULL, source_url TEXT,
  seen INTEGER NOT NULL DEFAULT 0, seen_at TEXT);
CREATE INDEX idx_results_seen ON ai_results (seen, received_at);
CREATE TABLE result_dates (
  id INTEGER PRIMARY KEY,
  result_id INTEGER NOT NULL REFERENCES ai_results(id) ON DELETE CASCADE,
  date TEXT NOT NULL, time TEXT,
  kind TEXT NOT NULL CHECK (kind IN ('exam','deadline','event','class','other')));
CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);
```

`accounts.config` JSON: IMAP `{"host","port","username"}`; Graph `{"tenant"}`. `folders` JSON array.

Settings keys (JSON values) and defaults: `ai.provider` = `"ollama"`, `jev.model` = `"jev-latest"`, `ollama.base_url` = `"http://localhost:11434"`, `ollama.model` = `"qwen3:8b"`, `microsoft.client_id` = `null`, `filters.blocked` = `[]`.

## 10. Tauri interface (`src-tauri`)

All commands return `Result<T, CommandError>`; `CommandError` serializes as `{ kind: "validation" | "not_found" | "conflict" | "auth" | "connection" | "ai" | "internal", message: string }`. Messages are user-facing, sanitized and never contain secrets or bodies. DTOs are `camelCase`; dates as ISO strings. `src/lib/api/types.ts` mirrors these types exactly.

### 10.1 DTOs (TypeScript notation)

```ts
type Category = 'notice'|'task'|'exam'|'event'|'change'|'general_info';
type Priority = 'low'|'medium'|'high'|'urgent';
type DateKind = 'exam'|'deadline'|'event'|'class'|'other';
type ResultSort = 'upcoming'|'priority'|'received';
type RunStatus = 'running'|'success'|'partial'|'failed';
type ErrorKind = 'auth'|'connection'|'protocol'|'ai'|'internal';

interface Counters { found: number; filtered: number; analyzed: number; failed: number; resultsCreated: number }
interface RunAccountDto { accountId: number; accountName: string; status: 'success'|'failed';
  counters: Counters; errorKind: ErrorKind|null; errorMessage: string|null }
interface RunDto { id: number; trigger: 'manual'; startedAt: string; finishedAt: string|null;
  status: RunStatus; accountsProcessed: number; counters: Counters; errorSummary: string|null;
  accounts: RunAccountDto[] }

interface ResultDateDto { date: string /*YYYY-MM-DD*/; time: string|null /*HH:MM*/; kind: DateKind }
interface ResultDto { id: number; accountId: number; accountName: string; subject: string;
  senderName: string|null; senderEmail: string; universityName: string|null; courseName: string|null;
  category: Category; priority: Priority; actionRequired: boolean; possibleChange: boolean;
  dates: ResultDateDto[]; receivedAt: string; detectedAt: string;
  sourceUrl: string|null; messageIdHeader: string|null; seen: boolean; seenAt: string|null }
interface ResultQuery { search?: string; seen?: boolean; category?: Category; accountId?: number;
  courseId?: number; sort: ResultSort; limit: number; offset: number }
interface Page<T> { items: T[]; total: number }

interface DashboardDto { lastRun: RunDto|null; runningRunId: number|null;
  attention: { accountId: number; accountName: string; message: string }[];
  counts: { newTotal: number; tasks: number; exams: number; events: number; changesAndNotices: number };
  newResults: ResultDto[] }

type ProviderDto = { type: 'imap'; host: string; port: number; username: string }
                 | { type: 'microsoft_graph'; tenant: string };
interface AccountDto { id: number; name: string; emailAddress: string; provider: ProviderDto;
  folders: string[]; defaultUniversityId: number|null; initialSyncDays: number; enabled: boolean;
  credentialsConfigured: boolean;
  lastSync: { at: string|null; status: 'never'|'ok'|'error'; error: string|null };
  createdAt: string; updatedAt: string }
interface AccountInput { name: string; emailAddress: string; folders: string[];
  defaultUniversityId: number|null; initialSyncDays: number; enabled: boolean;
  provider: { type: 'imap'; host: string; port: number; username: string; password?: string }
          | { type: 'microsoft_graph'; tenant: string } }
interface ConnectionTestDto { ok: boolean; errorKind: ErrorKind|null; message: string|null }

interface CourseDto { id: number; universityId: number; name: string; aliases: string[] }
interface UniversityDto { id: number; name: string; courses: CourseDto[] }

interface SettingsDto { aiProvider: 'ollama'|'jev'; jevModel: string; jevApiKeyConfigured: boolean;
  ollamaBaseUrl: string; ollamaModel: string; microsoftClientId: string|null; blockedSenders: string[] }
type SettingsInput = Omit<SettingsDto, 'jevApiKeyConfigured'>;
```

### 10.2 Commands

| Command | Input → Output | Notes |
|---|---|---|
| `check_now` | `() → number` (run id) | `RunError` → `conflict` (already running) / `validation` (no accounts, AI not configured) |
| `get_dashboard` | `{ sort: ResultSort } → DashboardDto` | `newResults` = all unseen, max 100; `attention` = enabled accounts whose last sync is `error` or that lack credentials |
| `get_results` | `ResultQuery → Page<ResultDto>` | search = case-insensitive substring over subject, sender name and email |
| `set_result_seen` | `{ id, seen: boolean } → ResultDto` | covers "mark as seen" and "mark as new" |
| `mark_all_seen` | `() → number` (rows updated) | |
| `get_run_history` | `{ limit: number } → RunDto[]` | newest first |
| `get_accounts` | `() → AccountDto[]` | |
| `add_account` | `AccountInput → AccountDto` | IMAP password required on create; stored in `SecretStore` |
| `update_account` | `{ id, input: AccountInput } → AccountDto` | IMAP password replaced only when provided; provider type cannot change (`validation`) |
| `remove_account` | `{ id } → ()` | deletes secrets and all account data (results included); UI confirms first |
| `test_account` | `{ id } → ConnectionTestDto` | |
| `connect_microsoft_account` | `{ id } → AccountDto` | requires `microsoft.client_id`; see §8.2 |
| `get_catalog` | `() → UniversityDto[]` | |
| `add_university` / `update_university` / `remove_university` | `{ name }` / `{ id, name }` / `{ id }` | duplicate name → `conflict` |
| `add_course` / `update_course` / `remove_course` | `{ universityId, name, aliases }` / `{ id, name, aliases }` / `{ id }` | |
| `get_settings` / `update_settings` | `() → SettingsDto` / `SettingsInput → SettingsDto` | URLs and model names validated |
| `set_jev_api_key` / `clear_jev_api_key` | `{ key } → ()` / `() → ()` | key never returned |
| `test_ai_provider` | `() → ConnectionTestDto` | asks one fixed Noul question |

Sort semantics: `upcoming` = results with a date ≥ today ordered by their nearest such date (ascending), then the rest by `receivedAt` descending; `priority` = urgent → low, ties by `receivedAt` descending; `received` = `receivedAt` descending.

Dashboard counts consider unseen results only: `tasks` = category `task`, `exams` = `exam`, `events` = `event`, `changesAndNotices` = `change` + `notice`.

### 10.3 Events
- `run:progress` `{ runId, accountId, accountName, stage: 'connecting'|'fetching'|'analyzing'|'done'|'failed', processed: number, total: number }`
- `run:finished` `{ runId, status: RunStatus }`

## 11. User interface (`src/`)

Stack: Vite, React, TypeScript (strict), Tailwind CSS v4, `react-router` (hash router), `@tanstack/react-query` for command data, `@tauri-apps/api`. Modals use the native `<dialog>` element. No component library. UI copy in English, sentence case.

Layout: left sidebar (Dashboard, Results with unseen badge, Runs, Accounts, Catalog, Settings) and a content area.

- **Dashboard**: header with last check (time, status, e.g. "partial: 1 of 2 accounts failed") and "Check now" (shows per-account progress from `run:progress`; disabled state replaced by a progress label while running); attention banner per account needing action; five counters (new, tasks, exams, events, changes and notices); "New results" list with a sort selector (nearest upcoming date — default —, priority, received date) and "Mark all as seen". Each result card: subject, priority badge, sender · course (university) · account · received date, category, dates with kind, "Action required"/"Possible change" tags, "Open email" (when `sourceUrl`) or "Copy Message-ID", "Seen".
- **Results**: search box plus a "Filters and sort" button (badge with the number of active filters) opening a modal with state, category, account, course and sort. Paged table (subject + sender/course/received, category, next date, priority; unseen dot). Clicking a row expands details (sender with address, account, received and detected dates, all dates, flags) with "Open email"/"Copy Message-ID" and "Mark as seen"/"Mark as new".
- **Runs**: list of runs (status, start, duration, trigger, counters), expandable per-account detail with error kind and message.
- **Accounts**: list (name, provider, email, default university, folders, last sync status), actions Test, Edit, Reconnect (Graph), enable toggle, Delete (confirmation: "Deletes this account and all its results. Disable it instead to keep its history."). Add/edit modal with an IMAP / Microsoft 365 switch. IMAP fields: display name, email, host, port (993), username, password, folders, default university, first-sync days. Microsoft fields: display name, email, folders, default university, first-sync days, then "Sign in with Microsoft". Note under secrets: "Saved in Windows Credential Manager, never in the database."
- **Catalog**: universities with their courses (name, aliases), add/edit/delete for both.
- **Settings**: AI provider selector (jev — "sends email text to TypeSafe"; Ollama — "stays on this PC"), jev API key (write-only: "Configured"/"Not configured", Replace key, Remove key), jev model, Ollama URL and model, "Test provider"; Microsoft Entra client ID; blocked senders and domains (one per line).

Errors from commands are shown inline next to the triggering control using `CommandError.message`.

## 12. Security and logging
- Secrets only in Windows Credential Manager. Never in SQLite, logs, errors, DTOs or the repository.
- `tracing` + rolling daily file appender in `{app_data}/logs`, level `info`; logs contain ids, counters, stages and sanitized errors; never bodies, subjects, tokens, passwords or keys.
- Providers are read-only (IMAP `BODY.PEEK`, Graph `Mail.Read`).
- The Tauri capability file allows only the app's own commands, the event API and opening URLs in the system browser (`opener` plugin, `https://` only).

## 13. Testing and validation
- `core` unit tests with fakes: normalization, filters, date extraction (Spanish/English formats, relative dates, year resolution, `mañana` morning exclusion), question building (conditional `university`/`course`/`date_*`), answer assembly and validation, `CheckRunner` (dedup, per-account isolation, cursor not advanced on transient AI errors, `failed` outcome on invalid responses, run status aggregation, single-run lock).
- `infra` integration tests: migrations and repositories on a temp database (including sort semantics and cascade deletes); IMAP parsing from `.eml` fixtures; Graph, jev and Ollama against `wiremock`; `#[ignore]` tests against real jev/Ollama (enabled via env vars).
- Fixtures: fictional Spanish emails in UV/UAH style (exam date change, assignment deadline, event, newsletter, administrative notice, reply thread).
- Frontend: `tsc --noEmit`, ESLint, Vitest + Testing Library for the API client, sort/filter modal state and result card rendering; `npm run build`.
- Rust gates: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- Manual acceptance (end of phase): add the IMAP and UAH accounts, test both, run "Check now" with Ollama and with jev, verify results, seen/new, filters and runs.

## 14. Git workflow
`main` ← `phase-1-email-analysis` ← feature branches (one per plan feature, created from and merged back into the phase branch). The phase branch merges into `main` only after the user validates Phase 1.

## 15. Risks
- **UAH tenant may require admin consent** for a third-party app. Mitigation: clear error message; fallback options are requesting approval from UAH IT or forwarding UAH mail to the IMAP account.
- **jev and Spanish**: the model's documentation notes non-English text is handled less well. Mitigation: English instructions, Spanish fixtures, manual evaluation with both providers.
- **Ollama quality/speed** depends on the local model and hardware. Mitigation: configurable model, 120 s timeout, per-message `failed` outcome.
- **Graph delta `$filter`** support on `receivedDateTime` may vary. Mitigation built into §8.2: on a 400 response to the filtered first request, retry without `$filter` and discard messages older than `since` client-side.
