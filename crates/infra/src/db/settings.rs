//! `SettingsRepository` implementation.
//!
//! Each [`Settings`] field is stored as its own row in the single-table `settings`
//! key/value store, JSON-encoded. A key missing from the table (nothing has ever
//! saved it) falls back to the corresponding [`Settings::default`] value.

use academic_core::domain::{AiProviderKind, Settings};
use academic_core::ports::{RepoResult, SettingsRepository};
use rusqlite::params;

use super::util::{map_err, parse_enum, parse_json, to_json};
use super::Database;

const KEY_AI_PROVIDER: &str = "ai.provider";
const KEY_JEV_MODEL: &str = "jev.model";
const KEY_OLLAMA_BASE_URL: &str = "ollama.base_url";
const KEY_OLLAMA_MODEL: &str = "ollama.model";
const KEY_MICROSOFT_CLIENT_ID: &str = "microsoft.client_id";
const KEY_FILTERS_BLOCKED: &str = "filters.blocked";

impl SettingsRepository for Database {
    fn load(&self) -> RepoResult<Settings> {
        let conn = self.conn()?;
        let mut stmt = conn
            .prepare("SELECT key, value FROM settings")
            .map_err(map_err)?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(map_err)?;

        let mut settings = Settings::default();
        for row in rows {
            let (key, value) = row.map_err(map_err)?;
            match key.as_str() {
                KEY_AI_PROVIDER => {
                    let raw: String = parse_json(&value)?;
                    settings.ai_provider = parse_enum::<AiProviderKind>(&raw)?;
                }
                KEY_JEV_MODEL => settings.jev_model = parse_json(&value)?,
                KEY_OLLAMA_BASE_URL => settings.ollama_base_url = parse_json(&value)?,
                KEY_OLLAMA_MODEL => settings.ollama_model = parse_json(&value)?,
                KEY_MICROSOFT_CLIENT_ID => settings.microsoft_client_id = parse_json(&value)?,
                KEY_FILTERS_BLOCKED => settings.blocked_senders = parse_json(&value)?,
                _ => {}
            }
        }
        Ok(settings)
    }

    fn save(&self, settings: &Settings) -> RepoResult<()> {
        let rows = [
            (KEY_AI_PROVIDER, to_json(&settings.ai_provider.as_str())?),
            (KEY_JEV_MODEL, to_json(&settings.jev_model)?),
            (KEY_OLLAMA_BASE_URL, to_json(&settings.ollama_base_url)?),
            (KEY_OLLAMA_MODEL, to_json(&settings.ollama_model)?),
            (
                KEY_MICROSOFT_CLIENT_ID,
                to_json(&settings.microsoft_client_id)?,
            ),
            (KEY_FILTERS_BLOCKED, to_json(&settings.blocked_senders)?),
        ];

        let mut conn = self.conn()?;
        let tx = conn.transaction().map_err(map_err)?;
        for (key, value) in rows {
            tx.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![key, value],
            )
            .map_err(map_err)?;
        }
        tx.commit().map_err(map_err)?;
        Ok(())
    }
}
