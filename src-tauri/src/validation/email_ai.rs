//! Validation of the email-account, AI-settings and result-list inputs (desktop only).

use academic_core::domain::{AccountDraft, ProviderConfig, ResultQuery, Settings};
use academic_infra::graph::oauth;

use crate::dto::{AccountInput, ProviderInput, ResultQueryInput, SettingsInput};
use crate::error::CommandError;

use super::{name, required_text};

pub const MAX_PAGE_SIZE: i64 = 200;
const MIN_PAGE_SIZE: i64 = 1;

/// An [`AccountInput`] that passed validation: the domain draft plus the IMAP
/// password, when one was supplied (never part of the draft, never persisted in
/// the database).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAccount {
    pub draft: AccountDraft,
    pub password: Option<String>,
}

pub fn email(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    let valid = trimmed.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty() && !domain.is_empty() && !trimmed.contains(char::is_whitespace)
    });
    if valid {
        Ok(trimmed.to_string())
    } else {
        Err(CommandError::validation("Enter a valid email address"))
    }
}

pub fn port(value: i64) -> Result<u16, CommandError> {
    u16::try_from(value)
        .ok()
        .filter(|port| *port >= 1)
        .ok_or_else(|| CommandError::validation("Enter a port between 1 and 65535"))
}

pub fn initial_sync_days(value: i64) -> Result<u32, CommandError> {
    u32::try_from(value)
        .ok()
        .filter(|days| (1..=365).contains(days))
        .ok_or_else(|| CommandError::validation("Enter between 1 and 365 days"))
}

/// Trims every folder, drops blanks and requires at least one to remain.
pub fn folders(values: Vec<String>) -> Result<Vec<String>, CommandError> {
    let folders = non_blank(values);
    if folders.is_empty() {
        Err(CommandError::validation("Add at least one folder"))
    } else {
        Ok(folders)
    }
}

/// Accepts `http://` and `https://` URLs with a non-empty rest and no whitespace.
pub fn url(value: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    let lower = trimmed.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("http://")
        .or_else(|| lower.strip_prefix("https://"));
    match rest {
        Some(rest) if !rest.is_empty() && !rest.contains(char::is_whitespace) => {
            Ok(trimmed.to_string())
        }
        _ => Err(CommandError::validation("Enter a valid URL")),
    }
}

pub fn model_name(value: &str) -> Result<String, CommandError> {
    required_text(value, "Enter a model name")
}

pub fn api_key(value: &str) -> Result<String, CommandError> {
    required_text(value, "Enter an API key")
}

/// Trims every entry and drops blanks (aliases, blocked senders).
pub fn non_blank(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

/// Clamps a page size into `1..=200`.
pub fn clamp_limit(value: i64) -> u32 {
    u32::try_from(value.clamp(MIN_PAGE_SIZE, MAX_PAGE_SIZE))
        .expect("limit is clamped into u32 range")
}

/// Builds the domain query: trims `search` (empty becomes `None`), clamps `limit`
/// into `1..=200` and negative offsets to 0.
pub fn result_query(input: ResultQueryInput) -> ResultQuery {
    let search = input
        .search
        .map(|search| search.trim().to_string())
        .filter(|search| !search.is_empty());
    ResultQuery {
        search,
        seen: input.seen,
        category: input.category,
        account_id: input.account_id,
        course_id: input.course_id,
        sort: input.sort,
        limit: clamp_limit(input.limit),
        offset: u32::try_from(input.offset.max(0)).unwrap_or(u32::MAX),
    }
}

pub fn account(input: AccountInput) -> Result<ValidatedAccount, CommandError> {
    let name = name(&input.name)?;
    let email_address = email(&input.email_address)?;
    let (provider, password) = provider(input.provider)?;
    let folders = folders(input.folders)?;
    let initial_sync_days = initial_sync_days(input.initial_sync_days)?;
    Ok(ValidatedAccount {
        draft: AccountDraft {
            name,
            email_address,
            provider,
            folders,
            default_university_id: input.default_university_id,
            initial_sync_days,
            enabled: input.enabled,
        },
        password,
    })
}

fn provider(input: ProviderInput) -> Result<(ProviderConfig, Option<String>), CommandError> {
    match input {
        ProviderInput::Imap {
            host,
            port: port_value,
            username,
            password,
        } => {
            let provider = ProviderConfig::Imap {
                host: required_text(&host, "Enter a host")?,
                port: port(port_value)?,
                username: required_text(&username, "Enter a username")?,
            };
            Ok((provider, password.filter(|password| !password.is_empty())))
        }
        ProviderInput::MicrosoftGraph { tenant } => Ok((
            ProviderConfig::MicrosoftGraph {
                tenant: tenant_id(&tenant)?,
            },
            None,
        )),
    }
}

/// Trims the Microsoft tenant and requires the shape the sign-in flow accepts
/// (a GUID, a domain or an alias such as `organizations`).
fn tenant_id(value: &str) -> Result<String, CommandError> {
    let tenant = required_text(value, "Enter a tenant")?;
    if oauth::is_valid_tenant(&tenant) {
        Ok(tenant)
    } else {
        Err(CommandError::validation("Enter a valid tenant"))
    }
}

pub fn settings(input: SettingsInput) -> Result<Settings, CommandError> {
    Ok(Settings {
        ai_provider: input.ai_provider,
        jev_model: model_name(&input.jev_model)?,
        ollama_base_url: url(&input.ollama_base_url)?,
        ollama_model: model_name(&input.ollama_model)?,
        microsoft_client_id: input
            .microsoft_client_id
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty()),
        blocked_senders: non_blank(input.blocked_senders),
    })
}
