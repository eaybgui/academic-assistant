//! `AccountRepository` implementation.

use academic_core::domain::{Account, AccountDraft, AccountId, ProviderConfig, ProviderKind};
use academic_core::ports::{AccountRepository, RepoError, RepoResult};
use chrono::{DateTime, Utc};
use rusqlite::{params, OptionalExtension, Row};
use serde::Deserialize;

use super::util::{map_err, parse_datetime, parse_enum, parse_json, to_json, to_rfc3339};
use super::Database;

const SELECT_COLUMNS: &str = "id, name, email_address, provider_type, config, folders, \
    default_university_id, initial_sync_days, enabled, created_at, updated_at";

struct AccountRow {
    id: i64,
    name: String,
    email_address: String,
    provider_type: String,
    config: String,
    folders: String,
    default_university_id: Option<i64>,
    initial_sync_days: i64,
    enabled: i64,
    created_at: String,
    updated_at: String,
}

fn read_row(row: &Row) -> rusqlite::Result<AccountRow> {
    Ok(AccountRow {
        id: row.get(0)?,
        name: row.get(1)?,
        email_address: row.get(2)?,
        provider_type: row.get(3)?,
        config: row.get(4)?,
        folders: row.get(5)?,
        default_university_id: row.get(6)?,
        initial_sync_days: row.get(7)?,
        enabled: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn into_account(row: AccountRow) -> RepoResult<Account> {
    let kind: ProviderKind = parse_enum(&row.provider_type)?;
    Ok(Account {
        id: row.id,
        name: row.name,
        email_address: row.email_address,
        provider: provider_config_from_json(kind, &row.config)?,
        folders: parse_json(&row.folders)?,
        default_university_id: row.default_university_id,
        initial_sync_days: row.initial_sync_days as u32,
        enabled: row.enabled != 0,
        created_at: parse_datetime(&row.created_at)?,
        updated_at: parse_datetime(&row.updated_at)?,
    })
}

/// Serializes a [`ProviderConfig`] to its `accounts.config` JSON representation,
/// which omits the type tag (that is stored separately in `provider_type`).
fn provider_config_to_json(provider: &ProviderConfig) -> RepoResult<String> {
    let value = match provider {
        ProviderConfig::Imap {
            host,
            port,
            username,
        } => serde_json::json!({ "host": host, "port": port, "username": username }),
        ProviderConfig::MicrosoftGraph { tenant } => serde_json::json!({ "tenant": tenant }),
    };
    serde_json::to_string(&value).map_err(|e| RepoError::Storage(format!("invalid JSON: {e}")))
}

fn provider_config_from_json(kind: ProviderKind, json: &str) -> RepoResult<ProviderConfig> {
    #[derive(Deserialize)]
    struct ImapConfig {
        host: String,
        port: u16,
        username: String,
    }
    #[derive(Deserialize)]
    struct GraphConfig {
        tenant: String,
    }

    match kind {
        ProviderKind::Imap => {
            let cfg: ImapConfig = parse_json(json)?;
            Ok(ProviderConfig::Imap {
                host: cfg.host,
                port: cfg.port,
                username: cfg.username,
            })
        }
        ProviderKind::MicrosoftGraph => {
            let cfg: GraphConfig = parse_json(json)?;
            Ok(ProviderConfig::MicrosoftGraph { tenant: cfg.tenant })
        }
    }
}

impl AccountRepository for Database {
    fn list(&self) -> RepoResult<Vec<Account>> {
        let conn = self.conn()?;
        let sql = format!("SELECT {SELECT_COLUMNS} FROM accounts ORDER BY id");
        let mut stmt = conn.prepare(&sql).map_err(map_err)?;
        let rows = stmt.query_map([], read_row).map_err(map_err)?;
        let mut accounts = Vec::new();
        for row in rows {
            accounts.push(into_account(row.map_err(map_err)?)?);
        }
        Ok(accounts)
    }

    fn get(&self, id: AccountId) -> RepoResult<Option<Account>> {
        let conn = self.conn()?;
        let sql = format!("SELECT {SELECT_COLUMNS} FROM accounts WHERE id = ?1");
        let row = conn
            .query_row(&sql, params![id], read_row)
            .optional()
            .map_err(map_err)?;
        row.map(into_account).transpose()
    }

    fn insert(&self, draft: &AccountDraft, now: DateTime<Utc>) -> RepoResult<Account> {
        let config_json = provider_config_to_json(&draft.provider)?;
        let folders_json = to_json(&draft.folders)?;
        let now_str = to_rfc3339(now);
        let id = {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO accounts (name, email_address, provider_type, config, folders, \
                 default_university_id, initial_sync_days, enabled, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                params![
                    draft.name,
                    draft.email_address,
                    draft.provider.kind().as_str(),
                    config_json,
                    folders_json,
                    draft.default_university_id,
                    draft.initial_sync_days as i64,
                    draft.enabled,
                    now_str,
                    now_str,
                ],
            )
            .map_err(map_err)?;
            conn.last_insert_rowid()
        };
        self.get(id)?
            .ok_or_else(|| RepoError::Storage("account not found after insert".into()))
    }

    fn update(
        &self,
        id: AccountId,
        draft: &AccountDraft,
        now: DateTime<Utc>,
    ) -> RepoResult<Account> {
        let config_json = provider_config_to_json(&draft.provider)?;
        let folders_json = to_json(&draft.folders)?;
        let changed = {
            let conn = self.conn()?;
            conn.execute(
                "UPDATE accounts SET name = ?1, email_address = ?2, provider_type = ?3, \
                 config = ?4, folders = ?5, default_university_id = ?6, initial_sync_days = ?7, \
                 enabled = ?8, updated_at = ?9 WHERE id = ?10",
                params![
                    draft.name,
                    draft.email_address,
                    draft.provider.kind().as_str(),
                    config_json,
                    folders_json,
                    draft.default_university_id,
                    draft.initial_sync_days as i64,
                    draft.enabled,
                    to_rfc3339(now),
                    id,
                ],
            )
            .map_err(map_err)?
        };
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        self.get(id)?.ok_or(RepoError::NotFound)
    }

    fn delete(&self, id: AccountId) -> RepoResult<()> {
        let conn = self.conn()?;
        let changed = conn
            .execute("DELETE FROM accounts WHERE id = ?1", params![id])
            .map_err(map_err)?;
        if changed == 0 {
            return Err(RepoError::NotFound);
        }
        Ok(())
    }
}
