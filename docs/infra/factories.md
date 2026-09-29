# Provider factories (`academic-infra::factory`)

`crates/infra/src/factory.rs` is the composition point between an account's or the
user's stored configuration and the concrete adapter that implements it: it turns an
`Account` into an `EmailProvider` and `Settings` into a `StructuredModel`, so the rest
of the app (the pipeline, `src-tauri`) only ever depends on the `academic_core::ports`
traits, never on `ImapEmailProvider` / `MicrosoftGraphEmailProvider` / `JevModel` /
`OllamaModel` directly.

## `http_client()`

```rust
pub fn http_client() -> reqwest::Client;
```

Builds the single `reqwest::Client` the composition root should construct once and
share across every adapter it builds (IMAP does not use it; Microsoft Graph, jev and
Ollama all take a `reqwest::Client` in their constructors). It is configured with:

- rustls TLS (the `reqwest` dependency is already built with the `rustls` feature and
  no default TLS backend, per the workspace `Cargo.toml`).
- `User-Agent: academic-assistant/<CARGO_PKG_VERSION>` (the crate's own compiled-in
  version).
- A 10 second connect timeout — how long it will wait to establish the TCP+TLS
  connection before giving up. This is deliberately separate from each adapter's own
  per-request timeout (jev: 60 s, Ollama: 120 s, Graph: 60 s — see
  [ai-providers.md](ai-providers.md) and [microsoft-graph.md](microsoft-graph.md)), which bounds how
  long it waits for a response once connected.

Building the client performs no I/O; it can be called any number of times (e.g. once
per composition root run, or once per test) without side effects.

## `DefaultEmailProviderFactory`

```rust
pub struct DefaultEmailProviderFactory { .. }
impl DefaultEmailProviderFactory {
    pub fn new(secrets: Arc<dyn SecretStore>, settings: Arc<dyn SettingsRepository>, http: reqwest::Client) -> Self;
}
impl EmailProviderFactory for DefaultEmailProviderFactory { .. }
```

`create(&self, account: &Account) -> Result<Box<dyn EmailProvider>, ProviderError>`
switches on `account.provider`:

- **`ProviderConfig::Imap { host, port, username }`**: reads the account's password
  from `secrets.get(&account_password_key(account.id))`.
  - Missing (`Ok(None)`) → `ProviderError::Auth("The password is missing. Edit the
    account to set it")`.
  - `SecretStore` failure (`Err`) → `ProviderError::Protocol("Could not read secure
    storage")` (the underlying `SecretError` is not included: no internal detail and no
    secret value in a user-facing message).
  - Otherwise builds an `ImapEmailProvider::new(account.id, host, port, username,
    password)`. No network I/O happens here; the provider only connects when
    `test_connection`/`fetch_new` is called.
- **`ProviderConfig::MicrosoftGraph { tenant }`**: loads settings via
  `settings.load()`.
  - `RepoError` (`Err`) → `ProviderError::Protocol("Could not load settings")` (again,
    no internal detail).
  - `settings.microsoft_client_id` missing → `ProviderError::Auth("Set the Microsoft
    client ID in Settings")`.
  - Otherwise builds `OAuthConfig { client_id, tenant: account's tenant,
    login_base_url: graph::LOGIN_BASE_URL }` and
    `MicrosoftGraphEmailProvider::new(account.id, http, oauth, graph::GRAPH_BASE_URL,
    secrets)`. The refresh token itself is read by the provider from `secrets` lazily,
    on first use, not by the factory.

The factory clones its `http` client and `Arc<dyn SecretStore>` per call; building
providers is cheap and side-effect free (no network calls, no secret writes).

## `DefaultModelProvider`

```rust
pub struct DefaultModelProvider { .. }
impl DefaultModelProvider {
    pub fn new(secrets: Arc<dyn SecretStore>, http: reqwest::Client) -> Self;
}
impl ModelProvider for DefaultModelProvider { .. }
```

`current(&self, settings: &Settings) -> Result<Arc<dyn StructuredModel>, String>`
switches on `settings.ai_provider`:

- **`AiProviderKind::Ollama`**: always succeeds — builds
  `OllamaModel::new(http, settings.ollama_base_url, settings.ollama_model)`. No secret
  is needed.
- **`AiProviderKind::Jev`**: reads the jev API key from `secrets.get(JEV_API_KEY)`.
  - Missing → `Err("Add your jev API key in Settings")`.
  - `SecretStore` failure → `Err("Could not read secure storage")`.
  - Otherwise builds `JevModel::new(http, JEV_BASE_URL, key, settings.jev_model)`.

The `String` error is the contract `ModelProvider::current` already defines (see
`crates/core/src/ports/model.rs`); the pipeline (`core::pipeline::CheckRunner`) maps a
`current` failure to `RunError::AiNotConfigured` with this message.

## Errors and security

- Every error message returned by this module is safe to show in the UI and to log:
  no secret value, no `SecretError`/`RepoError` internal detail, ever appears in a
  `ProviderError` or `String` this module returns.
- Building a provider never performs network I/O. Connectivity is only exercised when
  the caller later calls `EmailProvider::test_connection`/`fetch_new` or
  `StructuredModel::ask`.

## Testing

`crates/infra/tests/factory.rs` (integration tests) cover all four provider-selection
branches above, each asserting the `Ok`/`Err` outcome and, for `Ok`, the built
`StructuredModel::descriptor()` (`provider`/`model`) where applicable:

- IMAP: password present → `Ok`; password missing → `Err(Auth("The password is
  missing. Edit the account to set it"))`.
- Microsoft Graph: `microsoft_client_id` configured (via `Database::open_in_memory()`
  + `SettingsRepository::save`) → `Ok`; not configured → `Err(Auth("Set the Microsoft
  client ID in Settings"))`.
- Ollama: default settings → `Ok`, `descriptor() == { "ollama", settings.ollama_model
  }`.
- Jev: key present in `InMemorySecretStore` → `Ok`, `descriptor() == { "jev",
  settings.jev_model }`; key missing → `Err("Add your jev API key in Settings")`.
- `http_client()` can be called repeatedly without panicking or performing I/O.

Tests use `InMemorySecretStore` (`academic_infra::secrets`) and
`Database::open_in_memory()` (`academic_infra::db`, implementing
`SettingsRepository`) — no OS credential store, no real database file, no network
access.

## Known limitations

- `DefaultEmailProviderFactory` does not validate `account.folders` or any other
  account field beyond what's needed to pick a provider kind and read its secret;
  validating account input is the Tauri command layer's job
  ([../app/tauri-commands.md](../app/tauri-commands.md)).
