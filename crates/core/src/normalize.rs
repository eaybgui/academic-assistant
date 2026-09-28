//! HTML-to-text conversion, quote/signature stripping and content hashing (Task T3).

use std::sync::LazyLock;

use regex::Regex;
use sha2::{Digest, Sha256};

use crate::domain::EmailBody;

/// Maximum number of `char`s kept in a normalized body; the text is truncated to this
/// length without ever splitting a multibyte character.
pub const MAX_BODY_CHARS: usize = 12_000;

/// Column width `html2text` wraps HTML content to before we re-collapse whitespace.
/// Wide enough that ordinary email paragraphs are not hard-wrapped mid-sentence.
const HTML_RENDER_WIDTH: usize = 2_000;

/// Number of lines to look ahead of a `De:` line for a matching `Enviado:` line when
/// detecting an Outlook-style quoted header block.
const DE_ENVIADO_LOOKAHEAD: usize = 5;

static ON_WROTE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^on .+ wrote:\s*$").expect("valid regex"));
static EL_ESCRIBIO_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^el .+ escribi[oó]:\s*$").expect("valid regex"));

/// Converts an [`EmailBody`] to plain text, strips quoted replies and signatures,
/// collapses whitespace and truncates to [`MAX_BODY_CHARS`].
///
/// Steps, in order: HTML→text (if needed) → drop quoted-reply lines/blocks → drop the
/// signature (everything from a line equal to `-- ` onward) → collapse whitespace (at
/// most one consecutive blank line, no trailing spaces) → truncate by `char` count.
pub fn normalize_body(body: &EmailBody) -> String {
    let text = match body {
        EmailBody::Text(text) => text.clone(),
        EmailBody::Html(html) => html_to_text(html),
    };
    let text = strip_quoted_reply(&text);
    let text = strip_signature(&text);
    let text = collapse_whitespace(&text);
    truncate_chars(&text, MAX_BODY_CHARS)
}

/// SHA-256 hex digest of `lowercase(sender_email) + "\n" + trim(subject) + "\n" +
/// normalized_body`, used to deduplicate messages across syncs. Always lowercase hex.
pub fn content_hash(sender_email: &str, subject: &str, normalized_body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(sender_email.to_lowercase().as_bytes());
    hasher.update(b"\n");
    hasher.update(subject.trim().as_bytes());
    hasher.update(b"\n");
    hasher.update(normalized_body.as_bytes());
    hex::encode(hasher.finalize())
}

/// Renders HTML to plain text. `html2text::from_read` reads from an in-memory byte
/// slice, so the only realistic failure mode is a pathologically narrow width (not the
/// case here); any error falls back to an empty body rather than panicking.
fn html_to_text(html: &str) -> String {
    html2text::from_read(html.as_bytes(), HTML_RENDER_WIDTH).unwrap_or_default()
}

/// Drops lines starting with `>` and everything from the first quote-block marker
/// (`-----Original Message-----`, `On … wrote:`, `El … escribió:`, an Outlook
/// `De:`/`Enviado:` header block) onward.
fn strip_quoted_reply(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let cutoff = find_quote_cutoff(&lines).unwrap_or(lines.len());
    lines[..cutoff]
        .iter()
        .filter(|line| !line.trim_start().starts_with('>'))
        .copied()
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the index of the first line that starts a quoted-reply block, if any.
fn find_quote_cutoff(lines: &[&str]) -> Option<usize> {
    lines
        .iter()
        .enumerate()
        .find(|(idx, line)| is_quote_marker(lines, *idx, line))
        .map(|(idx, _)| idx)
}

fn is_quote_marker(lines: &[&str], idx: usize, line: &str) -> bool {
    let trimmed = line.trim();
    if trimmed.eq_ignore_ascii_case("-----Original Message-----") {
        return true;
    }
    if ON_WROTE_RE.is_match(trimmed) || EL_ESCRIBIO_RE.is_match(trimmed) {
        return true;
    }
    if trimmed.to_lowercase().starts_with("de:") {
        let lookahead_end = (idx + 1 + DE_ENVIADO_LOOKAHEAD).min(lines.len());
        return lines[idx + 1..lookahead_end]
            .iter()
            .any(|candidate| candidate.trim().to_lowercase().starts_with("enviado:"));
    }
    false
}

/// Drops everything from (and including) a line that is exactly `-- ` onward.
fn strip_signature(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    match lines.iter().position(|line| *line == "-- ") {
        Some(idx) => lines[..idx].join("\n"),
        None => text.to_string(),
    }
}

/// Trims trailing whitespace on every line and collapses runs of blank lines to at
/// most one, then trims the leading/trailing blank lines of the whole text.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::new();
    let mut previous_blank = false;
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() {
            if !previous_blank {
                out.push('\n');
            }
            previous_blank = true;
        } else {
            out.push_str(trimmed);
            out.push('\n');
            previous_blank = false;
        }
    }
    out.trim().to_string()
}

