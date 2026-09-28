//! Deterministic pre-filters applied to normalized emails before AI analysis (Task T3).

use crate::domain::{Email, FilterReason};

/// Configuration for [`apply`]: addresses/domains to block and the account's own
/// address (used to discard mail the account sent to itself).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterConfig {
    /// Blocked-sender entries. An entry starting with `@` blocks a whole domain
    /// (matched exactly against the sender's domain, not subdomains); any other entry
    /// blocks one exact address. Matching is case-insensitive and entries are trimmed;
    /// blank entries are ignored.
    pub blocked: Vec<String>,
    /// The account's own email address; mail sent from this address is discarded.
    pub own_address: String,
}

/// Outcome of [`apply`]: keep the email for analysis, or discard it with a reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    Keep,
    Discard(FilterReason),
}

/// Applies the deterministic pre-filters to `email` in order: blocked sender → blocked
/// domain → own message → empty (empty subject and empty `normalized_body`). Pure
/// function; performs no I/O.
pub fn apply(email: &Email, normalized_body: &str, config: &FilterConfig) -> FilterDecision {
    let sender_email = email.sender.email.trim().to_lowercase();
    let sender_domain = sender_email.rsplit_once('@').map(|(_, domain)| domain);

    let entries: Vec<String> = config
        .blocked
        .iter()
        .map(|entry| entry.trim().to_lowercase())
        .filter(|entry| !entry.is_empty())
        .collect();

    let blocked_sender = entries
        .iter()
        .any(|entry| !entry.starts_with('@') && *entry == sender_email);
    if blocked_sender {
        return FilterDecision::Discard(FilterReason::BlockedSender);
    }

    let blocked_domain = entries.iter().any(|entry| {
        entry
            .strip_prefix('@')
            .is_some_and(|domain| Some(domain) == sender_domain)
    });
    if blocked_domain {
        return FilterDecision::Discard(FilterReason::BlockedDomain);
    }

    let own_address = config.own_address.trim().to_lowercase();
    if !own_address.is_empty() && sender_email == own_address {
        return FilterDecision::Discard(FilterReason::OwnMessage);
    }

    if email.subject.trim().is_empty() && normalized_body.trim().is_empty() {
        return FilterDecision::Discard(FilterReason::Empty);
    }

    FilterDecision::Keep
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::Address;
    use chrono::{TimeZone, Utc};

    fn email_from(sender_email: &str, subject: &str) -> Email {
        Email {
            account_id: 1,
            folder: "INBOX".to_string(),
            provider_message_id: "1".to_string(),
            message_id_header: Some("<msg-1@uv.es>".to_string()),
            sender: Address {
                name: Some("Remitente".to_string()),
                email: sender_email.to_string(),
            },
            recipients: vec![Address {
                name: None,
                email: "usuario@uv.es".to_string(),
            }],
            subject: subject.to_string(),
            received_at: Utc.with_ymd_and_hms(2026, 9, 28, 9, 0, 0).unwrap(),
            body: crate::domain::EmailBody::Text(String::new()),
            source_url: None,
        }
    }

    fn config(blocked: &[&str], own_address: &str) -> FilterConfig {
        FilterConfig {
            blocked: blocked.iter().map(|s| s.to_string()).collect(),
            own_address: own_address.to_string(),
        }
    }

    #[test]
    fn keeps_a_normal_email() {
        let email = email_from("profesor@uv.es", "Recordatorio de examen");
        let decision = apply(
            &email,
            "El examen es el lunes.",
            &config(&[], "usuario@uv.es"),
        );
        assert_eq!(decision, FilterDecision::Keep);
    }

    #[test]
    fn discards_blocked_sender_exact_address() {
        let email = email_from("Spam@Example.com", "Oferta");
        let decision = apply(
            &email,
            "cuerpo",
            &config(&["spam@example.com"], "usuario@uv.es"),
        );
        assert_eq!(
            decision,
            FilterDecision::Discard(FilterReason::BlockedSender)
        );
    }

    #[test]
    fn discards_blocked_domain() {
        let email = email_from("noreply@Marketing.com", "Newsletter");
        let decision = apply(
            &email,
            "cuerpo",
            &config(&["@marketing.com"], "usuario@uv.es"),
        );
        assert_eq!(
            decision,
            FilterDecision::Discard(FilterReason::BlockedDomain)
        );
    }

    #[test]
    fn blocked_domain_does_not_match_subdomains() {
        let email = email_from("x@sub.uv.es", "Aviso");
        let decision = apply(&email, "cuerpo", &config(&["@uv.es"], "usuario@uv.es"));
        assert_eq!(decision, FilterDecision::Keep);
    }

    #[test]
    fn discards_own_message() {
        let email = email_from("Usuario@UV.ES", "Nota para mi mismo");
        let decision = apply(&email, "cuerpo", &config(&[], "usuario@uv.es"));
        assert_eq!(decision, FilterDecision::Discard(FilterReason::OwnMessage));
    }

    #[test]
    fn discards_empty_subject_and_body() {
        let email = email_from("profesor@uv.es", "   ");
        let decision = apply(&email, "   ", &config(&[], "usuario@uv.es"));
        assert_eq!(decision, FilterDecision::Discard(FilterReason::Empty));
    }

    #[test]
    fn blank_entries_are_ignored() {
        let email = email_from("profesor@uv.es", "Aviso");
        let decision = apply(&email, "cuerpo", &config(&["", "   "], "usuario@uv.es"));
        assert_eq!(decision, FilterDecision::Keep);
    }

    #[test]
    fn blocked_sender_takes_priority_over_blocked_domain() {
        // Blocking the whole domain would also catch this sender; verify the more
        // specific exact-address check is what actually fires (order: sender before
        // domain), by using an address that is both individually and domain-blocked.
        let email = email_from("noreply@uv.es", "Aviso");
        let decision = apply(
            &email,
            "cuerpo",
            &config(&["noreply@uv.es", "@uv.es"], "usuario@uv.es"),
        );
        assert_eq!(
            decision,
            FilterDecision::Discard(FilterReason::BlockedSender)
        );
    }
}
