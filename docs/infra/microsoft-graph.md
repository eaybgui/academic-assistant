# Microsoft Graph email provider and OAuth

`crates/infra/src/graph/` implements the `EmailProvider` port for Microsoft 365 /
Outlook.com accounts using Microsoft Graph's delta API, plus the Authorization
Code + PKCE sign-in flow against Microsoft's identity platform (v2 endpoints).
Public client, no client secret.

## Module layout

- `mod.rs` — constants (`LOGIN_BASE_URL`, `GRAPH_BASE_URL`, `SCOPES`), `OAuthConfig`,
  `TokenSet` (its `Debug` output redacts `access_token` and `refresh_token`), the shared
  60 s default HTTP request timeout (overridable per provider instance through the
  `#[doc(hidden)]` test hook `MicrosoftGraphEmailProvider::with_request_timeout`, which applies to
  Graph pages, `/me` and token refreshes; the public `oauth::refresh` and `signed_in_identity` always
  use the default), `SignedInIdentity { mail, user_principal_name }` (both optional,
  blank values become `None`, at least one is present) and `signed_in_identity`
  (`GET /me`, used both by `MicrosoftGraphEmailProvider::test_connection`, which only
  needs the call to succeed, and right after sign-in to confirm the token belongs to the
  expected account). A mailbox can be reported under either identifier (`mail` may be
  an alias or empty while `userPrincipalName` is the login), so the caller accepts the
  account when its address equals **either** of them, case-insensitively.
- `oauth.rs` (`pub mod oauth`) — PKCE (`new_pkce`), `authorize_url`, `sign_in`
  (interactive, loopback-based), `refresh` (refresh-token exchange) and
  `is_valid_tenant` (the tenant rule below, also used by the app to validate a tenant
  when an account is saved).
- `loopback.rs` — a minimal HTTP/1.1 listener bound to `127.0.0.1:0`, used only as
  the OAuth redirect target during `sign_in`. Not part of the public API.
- `provider.rs` — `MicrosoftGraphEmailProvider` (the `EmailProvider` impl): access
  token caching/refresh and the delta fetch/pagination logic.
- `mapping.rs` — pure functions: Graph message JSON → `Email`, `GET /me` response →
  `SignedInIdentity`, and HTTP/OAuth error responses → `ProviderError`. No I/O.

## App registration (one-time, per deployment)

