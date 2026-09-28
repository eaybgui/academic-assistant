# Secure storage (`academic-infra::secrets`)

Implements the `academic_core::ports::SecretStore` trait (`get`/`set`/`delete`, keyed by
an opaque string). Secrets never reach SQLite, logs, DTOs or the repository — see
`docs/superpowers/specs/2026-09-28-phase-1-email-analysis-design.md` §12.

## `KeyringSecretStore`

Unit struct backed by the OS credential store — on Windows, the Credential Manager, via
the `keyring` crate's `windows-native` backend.

Each key (e.g. `academic-assistant/jev-api-key`, `academic-assistant/account/{id}/password`
— see `academic_core::domain::secrets`) is stored as one generic credential with:
- service = `academic_core::domain::SECRET_SERVICE` (`"academic-assistant"`)
- target/user = the key itself

Behavior:
- `get`: returns `Ok(None)` when no entry exists (`keyring::Error::NoEntry`), `Ok(Some(value))`
  otherwise.
- `set`: creates or overwrites the entry.
- `delete`: returns `Ok(())` both when the entry existed and was removed, and when it
  didn't exist to begin with (per the `SecretStore` contract).
- Any other platform error (e.g. the credential store being locked) is mapped to
  `SecretError`. The error message only describes the platform failure (via
  `keyring::Error`'s own `Display`) — it never includes the secret value, since the
  value is never part of a `keyring::Error`.

`KeyringSecretStore` has no fields and no constructor function: construct it as a bare
unit value, `KeyringSecretStore`.

### Testing

`KeyringSecretStore` touches the real Windows Credential Manager, so its test
(`crates/infra/tests/secrets.rs::keyring_store_round_trips_a_value`) is `#[ignore]`. Run
it explicitly with:

```sh
cargo test -p academic-infra --test secrets -- --ignored
```

It uses a randomly generated key under `academic-assistant/test/...` and deletes it
before and after the test, so it's safe to run repeatedly and doesn't touch any key used
by the real application.

## `InMemorySecretStore`

A `Mutex<HashMap<String, String>>`-backed fake, `pub` with `#[derive(Default)]`. Used by
this crate's own tests and by other tasks' tests (analyzer, pipeline, Microsoft Graph,
`src-tauri` command tests, etc.) that need a `SecretStore` without touching the OS.
Values live only for the lifetime of the store (typically one test).

## Known limitations

- `KeyringSecretStore` has no in-process caching: every `get`/`set`/`delete` call hits
  the OS credential store directly. This is intentional — secrets change rarely (an API
  key, an account password, an OAuth refresh token) and callers are expected to read
  once per run rather than in a hot loop.
- Secret keys are plain UTF-8 strings; the caller (see `academic_core::domain::secrets`)
  is responsible for building collision-free keys (e.g. embedding an account id).
