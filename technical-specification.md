# Academic Assistant — Technical specification

This document describes the overall architecture. The detailed design of each phase lives in `docs/superpowers/specs/` (Phase 1: `2026-09-28-phase-1-email-analysis-design.md`; Phase 2: `2026-10-04-phase-2-lms-tracker-design.md`, whose section 19 "As built" records where the implementation differs from the design).

## 1. Stack
- Tauri 2.
- Rust (stable, MSVC toolchain on Windows).
- React + TypeScript (Vite).
- Tailwind CSS v4.
- SQLite (`rusqlite`, bundled).
- npm as the frontend package manager.

The application is local-first, modular and extensible.

## 2. Architecture

```text
Tauri Desktop
├── React + TypeScript + Tailwind        (src/)
│   ├── Dashboard (LMS tracker)
│   ├── Universities
│   ├── Announcements (email) / Email runs
│   └── Email accounts / Settings
│
└── Rust (Cargo workspace)
    ├── crates/core      domain, ports (traits), filters, date candidates,
    │                    analyzer, CheckRunner pipeline, LMS diff and
    │                    LmsSyncRunner — no I/O
    ├── crates/infra     SQLite, IMAP, Microsoft Graph, jev, Ollama,
    │                    Moodle and Blackboard clients,
    │                    Windows Credential Manager — implements core ports
    └── src-tauri        composition root + thin Tauri commands/events +
                         LMS sign-in windows
```

React never accesses email, learning platforms, SQLite or external providers directly. It communicates with Rust through Tauri commands and events.

Dependency rule: `src-tauri → infra → core`. `core` never depends on `infra` or `src-tauri`; the compiler enforces it through crate boundaries.

## 3. Layers

### Presentation
React/TypeScript: presentation, forms, configuration, read state and command invocation. No business logic.

### Application/Core (`crates/core`)
Domain model, ports, orchestration (pipeline), filters, date candidate extraction and AI question design. Pure Rust, testable with fakes.

### Infrastructure (`crates/infra`)
Concrete implementations: SQLite repositories and migrations, IMAP, Microsoft Graph, Moodle and Blackboard clients, jev, Ollama, secure storage.

Upper layers depend on abstractions, never on concrete implementations.

## 4. Email providers

```rust
#[async_trait]
trait EmailProvider: Send + Sync {
    async fn test_connection(&self) -> Result<(), ProviderError>;
    async fn fetch_new(&self, folder: &str, cursor: Option<&SyncCursor>, since: DateTime<Utc>)
        -> Result<FetchBatch, ProviderError>;
}
```

Implementations:

```text
EmailProvider
├── ImapEmailProvider
└── MicrosoftGraphEmailProvider
```

Each provider converts its protocol into the common `Email` model. Providers are read-only: IMAP uses `BODY.PEEK[]`; Graph uses the `Mail.Read` scope.

## 4b. Learning platforms (Phase 2)

```rust
#[async_trait]
trait LmsClient: Send + Sync {
    async fn verify(&self) -> Result<LmsIdentity, LmsError>;
    async fn list_courses(&self) -> Result<Vec<RemoteCourse>, LmsError>;
    async fn fetch_items(&self, courses: &[RemoteCourseRef], window: &SyncWindow)
        -> Result<Vec<RemoteItem>, LmsError>;   // all or nothing
}
```

```text
LmsClient
├── MoodleClient       Moodle mobile web services, with the user's mobile token
└── BlackboardClient   Blackboard Learn public REST API, with the user's session cookies
```

Both are read-only (Moodle read functions, Blackboard `GET`) and run only when the user presses Sync. A `LmsClientFactory` builds a client from the credential stored in the secret store; an `LmsAuthenticator` renews a credential silently. The app opens the university's own sign-in page in an embedded window and captures only a scoped credential (a Moodle token obtained through the mobile launch redirect, or the Blackboard cookies of the platform host); the university password is never seen. `LmsSyncRunner` (core) syncs universities one after another, isolates failures per university and diffs what the platform returned against the stored items (new, due date changed, removed). Details: `docs/infra/lms.md`.

## 5. Normalized model

```text
Email
├── account_id
├── folder
├── provider_message_id
├── message_id_header
├── sender
├── recipients
├── subject
├── received_at
├── body_text      (HTML converted to text, quoted replies/signatures removed)
└── source_url     (Graph webLink; none for IMAP)
```

