//! Recognizers for absolute date mentions: numeric (`dd/mm`, `dd-mm-yyyy`,
//! `yyyy-mm-dd`) and month-name (Spanish `d de <mes>`, English `<Month> d` /
//! `d <Month>`), each with an optional leading weekday kept in the span.

use std::sync::OnceLock;

use regex::Regex;

use super::vocab::{self, ENGLISH_MONTHS_ALT, SPANISH_MONTHS_ALT, WEEKDAYS_ALT};
use super::{RawMatch, Seed};

pub(super) fn find(text: &str) -> Vec<RawMatch> {
    let mut out = Vec::new();
    find_iso(text, &mut out);
    find_dmy_hyphen(text, &mut out);
    find_slash(text, &mut out);
    find_spanish_month_name(text, &mut out);
    find_english_month_day(text, &mut out);
    find_english_day_month(text, &mut out);
    for m in &mut out {
        extend_weekday_prefix(text, m);
    }
    out
}

fn find_iso(text: &str, out: &mut Vec<RawMatch>) {
    for caps in iso_re().captures_iter(text) {
        let m = caps.get(0).unwrap();
        let (Ok(year), Ok(month), Ok(day)) = (
            caps[1].parse::<i32>(),
            caps[2].parse::<u32>(),
            caps[3].parse::<u32>(),
        ) else {
            continue;
        };
        push(out, m.start(), m.end(), day, month, Some(year));
    }
}

fn find_dmy_hyphen(text: &str, out: &mut Vec<RawMatch>) {
    for caps in dmy_hyphen_re().captures_iter(text) {
        let m = caps.get(0).unwrap();
        let (Ok(day), Ok(month), Ok(year)) = (
            caps[1].parse::<u32>(),
            caps[2].parse::<u32>(),
            caps[3].parse::<i32>(),
        ) else {
            continue;
        };
        push(out, m.start(), m.end(), day, month, Some(year));
    }
}

fn find_slash(text: &str, out: &mut Vec<RawMatch>) {
    for caps in slash_re().captures_iter(text) {
        let m = caps.get(0).unwrap();
        let (Ok(day), Ok(month)) = (caps[1].parse::<u32>(), caps[2].parse::<u32>()) else {
            continue;
        };
        let year = caps.get(3).and_then(|g| parse_year(g.as_str()));
        push(out, m.start(), m.end(), day, month, year);
    }
}

/// Two-digit years map to `2000 + yy` (spec §6.3); four-digit years pass through.
fn parse_year(raw: &str) -> Option<i32> {
    let value: i32 = raw.parse().ok()?;
    Some(if raw.len() == 2 { 2000 + value } else { value })
}

fn find_spanish_month_name(text: &str, out: &mut Vec<RawMatch>) {
    for caps in spanish_month_re().captures_iter(text) {
        let m = caps.get(0).unwrap();
        let Ok(day) = caps[1].parse::<u32>() else {
            continue;
        };
        let Some(month) = vocab::spanish_month_number(&caps[2]) else {
            continue;
        };
        let year = caps.get(3).and_then(|g| g.as_str().parse::<i32>().ok());
        push(out, m.start(), m.end(), day, month, year);
    }
}

fn find_english_month_day(text: &str, out: &mut Vec<RawMatch>) {
    for caps in english_month_day_re().captures_iter(text) {
        let m = caps.get(0).unwrap();
        let Some(month) = vocab::english_month_number(&caps[1]) else {
            continue;
        };
        let Ok(day) = caps[2].parse::<u32>() else {
            continue;
        };
        let year = caps.get(3).and_then(|g| g.as_str().parse::<i32>().ok());
        push(out, m.start(), m.end(), day, month, year);
    }
}

fn find_english_day_month(text: &str, out: &mut Vec<RawMatch>) {
    for caps in english_day_month_re().captures_iter(text) {
        let m = caps.get(0).unwrap();
        let Ok(day) = caps[1].parse::<u32>() else {
            continue;
        };
        let Some(month) = vocab::english_month_number(&caps[2]) else {
            continue;
        };
        let year = caps.get(3).and_then(|g| g.as_str().parse::<i32>().ok());
        push(out, m.start(), m.end(), day, month, year);
    }
}

fn push(
    out: &mut Vec<RawMatch>,
    start: usize,
    end: usize,
    day: u32,
    month: u32,
    year: Option<i32>,
) {
    out.push(RawMatch::new(
        start,
        end,
        Seed::Absolute { day, month, year },
    ));
}

/// If `m` is immediately preceded (ignoring whitespace and an optional comma) by
/// a weekday name, pulls `m.start` back to include it, so the returned span keeps
/// the weekday (spec §6.3: "optional leading weekday").
fn extend_weekday_prefix(text: &str, m: &mut RawMatch) {
    if let Some(found) = weekday_prefix_re().find(&text[..m.start]) {
        m.start = found.start();
    }
}

fn iso_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(\d{4})-(\d{2})-(\d{2})\b").unwrap())
}

fn dmy_hyphen_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(\d{2})-(\d{2})-(\d{4})\b").unwrap())
}

fn slash_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\b(\d{1,2})/(\d{1,2})(?:/(\d{2,4}))?\b").unwrap())
}

fn spanish_month_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)\b(\d{{1,2}})\s+de\s+({SPANISH_MONTHS_ALT})(?:\s+de\s+(\d{{4}}))?\b"
        ))
        .unwrap()
    })
}

fn english_month_day_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)\b({ENGLISH_MONTHS_ALT})\s+(\d{{1,2}})(?:,?\s+(\d{{4}}))?\b"
        ))
        .unwrap()
    })
}

fn english_day_month_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(&format!(
            r"(?i)\b(\d{{1,2}})\s+({ENGLISH_MONTHS_ALT})(?:\s+(\d{{4}}))?\b"
        ))
        .unwrap()
    })
}

fn weekday_prefix_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(&format!(r"(?i)\b(?:{WEEKDAYS_ALT})\s*,?\s*$")).unwrap())
}
