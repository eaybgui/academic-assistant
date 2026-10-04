//! Pure mapping functions: Microsoft Graph JSON to `academic-core` domain types, and
//! HTTP/OAuth error responses to [`ProviderError`]. No I/O.

use chrono::{DateTime, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::Value;

use academic_core::domain::{AccountId, Address, Email, EmailBody};
use academic_core::ports::ProviderError;

use super::SignedInIdentity;

/// Maps a transport-level `reqwest` failure (DNS, TCP, TLS, timeout) to a `ProviderError`.
/// Never includes request bodies (which may carry tokens or codes) in the message.
pub(crate) fn map_transport_error(err: reqwest::Error) -> ProviderError {
    if err.is_timeout() || err.is_connect() {
        ProviderError::Connection(format!("Could not reach Microsoft: {err}"))
    } else {
        ProviderError::Protocol(format!("Microsoft request failed: {err}"))
    }
}

#[derive(Debug, Deserialize)]
struct OAuthErrorBody {
    error: String,
    error_description: Option<String>,
}

/// Maps a non-2xx JSON body from the Microsoft identity token endpoint to a
/// `ProviderError`, recognizing the cases the UI reacts to specially: an
/// expired/revoked refresh token and a tenant that requires admin consent.
pub(crate) fn map_token_error_body(status: StatusCode, body: &str) -> ProviderError {
    let parsed: Option<OAuthErrorBody> = serde_json::from_str(body).ok();
    let error = parsed
        .as_ref()
        .map(|b| b.error.as_str())
        .unwrap_or_default();
    let description = parsed.as_ref().and_then(|b| b.error_description.as_deref());

    map_oauth_error(error, description)
        .unwrap_or_else(|| ProviderError::Auth(format!("Microsoft sign-in failed ({status})")))
}

/// Maps an OAuth `error`/`error_description` pair — from either the token endpoint's
/// JSON body or the loopback redirect's query string — to a `ProviderError`, when the
/// `error` code is recognized (non-empty).
pub(crate) fn map_oauth_error(error: &str, description: Option<&str>) -> Option<ProviderError> {
    if let Some(description) = description {
        if description.contains("AADSTS65001") || description.contains("AADSTS90094") {
            return Some(ProviderError::Auth(
                "Your organization requires admin approval for this app".into(),
            ));
        }
    }
    if error == "invalid_grant" {
        return Some(ProviderError::Auth(
            "Your sign-in expired. Reconnect the account".into(),
        ));
    }
    if error.is_empty() {
        None
    } else {
        Some(ProviderError::Auth(format!(
            "Microsoft sign-in failed ({error})"
        )))
    }
}

#[derive(Debug, Deserialize)]
struct GraphErrorBody {
    error: GraphErrorDetail,
}

#[derive(Debug, Deserialize)]
struct GraphErrorDetail {
    message: Option<String>,
}

/// Maps a non-2xx response from a Microsoft Graph API call (delta fetch, `/me`) to a
/// `ProviderError`.
pub(crate) fn map_graph_error_status(status: StatusCode, body: &str) -> ProviderError {
    if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
        return ProviderError::Auth(
            "Microsoft Graph rejected the request. Reconnect the account".into(),
        );
    }
    let detail = serde_json::from_str::<GraphErrorBody>(body)
        .ok()
        .and_then(|b| b.error.message)
        .map(|m| format!(": {m}"))
        .unwrap_or_default();
    ProviderError::Protocol(format!("Microsoft Graph returned {status}{detail}"))
}

/// Extracts the signed-in user's identifiers from a `GET /me` response: `mail` and
/// `userPrincipalName`, each `None` when absent or blank. At least one must be
/// present.
pub(crate) fn parse_profile_identity(value: &Value) -> Result<SignedInIdentity, ProviderError> {
    let text = |field: &str| {
        value[field]
            .as_str()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
    };
    let identity = SignedInIdentity {
        mail: text("mail"),
        user_principal_name: text("userPrincipalName"),
    };
    if identity.mail.is_none() && identity.user_principal_name.is_none() {
        return Err(ProviderError::Protocol(
            "Microsoft Graph profile response has neither mail nor userPrincipalName".into(),
        ));
    }
    Ok(identity)
}

