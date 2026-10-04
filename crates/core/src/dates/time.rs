//! Recognizes a time expression attached to a date: `10:00`, `10.30`, `10h`,
//! `10:00h`, `10:00 h`, `a las 10`, `a las 10:30`, English `10am`/`3 pm`.

use std::sync::OnceLock;

use chrono::NaiveTime;
use regex::{Match, Regex};

pub(super) struct TimeMatch {
    pub end: usize,
    pub time: NaiveTime,
}

/// Looks for a time expression starting within 20 characters after `after` (a
/// byte offset into `text`, the end of the date match it would attach to — spec
/// §6.3). The search never crosses `boundary` (the start of the next raw date
/// match, or `text.len()` when there is none), so a time meant for a later date
/// is never attached to this one. Returns the one starting earliest, or `None`.
pub(super) fn attach(text: &str, after: usize, boundary: usize) -> Option<TimeMatch> {
    let window_end = char_offset(text, after, 40).min(boundary);
    let window = &text[after..window_end];

    let mut candidates = Vec::new();
    candidates.extend(a_las_matches(window));
    candidates.extend(colon_or_dot_matches(window));
    candidates.extend(hour_only_matches(window));
    candidates.extend(am_pm_matches(window));

    let best = candidates
        .into_iter()
        .filter(|(m, _)| window[..m.start()].chars().count() <= 20)
        .min_by_key(|(m, _)| (m.start(), std::cmp::Reverse(m.len())))?;

    let (m, time) = best;
    Some(TimeMatch {
        end: after + m.end(),
        time,
    })
}

/// Byte offset `chars` characters after `from` in `text`, clamped to `text.len()`.
fn char_offset(text: &str, from: usize, chars: usize) -> usize {
    text[from..]
        .char_indices()
        .nth(chars)
        .map(|(i, _)| from + i)
        .unwrap_or(text.len())
}

fn a_las_matches(window: &str) -> Vec<(Match<'_>, NaiveTime)> {
    a_las_re()
        .captures_iter(window)
        .filter_map(|c| {
            let hour: u32 = c[1].parse().ok()?;
            let minute: u32 = match c.get(2) {
                Some(g) => g.as_str().parse().ok()?,
                None => 0,
            };
            let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
            Some((c.get(0).unwrap(), time))
        })
        .collect()
}

fn colon_or_dot_matches(window: &str) -> Vec<(Match<'_>, NaiveTime)> {
    colon_or_dot_re()
        .captures_iter(window)
        .filter_map(|c| {
            let hour: u32 = c[1].parse().ok()?;
            let minute: u32 = c[2].parse().ok()?;
            let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
            Some((c.get(0).unwrap(), time))
        })
        .collect()
}

fn hour_only_matches(window: &str) -> Vec<(Match<'_>, NaiveTime)> {
    hour_only_re()
        .captures_iter(window)
        .filter_map(|c| {
            let hour: u32 = c[1].parse().ok()?;
            let time = NaiveTime::from_hms_opt(hour, 0, 0)?;
            Some((c.get(0).unwrap(), time))
        })
        .collect()
}

fn am_pm_matches(window: &str) -> Vec<(Match<'_>, NaiveTime)> {
    am_pm_re()
        .captures_iter(window)
        .filter_map(|c| {
            let hour12: u32 = c[1].parse().ok()?;
            let minute: u32 = match c.get(2) {
                Some(g) => g.as_str().parse().ok()?,
                None => 0,
            };
            let hour = to_24h(hour12, &c[3])?;
            let time = NaiveTime::from_hms_opt(hour, minute, 0)?;
            Some((c.get(0).unwrap(), time))
        })
        .collect()
}

fn to_24h(hour12: u32, meridiem: &str) -> Option<u32> {
    if !(1..=12).contains(&hour12) {
        return None;
    }
    Some(match (hour12, meridiem.to_lowercase().as_str()) {
        (12, "am") => 0,
        (h, "am") => h,
        (12, "pm") => 12,
        (h, "pm") => h + 12,
        _ => return None,
    })
}

fn a_las_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\ba\s+las\s+(\d{1,2})(?:[:.](\d{2}))?\b").unwrap())
}

fn colon_or_dot_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // The trailing `h` (`10:00h`, `10:00 h`) is grouped as one optional unit so
    // that, when there is no `h`, no trailing whitespace is consumed either.
    RE.get_or_init(|| Regex::new(r"\b(\d{1,2})[:.](\d{2})(?:\s*h)?\b").unwrap())
}

fn hour_only_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(\d{1,2})h\b").unwrap())
}

fn am_pm_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(\d{1,2})(?::(\d{2}))?\s*(am|pm)\b").unwrap())
}
