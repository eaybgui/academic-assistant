//! Tests for `parse_message` against `.eml` fixtures covering the message shapes
//! the IMAP provider must handle: plain text with accents, HTML-only, multipart
//! alternative, encoded-word subjects, a missing Message-ID and Cc recipients.

use academic_core::domain::EmailBody;
use academic_core::ports::ProviderError;
use academic_infra::imap::parse_message;
use chrono::{TimeZone, Utc};

fn fixture(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/eml")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("failed to read fixture {path:?}: {e}"))
}

#[test]
fn parses_plain_text_quoted_printable_with_accents() {
    let raw = fixture("plain_text_accents.eml");
    let email = parse_message(1, "INBOX", 101, &raw, None).unwrap();

    assert_eq!(email.account_id, 1);
    assert_eq!(email.folder, "INBOX");
    assert_eq!(email.provider_message_id, "101");
    assert_eq!(email.sender.email, "secretaria@uv.es");
    assert_eq!(email.sender.name.as_deref(), Some("Secretaria FIB"));
    match email.body {
        EmailBody::Text(text) => assert!(text.contains("exámenes"), "body was: {text}"),
        EmailBody::Html(_) => panic!("expected a text body"),
    }
}

#[test]
fn parses_html_only_message() {
    let raw = fixture("html_only.eml");
    let email = parse_message(1, "INBOX", 102, &raw, None).unwrap();

    match email.body {
        EmailBody::Html(html) => assert!(html.contains("<strong>")),
        EmailBody::Text(_) => panic!("expected an html body"),
    }
}

#[test]
fn multipart_alternative_prefers_text_plain() {
    let raw = fixture("multipart_alternative.eml");
    let email = parse_message(1, "INBOX", 103, &raw, None).unwrap();

    match email.body {
        EmailBody::Text(text) => assert!(text.contains("examen")),
        EmailBody::Html(_) => panic!("expected the plain-text alternative to win"),
    }
}

#[test]
fn decodes_encoded_word_subject() {
    let raw = fixture("encoded_word_subject.eml");
    let email = parse_message(1, "INBOX", 104, &raw, None).unwrap();

    assert_eq!(email.subject, "Cambio de fecha de examen");
}

#[test]
fn missing_message_id_yields_none() {
    let raw = fixture("missing_message_id.eml");
    let email = parse_message(1, "INBOX", 105, &raw, None).unwrap();

    assert_eq!(email.message_id_header, None);
}

#[test]
fn collects_cc_recipients_alongside_to() {
    let raw = fixture("cc_recipients.eml");
    let email = parse_message(1, "INBOX", 106, &raw, None).unwrap();

    assert!(email.recipients.iter().any(|a| a.email == "alumno@uv.es"));
    assert!(email.recipients.iter().any(|a| a.email == "delegado@uv.es"));
}

#[test]
fn message_id_header_has_no_angle_brackets() {
    let raw = fixture("cc_recipients.eml");
    let email = parse_message(1, "INBOX", 106, &raw, None).unwrap();

    let id = email.message_id_header.unwrap();
    assert!(!id.contains('<') && !id.contains('>'));
    assert_eq!(id, "notificacion-20240309@uv.es");
}

#[test]
fn internal_date_takes_priority_over_the_date_header() {
    let raw = fixture("plain_text_accents.eml");
    let internal = Utc.with_ymd_and_hms(2024, 3, 15, 6, 0, 0).unwrap();
    let email = parse_message(1, "INBOX", 101, &raw, Some(internal)).unwrap();

    assert_eq!(email.received_at, internal);
}

#[test]
fn falls_back_to_the_date_header_when_internal_date_is_missing() {
    let raw = fixture("plain_text_accents.eml");
    let email = parse_message(1, "INBOX", 101, &raw, None).unwrap();

    // Fixture's Date header is "Fri, 1 Mar 2024 09:00:00 +0100" -> 08:00 UTC.
    assert_eq!(
        email.received_at,
        Utc.with_ymd_and_hms(2024, 3, 1, 8, 0, 0).unwrap()
    );
}

#[test]
fn source_url_is_always_none_for_imap() {
    let raw = fixture("plain_text_accents.eml");
    let email = parse_message(1, "INBOX", 101, &raw, None).unwrap();

    assert_eq!(email.source_url, None);
}

#[test]
fn unparsable_bytes_are_a_protocol_error() {
    let err = parse_message(1, "INBOX", 1, b"", None).unwrap_err();
    assert!(matches!(err, ProviderError::Protocol(_)));
}

#[test]
fn missing_from_header_is_a_protocol_error() {
    let raw = b"To: alumno@uv.es\r\nSubject: sin remitente\r\nDate: Fri, 1 Mar 2024 09:00:00 +0100\r\n\r\nCuerpo.\r\n";
    let err = parse_message(1, "INBOX", 1, raw, None).unwrap_err();
    assert!(matches!(err, ProviderError::Protocol(_)));
}

#[test]
fn missing_date_everywhere_is_a_protocol_error() {
    let raw =
        b"From: secretaria@uv.es\r\nTo: alumno@uv.es\r\nSubject: sin fecha\r\n\r\nCuerpo.\r\n";
    let err = parse_message(1, "INBOX", 1, raw, None).unwrap_err();
    assert!(matches!(err, ProviderError::Protocol(_)));
}
