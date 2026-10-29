# Documentation index

Project-level documents live directly in `docs/`; per-component reference documents live in
the subfolders. Root-level [requirements.md](../requirements.md) and
[technical-specification.md](../technical-specification.md) describe what the product must do
and the overall technical direction.

## Guides

| File | Description |
|---|---|
| [developer-guide.html](developer-guide.html) | Functional guide for new developers (open in a browser): what each part does, how a check runs, how the AI analysis works, the database and a hands-on tutorial. **developer-guide.html describes Phase 1 only** (email analysis); it does not cover the LMS tracker |
| [setup.md](setup.md) | Prerequisites, Microsoft Entra app registration, running in dev, building, where data lives |
| [user-guide.md](user-guide.md) | Every screen and action, how a sync and an email check work, privacy notes, troubleshooting |
| [architecture.md](architecture.md) | Crates and dependency rule, data flow of a check, data model, security model, extension points |
| [development.md](development.md) | Repo layout, conventions, quality gates, branch workflow, how to add a migration/command/screen/provider |
| [testing.md](testing.md) | Test suites, fixtures, live (`#[ignore]`) tests and their environment variables, the LMS capture tool, manual acceptance checklists (Phase 1 and Phase 2) |

## Core (`crates/core`, pure logic, no I/O)

| File | Description |
|---|---|
| [core/domain-and-ports.md](core/domain-and-ports.md) | Domain types (email and LMS), enum conventions and every port trait (providers, LMS clients, models, secrets, repositories, clock, observers) |
| [core/normalization-and-filters.md](core/normalization-and-filters.md) | HTML to text, quote/signature stripping, content hash, deterministic filters |
| [core/date-extraction.md](core/date-extraction.md) | Spanish/English date and time candidate extraction and year resolution |
| [core/email-analyzer.md](core/email-analyzer.md) | Question set building, model calls and answer assembly (`analyzer` module) |
| [core/pipeline.md](core/pipeline.md) | `CheckRunner`: per-run, per-account and per-message processing, counters, run lock |

## Infrastructure (`crates/infra`, adapters)

| File | Description |
|---|---|
| [infra/database.md](infra/database.md) | SQLite connection, migrations 0001-0003, column encoding, the LMS tables and the repository implementations |
| [infra/secure-storage.md](infra/secure-storage.md) | `KeyringSecretStore` (Windows Credential Manager), the keys it holds (including LMS credentials) and the in-memory test fake |
| [infra/lms.md](infra/lms.md) | Moodle and Blackboard clients, the probe, sign-in capture and session renewal, the sync and its diff rules |
| [infra/imap.md](infra/imap.md) | IMAP provider: TLS, UID-based incremental sync, parsing, timeouts |
| [infra/microsoft-graph.md](infra/microsoft-graph.md) | Graph delta provider and the OAuth (PKCE, loopback) sign-in flow |
| [infra/ai-providers.md](infra/ai-providers.md) | jev and Ollama adapters: request/response mapping, retries, errors |
| [infra/factories.md](infra/factories.md) | `http_client`, `DefaultEmailProviderFactory`, `DefaultModelProvider` |

## Frontend (`src/`)

| File | Description |
|---|---|
| [frontend/overview.md](frontend/overview.md) | App structure, API client, events, query keys, conventions, testing |
| [frontend/tracker-universities.md](frontend/tracker-universities.md) | Dashboard (LMS tracker), Universities page and wizard, course selection, sync state |
| [frontend/dashboard-results-runs.md](frontend/dashboard-results-runs.md) | Announcements (email results), Email runs and the shared result components |
| [frontend/accounts-catalog-settings.md](frontend/accounts-catalog-settings.md) | Email accounts and Settings screens and the shared form primitives |

## Application shell (`src-tauri`)

| File | Description |
|---|---|
| [app/tauri-commands.md](app/tauri-commands.md) | Composition root, every Tauri command, DTOs, error mapping and the `run:*` / `lms-sync:*` events |
| [app/logging-and-data.md](app/logging-and-data.md) | Application log, app data directory (including the sign-in profile) and what is persisted |

## Design and plans (historical)

The documents above describe what the code does today. These are the original planning documents;
where they differ in detail, the component documents above are authoritative.

| File | Description |
|---|---|
| [superpowers/specs/2026-09-28-phase-1-email-analysis-design.md](superpowers/specs/2026-09-28-phase-1-email-analysis-design.md) | Approved Phase 1 design: contracts, pipeline, providers, schema, commands, UI |
| [superpowers/plans/2026-09-28-phase-1-email-analysis.md](superpowers/plans/2026-09-28-phase-1-email-analysis.md) | Phase 1 implementation plan: task breakdown, contracts and execution order |
| [superpowers/specs/2026-10-04-phase-2-lms-tracker-design.md](superpowers/specs/2026-10-04-phase-2-lms-tracker-design.md) | Phase 2 design (LMS tracker). Section 19 "As built" lists every amendment made during implementation and the findings from real platforms |
| [superpowers/plans/2026-10-04-phase-2-m1-lms-sign-in.md](superpowers/plans/2026-10-04-phase-2-m1-lms-sign-in.md), [2026-10-05-phase-2-m2-m3-items-and-sync.md](superpowers/plans/2026-10-05-phase-2-m2-m3-items-and-sync.md), [2026-10-08-phase-2-m4-m5-dashboard-and-integration.md](superpowers/plans/2026-10-08-phase-2-m4-m5-dashboard-and-integration.md) | Phase 2 implementation plans (sign-in gate; items and sync; dashboard and integration) |
