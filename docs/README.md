# Documentation index

Project-level documents live directly in `docs/`; per-component reference documents live in
the subfolders. Root-level [requirements.md](../requirements.md) and
[technical-specification.md](../technical-specification.md) describe what the product must do
and the overall technical direction.

## Guides

| File | Description |
|---|---|
| [developer-guide.html](developer-guide.html) | Functional guide for new developers (open in a browser): what each part does, how a check runs, how the AI analysis works, the database and a hands-on tutorial |
| [setup.md](setup.md) | Prerequisites, Microsoft Entra app registration, running in dev, building, where data lives |
| [user-guide.md](user-guide.md) | Every screen and action, how a check works, privacy notes, troubleshooting |
| [architecture.md](architecture.md) | Crates and dependency rule, data flow of a check, data model, security model, extension points |
| [development.md](development.md) | Repo layout, conventions, quality gates, branch workflow, how to add a migration/command/screen/provider |
| [testing.md](testing.md) | Test suites, fixtures, live (`#[ignore]`) tests and their environment variables, manual acceptance checklist |

## Core (`crates/core`, pure logic, no I/O)

| File | Description |
|---|---|
| [core/domain-and-ports.md](core/domain-and-ports.md) | Domain types, enum conventions and every port trait (providers, models, secrets, repositories, clock, observer) |
| [core/normalization-and-filters.md](core/normalization-and-filters.md) | HTML to text, quote/signature stripping, content hash, deterministic filters |
| [core/date-extraction.md](core/date-extraction.md) | Spanish/English date and time candidate extraction and year resolution |
| [core/email-analyzer.md](core/email-analyzer.md) | Question set building, model calls and answer assembly (`analyzer` module) |
| [core/pipeline.md](core/pipeline.md) | `CheckRunner`: per-run, per-account and per-message processing, counters, run lock |

## Infrastructure (`crates/infra`, adapters)

| File | Description |
|---|---|
| [infra/database.md](infra/database.md) | SQLite connection, migrations, column encoding and the repository implementations |
| [infra/secure-storage.md](infra/secure-storage.md) | `KeyringSecretStore` (Windows Credential Manager) and the in-memory test fake |
| [infra/imap.md](infra/imap.md) | IMAP provider: TLS, UID-based incremental sync, parsing, timeouts |
| [infra/microsoft-graph.md](infra/microsoft-graph.md) | Graph delta provider and the OAuth (PKCE, loopback) sign-in flow |
| [infra/ai-providers.md](infra/ai-providers.md) | jev and Ollama adapters: request/response mapping, retries, errors |
| [infra/factories.md](infra/factories.md) | `http_client`, `DefaultEmailProviderFactory`, `DefaultModelProvider` |

## Frontend (`src/`)

| File | Description |
|---|---|
| [frontend/overview.md](frontend/overview.md) | App structure, API client, events, query keys, conventions, testing |
| [frontend/dashboard-results-runs.md](frontend/dashboard-results-runs.md) | Dashboard, Results and Runs screens and the shared result components |
| [frontend/accounts-catalog-settings.md](frontend/accounts-catalog-settings.md) | Accounts, University catalog and Settings screens and the shared form primitives |

## Application shell (`src-tauri`)

| File | Description |
|---|---|
| [app/tauri-commands.md](app/tauri-commands.md) | Composition root, Tauri commands, DTOs, error mapping and events |
| [app/logging-and-data.md](app/logging-and-data.md) | Application log, app data directory and what is persisted |

## Design and plan (historical)

The documents above describe what the code does today. These are the original planning documents;
where they differ in detail, the component documents above are authoritative.

| File | Description |
|---|---|
| [superpowers/specs/2026-09-28-phase-1-email-analysis-design.md](superpowers/specs/2026-09-28-phase-1-email-analysis-design.md) | Approved Phase 1 design: contracts, pipeline, providers, schema, commands, UI |
| [superpowers/plans/2026-09-28-phase-1-email-analysis.md](superpowers/plans/2026-09-28-phase-1-email-analysis.md) | Phase 1 implementation plan: task breakdown, contracts and execution order |
