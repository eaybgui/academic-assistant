//! IMAP email provider (Task T8): implicit-TLS connection, incremental `UID SEARCH`
//! / `UID FETCH` sync and RFC 822 parsing. See `docs/infra/imap.md`.

mod connection;
mod cursor;
mod parse;

use std::fmt;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures_util::TryStreamExt;
use tokio::time::timeout;

use academic_core::domain::{AccountId, FetchBatch, SyncCursor, MAX_BATCH};
use academic_core::ports::{EmailProvider, ProviderError};

use connection::{connect, map_command_error, ImapSession};

pub use cursor::select_uids;
pub use parse::{parse_fetched_messages, parse_message, FetchedMessage};

/// Timeout applied to every IMAP command issued after login: `SELECT`, `UID SEARCH`,
/// and the whole `UID FETCH` stream consumption (not per message — the entire
/// batch). A stalled server must not hang a sync forever.
const COMMAND_TIMEOUT: Duration = Duration::from_secs(60);
/// `LOGOUT` is best-effort (its result is always ignored) and kept short so a slow
/// server does not delay returning the already-fetched result.
const LOGOUT_TIMEOUT: Duration = Duration::from_secs(5);

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

    /// `LOGOUT` is always attempted, best effort, on every path (success or
    /// failure); its result — including a timeout — is ignored.
    async fn logout(session: &mut ImapSession) {
        let _ = timeout(LOGOUT_TIMEOUT, session.logout()).await;
    }

    /// Runs the `UID FETCH` and consumes its response stream, collecting each
    /// message's raw bytes (never parsing here — see [`parse_fetched_messages`]).
    async fn fetch_messages(
        session: &mut ImapSession,
        uid_set: &str,
        capacity: usize,
    ) -> Result<Vec<FetchedMessage>, ProviderError> {
        let mut fetch_stream = session
            .uid_fetch(uid_set, "(UID INTERNALDATE BODY.PEEK[])")
            .await
            .map_err(map_command_error)?;

        let mut fetched = Vec::with_capacity(capacity);
        while let Some(fetch) = fetch_stream.try_next().await.map_err(map_command_error)? {
            let uid = fetch
                .uid
                .ok_or_else(|| ProviderError::Protocol("FETCH response missing UID".into()))?;
            let body = fetch
                .body()
                .ok_or_else(|| ProviderError::Protocol("FETCH response missing body".into()))?;
            let internal_date = fetch.internal_date().map(|dt| dt.with_timezone(&Utc));
            fetched.push((uid, body.to_vec(), internal_date));
        }
        Ok(fetched)
    }

    async fn fetch_new_on(
        &self,
        session: &mut ImapSession,
        folder: &str,
        cursor: Option<&SyncCursor>,
        since: DateTime<Utc>,
    ) -> Result<FetchBatch, ProviderError> {
        let mailbox = timeout(COMMAND_TIMEOUT, session.select(folder))
            .await
            .map_err(|_| command_timed_out("SELECT"))?
            .map_err(map_command_error)?;
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
        let found: Vec<u32> = timeout(COMMAND_TIMEOUT, session.uid_search(&search_query))
            .await
            .map_err(|_| command_timed_out("UID SEARCH"))?
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

        let fetched = timeout(
            COMMAND_TIMEOUT,
            Self::fetch_messages(session, &uid_set, selected.len()),
        )
        .await
        .map_err(|_| command_timed_out("FETCH"))??;

        let (emails, max_uid) = parse_fetched_messages(self.account_id, folder, &fetched);
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

fn command_timed_out(command: &str) -> ProviderError {
    ProviderError::Connection(format!("{command} timed out"))
}

#[async_trait]
impl EmailProvider for ImapEmailProvider {
    async fn test_connection(&self) -> Result<(), ProviderError> {
        let mut session = self.open_session().await?;
        Self::logout(&mut session).await;
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
        Self::logout(&mut session).await;
        result
    }
}