/// Maps one Microsoft Graph message JSON object (a `value[]` entry from a delta
/// response) to an [`Email`]. Returns `Ok(None)` for a `@removed` entry (deletions
/// reported by the delta feed, which this app ignores).
pub(crate) fn map_message(
    account_id: AccountId,
    folder: &str,
    value: &Value,
) -> Result<Option<Email>, ProviderError> {
    if value.get("@removed").is_some() {
        return Ok(None);
    }

    let id = value["id"].as_str().ok_or_else(|| {
        ProviderError::Protocol("Microsoft Graph message is missing its id".into())
    })?;

    let sender = parse_address(&value["from"]["emailAddress"]).ok_or_else(|| {
        ProviderError::Protocol("Microsoft Graph message is missing a sender".into())
    })?;

    let mut recipients = parse_address_list(&value["toRecipients"]);
    recipients.extend(parse_address_list(&value["ccRecipients"]));

    let subject = value["subject"].as_str().unwrap_or_default().to_string();

    let received_at = value["receivedDateTime"]
        .as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .ok_or_else(|| {
            ProviderError::Protocol("Microsoft Graph message is missing a received date".into())
        })?;

    let body = parse_body(&value["body"]);
    let message_id_header = value["internetMessageId"]
        .as_str()
        .map(strip_angle_brackets);
    let source_url = value["webLink"].as_str().map(str::to_string);

    Ok(Some(Email {
        account_id,
        folder: folder.to_string(),
        provider_message_id: id.to_string(),
        message_id_header,
        sender,
        recipients,
        subject,
        received_at,
        body,
        source_url,
    }))
}

fn parse_address(value: &Value) -> Option<Address> {
    let email = value["address"].as_str()?.to_string();
    let name = value["name"]
        .as_str()
        .filter(|n| !n.is_empty())
        .map(str::to_string);
    Some(Address { name, email })
}

fn parse_address_list(value: &Value) -> Vec<Address> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| parse_address(&entry["emailAddress"]))
        .collect()
}

fn parse_body(value: &Value) -> EmailBody {
    let content = value["content"].as_str().unwrap_or_default().to_string();
    match value["contentType"].as_str() {
        Some(ct) if ct.eq_ignore_ascii_case("text") => EmailBody::Text(content),
        _ => EmailBody::Html(content),
    }
}