The full message only exists in memory during processing.

## 6. Incremental synchronization

Each account and folder has independent state:

```text
SyncState
├── account_id
├── folder
├── cursor          IMAP: {uidvalidity, last_uid} · Graph: {delta_link}
├── last_run_at
├── last_success_at
├── status
└── last_error
```

Synchronization uses the provider cursor and, as a safety net, Message-ID and a content hash. It never relies on dates alone. The first sync (no cursor) fetches the last N days configured per account.

The cursor only advances once every message of the fetched batch has been persisted in a terminal state. If processing stops midway, the next run fetches the same batch again and deduplication skips what was already processed.

## 7. Pipeline

```text
Check now (Phase 1) / Scheduler (later)
        ↓
Enabled accounts
        ↓
Email sync (per account and folder)
        ↓
Normalization
        ↓
Deduplication + deterministic filters
        ↓
New candidates
        ↓
core::analyzer → StructuredModel
        ↓
Validated structured result
        ↓
SQLite (transaction per message)
        ↓
UI (events) + notifications (later)
```

Every stage records errors and metrics. Errors are isolated per account.

## 8. Filters
Before AI, the following can be discarded:
- already processed messages;
- duplicates;
- non-relevant folders (only configured folders are fetched);
- blocked senders/domains;
- messages sent by the account itself;
- empty messages (no subject and no body);
- future user rules.

The default strategy is conservative to avoid false negatives. No promotion heuristics in Phase 1.

## 9. AI: structured model abstraction

The AI providers used only answer typed questions; they do not generate free text. The abstraction reflects that:

```rust
#[async_trait]
trait StructuredModel: Send + Sync {
    fn descriptor(&self) -> ModelDescriptor;   // provider + model name
    async fn ask(&self, state: &serde_json::Value, questions: &QuestionSet)
        -> Result<AnswerSet, ModelError>;
}
```

Question types (the jev primitives):
- **Noul** — yes/no, answered with the probability of "yes".
- **Choice** — one option from a closed set (max 255 options).
- **Score** — one level from an ordered set (2–10 levels).

Implementations:

```text
StructuredModel
├── JevModel     external, TypeSafe API (POST https://api.typesafe.ai/v1/systemone)
└── OllamaModel  local, emulates the primitives with JSON-schema constrained output
```

`core::analyzer` is provider-agnostic: it builds the question set for an email, calls `StructuredModel::ask` once, and assembles and validates the result in code. Code, not the model, generates candidates (e.g. dates found by regex) and performs all normalization and date arithmetic.

## 10. AI result

```text
AIAnalysis
├── relevant            (Noul probability ≥ threshold)
├── university
├── course
├── professor           (the sender)
├── category            notice | task | exam | event | change | general_info
├── action_required
├── priority            low | medium | high | urgent
├── dates[]             {date, time?, kind: exam | deadline | event | class | other}
├── possible_change
└── confidence          min of answer confidences (null when the provider has none)
```

Fields may evolve during development.

## 11. Change detection

Results can be related semantically. Example:

```text
Result A: exam 20/06
        ↓
Result B: exam moved to 27/06
        ↓
Change: 20/06 → 27/06
```

Phase 1 only records the `possible_change` flag. Relating results is a later phase and will start with heuristics over structured data before more complex mechanisms.

## 12. SQLite

SQLite is the local source of truth: single-user desktop app, no server, single-file persistence, transactions, queries and search, easy backups, natural Rust integration.

The schema is managed through versioned migrations embedded in the binary.

## 13. Data model (overview)

- `universities`, `courses` — the learning platforms the user follows and the courses discovered on them (Phase 2; the Phase 1 manual catalog is gone). The tracked, available courses are also the catalog the email analyzer uses.
- `lms_items` — assignments, quizzes, tests and course events, with the user's local `done`/`hidden`/`note` fields.
- `accounts` — account metadata and non-secret configuration.
- `sync_states` — cursor and status per account and folder.
- `processed_messages` — deduplication and processing outcome per message.
- `ai_results`, `result_dates` — structured results and their dates.
- `runs`, `run_accounts` — run log with per-account detail.
- `settings` — key/value application settings.

