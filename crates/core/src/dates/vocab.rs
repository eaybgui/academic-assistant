//! Month-name and weekday-name vocabularies (Spanish and English), plus the regex
//! alternation fragments built from them. Shared by the absolute and relative
//! recognizers.

/// Spanish month names, used both as a regex alternation fragment and (lowercased)
/// as lookup keys in [`spanish_month_number`]. Includes `setiembre`, the older
/// spelling of `septiembre` still seen in some emails.
pub(super) const SPANISH_MONTHS_ALT: &str = "enero|febrero|marzo|abril|mayo|junio|julio|agosto|septiembre|setiembre|octubre|noviembre|diciembre";

/// English month names, full and 3-letter abbreviations.
pub(super) const ENGLISH_MONTHS_ALT: &str = "january|jan|february|feb|march|mar|april|apr|may|june|jun|july|jul|august|aug|september|sep|october|oct|november|nov|december|dec";

/// Spanish and English weekday names. Accented Spanish letters are expressed as a
/// character class so both the accented and unaccented spelling match.
pub(super) const WEEKDAYS_ALT: &str = "lunes|martes|mi[eé]rcoles|jueves|viernes|s[aá]bado|domingo|monday|tuesday|wednesday|thursday|friday|saturday|sunday";

/// Maps a Spanish month name (any case) to its 1-based month number.
pub(super) fn spanish_month_number(name: &str) -> Option<u32> {
    Some(match name.to_lowercase().as_str() {
        "enero" => 1,
        "febrero" => 2,
        "marzo" => 3,
        "abril" => 4,
        "mayo" => 5,
        "junio" => 6,
        "julio" => 7,
        "agosto" => 8,
        "septiembre" | "setiembre" => 9,
        "octubre" => 10,
        "noviembre" => 11,
        "diciembre" => 12,
        _ => return None,
    })
}

/// Maps an English month name or 3-letter abbreviation (any case) to its 1-based
/// month number.
pub(super) fn english_month_number(name: &str) -> Option<u32> {
    Some(match name.to_lowercase().as_str() {
        "january" | "jan" => 1,
        "february" | "feb" => 2,
        "march" | "mar" => 3,
        "april" | "apr" => 4,
        "may" => 5,
        "june" | "jun" => 6,
        "july" | "jul" => 7,
        "august" | "aug" => 8,
        "september" | "sep" => 9,
        "october" | "oct" => 10,
        "november" | "nov" => 11,
        "december" | "dec" => 12,
        _ => return None,
    })
}

/// Maps a Spanish or English weekday name (any case, accents optional) to a
/// [`chrono::Weekday`].
pub(super) fn weekday_from_name(name: &str) -> Option<chrono::Weekday> {
    use chrono::Weekday::{Fri, Mon, Sat, Sun, Thu, Tue, Wed};

    let normalized: String = name
        .to_lowercase()
        .chars()
        .map(|c| match c {
            'é' => 'e',
            'á' => 'a',
            other => other,
        })
        .collect();
    Some(match normalized.as_str() {
        "lunes" | "monday" => Mon,
        "martes" | "tuesday" => Tue,
        "miercoles" | "wednesday" => Wed,
        "jueves" | "thursday" => Thu,
        "viernes" | "friday" => Fri,
        "sabado" | "saturday" => Sat,
        "domingo" | "sunday" => Sun,
        _ => return None,
    })
}
