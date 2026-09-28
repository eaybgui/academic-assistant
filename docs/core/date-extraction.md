# `core::dates`: date candidate extraction

`crates/core/src/dates/` (Task T4) scans an email's subject and body for date
mentions and turns each into a [`DateCandidate`](../core/domain-and-ports.md)
(`{ span, date, time }`) for `core::analyzer` (Task T5) to confirm and classify.
It is pure: no I/O, no system clock. Every relative expression (`mañana`,
`próximo lunes`, ...) and every missing year is resolved from the caller-supplied
`received_at`, never from the machine's current time.

## Public API

```rust
pub const MAX_CANDIDATES: usize = 20;

pub fn extract(subject: &str, body: &str, received_at: DateTime<FixedOffset>) -> Vec<DateCandidate>;
```

`received_at` is the email's received time in the user's local offset. `extract`
scans `subject` first, then `body`, and returns candidates in order of
appearance, deduplicated by `(date, time)` (first occurrence's span wins) and
capped at `MAX_CANDIDATES`.

## Module layout

- `absolute.rs` — numeric and month-name absolute date recognizers.
- `relative.rs` — `hoy`/`today`, `mañana`/`tomorrow`, `pasado mañana`,
  `(el) próximo|próxima <weekday>` / `next <weekday>`.
- `time.rs` — time-of-day recognizer, attached to a date when it appears within
  20 characters after it, never crossing into the start of the next raw date
  match (so a time meant for a later date is never attached to this one).
- `resolve.rs` — missing-year inference and "next weekday" computation.
- `vocab.rs` — the Spanish/English month and weekday name tables shared by the
  recognizers above.
- `mod.rs` — orchestration (`extract`) and the shared `RawMatch`/`Seed` types
  recognizers produce before resolution.

## Recognized formats

**Absolute dates** (day-first for ambiguous numeric formats):
- Numeric: `dd/mm`, `dd/mm/yy`, `dd/mm/yyyy`, `dd-mm-yyyy`, `yyyy-mm-dd`.
- Spanish month name: `d de <mes>`, `d de <mes> de yyyy` — month names `enero`
  … `diciembre` (`septiembre`/`setiembre` both accepted), case-insensitive.
- English month name: `<Month> d`, `<Month> d, yyyy`, `d <Month>`,
  `d <Month> yyyy` — full names and 3-letter abbreviations, case-insensitive.
- Any absolute date may be preceded by a weekday name (Spanish or English, full
  name only, optionally followed by a comma); when present, the weekday is kept
  in the returned `span` (e.g. `lunes 27 de octubre`). The weekday is not
  validated against the resolved date (a "Monday" that doesn't actually fall on
  a Monday in the resolved year is not rejected).

**Relative dates**, resolved against `received_at.date_naive()`:
- `hoy` / `today` → +0 days.
- `mañana` / `tomorrow` → +1 day. Spanish `mañana` is **not** treated as a date
  when it means "morning": excluded whenever the word immediately before it is
  `la` or `esta` (covers `la mañana`, `esta mañana`, `por la mañana`, `de la
  mañana` — in each phrase the word directly before `mañana` is `la` or `esta`).
- `pasado mañana` → +2 days.
- `(el) próximo|próxima <weekday>` / `next <weekday>` → the next date on that
  weekday strictly after `received_at`'s date; if today already is that
  weekday, this resolves to +7 days (never today).

**Times**, attached to a date when found within 20 characters after it:
`10:00`, `10.30`, `10h`, `10:00h`, `10:00 h`, `a las 10`, `a las 10:30`,
English `10am` / `3 pm`. Hours 0–23, minutes 0–59 (12-hour `am`/`pm` forms use
1–12). When present, the time is included in the candidate's `span` and in
`DateCandidate.time`. The 20-character window never crosses the start of the
next raw date match in the text — e.g. in "el 27 de octubre o el 28 de
octubre a las 10:00" the `10:00` attaches to 28 October, not 27 October, so
27 October is still returned (with no time) instead of being swallowed into
the first candidate's span and then dropped as a false overlap.

## Year resolution

- When the text gives an explicit year, it is used directly. A two-digit year
  (`dd/mm/yy`) maps to `2000 + yy`.
- When no year is given, the day/month is tried against the three candidate
  years `received_at.year() - 1 ..= received_at.year() + 1`. Among the
  calendar-valid candidates that fall on or after `received_at - 30 days`, the
  one closest to `received_at` is kept. The day/month is dropped entirely
  (the candidate is discarded, not resolved) when no calendar-valid candidate
  satisfies that cutoff — either because the day/month is invalid in all
  three candidate years (e.g. 29 February against three non-leap years), or
  because every calendar-valid candidate falls more than 30 days before
  `received_at`. Spec §6.3 requires the resolved date to never be earlier
  than 30 days before `received_at`, so there is no fallback outside the
  cutoff window.

## Validation, ordering, deduplication

- A date that fails `NaiveDate` construction in its resolved year (e.g.
  31 February) is dropped, regardless of whether the year was explicit or
  inferred.
- Overlapping recognizer matches are resolved by keeping the one that starts
  earliest; on a tie, the longer (more specific) one. A later match that starts
  before the previous kept match's end (including any attached time) is
  discarded rather than re-examined.
- After scanning subject then body, candidates are deduplicated by
  `(date, time)`: the first occurrence's `span` is kept, later ones dropped.
  The result is then capped at `MAX_CANDIDATES` (20), preserving order of
  appearance.

## Errors

None — `extract` cannot fail. Text that matches no recognizer, or whose only
matches resolve to invalid dates, yields an empty `Vec`.

## Testing

`crates/core/src/dates/tests.rs` covers (with
`received_at = 2026-09-28T09:00:00+02:00`, a Monday, unless otherwise noted):
every absolute format, the weekday-prefix span, every relative form (including
both "próximo lunes" landing next week and "próximo viernes" landing this same
week), every time form (including two adjacent dates where a trailing time must
attach only to the second one, for both absolute and relative dates), all four
`mañana`-as-morning exclusions, year inference wrapping forward and backward
across a new year, a same-year case, and a case where the only calendar-valid
candidate falls outside the 30-day cutoff and must be dropped rather than
returned; an invalid date being dropped, a duplicate date keeping the first
span, capping at 20 candidates with explicit-year dates, and subject being
scanned before body.
Run with `cargo test -p academic-core dates::`.

## Known limitations

- Weekday names in the leading-weekday-prefix and `próximo <weekday>` forms are
  full names only (no abbreviations like `lun.`).
- A resolved absolute date is not checked against its stated weekday prefix
  (see above) — the model only sees the span text and confirmed date.
- The English month/day recognizers do not disambiguate `may` (the month) from
  the English modal verb; this mirrors the inherent ambiguity of the word and
  is not addressed further, since it is outside the cases described in the
  design spec (§6.3).
