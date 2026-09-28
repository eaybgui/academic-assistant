//! Recognizers for relative date expressions: `hoy`/`today`, `mañana`/`tomorrow`,
//! `pasado mañana` and `(el) próximo <weekday>` / `next <weekday>`.

use std::sync::OnceLock;

use regex::Regex;

use super::vocab::{self, WEEKDAYS_ALT};
use super::{RawMatch, RelativeKind, Seed};

pub(super) fn find(text: &str) -> Vec<RawMatch> {
    let mut out = Vec::new();
    find_day_after_tomorrow(text, &mut out);
    find_next_weekday(text, &mut out);
    find_manana_or_tomorrow(text, &mut out);
    find_today(text, &mut out);
    out
}

fn find_day_after_tomorrow(text: &str, out: &mut Vec<RawMatch>) {
    for m in day_after_tomorrow_re().find_iter(text) {
        out.push(RawMatch::new(
            m.start(),
            m.end(),
            Seed::Relative(RelativeKind::DayAfterTomorrow),
        ));
    }
}

fn find_next_weekday(text: &str, out: &mut Vec<RawMatch>) {
    for caps in next_weekday_re().captures_iter(text) {
        let m = caps.get(0).unwrap();
        if let Some(weekday) = vocab::weekday_from_name(&caps[1]) {
            out.push(RawMatch::new(
                m.start(),
                m.end(),
                Seed::Relative(RelativeKind::NextWeekday(weekday)),
            ));
        }
    }
}

/// `mañana`/`tomorrow` = +1 day, except Spanish `mañana` when it means "morning"
/// rather than "tomorrow": excluded when immediately preceded by `la` or `esta`
/// (covers `la mañana`, `esta mañana`, `por la mañana`, `de la mañana` — spec
/// §6.3 — since in each case the word directly before `mañana` is `la` or `esta`).
fn find_manana_or_tomorrow(text: &str, out: &mut Vec<RawMatch>) {
    for m in manana_or_tomorrow_re().find_iter(text) {
        if is_manana(m.as_str()) && has_excluded_prefix(text, m.start()) {
            continue;
        }
        out.push(RawMatch::new(
            m.start(),
            m.end(),
            Seed::Relative(RelativeKind::Tomorrow),
        ));
    }
}

fn is_manana(matched: &str) -> bool {
    matched.to_lowercase().starts_with('m')
}

fn has_excluded_prefix(text: &str, start: usize) -> bool {
    excluded_manana_prefix_re().is_match(&text[..start])
}

fn find_today(text: &str, out: &mut Vec<RawMatch>) {
    for m in today_re().find_iter(text) {
        out.push(RawMatch::new(
            m.start(),
            m.end(),
            Seed::Relative(RelativeKind::Today),
        ));
    }
}

fn day_after_tomorrow_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\bpasado\s+ma[nñ]ana\b").unwrap())
}

fn next_weekday_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)\b(?:el\s+)?(?:pr[oó]ximo|pr[oó]xima|next)\s+({WEEKDAYS_ALT})\b"
        ))
        .unwrap()
    })
}

fn manana_or_tomorrow_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(ma[nñ]ana|tomorrow)\b").unwrap())
}

fn excluded_manana_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(?:la|esta)\s*$").unwrap())
}

fn today_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)\b(hoy|today)\b").unwrap())
}
