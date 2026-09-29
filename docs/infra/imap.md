# `academic-infra::imap`: IMAP email provider

Implements the `academic-core` [`EmailProvider`](../core/domain-and-ports.md#email-providers-emailrs) port for
plain IMAP accounts (`crates/infra/src/imap/`). Connects over implicit TLS, never marks
messages `\Seen` (`BODY.PEEK[]`), and does incremental sync by tracking the highest UID
already fetched.

Module layout (`crates/infra/src/imap/`):

```
mod.rs          ImapEmailProvider (EmailProvider impl), fetch orchestration
connection.rs   TCP + TLS + LOGIN, async-imap error -> ProviderError mapping
parse.rs        parse_message / parse_fetched_messages: raw RFC 822 bytes -> Email
cursor.rs       select_uids: pure UID batch planning
```

## Public API

```rust
pub struct ImapEmailProvider { /* private */ }

impl ImapEmailProvider {
    pub fn new(
        account_id: AccountId,
        host: String,
        port: u16,
        username: String,
        password: String,
    ) -> Self;
}

impl EmailProvider for ImapEmailProvider {
    async fn test_connection(&self) -> Result<(), ProviderError>;
    async fn fetch_new(
        &self,
        folder: &str,
        cursor: Option<&SyncCursor>,
        since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError>;
}

/// Pure parsing, used by the provider and tested with `.eml` fixtures.
pub fn parse_message(
    account_id: AccountId,
    folder: &str,
    uid: u32,
    raw: &[u8],
    internal_date: Option<DateTime<Utc>>,
) -> Result<Email, ProviderError>;

/// Pure per-batch parsing: skips unparseable messages (logging only the UID), returns
/// the emails oldest first plus the highest fetched UID (the cursor high-water mark).
pub fn parse_fetched_messages(
    account_id: AccountId,
    folder: &str,
    fetched: &[FetchedMessage],
) -> (Vec<Email>, Option<u32>);

/// `(uid, raw RFC 822 bytes, INTERNALDATE)`.
pub type FetchedMessage = (u32, Vec<u8>, Option<DateTime<Utc>>);

/// Pure cursor planning, tested without a server.
pub fn select_uids(found: &[u32], cursor_last_uid: Option<u32>, limit: usize) -> Vec<u32>;
```

`ImapEmailProvider` holds the plaintext password in memory only for the lifetime of the
value (it is supplied by the caller — `academic-infra`'s factory reads it from
`SecretStore` — never read or written by this module). Its `Debug` impl redacts the
password field; it is never included in error messages or logs.

## Connection (`connection.rs`)

1. TCP-connects to `host:port` with a 20 s timeout.
2. Wraps the socket in a TLS client stream (`tokio-rustls`, root store from
   `webpki-roots`, `rustls`'s default crypto provider — already selected process-wide by
   the `aws-lc-rs` feature enabled transitively through `reqwest`'s `rustls` feature); the handshake has its own 20 s timeout.
3. Reads the server greeting (20 s timeout; a timeout is `Connection("greeting from host:port timed out")`), then issues `LOGIN username password`.

Error mapping (the password itself is never included in any error):

| Situation | `ProviderError` |
|---|---|
| `LOGIN` rejected (`NO` response) | `Auth(message)` |
| TCP/TLS connect or handshake failure or timeout, connection lost | `Connection(message)` |
| Anything else (`BAD`, parse/validate failures) | `Protocol(message)` |

The same mapping (minus the `NO -> Auth` case, since only `LOGIN` carries that meaning)
applies to every other IMAP command (`SELECT`, `UID SEARCH`, `UID FETCH`, `LOGOUT`) via
`map_command_error`.

### I/O timeouts

Every command after the connection is established is bounded by a timeout, and a
timeout maps to `ProviderError::Connection` (`"<command> timed out"`; for `LOGIN` the
message is `"LOGIN to host:port timed out"`):

| Command | Timeout |
|---|---|
| `LOGIN` | 60 s |
| `SELECT`, `UID SEARCH` | 60 s each |
| `UID FETCH` (issuing it and consuming the whole response stream, not per message) | 60 s total |
| `LOGOUT` (best effort, result ignored) | 5 s |

## Fetch algorithm (`mod.rs`)

`fetch_new(folder, cursor, since)`:

1. Connect and log in (`connection::connect`).
2. `SELECT folder`. If the mailbox's `UIDVALIDITY` differs from the cursor's (or the
   cursor isn't an `Imap` cursor), the cursor is treated as absent — the mailbox was
   recreated, so its UIDs mean something else now. Message-level deduplication
   (`ProcessedMessageRepository`, outside this crate) is what makes re-fetching safe.
3. Search:
   - No effective cursor: `UID SEARCH SINCE <dd-Mon-yyyy>` (English month abbreviation,
     from `since`, formatted with `chrono`'s default `%b`, which is locale-independent).
   - With a cursor (`last_uid`): `UID SEARCH UID {last_uid + 1}:*`.
4. `select_uids` picks the UIDs to fetch this round: filters out anything `<= last_uid`
   (this is what absorbs the `*` quirk — a `last_uid+1:*` search on a mailbox with no
   new mail can still return the single highest existing UID, which must not be
   re-fetched), sorts ascending, and caps at `MAX_BATCH` (200).
5. If nothing was selected: the next cursor is the current one unchanged when a cursor
   was in effect, or `{uidvalidity, last_uid: UIDNEXT - 1}` when starting fresh (so the
   next run searches strictly from the current end of the mailbox).
6. Otherwise: `UID FETCH {uids} (UID INTERNALDATE BODY.PEEK[])` — `BODY.PEEK[]` never
   sets `\Seen`. The raw responses are collected and handed to `parse_fetched_messages`.
   A message that fails to parse is **skipped** (a `tracing::warn!` with only its UID,
   never its content) instead of failing the batch, so one malformed message cannot
   block the folder forever. Every fetched UID, parsed or skipped, counts toward the
   cursor: the next cursor is `{uidvalidity, last_uid: max(uid fetched)}`. Returned
   emails are sorted oldest first (`received_at`, ties by provider message id), per the
   `EmailProvider` contract.
7. `LOGOUT` is always attempted (best effort, 5 s timeout — its result is ignored) before
   returning, on every path, success or failure.

`source_url` is always `None` for IMAP (there is no equivalent of Graph's `webLink`).

## Message parsing (`parse.rs`)

`parse_message` uses `mail-parser` to turn the raw bytes into an `academic_core::domain::Email`:

- **Sender** (`Address`): first entry of the `From` header (name + address). Missing
  `From`, or a `From` with no email address, is a `Protocol` error — the pipeline cannot
  process a message without a sender.
- **Recipients**: `To` entries followed by `Cc` entries, in order; entries without a
  usable email address are silently skipped.
- **Subject**: `Subject` header, decoded (including RFC 2047 encoded-words, e.g.
  `=?UTF-8?B?...?=`, which `mail-parser` decodes automatically); empty string if absent.
- **Received time**: `internal_date` (the IMAP `INTERNALDATE`, fetched alongside the
  body) when present; otherwise the message's `Date` header, converted to UTC. Both
  missing is a `Protocol` error.
- **Body**: prefers a genuine `text/plain` part; falls back to `text/html` when no plain
  part exists; as a last resort defers to `mail-parser`'s own text/HTML cross-derivation
  (covers unusual MIME shapes with neither a direct `Text` nor `Html` part indexed).
- **`message_id_header`**: the `Message-ID` header without angle brackets (`mail-parser`
  strips them during parsing); `None` when the header is absent.
- **`provider_message_id`**: the IMAP `uid`, stringified.
- **`source_url`**: always `None`.
- Malformed input that `mail-parser` cannot find any headers in is a `Protocol` error.

## Cursor planning (`cursor.rs`)

`select_uids(found, cursor_last_uid, limit)` is a pure function: filters `found` to UIDs
strictly greater than `cursor_last_uid` (when present), sorts ascending, and truncates to
`limit`. It has no I/O and is exhaustively unit-tested (`tests/imap_cursor.rs`) without a
server.

## Batch parsing (`parse.rs`, `parse_fetched_messages`)

Pure aside from the warning log. Input: one `(uid, raw bytes, internal date)` triple per
fetched message. Output: `(emails, max_uid)`, where `max_uid` covers every input UID
(including skipped ones) and `emails` are sorted by `(received_at, provider_message_id)`.

## Testing

- `tests/fixtures/eml/*.eml` — fictional Spanish messages (`uv.es`/`example.com`
  domains) covering: plain-text quoted-printable with accented characters, HTML-only,
  multipart/alternative (plain + HTML), an RFC 2047 encoded-word subject, a message
  missing `Message-ID`, and `Cc` recipients.
- `tests/imap_parse.rs` — `parse_message` against every fixture above, plus the error
  paths (unparsable bytes, missing `From`, missing date everywhere).
- `tests/imap_parse.rs` also covers `parse_fetched_messages`: a bad message in the middle
  is skipped while the others are returned and `max_uid` still advances past it; all
  messages failing still yields the high-water mark; oldest-first ordering with the
  provider-id tie-break.
- `tests/imap_cursor.rs` — `select_uids`: no cursor, cursor filtering (including the `*`
  quirk), the `MAX_BATCH` limit, and an empty `found` list.
- `tests/imap_live.rs` — `#[ignore]`d live smoke test (`test_connection` + `fetch_new`
  over the last 2 days) against a real server, driven by the `IMAP_HOST`, `IMAP_USER`,
  `IMAP_PASSWORD` (required) and `IMAP_PORT` (optional, default 993) environment
  variables:

  ```text
  IMAP_HOST=imap.example.com IMAP_USER=me@example.com IMAP_PASSWORD=secret \
      cargo test -p academic-infra --test imap_live -- --ignored
  ```

## Known limitations

- No `IDLE`/push support — sync is always a one-shot `fetch_new` call, matching the
  `EmailProvider` port.
- STARTTLS (explicit TLS upgrade on port 143) is not implemented, only implicit TLS
  (port 993).
- A server that omits `UIDVALIDITY` or `UIDNEXT` on `SELECT` (required by RFC 3501 but
  not universally honored by broken servers) causes a `Protocol` error rather than a
  degraded fallback.