fn strip_angle_brackets(id: &str) -> String {
    id.trim_start_matches('<').trim_end_matches('>').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn map_oauth_error_recognizes_invalid_grant() {
        let err = map_oauth_error("invalid_grant", Some("AADSTS70008: expired")).unwrap();
        assert!(
            matches!(err, ProviderError::Auth(msg) if msg == "Your sign-in expired. Reconnect the account")
        );
    }

    #[test]
    fn map_oauth_error_recognizes_admin_consent_65001() {
        let err = map_oauth_error(
            "invalid_grant",
            Some("AADSTS65001: the user or administrator has not consented"),
        )
        .unwrap();
        assert!(
            matches!(err, ProviderError::Auth(msg) if msg == "Your organization requires admin approval for this app")
        );
    }

    #[test]
    fn map_oauth_error_recognizes_admin_consent_90094() {
        let err = map_oauth_error(
            "interaction_required",
            Some("AADSTS90094: admin consent required"),
        )
        .unwrap();
        assert!(
            matches!(err, ProviderError::Auth(msg) if msg == "Your organization requires admin approval for this app")
        );
    }

    #[test]
    fn map_oauth_error_falls_back_to_generic_message_for_other_codes() {
        let err = map_oauth_error("invalid_request", None).unwrap();
        assert!(matches!(err, ProviderError::Auth(msg) if msg.contains("invalid_request")));
    }

    #[test]
    fn map_oauth_error_is_none_when_error_code_is_empty() {
        assert!(map_oauth_error("", None).is_none());
    }

    #[test]
    fn map_token_error_body_parses_json_error() {
        let body = r#"{"error":"invalid_grant","error_description":"AADSTS70008: expired"}"#;
        let err = map_token_error_body(StatusCode::BAD_REQUEST, body);
        assert!(
            matches!(err, ProviderError::Auth(msg) if msg == "Your sign-in expired. Reconnect the account")
        );
    }

    #[test]
    fn map_token_error_body_falls_back_on_unparsable_body() {
        let err = map_token_error_body(StatusCode::INTERNAL_SERVER_ERROR, "not json");
        assert!(matches!(err, ProviderError::Auth(msg) if msg.contains("500")));
    }

    #[test]
    fn map_graph_error_status_maps_401_and_403_to_auth() {
        assert!(matches!(
            map_graph_error_status(StatusCode::UNAUTHORIZED, ""),
            ProviderError::Auth(_)
        ));
        assert!(matches!(
            map_graph_error_status(StatusCode::FORBIDDEN, ""),
            ProviderError::Auth(_)
        ));
    }

    #[test]
    fn map_graph_error_status_maps_other_statuses_to_protocol_with_detail() {
        let body = r#"{"error":{"code":"BadRequest","message":"Cannot filter"}}"#;
        let err = map_graph_error_status(StatusCode::BAD_REQUEST, body);
        assert!(matches!(err, ProviderError::Protocol(msg) if msg.contains("Cannot filter")));
    }

    #[test]
    fn parse_profile_identity_returns_both_identifiers() {
        let value = json!({"mail": "user@uv.es", "userPrincipalName": "user@upn.example"});
        let identity = parse_profile_identity(&value).unwrap();
        assert_eq!(identity.mail.as_deref(), Some("user@uv.es"));
        assert_eq!(
            identity.user_principal_name.as_deref(),
            Some("user@upn.example")
        );
    }

    #[test]
    fn parse_profile_identity_treats_null_or_blank_mail_as_absent() {
        for mail in [json!(null), json!(""), json!("  ")] {
            let value = json!({"mail": mail, "userPrincipalName": "user@upn.example"});
            let identity = parse_profile_identity(&value).unwrap();
            assert_eq!(identity.mail, None);
            assert_eq!(
                identity.user_principal_name.as_deref(),
                Some("user@upn.example")
            );
        }
    }

    #[test]
    fn parse_profile_identity_errors_when_both_missing() {
        assert!(parse_profile_identity(&json!({})).is_err());
        assert!(parse_profile_identity(&json!({"mail": "", "userPrincipalName": null})).is_err());
    }

    fn sample_message() -> Value {
        json!({
            "id": "AAMk-1",
            "internetMessageId": "<abc123@uv.es>",
            "subject": "Examen aplazado",
            "from": {"emailAddress": {"name": "Secretaría", "address": "secretaria@uv.es"}},
            "toRecipients": [{"emailAddress": {"name": "Ana", "address": "ana@uv.es"}}],
            "ccRecipients": [{"emailAddress": {"name": "", "address": "cc@uv.es"}}],
            "receivedDateTime": "2026-09-20T08:30:00Z",
            "body": {"contentType": "text", "content": "El examen se aplaza."},
            "webLink": "https://outlook.office.com/mail/deeplink"
        })
    }

    #[test]
    fn map_message_maps_a_full_message() {
        let email = map_message(7, "inbox", &sample_message()).unwrap().unwrap();
        assert_eq!(email.account_id, 7);
        assert_eq!(email.folder, "inbox");
        assert_eq!(email.provider_message_id, "AAMk-1");
        assert_eq!(email.message_id_header.as_deref(), Some("abc123@uv.es"));
        assert_eq!(email.sender.email, "secretaria@uv.es");
        assert_eq!(email.sender.name.as_deref(), Some("Secretaría"));
        assert_eq!(email.recipients.len(), 2);
        assert_eq!(email.recipients[1].name, None);
        assert_eq!(email.subject, "Examen aplazado");
        assert_eq!(
            email.source_url.as_deref(),
            Some("https://outlook.office.com/mail/deeplink")
        );
        assert!(matches!(email.body, EmailBody::Text(ref t) if t == "El examen se aplaza."));
    }

    #[test]
    fn map_message_returns_none_for_removed_entries() {
        let value = json!({"id": "AAMk-2", "@removed": {"reason": "deleted"}});
        assert!(map_message(7, "inbox", &value).unwrap().is_none());
    }

    #[test]
    fn map_message_errors_when_sender_missing() {
        let mut value = sample_message();
        value["from"] = Value::Null;
        assert!(map_message(7, "inbox", &value).is_err());
    }

    #[test]
    fn map_message_errors_when_received_at_missing() {
        let mut value = sample_message();
        value["receivedDateTime"] = Value::Null;
        assert!(map_message(7, "inbox", &value).is_err());
    }

    #[test]
    fn map_message_defaults_to_html_body_for_unknown_content_type() {
        let mut value = sample_message();
        value["body"] = json!({"contentType": "html", "content": "<p>hola</p>"});
        let email = map_message(7, "inbox", &value).unwrap().unwrap();
        assert!(matches!(email.body, EmailBody::Html(ref h) if h == "<p>hola</p>"));
    }

    #[test]
    fn map_message_handles_missing_internet_message_id() {
        let mut value = sample_message();
        value["internetMessageId"] = Value::Null;
        let email = map_message(7, "inbox", &value).unwrap().unwrap();
        assert_eq!(email.message_id_header, None);
    }
}
