use chrono::{DateTime, FixedOffset, NaiveDate, NaiveTime};

use super::extract;

/// The reference `received_at` used by every test unless noted otherwise:
/// 2026-09-28T09:00:00+02:00, a Monday.
fn received() -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339("2026-09-28T09:00:00+02:00").unwrap()
}

fn received_at(rfc3339: &str) -> DateTime<FixedOffset> {
    DateTime::parse_from_rfc3339(rfc3339).unwrap()
}

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn time(h: u32, m: u32) -> NaiveTime {
    NaiveTime::from_hms_opt(h, m, 0).unwrap()
}

// ---- Absolute formats -----------------------------------------------------

#[test]
fn absolute_dd_mm_slash_infers_year() {
    let out = extract("", "Nos vemos el 27/10.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27/10");
    assert_eq!(out[0].date, date(2026, 10, 27));
    assert_eq!(out[0].time, None);
}

#[test]
fn absolute_dd_mm_yy_slash() {
    let out = extract("", "Entrega: 27/10/26.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27/10/26");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_dd_mm_yyyy_slash() {
    let out = extract("", "Entrega: 27/10/2026.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27/10/2026");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_dd_mm_yyyy_hyphen() {
    let out = extract("", "Entrega: 27-10-2026.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27-10-2026");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_iso_yyyy_mm_dd() {
    let out = extract("", "Entrega: 2026-10-27.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "2026-10-27");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_spanish_day_de_month_infers_year() {
    let out = extract("", "La entrega es el 27 de octubre.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27 de octubre");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_spanish_day_de_month_de_year() {
    let out = extract("", "La entrega es el 27 de octubre de 2026.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27 de octubre de 2026");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_spanish_setiembre_alternate_spelling() {
    let out = extract("", "El examen es el 1 de setiembre.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "1 de setiembre");
    assert_eq!(out[0].date, date(2026, 9, 1));
}

#[test]
fn absolute_english_month_day_infers_year() {
    let out = extract("", "The deadline is October 27.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "October 27");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_english_month_day_year() {
    let out = extract("", "The deadline is October 27, 2026.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "October 27, 2026");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_english_day_month_infers_year() {
    let out = extract("", "The deadline is 27 October.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27 October");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn absolute_english_day_month_year() {
    let out = extract("", "The deadline is 27 October 2026.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27 October 2026");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn weekday_prefix_is_kept_in_span() {
    let out = extract(
        "",
        "Recuerda que lunes 27 de octubre tenemos entrega.",
        received(),
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "lunes 27 de octubre");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

// ---- Relative forms ---------------------------------------------------------

#[test]
fn relative_hoy() {
    let out = extract("", "La entrega es hoy.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "hoy");
    assert_eq!(out[0].date, date(2026, 9, 28));
}

#[test]
fn relative_today_english() {
    let out = extract("", "The deadline is today.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].date, date(2026, 9, 28));
}

#[test]
fn relative_manana_tomorrow() {
    let out = extract("", "La entrega es mañana.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "mañana");
    assert_eq!(out[0].date, date(2026, 9, 29));
}

#[test]
fn relative_tomorrow_english() {
    let out = extract("", "The deadline is tomorrow.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].date, date(2026, 9, 29));
}

#[test]
fn relative_pasado_manana() {
    let out = extract("", "Nos vemos pasado mañana.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "pasado mañana");
    assert_eq!(out[0].date, date(2026, 9, 30));
}

#[test]
fn relative_proximo_lunes_skips_a_full_week_when_today_is_monday() {
    // received() is itself a Monday (2026-09-28); "próximo lunes" must resolve
    // strictly after today, i.e. the following Monday.
    let out = extract("", "Se pospone al próximo lunes.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "próximo lunes");
    assert_eq!(out[0].date, date(2026, 10, 5));
}

#[test]
fn relative_next_monday_english() {
    let out = extract("", "Postponed to next Monday.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].date, date(2026, 10, 5));
}

#[test]
fn relative_proximo_viernes_this_same_week() {
    let out = extract("", "Se pospone al próximo viernes.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].date, date(2026, 10, 2));
}

// ---- Time attachment ---------------------------------------------------------

#[test]
fn time_colon() {
    let out = extract("", "El 27/10/2026 10:00 en el aula.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27/10/2026 10:00");
    assert_eq!(out[0].time, Some(time(10, 0)));
}

#[test]
fn time_dot() {
    let out = extract("", "El 27/10/2026 10.30 en el aula.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].time, Some(time(10, 30)));
}

#[test]
fn time_hour_only_h_suffix() {
    let out = extract("", "El 27/10/2026 10h en el aula.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27/10/2026 10h");
    assert_eq!(out[0].time, Some(time(10, 0)));
}

#[test]
fn time_colon_h_suffix_no_space() {
    let out = extract("", "El 27/10/2026 10:00h en el aula.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].time, Some(time(10, 0)));
}

#[test]
fn time_colon_h_suffix_with_space() {
    let out = extract("", "El 27/10/2026 10:00 h en el aula.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].time, Some(time(10, 0)));
}

#[test]
fn time_a_las_hour_only() {
    let out = extract("", "El 27/10/2026 a las 10 en el aula.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27/10/2026 a las 10");
    assert_eq!(out[0].time, Some(time(10, 0)));
}

#[test]
fn time_a_las_with_minutes() {
    let out = extract("", "El 27/10/2026 a las 10:30 en el aula.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].time, Some(time(10, 30)));
}

#[test]
fn time_english_am() {
    let out = extract("", "On 27 October 2026 10am in the hall.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].time, Some(time(10, 0)));
}

#[test]
fn time_english_pm() {
    let out = extract("", "On 27 October 2026 3 pm in the hall.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].time, Some(time(15, 0)));
}

#[test]
fn time_not_attached_when_too_far_away() {
    let out = extract(
        "",
        "El 27/10/2026 se comunicará más información sobre el horario, que sera a las 10:00.",
        received(),
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].time, None);
}

#[test]
fn time_attachment_does_not_swallow_the_next_date() {
    let out = extract(
        "",
        "el 27 de octubre o el 28 de octubre a las 10:00",
        received(),
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].date, date(2026, 10, 27));
    assert_eq!(out[0].time, None);
    assert_eq!(out[1].date, date(2026, 10, 28));
    assert_eq!(out[1].time, Some(time(10, 0)));
}

#[test]
fn time_attachment_does_not_swallow_the_next_relative_date() {
    let out = extract("", "hoy y mañana a las 10", received());
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].date, date(2026, 9, 28));
    assert_eq!(out[0].time, None);
    assert_eq!(out[1].date, date(2026, 9, 29));
    assert_eq!(out[1].time, Some(time(10, 0)));
}

// ---- Morning exclusions ("mañana" meaning "morning", not "tomorrow") --------

#[test]
fn morning_exclusion_esta_manana() {
    let out = extract("", "Ya se envió esta mañana.", received());
    assert!(out.is_empty());
}

#[test]
fn morning_exclusion_la_manana() {
    let out = extract("", "Se hará durante la mañana.", received());
    assert!(out.is_empty());
}

#[test]
fn morning_exclusion_por_la_manana() {
    let out = extract("", "La clase es por la mañana.", received());
    assert!(out.is_empty());
}

#[test]
fn morning_exclusion_de_la_manana() {
    let out = extract("", "La clase es a las 10 de la mañana.", received());
    assert!(out.is_empty());
}

// ---- Year inference ------------------------------------------------------

#[test]
fn year_inference_wraps_forward_across_new_year() {
    let out = extract(
        "",
        "Nos vemos el 15 de enero.",
        received_at("2026-12-20T09:00:00+02:00"),
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].date, date(2027, 1, 15));
}

#[test]
fn year_inference_wraps_backward_across_new_year() {
    let out = extract(
        "",
        "Nos vemos el 20 de diciembre.",
        received_at("2026-01-10T09:00:00+02:00"),
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].date, date(2025, 12, 20));
}

#[test]
fn year_inference_stays_in_same_year_within_30_day_window() {
    let out = extract("", "Nos vemos el 1 de septiembre.", received());
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].date, date(2026, 9, 1));
}

#[test]
fn year_inference_drops_candidate_when_only_valid_year_is_more_than_30_days_before_received() {
    // received.year() - 1 == 2024 (leap, so 29 February is valid there); 2025 and
    // 2026 are not leap years, so that is the only calendar-valid candidate among
    // the three tried — and it falls more than 30 days before `received`, so spec
    // §6.3 ("never earlier than 30 days before received_at") requires dropping it
    // rather than falling back to it.
    let out = extract(
        "",
        "Nos vemos el 29 de febrero.",
        received_at("2025-09-28T09:00:00+02:00"),
    );
    assert!(out.is_empty());
}

// ---- Invalid dates, duplicates, ordering, cap -----------------------------

#[test]
fn invalid_date_is_dropped() {
    let out = extract("", "La fecha 31/02/2026 no existe.", received());
    assert!(out.is_empty());
}

#[test]
fn duplicate_date_keeps_first_span() {
    let out = extract(
        "",
        "Reunión el 27/10/2026 y recordad que es el 27 de octubre de 2026.",
        received(),
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].span, "27/10/2026");
    assert_eq!(out[0].date, date(2026, 10, 27));
}

#[test]
fn cap_at_max_candidates() {
    let body: String = (1..=25).map(|d| format!("{d}/1/2026 ")).collect();
    let out = extract("", &body, received());
    assert_eq!(out.len(), super::MAX_CANDIDATES);
    for (i, candidate) in out.iter().enumerate() {
        assert_eq!(candidate.date, date(2026, 1, (i + 1) as u32));
    }
}

#[test]
fn subject_is_scanned_before_body() {
    let out = extract(
        "Contactar antes del 1/9/2026",
        "Después del 2/9/2026 no hay clases.",
        received(),
    );
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].date, date(2026, 9, 1));
    assert_eq!(out[1].date, date(2026, 9, 2));
}

#[test]
fn no_candidates_in_plain_text() {
    let out = extract(
        "Reunión de departamento",
        "Sin fechas mencionadas aquí.",
        received(),
    );
    assert!(out.is_empty());
}
