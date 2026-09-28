//! Date candidate extraction from email subject and body text (Task T4).
//!
//! [`extract`] scans a subject then a body for date mentions — absolute
//! (`27/10/2026`, `27 de octubre`, `October 27`, ...) and relative (`mañana`,
//! `próximo lunes`, ...) — resolves each to a concrete [`NaiveDate`], attaches a
//! nearby time when present, and returns them as [`DateCandidate`]s for the AI
//! analyzer (Task T5) to confirm and classify. This module never calls the system
//! clock: every relative date is computed from the caller-supplied `received_at`.
//!
//! The work is split into:
//! - [`absolute`]: numeric and month-name absolute date recognizers.
//! - [`relative`]: `hoy`/`mañana`/`pasado mañana`/`próximo <weekday>` and their
//!   English equivalents.
//! - [`time`]: time-of-day recognizer, attached to a date when it follows within
//!   20 characters.
//! - [`resolve`]: missing-year inference and "next weekday" computation.
//! - [`vocab`]: the Spanish/English month and weekday name tables both of the
//!   above recognizer modules draw on.

mod absolute;
mod relative;
mod resolve;
mod time;
mod vocab;

#[cfg(test)]
mod tests;

use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime, Weekday};

use crate::domain::DateCandidate;

/// Maximum number of candidates [`extract`] returns, in order of appearance,
/// after deduplication (spec §6.3).
pub const MAX_CANDIDATES: usize = 20;

/// A raw, unresolved date mention found by a recognizer, before year inference,
/// time attachment and validation.
struct RawMatch {
    start: usize,
    end: usize,
    seed: Seed,
}

impl RawMatch {
    fn new(start: usize, end: usize, seed: Seed) -> Self {
        Self { start, end, seed }
    }
}

/// What a [`RawMatch`] needs resolved into a [`NaiveDate`].
enum Seed {
    /// `year` is `None` when the text did not include one (spec §6.3: missing
    /// year is inferred from `received_at`).
    Absolute {
        day: u32,
        month: u32,
        year: Option<i32>,
    },
    Relative(RelativeKind),
}

enum RelativeKind {
    Today,
    Tomorrow,
    DayAfterTomorrow,
    NextWeekday(Weekday),
}

/// Scans `subject` then `body` for date mentions and returns them as
/// [`DateCandidate`]s, in order of appearance, deduplicated by `(date, time)`
/// (first occurrence wins) and capped at [`MAX_CANDIDATES`].
///
/// `received_at` is the email's received time in the user's local offset; it is
/// the only source of "now" this function uses (missing years and relative
/// expressions like `mañana` or `próximo lunes` are resolved against
/// `received_at.date_naive()`).
pub fn extract(
    subject: &str,
    body: &str,
    received_at: DateTime<FixedOffset>,
) -> Vec<DateCandidate> {
    let received = received_at.date_naive();
    let mut candidates = Vec::new();
    extract_from_text(subject, received, &mut candidates);
    extract_from_text(body, received, &mut candidates);
    dedup_and_cap(candidates)
}

/// Finds every non-overlapping date mention in `text`, resolves it to a date,
/// attaches a following time when present, and appends the resulting candidates
/// (still possibly containing cross-text duplicates) to `out`.
fn extract_from_text(text: &str, received: NaiveDate, out: &mut Vec<DateCandidate>) {
    let mut raw = absolute::find(text);
    raw.extend(relative::find(text));
    // Earliest start first; on a tie, the longer (more specific) match first.
    raw.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

    let mut cursor = 0usize;
    for (i, m) in raw.iter().enumerate() {
        if m.start < cursor {
            continue; // overlaps a match already kept (or already ruled invalid)
        }

        let Some(date) = resolve_date(&m.seed, received) else {
            cursor = m.end;
            continue; // invalid calendar date (e.g. 31/02) — dropped, spec §6.3
        };

        // A time may not be attached past the start of the next raw date match, so
        // that a time meant for a later date is never stolen by this one (which
        // would also make that later date look like it overlaps and get skipped).
        let time_boundary = raw[i + 1..]
            .iter()
            .find(|next| next.start >= m.end)
            .map_or(text.len(), |next| next.start);

        let mut span_end = m.end;
        let mut time: Option<NaiveTime> = None;
        if let Some(found) = time::attach(text, m.end, time_boundary) {
            time = Some(found.time);
            span_end = found.end;
        }

        out.push(DateCandidate {
            span: text[m.start..span_end].to_string(),
            date,
            time,
        });
        cursor = span_end;
    }
}

fn resolve_date(seed: &Seed, received: NaiveDate) -> Option<NaiveDate> {
    match *seed {
        Seed::Absolute {
            day,
            month,
            year: Some(year),
        } => NaiveDate::from_ymd_opt(year, month, day),
        Seed::Absolute {
            day,
            month,
            year: None,
        } => resolve::infer_year(day, month, received),
        Seed::Relative(RelativeKind::Today) => Some(received),
        Seed::Relative(RelativeKind::Tomorrow) => received.succ_opt(),
        Seed::Relative(RelativeKind::DayAfterTomorrow) => {
            received.succ_opt().and_then(|d| d.succ_opt())
        }
        Seed::Relative(RelativeKind::NextWeekday(weekday)) => {
            Some(resolve::next_weekday_after(received, weekday))
        }
    }
}

/// Keeps the first occurrence of each `(date, time)` pair, in order, up to
/// [`MAX_CANDIDATES`].
fn dedup_and_cap(candidates: Vec<DateCandidate>) -> Vec<DateCandidate> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for candidate in candidates {
        if seen.insert((candidate.date, candidate.time)) {
            out.push(candidate);
            if out.len() == MAX_CANDIDATES {
                break;
            }
        }
    }
    out
}
