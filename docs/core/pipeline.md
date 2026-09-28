# `core::pipeline`: check pipeline (`CheckRunner`)

`crates/core/src/pipeline/` (Task T12) orchestrates a manual "check now" across
every enabled account: connect, fetch each configured folder, and run every
fetched email through dedup → deterministic filters → date extraction → AI
analysis → persistence (spec §5, §5.1). It depends only on the ports in
`core::ports` and the pure modules `core::normalize`, `core::filters`,
`core::dates` and `core::analyzer` — no I/O of its own, no logging, no
`tokio::spawn`. `execute` is a plain `async fn`; the Tauri layer is responsible
for spawning it and for wiring `run:progress`/`run:finished` events off
[`RunObserver`].

## Public API

```rust
pub struct Repositories {
    pub accounts: Arc<dyn AccountRepository>,
    pub catalog: Arc<dyn CatalogRepository>,
    pub sync: Arc<dyn SyncStateRepository>,
    pub processed: Arc<dyn ProcessedMessageRepository>,
    pub results: Arc<dyn ResultRepository>,
    pub runs: Arc<dyn RunRepository>,
    pub settings: Arc<dyn SettingsRepository>,
}

pub struct CheckRunner { /* private */ }
impl CheckRunner {
    pub fn new(repos: Repositories, providers: Arc<dyn EmailProviderFactory>,
               models: Arc<dyn ModelProvider>, observer: Arc<dyn RunObserver>,
               clock: Arc<dyn Clock>) -> Self;
    pub fn prepare(&self, trigger: RunTrigger) -> Result<PreparedRun, RunError>;
    pub async fn execute(&self, run: PreparedRun) -> RunStatus;
    pub fn running_run(&self) -> Option<RunId>;
}

pub struct PreparedRun { pub run_id: RunId, /* model + settings snapshot + enabled
                                                accounts + lock guard, private */ }
```

`CheckRunner` is stateless besides the single-run lock: a run is driven by
calling `prepare` (fast, synchronous, no mail I/O) and then `execute` (the
actual sync/analyze work) with the `PreparedRun` it returned.

## Module layout

- `mod.rs` — `CheckRunner`/`PreparedRun`/`Repositories`, the public API, and the
  run-level orchestration: precondition checks, the catalog load, the per-account
  loop, and the run's final status/counters/summary.
- `account.rs` — per-account processing: connect (`EmailProviderFactory::create`),
  fetch each folder, run every message through `message::process_message`,
  accumulate that account's `Counters`, and report `Connecting`/`Fetching`/
  `Analyzing`/`Done`/`Failed` progress.
- `message.rs` — the per-message decision table (spec §5.1): dedup against
  `processed_messages`, `filters::apply`, `dates::extract`, `analyzer::analyze`,
  and persistence of whichever outcome resulted.
- `lock.rs` — the single-run lock (`RunLock`/`RunLockGuard`), an RAII guard that
  releases automatically when dropped, so `prepare`/`execute` can never leak it
  on an error path.

## `prepare`

Order (spec §5 step 1; the run row is only created once every check has
passed, so a failed `prepare` never creates a `runs` row):

1. The single-run lock is busy → `RunError::AlreadyRunning`.
2. `settings.load()` (repo error → `RunError::Storage`).
3. `accounts.list()` (repo error → `RunError::Storage`), filtered to
   `enabled == true`; empty → `RunError::NoEnabledAccounts`.
4. `models.current(&settings)`; `Err(message)` → `RunError::AiNotConfigured(message)`.
5. `runs.create(trigger, clock.now())` (repo error → `RunError::Storage`), then
   the lock guard is told the resulting `run_id` (so `running_run()` reports it).