The concrete schema is defined by the migrations in `crates/infra/migrations/` (`0001_init.sql` Phase 1, `0002_lms.sql` and `0003_lms_items.sql` Phase 2); see `docs/infra/database.md`.

## 14. Retention

The full body of each email is never stored. The application keeps AI results, subject and sender, deduplication metadata, synchronization state, run history and (later) relations between results. For learning platforms it keeps courses and items (title, due date, status, grade, link) so that changes can be detected between syncs; removed items are kept and flagged.

## 15. Credentials

- OAuth2 when available (Microsoft 365: authorization code + PKCE with a loopback redirect).
- Passwords, refresh tokens, API keys and learning-platform credentials (Moodle token, Blackboard session cookies) in Windows Credential Manager (`keyring` crate). The Credential Manager target of a key is `{key}.academic-assistant`.
- SQLite never stores secrets; credential entries are derived from ids (e.g. `academic-assistant/account/{id}/password`, `academic-assistant/university/{id}/moodle-token`).
- Learning-platform credentials are scoped (a mobile web-service token, or the cookies of the platform host) and verified against the platform before they are stored. The university password is never captured.
- The sign-in window has no IPC access to the app. The URLs it visits are never logged.
- Logs and error messages never include credentials, tokens, cookies, API keys, email bodies, item titles or notes.

## 16. Scheduler (later phase)

Rust will manage manual runs, 1–2 daily runs, configurable times, background execution and reasonable recovery from errors. The Windows solution may use a tray process with autostart or a headless binary reusing the same crates, without depending on React being open.

Phases 1 and 2 are manual: Check now (email) and Sync (learning platforms).

## 17. Notifications (later phase)

```text
NotificationService
├── notify_check_completed()
├── notify_new_results()
└── notify_error()
```

The implementation will use native Windows notifications.

## 18. React ↔ Rust

Commands and events are defined in the phase design documents and listed in `docs/app/tauri-commands.md` (including the `run:*` and `lms-sync:*` events). Critical logic stays in Rust. DTOs use `camelCase` and are mirrored manually in `src/lib/api/types.ts`.

## 19. Configuration

Persisted: accounts, enabled state, AI provider and its configuration, Microsoft client ID, blocked senders/domains, first-sync window, universities (platform, URL, login state) and the tracked courses. Later: schedules, notifications and future preferences.

## 20. Errors and observability

Errors are isolated per account/component whenever possible.

```text
Run
├── UV → OK
├── UAH → authentication error
└── AI → OK
```

Duration, accounts, found/filtered/analyzed messages, results and errors are recorded. Application logs use `tracing` with a rotating file in the app data directory.

## 21. Source extensibility

Phase 1 implemented email sources. Phase 2 added the first non-email sources as a separate port, `LmsClient`, alongside `EmailProvider`, with its implementations in `infra` and its own sync runner and tables, rather than forcing platform items into the email `Email` type. Future sources follow the same pattern: a new port in `core`, adapters in `infra`, reusing the storage and presentation layers.

```text
Sources
├── Email (IMAP, Microsoft Graph)            Phase 1
├── Learning platforms (Moodle, Blackboard)  Phase 2
├── Google Calendar export                   planned
└── Course web pages (scraping)              later
```

## 22. Strategy for future platforms

Order of preference:
1. Official API.
2. Standard protocol.
3. Stable HTTP scraping.
4. Browser automation only when necessary.

No autonomous agent is used if a stable interface can solve the problem.

## 23. Agents

The core is deterministic:

```text
fetch → filter → analyze → structure → store
```

AI is used for classification and extraction. Browser agents remain a possible future tool for especially difficult sources.

## 24. Testing

### Unit tests (`core`)
- filters;
- deduplication;
- date candidate extraction and resolution;
- question building and answer assembly;
- pipeline with fakes (including the cursor rule).

### Integration tests (`infra`)
- SQLite migrations and repositories;
- IMAP parsing from `.eml` fixtures;
- Microsoft Graph, jev, Ollama, Moodle and Blackboard against HTTP mocks (`wiremock`) with invented data;
- real providers in `#[ignore]` tests, run manually (`lms_live` for the learning platforms).

### Unit tests (`core`, LMS)
- URL normalization, session check and renewal, effective status, item diff and `LmsSyncRunner` with fakes.

All providers can be replaced with fakes.
