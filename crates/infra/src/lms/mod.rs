//! LMS adapters (spec §5): the public platform probe, Moodle and Blackboard
//! clients, and the factory that builds them from stored credentials.

pub mod blackboard;
pub mod factory;
pub mod moodle;
pub mod probe;

use std::time::Duration;

use academic_core::ports::LmsError;
use chrono::{DateTime, TimeZone, Utc};

use crate::http_util::describe_transport_error;

/// Per-request timeout for every LMS call.
pub(crate) const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) fn connection_error(error: &reqwest::Error) -> LmsError {
    LmsError::Connection(format!(
        "Could not reach the platform: {}",
        describe_transport_error(error)
    ))
}

/// Text of an HTML fragment: tags removed, `&nbsp;`/`&amp;` decoded, whitespace collapsed.
pub(crate) fn strip_tags(html: &str) -> String {
    let mut text = String::with_capacity(html.len());
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    let decoded = text.replace("&nbsp;", " ").replace("&amp;", "&");
    decoded.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `8.5` -> "8.5", `10.0` -> "10", `7.25` -> "7.25" (at most 2 decimals, trailing zeros dropped).
pub(crate) fn format_number(value: f64) -> String {
    let formatted = format!("{value:.2}");
    formatted
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// Moodle timestamps: 0 or negative means "no date".
pub(crate) fn unix_to_utc(seconds: i64) -> Option<DateTime<Utc>> {
    (seconds > 0)
        .then(|| Utc.timestamp_opt(seconds, 0).single())
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_tags_keeps_text_only() {
        assert_eq!(
            strip_tags("<span class=\"x\">8,50</span> / 10,00"),
            "8,50 / 10,00"
        );
        assert_eq!(
            strip_tags(
                "A&nbsp;&amp; B
  C"
            ),
            "A & B C"
        );
        assert_eq!(strip_tags("plain"), "plain");
    }

    #[test]
    fn format_number_drops_trailing_zeros() {
        assert_eq!(format_number(10.0), "10");
        assert_eq!(format_number(8.5), "8.5");
        assert_eq!(format_number(7.256), "7.26");
    }

    #[test]
    fn non_positive_timestamps_are_no_date() {
        assert_eq!(unix_to_utc(0), None);
        assert_eq!(unix_to_utc(-5), None);
        assert_eq!(
            unix_to_utc(1_791_000_000).unwrap().timestamp(),
            1_791_000_000
        );
    }
}
