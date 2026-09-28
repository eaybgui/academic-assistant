# Academic Assistant — Technical specification

This document describes the overall architecture. The detailed design of each phase lives in `docs/superpowers/specs/` (Phase 1: `2026-09-28-phase-1-email-analysis-design.md`).

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
│   ├── Dashboard
│   ├── Results / Runs
│   └── Accounts / Catalog / Settings
│
└── Rust (Cargo workspace)
    ├── crates/core      domain, ports (traits), filters, date candidates,
    │                    EmailAnalyzer, CheckRunner pipeline — no I/O
    ├── crates/infra     SQLite, IMAP, Microsoft Graph, jev, Ollama,
    │                    Windows Credential Manager — implements core ports
    └── src-tauri        composition root + thin Tauri commands/events
```

React never accesses email, SQLite or external providers directly. It communicates with Rust through Tauri commands and events.

Dependency rule: `src-tauri → infra → core`. `core` never depends on `infra` or `src-tauri`; the compiler enforces it through crate boundaries.

## 3. Layers

### Presentation
React/TypeScript: presentation, forms, configuration, read state and command invocation. No business logic.

### Application/Core (`crates/core`)
Domain model, ports, orchestration (pipeline), filters, date candidate extraction and AI question design. Pure Rust, testable with fakes.

### Infrastructure (`crates/infra`)
Concrete implementations: SQLite repositories and migrations, IMAP, Microsoft Graph, jev, Ollama, secure storage.

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
├── ImapEmailProvider            (Phase 1)
└── MicrosoftGraphEmailProvider  (Phase 1)
```

Each provider converts its protocol into the common `Email` model. Providers are read-only: IMAP uses `BODY.PEEK[]`; Graph uses the `Mail.Read` scope.

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
EmailAnalyzer → StructuredModel
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

`EmailAnalyzer` (in `core`) is provider-agnostic: it builds the question set for an email, calls `StructuredModel::ask` once, and assembles and validates the result in code. Code, not the model, generates candidates (e.g. dates found by regex) and performs all normalization and date arithmetic.

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

- `universities`, `courses` — catalog used by the AI (future scraping will populate it).
- `accounts` — account metadata and non-secret configuration.
- `sync_states` — cursor and status per account and folder.
- `processed_messages` — deduplication and processing outcome per message.
- `ai_results`, `result_dates` — structured results and their dates.
- `runs`, `run_accounts` — run log with per-account detail.
- `settings` — key/value application settings.

The concrete Phase 1 schema is defined in the Phase 1 design document.

## 14. Retention

The full body of each email is never stored. The application keeps AI results, subject and sender, deduplication metadata, synchronization state, run history and (later) relations between results.

## 15. Credentials

- OAuth2 when available (Microsoft 365: authorization code + PKCE with a loopback redirect).
- Passwords, refresh tokens and API keys in Windows Credential Manager (`keyring` crate).
- SQLite never stores secrets; credential entries are derived from ids (e.g. `academic-assistant/account/{id}`).
- Logs and error messages never include credentials, tokens, API keys or email bodies.

## 16. Scheduler (later phase)

Rust will manage manual runs, 1–2 daily runs, configurable times, background execution and reasonable recovery from errors. The Windows solution may use a tray process with autostart or a headless binary reusing the same crates, without depending on React being open.

## 17. Notifications (later phase)

```text
NotificationService
├── notify_check_completed()
├── notify_new_results()
└── notify_error()
```

The implementation will use native Windows notifications.

## 18. React ↔ Rust

Commands and events are defined in the phase design documents. Critical logic stays in Rust. DTOs use `camelCase` and are mirrored manually in `src/lib/api/types.ts`.

## 19. Configuration

Persisted: accounts, enabled state, AI provider and its configuration, Microsoft client ID, blocked senders/domains, first-sync window. Later: schedules, notifications and future preferences.

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

Phase 1 implements email sources only. When new sources arrive (web scraping, Moodle, Blackboard, calendars), a new port will be added to `core` alongside `EmailProvider`, with its implementations in `infra`, reusing the analysis, storage and presentation layers. The exact abstraction will be designed with the first non-email source rather than anticipated now.

```text
Future sources
├── Email (IMAP, Microsoft Graph)
├── Course web pages (scraping → course catalog)
├── UVMoodleSource
├── UAHBlackboardSource
└── CalendarSource
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
- Microsoft Graph, jev and Ollama against HTTP mocks (`wiremock`);
- real providers in `#[ignore]` tests, run manually.

All providers can be replaced with fakes.
