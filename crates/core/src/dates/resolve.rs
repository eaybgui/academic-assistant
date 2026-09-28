//! Turns a recognized day/month(/year) or relative expression into a concrete,
//! validated [`chrono::NaiveDate`].

use chrono::{Datelike, Duration, NaiveDate, Weekday};

/// Resolves a day/month with no explicit year: tries the three candidate years
/// `received.year() - 1 ..= received.year() + 1` (spec §6.3), keeping the
/// calendar-valid candidate closest to `received` among those that fall on or
/// after `received - 30 days`. Falls back to the closest calendar-valid candidate
/// outside that window when none qualify, and returns `None` only when the
/// day/month combination is invalid in all three candidate years (e.g. 29
/// February against three non-leap years).
pub(super) fn infer_year(day: u32, month: u32, received: NaiveDate) -> Option<NaiveDate> {
    let cutoff = received - Duration::days(30);
    let mut within_cutoff: Option<NaiveDate> = None;
    let mut any: Option<NaiveDate> = None;

    for year in [received.year() - 1, received.year(), received.year() + 1] {
        let Some(candidate) = NaiveDate::from_ymd_opt(year, month, day) else {
            continue;
        };
        if is_closer(candidate, any, received) {
            any = Some(candidate);
        }
        if candidate >= cutoff && is_closer(candidate, within_cutoff, received) {
            within_cutoff = Some(candidate);
        }
    }
    within_cutoff.or(any)
}

fn is_closer(candidate: NaiveDate, current_best: Option<NaiveDate>, received: NaiveDate) -> bool {
    match current_best {
        None => true,
        Some(best) => distance(candidate, received) < distance(best, received),
    }
}

fn distance(date: NaiveDate, received: NaiveDate) -> i64 {
    (date - received).num_days().abs()
}

/// The next date, strictly after `today`, that falls on `target` weekday. If
/// `today` itself is `target`, this returns `today + 7 days` (spec §6.3: "the
/// next such weekday strictly after today").
pub(super) fn next_weekday_after(today: NaiveDate, target: Weekday) -> NaiveDate {
    let mut candidate = today + Duration::days(1);
    while candidate.weekday() != target {
        candidate += Duration::days(1);
    }
    candidate
}
