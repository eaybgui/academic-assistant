# `academic-core`: normalization and filters

Two pure modules used by the pipeline before an email reaches AI analysis:

- `crate::normalize` — turns a raw [`Email`](./domain-and-ports.md#emails-emailrs)'s body
  into plain text suitable for hashing and for the analyzer's prompt.
- `crate::filters` — deterministic, non-AI rules that decide whether a normalized
  email is worth sending to the analyzer at all.

Both modules are pure: no I/O, no logging, no dependency on `academic-infra`. They are
called from `core::pipeline::CheckRunner` (Task T12) in this order, per message:
`normalize_body` → `content_hash` (for dedup, see below) → `filters::apply`.

## `normalize` (`crates/core/src/normalize.rs`)

### Public API

```rust
pub const MAX_BODY_CHARS: usize = 12_000;
pub fn normalize_body(body: &EmailBody) -> String;
pub fn content_hash(sender_email: &str, subject: &str, normalized_body: &str) -> String;
```

`normalize_body` converts an [`EmailBody`] to a normalized plain-text string in five
steps, always in this order:

1. **HTML → text.** `EmailBody::Text(s)` is used as-is; `EmailBody::Html(s)` is rendered
   with [`html2text::from_read`](https://docs.rs/html2text) at a wide column width
   (2,000), chosen so ordinary paragraphs are not hard-wrapped mid-sentence. `html2text`
   reads from an in-memory byte slice, so the only realistic failure is a pathologically
   narrow width (not the case here); any error falls back to an empty string rather than
   panicking, since `core` must never panic on adversarial provider content.
2. **Strip quoted replies.** Any line starting with `>` is dropped wherever it occurs.
   Separately, the first line that matches one of the following "quote block" markers
   ends the message — that line and everything after it is discarded:
   - A line (trimmed) case-insensitively equal to `-----Original Message-----`.
   - A line matching `On .+ wrote:` (case-insensitive; English clients).
   - A line matching `El .+ escribi[oó]:` (case-insensitive; Spanish clients; accent on
     `escribió` optional).
   - A line starting with `De:` or `From:` (case-insensitive) that is followed, within
     the next 5 lines, by a line starting with `Enviado:`, `Enviado el:` or `Sent:`
     (case-insensitive) — an Outlook-style forwarded/replied header block
     (`De:`/`Enviado:`/`Para:`/`Asunto:` or `From:`/`Sent:`/`To:`/`Subject:`). Markdown
     bold markers (`**`) are stripped from each line before matching, so a header block
     `html2text` has rendered from bold HTML (e.g. `**De:**` / `**Enviado:**`) is still
     recognized.
   These removals combine correctly regardless of which marker appears first in the
   text, since each is a monotonic truncation (the result always keeps only the text
   before the earliest marker).
3. **Strip signature.** Everything from a line that is *exactly* `-- ` (RFC 3676 sig
   delimiter: two dashes and a trailing space) onward is dropped. This runs on the
   output of step 2, before whitespace is collapsed, because collapsing would trim the
   delimiter's trailing space and make it unmatchable.
4. **Collapse whitespace.** Every run of one or more consecutive spaces/tabs within a
   line (leading, trailing or internal — including a lone tab) is collapsed to a single
   space; runs of two or more consecutive blank lines are collapsed to at most one
   blank line; the whole result is trimmed of leading/trailing blank lines.
5. **Truncate.** The text is cut to at most `MAX_BODY_CHARS` (12,000) `char`s — counted
   as Unicode scalar values, never bytes, so a multibyte character is never split.

`content_hash(sender_email, subject, normalized_body)` returns the lowercase hex SHA-256
digest of `lowercase(sender_email) + "\n" + trim(subject) + "\n" + normalized_body`. It
is deterministic and used by the pipeline (Task T12) to detect duplicate messages across
syncs (`processed_messages.content_hash`, see §5.1 of the design spec). Only the sender
is lowercased (not trimmed) and only the subject is trimmed, matching the contract in
the implementation plan exactly; callers that need a trimmed sender should trim before
calling.

### Edge cases

- A body with no quote/signature markers is unaffected save for whitespace collapsing
  and truncation.
- `EmailBody::Html("")` normalizes to `""`.
- Two emails with the same body but different sender casing hash identically; different
  bodies (or subjects, after trimming) hash differently.

## `filters` (`crates/core/src/filters.rs`)

### Public API

```rust
pub struct FilterConfig { pub blocked: Vec<String>, pub own_address: String }
pub enum FilterDecision { Keep, Discard(FilterReason) }
pub fn apply(email: &Email, normalized_body: &str, config: &FilterConfig) -> FilterDecision;
```

`FilterReason` (`Duplicate | BlockedSender | BlockedDomain | OwnMessage | Empty`) is
defined in `crate::domain` (`domain/results.rs`) since it is also used outside
filtering, as the `processed_messages.reason` value for `duplicate` (a dedup outcome
computed by the pipeline before filters ever run — `filters::apply` itself never
produces `FilterReason::Duplicate`).

`apply` checks, in this fixed order, stopping at the first match:

1. **Blocked sender.** Any `config.blocked` entry that does *not* start with `@` and,
   case-insensitively and trimmed, equals the sender's address exactly →
   `Discard(BlockedSender)`.
2. **Blocked domain.** Any entry starting with `@` whose remainder, case-insensitively
   and trimmed, equals the sender address's domain *exactly* → `Discard(BlockedDomain)`.
   `@uv.es` blocks `x@uv.es` but **not** `x@sub.uv.es` (no subdomain matching).
3. **Own message.** The sender address equals `config.own_address` (case-insensitive,
   trimmed) → `Discard(OwnMessage)`.
4. **Empty.** `email.subject` and `normalized_body` are both empty after trimming →
   `Discard(Empty)`.

Otherwise: `Keep`. Blank entries in `config.blocked` (empty after trimming) are ignored.
`apply` takes the already-normalized body as a parameter rather than normalizing it
itself, so callers only normalize once per message and reuse the result for both
`content_hash` and filtering.

### Edge cases

- `config.blocked` containing both an exact address and its domain: the exact-address
  check runs first, so the reported reason is `BlockedSender`, not `BlockedDomain`.
- `config.own_address` empty (e.g. account not yet configured): the own-message check
  never matches, since an empty pattern would otherwise match blank senders spuriously.

## Errors

Neither module returns a `Result`: both are pure, total functions over their inputs.
`normalize_body` cannot fail (HTML rendering errors degrade to an empty string rather
than propagating); `apply` cannot fail (it always returns a `FilterDecision`).

## How to test

```
cargo test -p academic-core --lib normalize
cargo test -p academic-core --lib filters
```

Unit tests live in `#[cfg(test)] mod tests` at the bottom of each file and cover: HTML→
text conversion; `>`-line removal; each quote-block marker, including the Spanish
`Enviado el:` spelling, the English `From:`/`Sent:` pair, and both `De:`/`Enviado:` and
`From:`/`Sent:` rendered as bold Markdown; signature removal; whitespace collapsing
(both blank-line runs and in-line space/tab runs); multibyte-safe truncation at
`MAX_BODY_CHARS`; hash stability/case-insensitivity/body-sensitivity; every
`FilterReason`; the `Keep` case; domain-vs-subdomain matching; blank blocked-entry
handling; and filter-order precedence.

## Known limitations

- Quote-block detection is line-pattern based, not a full email-thread parser; unusual
  or localized mail-client reply headers other than the patterns listed above are not
  recognized and their quoted content is not stripped (only bare `>`-prefixed lines
  within them are removed).
- `html2text` rendering (e.g. how tables, links or emphasis are flattened to text) is
  whatever the `html2text` 0.17 default (non-rich) decorator produces; this module does
  not attempt to normalize that output beyond the whitespace-collapsing step above.
