# Development guide

## Repository layout

```text
Cargo.toml                  workspace: crates/core, crates/infra, src-tauri (shared dependency versions)
rust-toolchain.toml         stable channel with rustfmt and clippy
crates/core/                academic-core: pure domain, ports and logic
  src/domain/               Account, Email, analysis, catalog, run, result and settings types, enums
  src/ports/                EmailProvider(+Factory), StructuredModel, ModelProvider, SecretStore,
                            repository traits, Clock, RunObserver
  src/normalize.rs          HTML to text, quote/signature stripping, content hash
  src/filters.rs            deterministic filters
  src/dates/                date candidate extraction (absolute, relative, time, year resolution)
  src/analyzer/             question building and answer assembly
  src/pipeline/             CheckRunner: run, account and message processing, run lock
  tests/                    pipeline integration tests with in-memory fakes (tests/support/)
crates/infra/               academic-infra: adapters
  migrations/               SQL migrations (0001_init.sql)
  src/db/                   Database and one repository impl per module
  src/secrets.rs            KeyringSecretStore, InMemorySecretStore
  src/imap/, src/graph/     email providers (Graph includes OAuth and the loopback listener)
  src/jev.rs, src/ollama.rs structured model adapters (shared helper: src/http_util.rs)
  src/factory.rs            http_client, DefaultEmailProviderFactory, DefaultModelProvider
  tests/                    integration tests, fixtures/ (.eml and JSON), support/
src-tauri/                  academic-assistant: composition root, commands, DTOs, events, logging
  src/lib.rs                Tauri builder and the command registration list
  src/setup.rs              startup: data directory, logging, database, AppState
  src/state.rs              AppState (repositories, adapters, CheckRunner)
  src/commands/             one module per area: runs, results, accounts, catalog, settings
  src/dto/                  DTOs per area (accounts, catalog, results, runs, settings)
  src/validation.rs         input validation and normalization
  src/error.rs              CommandError and error mapping
  src/account_status.rs     credentials/last-sync status and dashboard attention list
  src/observer.rs           TauriRunObserver (run:progress / run:finished events)
  src/test_support.rs       test harness (in-memory database and secrets, fake adapters)
  capabilities/default.json Tauri capability for the main window
  tauri.conf.json           window, CSP, bundle configuration
src/                        React app
  lib/api/                  types.ts (DTO mirror), client.ts (invoke wrappers), events.ts, queryKeys.ts
  components/               Layout, Modal, ErrorMessage, forms/, results/
  features/                 check-run, dashboard, results, runs, accounts, catalog, settings
docs/                       this documentation (index: docs/README.md)
```

Dependency rule: `src-tauri -> infra -> core`. `core` has no I/O dependencies.
See [architecture.md](architecture.md).

## Conventions

- **Workflow (`AGENTS.md`)**: work is split between a planner/orchestrator and implementing
  sub-agents. The planner analyzes, designs the solution, defines contracts and tasks and
  reviews results; it does not write production code. Sub-agents implement exactly what their
  task specifies, touch only the files they own, and stop and report if a decision beyond their
  scope is needed. Small, localized, low-risk changes may be made directly; large or
  cross-cutting changes are planned first. `AGENTS.md` is written in Spanish and is the source
  of truth.
- **Scope**: make the minimal change that satisfies the task. Do not refactor unrelated code,
  change contracts without updating every consumer, or add speculative abstractions.
- **Clean code**: single responsibility per module and function, no duplicated logic (extract it
  when reuse is real), no dead code (unused functions, imports, commented-out code, unreachable
  branches), simple explicit solutions.
- **No secrets**: never put passwords, tokens, API keys or real credentials in code, tests,
  fixtures, logs, commits or docs. Tests use fake values such as `test-key`. Secrets go through
  `SecretStore` only.
- **Privacy in logs and errors**: log ids, counters and stages; never bodies, subjects, tokens or
  keys. Error messages shown to the user must be sanitized.
- **Rust**: `core` stays pure. Repository traits are synchronous, provider and model traits are
  `async` (`async-trait`). Domain enums serialize as `snake_case` and implement `as_str` /
  `FromStr` through the `enum_str!` macro.
- **Frontend**: TypeScript strict, UI copy in English sentence case, no component library, all
  backend access through `src/lib/api/` and React Query with the `queryKeys` factories.
- **Documentation**: each component has a Markdown file that describes what exists (purpose,
  API, behavior, errors, how to test, limitations). Update it in the same change as the code.

## Quality gates

Run all that apply before considering a change done. From the repository root:

```powershell
# Rust (if cargo is not on PATH: $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path")
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace

# Frontend (run npm install first)
npm run typecheck
npm run lint
npm run test
npm run build
```

While iterating, narrow the Rust tests with `-p academic-core`, `-p academic-infra` or
`-p academic-assistant`, and the frontend tests with `npx vitest run <path>`. Fix warnings you
introduce; do not silence lints with `allow` without a justification. Before a release or a
merge into the phase branch, also verify the desktop app compiles end to end:

```powershell
npm run build
npx tauri build --debug --no-bundle
```

See [testing.md](testing.md) for the individual suites.

## Branch workflow

```text
main  <-  phase-1-email-analysis  <-  feature/*
```

