//! IMAP email provider (Task T8): implicit-TLS connection, incremental `UID SEARCH`
//! / `UID FETCH` sync and RFC 822 parsing. See `docs/infra/imap.md`.

mod connection;
mod cursor;
mod parse;

use std::fmt;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;

use academic_core::domain::{AccountId, FetchBatch, SyncCursor, MAX_BATCH};
use academic_core::ports::{EmailProvider, ProviderError};

use connection::{connect, map_command_error, ImapSession};

pub use cursor::select_uids;
pub use parse::parse_message;

/// Read-only [`EmailProvider`] for a single IMAP mailbox, connecting over implicit
/// TLS and fetching with `BODY.PEEK[]` so messages are never marked `\Seen`.
pub struct ImapEmailProvider {
    account_id: AccountId,
    host: String,
    port: u16,
    username: String,
    password: String,
}

impl ImapEmailProvider {
    pub fn new(
        account_id: AccountId,
        host: String,
        port: u16,
        username: String,
        password: String,
    ) -> Self {
        Self {
            account_id,
            host,
            port,
            username,
            password,
        }
    }

    async fn open_session(&self) -> Result<ImapSession, ProviderError> {
        connect(&self.host, self.port, &self.username, &self.password).await
    }

    async fn fetch_new_on(
        &self,
        session: &mut ImapSession,
        folder: &str,
        cursor: Option<&SyncCursor>,
        since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError> {
        let mailbox = session.select(folder).await.map_err(map_command_error)?;
        let uidvalidity = mailbox.uid_validity.ok_or_else(|| {
            ProviderError::Protocol("server did not return UIDVALIDITY on SELECT".into())
        })?;

        // A cursor from a different UIDVALIDITY is stale (the mailbox was recreated);
        // deduplication on the way in makes it safe to just treat this as no cursor.
        let cursor_last_uid = match cursor {
            Some(SyncCursor::Imap {
                uidvalidity: cursor_uidvalidity,
                last_uid,
            }) if *cursor_uidvalidity == uidvalidity => Some(*last_uid),
            _ => None,
        };

        let search_query = match cursor_last_uid {
            Some(last_uid) => format!("UID {}:*", last_uid.saturating_add(1)),
            None => format!("SINCE {}", since.format("%d-%b-%Y")),
        };
        let found: Vec<u32> = session
            .uid_search(&search_query)
            .await
            .map_err(map_command_error)?
            .into_iter()
            .collect();

        let selected = select_uids(&found, cursor_last_uid, MAX_BATCH);
        if selected.is_empty() {
            let next_cursor = match cursor_last_uid {
                Some(last_uid) => SyncCursor::Imap {
                    uidvalidity,
                    last_uid,
                },
                None => {
                    let uid_next = mailbox.uid_next.ok_or_else(|| {
                        ProviderError::Protocol("server did not return UIDNEXT on SELECT".into())
                    })?;
                    SyncCursor::Imap {
                        uidvalidity,
                        last_uid: uid_next.saturating_sub(1),
                    }
                }
            };
            return Ok(FetchBatch {
                emails: Vec::new(),
                next_cursor,
            });
        }

        let uid_set = selected
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut fetch_stream = session
            .uid_fetch(&uid_set, "(UID INTERNALDATE BODY.PEEK[])")
            .await
            .map_err(map_command_error)?;

        let mut emails = Vec::with_capacity(selected.len());
        let mut max_uid: Option<u32> = None;
        while let Some(fetch) = fetch_stream.try_next().await.map_err(map_command_error)? {
            let uid = fetch
                .uid
                .ok_or_else(|| ProviderError::Protocol("FETCH response missing UID".into()))?;
            let body = fetch
                .body()
                .ok_or_else(|| ProviderError::Protocol("FETCH response missing body".into()))?;
            let internal_date = fetch.internal_date().map(|dt| dt.with_timezone(&Utc));

            emails.push(parse_message(
                self.account_id,
                folder,
                uid,
                body,
                internal_date,
            )?);
            max_uid = Some(max_uid.map_or(uid, |current| current.max(uid)));
        }
        drop(fetch_stream);

        let max_uid = max_uid.ok_or_else(|| {
            ProviderError::Protocol("server did not return the requested messages".into())
        })?;
        Ok(FetchBatch {
            emails,
            next_cursor: SyncCursor::Imap {
                uidvalidity,
                last_uid: max_uid,
            },
        })
    }
}

impl fmt::Debug for ImapEmailProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImapEmailProvider")
            .field("account_id", &self.account_id)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .finish()
    }
}

#[async_trait]
impl EmailProvider for ImapEmailProvider {
    async fn test_connection(&self) -> Result<(), ProviderError> {
        let mut session = self.open_session().await?;
        let _ = session.logout().await;
        Ok(())
    }

    async fn fetch_new(
        &self,
        folder: &str,
        cursor: Option<&SyncCursor>,
        since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError> {
        let mut session = self.open_session().await?;
        let result = self.fetch_new_on(&mut session, folder, cursor, since).await;
        let _ = session.logout().await;
        result
    }
}
