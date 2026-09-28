//! [`MicrosoftGraphEmailProvider`]: the `EmailProvider` adapter for Microsoft 365 /
//! Outlook.com mailboxes, backed by the Graph delta API and OAuth token refresh.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Duration as ChronoDuration, SecondsFormat, Utc};
use reqwest::{Client, Response, StatusCode};
use serde_json::Value;
use url::Url;

use academic_core::domain::{
    account_refresh_token_key, AccountId, FetchBatch, SyncCursor, MAX_BATCH,
};
use academic_core::ports::{EmailProvider, ProviderError, SecretStore};

use super::mapping::{map_graph_error_status, map_message, map_transport_error};
use super::{oauth, signed_in_email, OAuthConfig};

/// Fields requested for every message; keeps the payload small and matches exactly
/// what [`super::mapping::map_message`] reads.
const GRAPH_SELECT_FIELDS: &str =
    "id,internetMessageId,subject,from,toRecipients,ccRecipients,receivedDateTime,body,webLink";
/// Requests a plain-text body and caps each page at 50 messages, per spec §8.2.
const GRAPH_PREFER_HEADER: &str = "outlook.body-content-type=\"text\", odata.maxpagesize=50";
/// An access token is refreshed this long before it actually expires.
const TOKEN_EXPIRY_SKEW: ChronoDuration = ChronoDuration::seconds(60);

struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
}

/// `EmailProvider` for Microsoft 365 / Outlook.com mailboxes via Microsoft Graph.
///
/// The access token lives only in memory (behind a `Mutex`, refreshed on demand);
/// the refresh token is read from and, when Microsoft rotates it, written back to
/// the `SecretStore`.
pub struct MicrosoftGraphEmailProvider {
    account_id: AccountId,
    http: Client,
    oauth: OAuthConfig,
    graph_base_url: String,
    secrets: Arc<dyn SecretStore>,
    cached_token: Mutex<Option<CachedToken>>,
}

impl MicrosoftGraphEmailProvider {
    pub fn new(
        account_id: AccountId,
        http: Client,
        oauth: OAuthConfig,
        graph_base_url: String,
        secrets: Arc<dyn SecretStore>,
    ) -> Self {
        Self {
            account_id,
            http,
            oauth,
            graph_base_url,
            secrets,
            cached_token: Mutex::new(None),
        }
    }

    /// Returns a valid access token: the cached one when it still has more than
    /// [`TOKEN_EXPIRY_SKEW`] left, otherwise a freshly refreshed one (which is also
    /// re-cached, and whose rotated refresh token is persisted).
    async fn access_token(&self) -> Result<String, ProviderError> {
        if let Some(token) = self.cached_valid_token() {
            return Ok(token);
        }

        let refresh_token = self
            .secrets
            .get(&account_refresh_token_key(self.account_id))
            .map_err(|e| ProviderError::Auth(format!("Could not read the stored sign-in: {e}")))?
            .ok_or_else(|| ProviderError::Auth("Sign in to this account".into()))?;

        let tokens = oauth::refresh(&self.http, &self.oauth, &refresh_token).await?;

        self.secrets
            .set(
                &account_refresh_token_key(self.account_id),
                &tokens.refresh_token,
            )
            .map_err(|e| {
                ProviderError::Auth(format!("Could not store the refreshed sign-in: {e}"))
            })?;

        *self.cached_token.lock().unwrap() = Some(CachedToken {
            access_token: tokens.access_token.clone(),
            expires_at: tokens.expires_at,
        });

        Ok(tokens.access_token)
    }

    fn cached_valid_token(&self) -> Option<String> {
        let guard = self.cached_token.lock().unwrap();
        guard.as_ref().and_then(|cached| {
            if cached.expires_at > Utc::now() + TOKEN_EXPIRY_SKEW {
                Some(cached.access_token.clone())
            } else {
                None
            }
        })
    }

    async fn get_page(&self, url: &str, access_token: &str) -> Result<Response, ProviderError> {
        self.http
            .get(url)
            .bearer_auth(access_token)
            .header("Prefer", GRAPH_PREFER_HEADER)
            .send()
            .await
            .map_err(map_transport_error)
    }
}