- Feature branches are created from the current tip of the phase branch, one per plan task
  (`feature/<id>-<slug>`), and merged back into it with `git merge --no-ff` after the quality
  gates pass on the merged tree.
- The phase branch merges into `main` only after the user validates the phase.
- Commit messages follow Conventional Commits (`feat:`, `fix:`, `docs:`, `test:`, `refactor:`).
- Do not push or merge from a task branch unless the workflow asks you to.

## How to add things

### A database migration

The schema lives in `crates/infra/migrations/NNNN_name.sql` and is embedded with `include_str!`
in `crates/infra/src/db/mod.rs`. Version tracking uses the `schema_migrations` table.

1. Create `crates/infra/migrations/0002_<name>.sql` with the schema change. Never edit an
   already released migration.
2. `apply_migrations` currently handles only version 1 (one `MIGRATION_0001` constant and a single
   "is version 1 applied" check). Turn it into an ordered list of `(version, sql)` pairs applied
   in order, each in its own transaction together with its `schema_migrations` row, so existing
   databases upgrade and new ones run every step.
3. Add or update repository code in `crates/infra/src/db/` and any port/domain types in
   `crates/core` that need the new columns.
4. Add tests in `crates/infra/tests/db_migrations.rs` (fresh database and upgrade from version 1)
   and in the affected `db_*.rs` file. Update [infra/database.md](infra/database.md) and the data
   model table in [architecture.md](architecture.md).

### A Tauri command

Contract first: define the input/output DTO shape and the error kinds before coding.

1. **Rust command**: add the function to the matching module in `src-tauri/src/commands/`
   (`runs`, `results`, `accounts`, `catalog`, `settings`), returning `Result<T, CommandError>`.
   Validate inputs, use the shared `AppState`, never return or log secrets, bodies or subjects.
   Register it in the `tauri::generate_handler!` list in `src-tauri/src/lib.rs`.
2. **DTOs and validation**: add or extend the serde types in the matching module of
   `src-tauri/src/dto/` with `#[serde(rename_all = "camelCase")]`, plus conversions from/to the
   domain types, and validate new inputs in `src-tauri/src/validation.rs`. Map new error
   sources in `src-tauri/src/error.rs`.
3. **TypeScript**: mirror the DTOs exactly in `src/lib/api/types.ts`, and add one wrapper to the
   `api` object in `src/lib/api/client.ts` using the exact command name and argument object
   (`invoke('my_command', { camelCaseArg })`; Tauri maps camelCase keys to snake_case
   parameters). Add a key factory in `src/lib/api/queryKeys.ts` if the command reads data.
4. **Tests**: DTO/JSON-shape and validation unit tests in Rust, and command tests against the
   harness in `src-tauri/src/test_support.rs`; an assertion of the command name and arguments in
   `src/lib/api/client.test.ts`.
5. **Docs**: update [app/tauri-commands.md](app/tauri-commands.md) and
   [frontend/overview.md](frontend/overview.md) if the API surface changes.

### A screen

1. Create the page under `src/features/<area>/`. Read data with `useQuery` and
   `queryKeys.<x>(...)`; write with `useMutation`, invalidate the relevant key prefixes on
   success, and show failures with `toCommandError` and `ErrorMessage`.
2. Reuse `Modal`, the form primitives in `src/components/forms/` and the result components in
   `src/components/results/`; do not fork them.
3. Register the route in `src/App.tsx` and add a link to `NAV_ITEMS` in
   `src/components/Layout.tsx`.
4. Add Vitest + Testing Library tests next to the page, mocking `../../lib/api/client`.
5. Document it in the matching file under `docs/frontend/`.

### A new email provider

1. Extend `ProviderConfig` / `ProviderKind` (and `SyncCursor` if needed) in
   `crates/core/src/domain/`.
2. Implement `EmailProvider` in a new module in `crates/infra/src/` following the contract
   (read-only, at most `MAX_BATCH` emails oldest first, resumable cursor, errors mapped to
   `ProviderError`). Take secrets from `SecretStore`; never log message content.
3. Handle the variant in `DefaultEmailProviderFactory::create` (`crates/infra/src/factory.rs`).
4. Add a migration if the `accounts.provider_type` `CHECK` constraint must allow the new value,
   and extend `ProviderDto`, `ProviderInput` (in `AccountInput`) and the account form.
5. Test with fixtures and `wiremock` (or a fake server); add an `#[ignore]` live test if
   appropriate. Add a doc under `docs/infra/`.

### A new AI provider

1. Implement `StructuredModel` in a new module in `crates/infra/src/`: build the request from
   the `QuestionSet`, map answers to `Answer` values, map failures to `ModelError` (`Auth`,
   `RateLimited`, `Unavailable`, `InvalidResponse`). Never put keys or content in error messages.
2. Add an `AiProviderKind` variant, handle it in `DefaultModelProvider::current`
   (`crates/infra/src/factory.rs`), and add its settings to `Settings`, `SettingsDto`,
   `SettingsInput` and the Settings screen. Keep secrets in `SecretStore`.
3. Test with `wiremock` and add an `#[ignore]` live test that reads its configuration from
   environment variables. Update [infra/ai-providers.md](infra/ai-providers.md).
