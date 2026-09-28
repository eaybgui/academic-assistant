//! `ProcessedMessageRepository` implementation.

use academic_core::domain::{AccountId, ProcessedMessage};
use academic_core::ports::{ProcessedMessageRepository, RepoResult};
use rusqlite::params;

use super::util::{map_err, to_rfc3339};
use super::Database;

impl ProcessedMessageRepository for Database {
    fn exists(
        &self,
        account_id: AccountId,
        folder: &str,
        provider_message_id: &str,
    ) -> RepoResult<bool> {
        let conn = self.conn()?;
        let exists: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM processed_messages \
                 WHERE account_id = ?1 AND folder = ?2 AND provider_message_id = ?3)",
                params![account_id, folder, provider_message_id],
                |row| row.get(0),
            )
            .map_err(map_err)?;
        Ok(exists)
    }

    /// A message is a duplicate for an account when either its `Message-ID` header
    /// matches a previously processed message, or its content hash does (matching
    /// the `idx_processed_msgid`/`idx_processed_hash` indexes, both scoped by
    /// `account_id`). A missing header only falls back to the hash check: two
    /// headerless emails are not considered duplicates of each other by that
    /// criterion alone.
    fn is_duplicate(
        &self,
        account_id: AccountId,
        message_id_header: Option<&str>,
        content_hash: &str,
    ) -> RepoResult<bool> {
        let conn = self.conn()?;
        let exists: bool = match message_id_header {
            Some(header) => conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM processed_messages \
                     WHERE account_id = ?1 AND (message_id_header = ?2 OR content_hash = ?3))",
                    params![account_id, header, content_hash],
                    |row| row.get(0),
                )
                .map_err(map_err)?,
            None => conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM processed_messages \
                     WHERE account_id = ?1 AND content_hash = ?2)",
                    params![account_id, content_hash],
                    |row| row.get(0),
                )
                .map_err(map_err)?,
        };
        Ok(exists)
    }

    fn insert(&self, message: &ProcessedMessage) -> RepoResult<i64> {
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO processed_messages (account_id, folder, provider_message_id, \
             message_id_header, content_hash, received_at, outcome, reason, run_id, processed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                message.account_id,
                message.folder,
                message.provider_message_id,
                message.message_id_header,
                message.content_hash,
                to_rfc3339(message.received_at),
                message.outcome.as_str(),
                message.reason,
                message.run_id,
                to_rfc3339(message.processed_at),
            ],
        )
        .map_err(map_err)?;
        Ok(conn.last_insert_rowid())
    }
}