A user (or the app's maintainer) must register an app in Entra ID before any
account can sign in:

1. Entra ID → **App registrations** → **New registration**.
2. Supported account types: **Accounts in any organizational directory** (or
   narrower, per tenant policy).
3. Platform: **Mobile and desktop applications**, redirect URI `http://localhost`
   (Entra allows any port on `localhost` for this platform type; the app requests a
   random free port at sign-in time and derives the exact `redirect_uri` from it).
4. API permissions (delegated): `Mail.Read`, `User.Read`, `offline_access`.
5. **Authentication** → enable **Allow public client flows**.
6. No client secret is created or used.

The resulting **Application (client) ID** is the app-wide `microsoft.client_id`
setting (`Settings::microsoft_client_id`); each account additionally has its own
`tenant` (`ProviderConfig::MicrosoftGraph { tenant }`, defaulting to
`DEFAULT_GRAPH_TENANT = "organizations"`).

## Sign-in flow (`oauth::sign_in`)

0. The tenant is validated first (`oauth::is_valid_tenant`: `^[A-Za-z0-9.-]+$`, non-empty; the app applies the same rule when an account is saved); anything else fails with
   `ProviderError::Auth("Invalid Microsoft tenant")` before any URL is built, listener
   started or browser opened. The same check guards the `/token` URL in `refresh` and the
   code exchange. `authorize_url` therefore assumes an already-validated tenant; it never
   panics (on an unparseable URL it returns the raw concatenation).
1. `LoopbackServer::bind()` binds `127.0.0.1:0` and derives
   `redirect_uri = "http://localhost:{port}"`.
2. `oauth::new_pkce()` generates a verifier (32 random bytes, base64url, no padding)
   and its `S256` challenge; a random `state` is generated the same way.
3. `oauth::authorize_url` builds the `/{tenant}/oauth2/v2.0/authorize` URL with
   `client_id`, `response_type=code`, `redirect_uri`, `response_mode=query`,
   `scope`, `state`, `code_challenge`, `code_challenge_method=S256` and
   `prompt=select_account`.
4. The caller-supplied `open_browser` callback is invoked with that URL (in
   `src-tauri`, this opens the system browser; it never blocks — `sign_in` waits on
   the loopback listener instead).
5. `LoopbackServer::wait_for_redirect` waits (up to the given `timeout`, the design
   spec's guidance is 5 minutes) for `GET /?code=...&state=...` (or `?error=...`).
   Only the root path `/` is accepted; any other path or method (notably a browser's
   `/favicon.ico`) gets a 404 and the listener keeps waiting. Each connection is capped at
   8 KB of request data and 2 s to send it (bounded by the remaining overall timeout), so
   an idle or stalled connection is dropped and cannot block the real redirect.
6. The returned `state` is checked against the one generated in step 2; a mismatch
   fails with `ProviderError::Auth`.
7. An `error` query parameter (the user cancelled, or Microsoft rejected the
   request) is mapped the same way token-endpoint errors are (see below).
8. The authorization `code` is exchanged for tokens at `/{tenant}/oauth2/v2.0/token`
   using the PKCE verifier.

`sign_in` returns a `TokenSet`; **the caller is responsible** for calling
`signed_in_identity` to confirm the returned account's address matches the one the
user picked (see above), and for persisting `refresh_token` via
`SecretStore::set(&account_refresh_token_key(id), ..)` (this module never touches
the `SecretStore` during sign-in — only `MicrosoftGraphEmailProvider` does, during
refresh).

## Token refresh and caching (`MicrosoftGraphEmailProvider`)

- The access token is **only ever kept in memory**, behind a `Mutex`, and is
  refreshed when missing or within 60 seconds of its stated expiry.
- Refreshing reads the refresh token from
  `SecretStore::get(&account_refresh_token_key(account_id))`; if absent, the
  provider returns `ProviderError::Auth("Sign in to this account")` without any
  network call.
- A successful refresh calls `oauth::refresh`. Microsoft does not always rotate the
  refresh token: when the response omits one, `oauth::refresh` reuses the token passed in
  (the initial code exchange still requires one). The provider writes the refresh token
  to the `SecretStore` only when it differs from the stored one, before caching the new
  access token — so a crash between refresh and the next use never leaves a stale
  refresh token behind, and unchanged tokens cause no credential-store write.
- The token cache mutex is poison-tolerant (a panic elsewhere cannot make later calls
  panic).
- `test_connection` is exactly "refresh, then `GET /me`".

## Fetching mail (`EmailProvider::fetch_new`)

- No cursor: builds `GET {graph_base_url}/me/mailFolders/{folder}/messages/delta`
  with `$select=id,internetMessageId,subject,from,toRecipients,ccRecipients,receivedDateTime,body,webLink`
  and `$filter=receivedDateTime ge {since}` (`since` as RFC 3339 UTC, e.g.
  `2026-09-01T00:00:00Z`). The `folder` path segment is URL-encoded by the `url`
  crate's path-segment API.
- With a cursor (`SyncCursor::Graph { link }`): `GET` that link directly — no
  `$select`/`$filter` is added (the link already encodes the continuation state).
- Every request carries `Prefer: outlook.body-content-type="text",
  odata.maxpagesize=50`.
- **400 fallback**: if the very first (filtered, no-cursor) request returns 400,
  it is retried once without `$filter`. The saved link from that path has no `$filter`,
  so every mapped message with `received_at < since` is discarded client-side on every
  page of **every** call (with or without a cursor, not only after the fallback in the
  current run). Discarded messages do not count toward the batch limit.
- Pagination follows `@odata.nextLink` until the response carries `@odata.deltaLink`,
  or fewer than one page (50) of room remains below `MAX_BATCH` (200): the check happens
  before requesting the next page, so a batch never exceeds `MAX_BATCH` and the saved
  cursor stays page-aligned. `next_cursor` is always the last link obtained — the
  unfetched `nextLink` when capped, or the `deltaLink` when the feed is exhausted.
- A message that cannot be mapped is **skipped** (a `tracing::warn!` with only its Graph
  id, never content) instead of failing the batch, so one bad message cannot block the
  folder; the cursor still advances past it.
- Returned emails are sorted oldest first (`received_at`, ties by provider message id).
- Entries with `@removed` (deletions reported by the delta feed) are skipped; this
  app has no concept of removing an already-stored result, so they are simply
  ignored.
- Mapping: `from.emailAddress` → sender (missing → `ProviderError::Protocol`);
  `toRecipients` + `ccRecipients` → recipients; `receivedDateTime` (missing/invalid
  → `Protocol`); `body.contentType` `"text"` → `EmailBody::Text`, anything else →
  `EmailBody::Html`; `internetMessageId` with angle brackets stripped; `webLink` →
  `source_url`.

## Errors

| Situation | `ProviderError` | Message |
|---|---|---|
| No refresh token stored | `Auth` | "Sign in to this account" |
| Token endpoint `error: invalid_grant` | `Auth` | "Your sign-in expired. Reconnect the account" |
| Token/redirect error description contains `AADSTS65001` or `AADSTS90094` | `Auth` | "Your organization requires admin approval for this app" |
| Other token endpoint error | `Auth` | "Microsoft sign-in failed ({error code})" |
| Graph API 401/403 | `Auth` | "Microsoft Graph rejected the request. Reconnect the account" |
| DNS/TCP/TLS/timeout (every Graph/OAuth request has a 60 s timeout) | `Connection` | sanitized `reqwest` error text (no bodies) |
| Invalid tenant (not `^[A-Za-z0-9.-]+$`) | `Auth` | "Invalid Microsoft tenant" |
| Other non-2xx Graph responses, malformed JSON, missing next/delta link | `Protocol` | sanitized detail |

Access tokens, refresh tokens, authorization codes and PKCE verifiers are never
logged and never appear in any error message (only OAuth error *codes* and
Microsoft's own `error_description` text, which carry no secrets, are included).

## Testing

- Unit tests (in `mapping.rs`, `oauth.rs`, `provider.rs`) cover pure logic: OAuth
  error classification, message/profile JSON mapping, PKCE challenge computation,
  the authorize URL's query parameters, and delta URL construction.
- `crates/infra/tests/graph_oauth.rs` (wiremock): token exchange form fields;
  `invalid_grant` and `AADSTS65001` mapping; refresh reusing the passed-in refresh token
  when the response has none; invalid tenants rejected by `refresh` and `sign_in` without
  network calls or opening the browser; a full `sign_in` run where the
  `open_browser` callback performs the redirect GET itself (parsing
  `redirect_uri`/`state` out of the authorize URL) against a mocked token endpoint;
  a mismatched-`state` case that must never reach the token endpoint.
- `crates/infra/tests/graph_provider.rs` (wiremock): `test_connection` refreshing
  and calling `/me`; `signed_in_identity` returning both `mail` and
  `userPrincipalName` and mapping a 401 to `Auth`; the missing-refresh-token case; access-token caching across
  two calls (token endpoint hit once); a refresh without rotation leaving the stored
  token untouched (spy `SecretStore` counts `set` calls); an unmappable message skipped
  while the rest and the cursor are returned; a fallback-path cursor still dropping old
  messages on a later call; oldest-first ordering; the page-aligned cap across four
  pages; a slow response timing out (`set_delay` with a 50 ms timeout set through the
  test-only `with_request_timeout` builder); the first request's `$filter`/`Prefer`
  header and full message mapping; `nextLink` → `deltaLink` pagination; resuming
  directly from a cursor link; `@removed` entries ignored; the 200-message cap
  (asserting the *unfetched* `nextLink` becomes the cursor); the 400-without-filter
  fallback with client-side discarding of older messages; a 401 mapped to `Auth`.
- `crates/infra/tests/fixtures/graph/` holds a realistic message payload
  (`message_exam_change.json`, a Spanish exam-reschedule notice) and a token
  response (`token_response.json`, unused by the current tests but kept for future
  ones that want a canned success body).
- Unit tests in `loopback.rs`: other paths get 404 while the listener keeps waiting, and
  an idle connection does not block a later valid redirect. `TokenSet` `Debug` redaction
  and tenant validation are unit-tested in `mod.rs` and `oauth.rs`.

## Known limitations

- `LoopbackServer` implements just enough of HTTP/1.1 to read a `GET` request line
  and respond; it is only ever used for the one local redirect from the system
  browser and is not a general-purpose HTTP server.
- The loopback listener has no configurable timeout constant exposed from this
  module — the caller (`src-tauri`) passes the timeout to `oauth::sign_in`
  (spec §8.2 recommends 5 minutes).
- `fetch_new` does not special-case a `SyncCursor::Imap` cursor being passed in by
  mistake (a caller bug outside this provider's control): it is treated the same
  as no cursor (a fresh, filtered first request).
