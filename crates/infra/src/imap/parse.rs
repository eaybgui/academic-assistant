//! Pure parsing of a raw RFC 822 message into a domain [`Email`], used by
//! [`super::ImapEmailProvider`] and tested directly against `.eml` fixtures.

use chrono::{DateTime, TimeZone, Utc};
use mail_parser::{Addr, Address as MailAddress, Message, MessageParser, PartType};

use academic_core::domain::{AccountId, Address, Email, EmailBody};
use academic_core::ports::ProviderError;

/// Parses a raw message fetched from `folder` under `uid` into an [`Email`].
///
/// `internal_date` is the IMAP `INTERNALDATE` fetched alongside the body; when absent
/// the message's `Date` header is used instead. A message missing both a usable
/// `From` header and a date (from either source) is rejected as a protocol error,
/// since the pipeline cannot process an email without a sender or a received time.
pub fn parse_message(
    account_id: AccountId,
    folder: &str,
    uid: u32,
    raw: &[u8],
    internal_date: Option<DateTime<Utc>>,
) -> Result<Email, ProviderError> {
    let message = MessageParser::default()
        .parse(raw)
        .ok_or_else(|| ProviderError::Protocol("could not parse message".into()))?;

    let sender = sender_address(&message)?;
    let recipients = collect_addresses(message.to())
        .into_iter()
        .chain(collect_addresses(message.cc()))
        .collect();
    let subject = message.subject().unwrap_or_default().to_string();
    let received_at = internal_date
        .or_else(|| message.date().and_then(header_date_to_utc))
        .ok_or_else(|| ProviderError::Protocol("message has no usable date".into()))?;
    let body = extract_body(&message);
    let message_id_header = message.message_id().map(|id| id.to_string());

    Ok(Email {
        account_id,
        folder: folder.to_string(),
        provider_message_id: uid.to_string(),
        message_id_header,
        sender,
        recipients,
        subject,
        received_at,
        body,
        source_url: None,
    })
}

fn sender_address(message: &Message<'_>) -> Result<Address, ProviderError> {
    let addr = message
        .from()
        .and_then(MailAddress::first)
        .ok_or_else(|| ProviderError::Protocol("message has no From header".into()))?;
    to_domain_address(addr)
        .ok_or_else(|| ProviderError::Protocol("message has no From header".into()))
}

fn collect_addresses(address: Option<&MailAddress<'_>>) -> Vec<Address> {
    address
        .map(|a| a.iter().filter_map(to_domain_address).collect())
        .unwrap_or_default()
}

fn to_domain_address(addr: &Addr<'_>) -> Option<Address> {
    let email = addr.address()?.to_string();
    Some(Address {
        name: addr.name().map(|n| n.to_string()),
        email,
    })
}

/// Converts an RFC 5322 `Date` header (as parsed by `mail-parser`) into UTC.
fn header_date_to_utc(date: &mail_parser::DateTime) -> Option<DateTime<Utc>> {
    let offset_seconds = {
        let total = i32::from(date.tz_hour) * 3600 + i32::from(date.tz_minute) * 60;
        if date.tz_before_gmt {
            -total
        } else {
            total
        }
    };
    let offset = chrono::FixedOffset::east_opt(offset_seconds)?;
    let local = offset
        .with_ymd_and_hms(
            i32::from(date.year),
            u32::from(date.month),
            u32::from(date.day),
            u32::from(date.hour),
            u32::from(date.minute),
            u32::from(date.second),
        )
        .single()?;
    Some(local.with_timezone(&Utc))
}

/// Prefers a genuine `text/plain` body part over `text/html`; falls back to
/// whatever `mail-parser` can derive (including HTML-to-text conversion) only when
/// no directly-typed part of either kind exists.
fn extract_body(message: &Message<'_>) -> EmailBody {
    if let Some(text) = first_part_of_type(message, &message.text_body, |part| match part {
        PartType::Text(t) => Some(t.to_string()),
        _ => None,
    }) {
        return EmailBody::Text(text);
    }
    if let Some(html) = first_part_of_type(message, &message.html_body, |part| match part {
        PartType::Html(h) => Some(h.to_string()),
        _ => None,
    }) {
        return EmailBody::Html(html);
    }
    if let Some(text) = message.body_text(0) {
        return EmailBody::Text(text.into_owned());
    }
    if let Some(html) = message.body_html(0) {
        return EmailBody::Html(html.into_owned());
    }
    EmailBody::Text(String::new())
}

/// One message as fetched from the server: `(uid, raw RFC 822 bytes, INTERNALDATE)`.
pub type FetchedMessage = (u32, Vec<u8>, Option<DateTime<Utc>>);

/// Parses each [`FetchedMessage`] fetched for one `UID FETCH`
/// batch into an [`Email`], skipping (and logging, by UID only, never content) any
/// message that fails to parse instead of failing the whole batch — one malformed
/// message must not block the rest of the folder from syncing. Every fetched UID,
/// parsed or not, counts toward the returned high-water mark so the cursor still
/// advances past it. The returned emails are sorted oldest first (`received_at`, then
/// `provider_message_id` to break ties), per the `EmailProvider` contract.
///
/// Pure aside from the `tracing::warn!` call on a skipped message, so it is testable
/// without a live IMAP session.
pub fn parse_fetched_messages(
    account_id: AccountId,
    folder: &str,
    fetched: &[FetchedMessage],
) -> (Vec<Email>, Option<u32>) {
    let mut emails = Vec::with_capacity(fetched.len());
    let mut max_uid: Option<u32> = None;
    for (uid, raw, internal_date) in fetched {
        max_uid = Some(max_uid.map_or(*uid, |current| current.max(*uid)));
        match parse_message(account_id, folder, *uid, raw, *internal_date) {
            Ok(email) => emails.push(email),
            Err(_) => tracing::warn!(uid = *uid, "skipping IMAP message that failed to parse"),
        }
    }
    emails.sort_by(|a, b| {
        a.received_at
            .cmp(&b.received_at)
            .then_with(|| a.provider_message_id.cmp(&b.provider_message_id))
    });
    (emails, max_uid)
}

fn first_part_of_type<'x, F>(
    message: &'x Message<'x>,
    indices: &[u32],
    extract: F,
) -> Option<String>
where
    F: Fn(&'x PartType<'x>) -> Option<String>,
{
    indices.iter().find_map(|&idx| {
        message
            .parts
            .get(idx as usize)
            .and_then(|part| extract(&part.body))
    })
}
