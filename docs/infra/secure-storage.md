# Secure storage (`academic-infra::secrets`)

Implements the `academic_core::ports::SecretStore` trait (`get`/`set`/`delete`, keyed by
an opaque string). Secrets never reach SQLite, logs, DTOs or the repository; see the security model in
[../architecture.md](../architecture.md#security-and-privacy-model).

## `KeyringSecretStore`

Unit struct backed by the OS credential store — on Windows, the Credential Manager, via
the `keyring` crate's `windows-native` backend.

Each key (e.g. `academic-assistant/jev-api-key`, `academic-assistant/account/{id}/password`
— see `academic_core::domain::secrets`) is stored as one generic credential with:
- service = `academic_core::domain::SECRET_SERVICE` (`"academic-assistant"`)
- target/user = the key itself

Values are stored and read as raw UTF-8 bytes via `keyring::Entry::set_secret`/
`get_secret`, not `set_password`/`get_password`. `keyring`'s `set_password` re-encodes
the value as UTF-16 for the native Windows credential blob and caps it at 1280 chars
(`CRED_MAX_CREDENTIAL_BLOB_SIZE`, 2560 bytes / 2 bytes-per-UTF-16-unit) — too small for
some Microsoft Entra refresh tokens. Storing the UTF-8 bytes directly avoids the
UTF-16 re-encoding and uses the same 2560-byte cap as a byte count instead.

Behavior:
- `get`: returns `Ok(None)` when no entry exists (`keyring::Error::NoEntry`), otherwise
  reads the stored bytes and decodes them as UTF-8; a stored value that isn't valid
  UTF-8 (which nothing in this codebase can produce, since `set` only ever writes
  `str::as_bytes()`) is mapped to `SecretError` rather than panicking or silently
  losing data.
- `set`: creates or overwrites the entry.
- `delete`: returns `Ok(())` both when the entry existed and was removed, and when it
  didn't exist to begin with (per the `SecretStore` contract).
- Any other platform error (e.g. the credential store being locked) is mapped to
  `SecretError`. The error message only describes the platform failure (via
  `keyring::Error`'s own `Display`) — it never includes the secret value, since the
  value is never part of a `keyring::Error`. `SecretError`'s own `Display` impl already
  prefixes the message with "Secure storage error: ", so `KeyringSecretStore` does not
  add a prefix of its own.

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
by the real application. A second `#[ignore]`d test,
`keyring_store_round_trips_a_large_value`, round-trips a ~2 KB value the same way, to
verify `set_secret`/`get_secret` isn't limited to the 1280-char cap `set_password` would
impose.

## When secrets are written and removed

The store itself is passive; the Tauri commands ([../app/tauri-commands.md](../app/tauri-commands.md))
decide when keys change:

- IMAP password (`academic-assistant/account/{id}/password`): written when an IMAP account is
  added or edited with a password, removed when the account is deleted.
- Microsoft refresh token (`academic-assistant/account/{id}/refresh-token`): written after a
  successful sign-in whose mailbox matches the account, removed when the account is deleted.
  Changing a Microsoft account's email or tenant discards it, so the account has to be signed
  in again.
- jev API key (`academic-assistant/jev-api-key`): written and removed from Settings.

Account ids are never reused (`accounts.id` is `AUTOINCREMENT`), so a key left behind by a
deleted account can never be picked up by a later one.

## `InMemorySecretStore`

A `Mutex<HashMap<String, String>>`-backed fake, `pub` with `#[derive(Default)]`. Used by
this crate's own tests (factories, Microsoft Graph) and by the `src-tauri` command
tests, which need a `SecretStore` without touching the OS.
Values live only for the lifetime of the store (typically one test).

## Known limitations

- `KeyringSecretStore` has no in-process caching: every `get`/`set`/`delete` call hits
  the OS credential store directly. This is intentional — secrets change rarely (an API
  key, an account password, an OAuth refresh token) and callers are expected to read
  once per run rather than in a hot loop.
- Secret keys are plain UTF-8 strings; the caller (see `academic_core::domain::secrets`)
  is responsible for building collision-free keys (e.g. embedding an account id).