The lock is acquired at step 1 and released automatically (the guard's `Drop`)
if any later step returns `Err` — including a `Storage` error from `runs.create`
itself. On success, the guard moves into the returned `PreparedRun` and is
released when `execute` consumes it.

`PreparedRun` snapshots the selected model, the loaded `Settings` and the
enabled-account list at `prepare` time, so a setting or account change that
happens while the run is executing does not affect it.

## `execute`

1. Loads the catalog once for the whole run (`catalog.catalog()`). If this
   fails, every account in `PreparedRun` is recorded as `Failed`/`ErrorKind::Internal`
   (one `runs.record_account` per account, so the run's history stays
   consistent with "one row per attempted account"), the run finishes `Failed`,
   `observer.finished` fires once, and `execute` returns `RunStatus::Failed`.
   This is the one failure mode `execute` can hit before any account is
   processed; it is not explicitly itemized in the per-message table because it
   happens above the per-account loop.
2. Otherwise, every enabled account from `PreparedRun` is processed
   **sequentially** via `account::process_account` (see below), accumulating a
   run-wide `Counters` (field-by-field sum of every account's `Counters`) and a
   failed-account count. Each account's `RunAccountResult` is recorded via
   `runs.record_account` regardless of outcome.
3. Final status (spec §5 step 5): `Success` if no account failed, `Failed` if
   every attempted account failed, `Partial` otherwise. `error_summary` is
   `Some("<failed> of <accounts_processed> accounts failed")` whenever the
   status isn't `Success`, else `None`.
4. `runs.finish(...)` is called with the final status/counters/summary,
   `observer.finished(run_id, status)` fires exactly once, and the lock is
   released (the guard, held inside the now-consumed `PreparedRun`, is dropped
   when `execute`'s local bindings go out of scope — on every path, since Rust
   always runs destructors on return).

`execute` never panics on a provider or model error and always returns a
`RunStatus` (never a `Result`): every failure mode below is captured in the
per-account/per-message outcome instead of propagated as a Rust error.
Secondary repository writes during execution (`record_account`, `finish`,
`sync.record_success`/`record_failure`) are best-effort: their `RepoResult` is
intentionally ignored (`let _ = ...`), since `execute`'s contract is to always
finish and report a `RunStatus`, and `core` has no logging facility to surface
a write failure through. A `RunObserver`/`tracing` layer at the Tauri boundary
is the place to capture such failures if that becomes necessary.

## Per-account processing (`account.rs`, spec §5 steps 3-4)

1. `Connecting` progress, then `providers.create(account)`. On error: `Failed`
   progress, `RunAccountResult { status: Failed, error_kind: error.kind(),
   error_message: error.to_string(), counters: Counters::default() }` — no
   folder is touched.
2. For each folder in `account.folders`, **in order**:
   - `Fetching` progress.
   - `sync.get(account_id, folder)` for the stored cursor (repo error: treated
     like a fetch failure below, with `ErrorKind::Internal`, and this folder is
     skipped — `continue` to the next folder).
   - `since = clock.now() - Duration::days(account.initial_sync_days)`.
   - `provider.fetch_new(folder, cursor, since)`. On error: `sync.record_failure`
     is called for this folder, the account is marked failed with the
     provider's `error.kind()`, and processing **continues with the next
     folder** (spec §5 step 3.5's per-folder error handling).
   - On success, every email in the batch is processed in order through
     `message::process_message` — see below. `Analyzing` progress fires every 10
     messages and on the batch's last message (or immediately, on top of that,
     the message that stops the account).
   - A transient model error or a repository error while processing a message
     stops **this folder and the whole account**: `sync.record_failure` is
     called (so the folder's `sync_states` row is left `error`, cursor
     un-advanced) and the outer folder loop breaks — no further folders for
     this account are attempted either.
   - Otherwise (the folder's batch was fully processed without stopping):
     `sync.record_success(account_id, folder, batch.next_cursor, now)`.
3. After every folder (or after an early stop), one final `Done` or `Failed`
   progress event fires, and the account's `RunAccountResult` is returned:
   `Failed` (with the *first* error kind/message the account hit) if any folder
   failed to fetch, or a message stopped the folder; `Success` otherwise.

An account can have some folders succeed and others fail (a `fetch_new` error
does not stop the account, only that folder); it is only a transient AI error
or a repository error partway through a folder's messages that stops every
remaining folder too.

## Per-message decision (`message.rs`, spec §5.1)

For each fetched email, in order:

| Step | Outcome |
|---|---|
| `processed.exists(account_id, folder, provider_message_id)` is `true` | Skipped — not counted as `found`, nothing else happens |
| `normalize::normalize_body` → `normalize::content_hash` → `processed.is_duplicate(account_id, message_id_header, content_hash)` is `true` | `processed.insert` with outcome `Filtered`, reason `"duplicate"` |
| `filters::apply(email, normalized_body, cfg)` discards | `processed.insert` with outcome `Filtered`, reason = the `FilterReason`'s `as_str()` (`blocked_sender`, `blocked_domain`, `own_message`, `empty`) |
| `dates::extract(subject, normalized_body, email.received_at.with_timezone(&clock.local_offset()))`, then `analyzer::analyze` → `Irrelevant` | `processed.insert` with outcome `Irrelevant`, reason `None` |
| `analyzer::analyze` → `Relevant(analysis)` | `results.save_analyzed(processed_message, new_result)` — one transaction, outcome `Analyzed` |
| `analyzer::analyze` → `Err(ModelError::InvalidResponse(message))` | `processed.insert` with outcome `Failed`, reason = a sanitized `message` (whitespace/newlines collapsed to single spaces, truncated to 500 `char`s); the message is *not* counted as failing the account, and processing continues with the next message |
| `analyzer::analyze` → `Err(other)` (`Auth`, `RateLimited`, `Unavailable` — the transient variants, per `ModelError::is_transient`) | Propagated up as a folder/account-stopping error (see `account.rs` above), `ErrorKind::Ai` |
| A `RepoError` from any of the repository calls above | Propagated up the same way, `ErrorKind::Internal` |

`FilterConfig.own_address` is the account's `email_address`; `FilterConfig.blocked`
is `settings.blocked_senders`. `ProcessedMessage.run_id`/`processed_at` and, for a
saved result, `NewResult.detected_at`/`run_id` are the current run's id and
`clock.now()` at the moment each is written.

### Counters (spec §5.1 last paragraph)

Accumulated per account, then summed into the run's `Counters` in `execute`:

- `found`: every message not skipped by `exists` (i.e. every row above the
  first one in the table). A message that stops the account with a repository
  error is the one exception: since the failure can happen inside the `exists`
  check itself, it is **not** counted as `found` (there is no way to know
  whether it would have been "found" without risking double-processing on a
  retry). A transient model error, in contrast, only ever happens after
  `exists`/dedup/filters have already run, so it *is* counted as `found`.
- `filtered`: `Filtered` outcomes (dedup or deterministic filters).
- `analyzed`: `Irrelevant` + `Relevant` + `InvalidResponse` outcomes — every
  message actually sent to the model.
- `failed`: `InvalidResponse` outcomes only.
- `results_created`: `Relevant` outcomes whose `save_analyzed` call succeeded.

## The single-run lock (`lock.rs`)

`RunLock` is a cheap-to-clone handle (`Arc<Mutex<LockState>>`) shared between
every `CheckRunner` method. `try_acquire()` fails (`None`) if a run is already
in progress; on success it returns a `RunLockGuard` that releases the lock when
dropped — from `prepare`'s early-return paths directly, or later from inside
`PreparedRun` once `execute` finishes. `running_run()` reports the in-progress
run's id once `prepare` has created its row (`RunLockGuard::set_run_id`); before
that (the brief window between acquiring the lock and the run row actually
being created — both synchronous, no `.await` between them, so effectively
unobservable from another thread in practice) it reports `None`, same as when
idle. The mutex is never allowed to panic the caller: a poisoned lock (from a
prior panic while held, which nothing in this pure module should ever trigger)
recovers via `PoisonError::into_inner` rather than propagating the panic.

## Errors

`prepare` returns `RunError` (`AlreadyRunning`, `NoEnabledAccounts`,
`AiNotConfigured`, `Storage`) — see above for exactly which precondition maps
to which variant. `execute` never returns a `Result`; every failure is folded
into a `RunAccountResult`/`Counters`/final `RunStatus` instead (see "Per-account
processing" and "Per-message decision" above).

## How to test

```
cargo test -p academic-core --lib pipeline::      # RunLock unit tests, message::sanitize
cargo test -p academic-core --test pipeline       # full CheckRunner integration tests
```

`crates/core/tests/support/` provides in-memory fakes for every port
`CheckRunner` depends on:

- `TestStore` implements all seven repository traits on one shared
  `Arc<Mutex<...>>`-backed struct (so, exactly like a single SQLite connection
  would, a row written through `ResultRepository::save_analyzed` is visible to
  a later `ProcessedMessageRepository::is_duplicate` check in the same test).
- `FakeEmailProviderFactory`/its internal `FakeProvider` queue canned
  `fetch_new` responses per account and record every call's folder/cursor/since.
- `FakeStructuredModel` queues canned `ask` responses, consumed one per call, in
  order; `FakeModelProvider` wraps one in a `ModelProvider`, or always fails
  with a fixed message.
- `FakeObserver` records every `progress`/`finished` call.
- `FakeClock` is a fixed `now`/`local_offset`, set at construction.
- `fixtures.rs` provides small builders (`account`, `email`, `relevant_answers`,
  `irrelevant_answers`) tuned so that, by default (no default university, empty
  catalog, no date candidates), the analyzer's question set is always just its
  five mandatory questions — tests that need a `university`/`course`/`date_i`
  question add the matching catalog entry or answer explicitly.

`crates/core/tests/pipeline.rs` covers: the happy path (results created, cursor
advanced); already-`exists` messages skipped and uncounted; a duplicate by
content hash filtered; each deterministic filter reason; an irrelevant email
stored without a result; `InvalidResponse` recorded `Failed` without stopping
the account (cursor still advances); every transient `ModelError` variant
(`Auth`, `RateLimited`, `Unavailable`) stopping the account (cursor not
advanced, `sync.record_failure` called) while a second account still runs to
completion; a `fetch_new` error failing one folder while a later folder still
runs; provider-creation errors mapping every `ProviderError` variant to the
matching `ErrorKind`; a catalog-load failure failing every enabled account;
the stored cursor/`since` window being passed to `fetch_new`; run statuses
`Success`/`Partial`/`Failed` and `error_summary`; `AlreadyRunning` while a run
is genuinely executing (a `tokio::sync::Notify`-blocked fake provider) and the
lock being released afterward; `NoEnabledAccounts`/`AiNotConfigured` without a
run row being created; the lock releasing after a failed `prepare` and after a
completed `execute`; the `Connecting`/`Fetching`/`Analyzing`/`Done` progress
sequence; and a date candidate ("mañana") resolving against the local offset
rather than UTC.

## Known limitations

- Secondary repository writes during `execute` (`record_account`, `finish`,
  `sync.record_success`/`record_failure`) ignore their own `RepoResult`
  (documented above, under "`execute`"). If the underlying store is failing
  broadly, a run can finish reporting a status that a later `runs.get` cannot
  actually retrieve consistently — this pipeline has no way to surface that,
  by design (see the contract: `execute` always returns a `RunStatus`, never a
  `Result`).
- The account's failure `error_kind`/`error_message` is the *first* folder- or
  message-level error the account hits, not an aggregate of every folder that
  failed (an account with two independently failing folders reports only the
  first one's error).
- `InvalidResponse`'s `reason` is only lightly sanitized (whitespace/newline
  collapsing and a 500-character cap) — it is not scanned for secrets. Provider
  error messages are expected to already be safe to store (the underlying
  `StructuredModel` implementations are responsible for not leaking credentials
  into `ModelError::InvalidResponse`'s message).