/// Truncates `s` to at most `max_chars` `char`s, never splitting a multibyte character.
fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(body: &str) -> EmailBody {
        EmailBody::Text(body.to_string())
    }

    #[test]
    fn html_is_converted_to_text() {
        let body = EmailBody::Html(
            "<p>Hola <strong>mundo</strong></p><p>Segundo parrafo.</p>".to_string(),
        );
        let normalized = normalize_body(&body);
        assert!(normalized.contains("Hola"), "got: {normalized:?}");
        assert!(normalized.contains("mundo"), "got: {normalized:?}");
        assert!(
            normalized.contains("Segundo parrafo."),
            "got: {normalized:?}"
        );
        assert!(!normalized.contains('<'), "got: {normalized:?}");
    }

    #[test]
    fn quoted_lines_starting_with_gt_are_removed() {
        let body = text(
            "Nuevo contenido.\n> Contenido citado linea uno.\n> Linea dos.\nMas contenido nuevo.",
        );
        let normalized = normalize_body(&body);
        assert_eq!(normalized, "Nuevo contenido.\nMas contenido nuevo.");
    }

    #[test]
    fn cuts_at_original_message_marker() {
        let body = text(
            "Ver detalles abajo.\n\n-----Original Message-----\nFrom: x@y.com\nSubject: Old\n\nContenido antiguo.",
        );
        assert_eq!(normalize_body(&body), "Ver detalles abajo.");
    }

    #[test]
    fn cuts_at_english_wrote_marker() {
        let body =
            text("Gracias, quedo pendiente.\n\nOn Mon, 1 Sep 2026, X wrote:\n> Contenido citado.");
        assert_eq!(normalize_body(&body), "Gracias, quedo pendiente.");
    }

    #[test]
    fn cuts_at_spanish_escribio_marker() {
        let body = text(
            "Gracias, quedo pendiente.\n\nEl lun, 1 sept 2026 a las 10:00, X escribió:\n> Contenido citado.",
        );
        assert_eq!(normalize_body(&body), "Gracias, quedo pendiente.");
    }

    #[test]
    fn cuts_at_outlook_de_enviado_block() {
        let body = text(
            "Contenido nuevo.\n\nDe: Remitente Original <original@uv.es>\nEnviado: lunes, 1 de septiembre de 2025 10:00\nPara: Usuario <usuario@uv.es>\nAsunto: RE: Algo\n\nContenido citado.",
        );
        assert_eq!(normalize_body(&body), "Contenido nuevo.");
    }

    #[test]
    fn signature_after_double_dash_is_removed() {
        let body = text(&format!(
            "Hola,\n\nGracias por la informacion.\n\n{}\nJuan Perez\nUniversidad",
            "-- "
        ));
        assert_eq!(
            normalize_body(&body),
            "Hola,\n\nGracias por la informacion."
        );
    }

    #[test]
    fn whitespace_is_collapsed_to_at_most_one_blank_line() {
        let body = text("Linea uno.\n\n\n\nLinea dos.   \n\nLinea tres.");
        assert_eq!(
            normalize_body(&body),
            "Linea uno.\n\nLinea dos.\n\nLinea tres."
        );
    }

    #[test]
    fn truncates_at_max_body_chars_without_splitting_multibyte_chars() {
        let long_body = "ñ".repeat(MAX_BODY_CHARS + 1_000);
        let body = text(&long_body);
        let normalized = normalize_body(&body);
        assert_eq!(normalized.chars().count(), MAX_BODY_CHARS);
        assert_eq!(normalized, "ñ".repeat(MAX_BODY_CHARS));
    }

    #[test]
    fn content_hash_is_stable() {
        let a = content_hash("user@uv.es", "Reunion", "cuerpo");
        let b = content_hash("user@uv.es", "Reunion", "cuerpo");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64);
        assert!(a
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn content_hash_is_case_insensitive_on_sender_and_trims_subject() {
        let a = content_hash("User@UV.ES", "  Reunion  ", "cuerpo");
        let b = content_hash("user@uv.es", "Reunion", "cuerpo");
        assert_eq!(a, b);
    }

    #[test]
    fn content_hash_is_sensitive_to_body() {
        let a = content_hash("user@uv.es", "Reunion", "cuerpo uno");
        let b = content_hash("user@uv.es", "Reunion", "cuerpo dos");
        assert_ne!(a, b);
    }
}