#[async_trait]
impl EmailProvider for MicrosoftGraphEmailProvider {
    async fn test_connection(&self) -> Result<(), ProviderError> {
        let access_token = self.access_token().await?;
        signed_in_email(&self.http, &self.graph_base_url, &access_token).await?;
        Ok(())
    }

    async fn fetch_new(
        &self,
        folder: &str,
        cursor: Option<&SyncCursor>,
        since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError> {
        let access_token = self.access_token().await?;

        let has_cursor = matches!(cursor, Some(SyncCursor::Graph { .. }));
        let mut next_url = match cursor {
            Some(SyncCursor::Graph { link }) => link.clone(),
            _ => build_delta_url(&self.graph_base_url, folder, since, true),
        };

        let mut emails = Vec::new();
        let mut discard_before: Option<DateTime<Utc>> = None;
        let mut may_retry_without_filter = !has_cursor;

        let last_link = loop {
            let response = self.get_page(&next_url, &access_token).await?;
            let status = response.status();

            if status == StatusCode::BAD_REQUEST && may_retry_without_filter {
                may_retry_without_filter = false;
                next_url = build_delta_url(&self.graph_base_url, folder, since, false);
                discard_before = Some(since);
                continue;
            }
            may_retry_without_filter = false;

            if !status.is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(map_graph_error_status(status, &body));
            }

            let body: Value = response.json().await.map_err(|_| {
                ProviderError::Protocol("Microsoft Graph returned an invalid response".into())
            })?;

            for item in body["value"].as_array().into_iter().flatten() {
                if let Some(email) = map_message(self.account_id, folder, item)? {
                    if discard_before.is_some_and(|cutoff| email.received_at < cutoff) {
                        continue;
                    }
                    emails.push(email);
                }
            }

            if let Some(next) = body["@odata.nextLink"].as_str() {
                if emails.len() >= MAX_BATCH {
                    break next.to_string();
                }
                next_url = next.to_string();
                continue;
            }

            if let Some(delta) = body["@odata.deltaLink"].as_str() {
                break delta.to_string();
            }

            return Err(ProviderError::Protocol(
                "Microsoft Graph response had neither a next nor a delta link".into(),
            ));
        };

        Ok(FetchBatch {
            emails,
            next_cursor: SyncCursor::Graph { link: last_link },
        })
    }
}

/// Builds the initial `delta` request URL for `folder`. With `with_filter`, restricts
/// results to `receivedDateTime ge {since}` server-side (the normal path); without it,
/// the caller must discard older messages itself (the 400 fallback, spec §8.2).
fn build_delta_url(
    graph_base_url: &str,
    folder: &str,
    since: DateTime<Utc>,
    with_filter: bool,
) -> String {
    let mut url = Url::parse(graph_base_url).expect("GRAPH_BASE_URL is a valid absolute URL");
    {
        let mut segments = url
            .path_segments_mut()
            .expect("Graph base URL is not a cannot-be-a-base URL");
        segments.push("me");
        segments.push("mailFolders");
        segments.push(folder);
        segments.push("messages");
        segments.push("delta");
    }
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("$select", GRAPH_SELECT_FIELDS);
        if with_filter {
            let since_str = since.to_rfc3339_opts(SecondsFormat::Secs, true);
            query.append_pair("$filter", &format!("receivedDateTime ge {since_str}"));
        }
    }
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_delta_url_with_filter_url_encodes_the_folder_and_since() {
        let since = "2026-01-15T10:30:00Z".parse::<DateTime<Utc>>().unwrap();
        let url = build_delta_url(
            "https://graph.microsoft.com/v1.0",
            "Archivo de correo",
            since,
            true,
        );
        assert!(url.starts_with(
            "https://graph.microsoft.com/v1.0/me/mailFolders/Archivo%20de%20correo/messages/delta?"
        ));
        assert!(url.contains("%24select=") || url.contains("$select="));
        assert!(url.contains("receivedDateTime"));
        assert!(url.contains("2026-01-15T10%3A30%3A00Z") || url.contains("2026-01-15T10:30:00Z"));
    }

    #[test]
    fn build_delta_url_without_filter_omits_the_filter_param() {
        let since = Utc::now();
        let url = build_delta_url("https://graph.microsoft.com/v1.0", "inbox", since, false);
        assert!(!url.contains("filter"));
    }
}
