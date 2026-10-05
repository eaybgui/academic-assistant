//! Validation and normalization of command inputs.
//!
//! Messages are user-facing: sentence case, no trailing period. Every function
//! returns the trimmed/normalized value that the commands then hand to the domain.

use academic_core::domain::{AccountDraft, ProviderConfig, ResultQuery, Settings};
use academic_infra::graph::oauth;

use crate::dto::{AccountInput, ProviderInput, ResultQueryInput, SettingsInput};
use crate::error::CommandError;

pub const MAX_PAGE_SIZE: i64 = 200;
const MAX_NOTE_CHARS: usize = 2000;
const MIN_PAGE_SIZE: i64 = 1;

/// An [`AccountInput`] that passed validation: the domain draft plus the IMAP
/// password, when one was supplied (never part of the draft, never persisted in
/// the database).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedAccount {
    pub draft: AccountDraft,
    pub password: Option<String>,
}

/// Trims `value` and rejects it when empty, with `message` as the error.
pub fn required_text(value: &str, message: &str) -> Result<String, CommandError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Err(CommandError::validation(message))
    } else {
        Ok(trimmed.to_string())
    }
}

pub fn name(value: &str) -> Result<String, CommandError> {
    required_text(value, "Enter a name")
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

/// Trims a personal item note; blank becomes `None`; at most 2000 characters.
pub fn item_note(value: Option<String>) -> Result<Option<String>, CommandError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.chars().count() > MAX_NOTE_CHARS {
        return Err(CommandError::validation(
            "Notes can have at most 2000 characters",
        ));
    }
    Ok((!trimmed.is_empty()).then(|| trimmed.to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use academic_core::domain::{AiProviderKind, ResultSort};

    fn message<T: std::fmt::Debug>(result: Result<T, CommandError>) -> String {
        result.unwrap_err().message
    }

    #[test]
    fn item_notes_are_trimmed_blank_means_none_and_length_is_capped() {
        assert_eq!(item_note(None).unwrap(), None);
        assert_eq!(item_note(Some("  ".into())).unwrap(), None);
        assert_eq!(
            item_note(Some(" call the tutor ".into()))
                .unwrap()
                .as_deref(),
            Some("call the tutor")
        );
        assert!(item_note(Some("ñ".repeat(2000))).is_ok());
        assert_eq!(
            message(item_note(Some("ñ".repeat(2001)))),
            "Notes can have at most 2000 characters"
        );
    }

    fn imap_input() -> AccountInput {
        AccountInput {
            name: "  UV IMAP ".into(),
            email_address: " me@uv.es ".into(),
            folders: vec![" INBOX ".into(), "  ".into()],
            default_university_id: Some(2),
            initial_sync_days: 14,
            enabled: true,
            provider: ProviderInput::Imap {
                host: " imap.uv.es ".into(),
                port: 993,
                username: " me ".into(),
                password: Some("test-password".into()),
            },
        }
    }

    #[test]
    fn name_is_trimmed_and_required() {
        assert_eq!(name("  UV ").unwrap(), "UV");
        assert_eq!(message(name("   ")), "Enter a name");
    }

    #[test]
    fn email_requires_local_part_and_domain() {
        assert_eq!(email(" a@b.c ").unwrap(), "a@b.c");
        for invalid in ["", "abc", "@b.c", "a@", "a b@c.d"] {
            assert_eq!(message(email(invalid)), "Enter a valid email address");
        }
    }

    #[test]
    fn port_must_be_between_1_and_65535() {
        assert_eq!(port(1).unwrap(), 1);
        assert_eq!(port(65535).unwrap(), 65535);
        for invalid in [0, -1, 65536, 100_000] {
            assert_eq!(message(port(invalid)), "Enter a port between 1 and 65535");
        }
    }

    #[test]
    fn initial_sync_days_must_be_between_1_and_365() {
        assert_eq!(initial_sync_days(1).unwrap(), 1);
        assert_eq!(initial_sync_days(365).unwrap(), 365);
        for invalid in [0, -5, 366] {
            assert_eq!(
                message(initial_sync_days(invalid)),
                "Enter between 1 and 365 days"
            );
        }
    }

    #[test]
    fn folders_are_trimmed_and_must_not_be_empty() {
        assert_eq!(
            folders(vec![" INBOX ".into(), "".into(), "Courses".into()]).unwrap(),
            vec!["INBOX", "Courses"]
        );
        assert_eq!(
            message(folders(vec!["  ".into()])),
            "Add at least one folder"
        );
        assert_eq!(message(folders(vec![])), "Add at least one folder");
    }

    #[test]
    fn url_requires_http_or_https_with_a_host() {
        assert_eq!(
            url(" http://localhost:11434 ").unwrap(),
            "http://localhost:11434"
        );
        assert_eq!(url("HTTPS://example.com").unwrap(), "HTTPS://example.com");
        for invalid in ["", "localhost:11434", "ftp://x", "http://", "http://a b"] {
            assert_eq!(message(url(invalid)), "Enter a valid URL");
        }
    }

    #[test]
    fn model_name_and_api_key_are_required() {
        assert_eq!(model_name(" qwen3:8b ").unwrap(), "qwen3:8b");
        assert_eq!(message(model_name("")), "Enter a model name");
        assert_eq!(api_key(" k ").unwrap(), "k");
        assert_eq!(message(api_key("  ")), "Enter an API key");
    }

    #[test]
    fn non_blank_trims_and_drops_empty_entries() {
        assert_eq!(
            non_blank(vec![" a ".into(), "".into(), "  ".into(), "b".into()]),
            vec!["a", "b"]
        );
    }

    #[test]
    fn limit_is_clamped_between_1_and_200() {
        assert_eq!(clamp_limit(-3), 1);
        assert_eq!(clamp_limit(0), 1);
        assert_eq!(clamp_limit(25), 25);
        assert_eq!(clamp_limit(500), 200);
    }

    #[test]
    fn result_query_trims_search_and_clamps_paging() {
        let query = result_query(ResultQueryInput {
            search: Some("  exam  ".into()),
            seen: Some(false),
            category: None,
            account_id: Some(1),
            course_id: None,
            sort: ResultSort::Priority,
            limit: 1000,
            offset: -4,
        });
        assert_eq!(query.search.as_deref(), Some("exam"));
        assert_eq!(query.seen, Some(false));
        assert_eq!(query.account_id, Some(1));
        assert_eq!(query.sort, ResultSort::Priority);
        assert_eq!(query.limit, 200);
        assert_eq!(query.offset, 0);
    }

    #[test]
    fn result_query_turns_blank_search_into_none() {
        let query = result_query(ResultQueryInput {
            search: Some("   ".into()),
            seen: None,
            category: None,
            account_id: None,
            course_id: None,
            sort: ResultSort::Received,
            limit: 25,
            offset: 50,
        });
        assert_eq!(query.search, None);
        assert_eq!(query.offset, 50);
    }

    #[test]
    fn imap_account_is_normalized() {
        let validated = account(imap_input()).unwrap();
        assert_eq!(validated.password.as_deref(), Some("test-password"));
        assert_eq!(validated.draft.name, "UV IMAP");
        assert_eq!(validated.draft.email_address, "me@uv.es");
        assert_eq!(validated.draft.folders, vec!["INBOX"]);
        assert_eq!(validated.draft.default_university_id, Some(2));
        assert_eq!(
            validated.draft.provider,
            ProviderConfig::Imap {
                host: "imap.uv.es".into(),
                port: 993,
                username: "me".into()
            }
        );
    }

    #[test]
    fn empty_imap_password_counts_as_not_provided() {
        let mut input = imap_input();
        if let ProviderInput::Imap { password, .. } = &mut input.provider {
            *password = Some(String::new());
        }
        assert_eq!(account(input).unwrap().password, None);
    }

    #[test]
    fn graph_account_has_no_password() {
        let mut input = imap_input();
        input.provider = ProviderInput::MicrosoftGraph {
            tenant: " organizations ".into(),
        };
        let validated = account(input).unwrap();
        assert_eq!(validated.password, None);
        assert_eq!(
            validated.draft.provider,
            ProviderConfig::MicrosoftGraph {
                tenant: "organizations".into()
            }
        );
    }

    #[test]
    fn graph_tenant_must_be_a_guid_domain_or_alias() {
        for tenant in [
            "organizations",
            "contoso.onmicrosoft.com",
            "11111111-2222-3333-4444-555555555555",
        ] {
            let mut input = imap_input();
            input.provider = ProviderInput::MicrosoftGraph {
                tenant: tenant.into(),
            };
            assert!(account(input).is_ok(), "{tenant}");
        }
        for tenant in ["tenant with spaces", "../etc", "a/b", "t?x=1"] {
            let mut input = imap_input();
            input.provider = ProviderInput::MicrosoftGraph {
                tenant: tenant.into(),
            };
            assert_eq!(message(account(input)), "Enter a valid tenant", "{tenant}");
        }
        let mut input = imap_input();
        input.provider = ProviderInput::MicrosoftGraph { tenant: " ".into() };
        assert_eq!(message(account(input)), "Enter a tenant");
    }

    #[test]
    fn account_reports_the_first_invalid_field() {
        let mut input = imap_input();
        input.name = " ".into();
        input.email_address = "bad".into();
        assert_eq!(message(account(input)), "Enter a name");

        let mut input = imap_input();
        input.email_address = "bad".into();
        assert_eq!(message(account(input)), "Enter a valid email address");

        let mut input = imap_input();
        input.provider = ProviderInput::Imap {
            host: " ".into(),
            port: 993,
            username: "me".into(),
            password: None,
        };
        assert_eq!(message(account(input)), "Enter a host");

        let mut input = imap_input();
        input.provider = ProviderInput::Imap {
            host: "h".into(),
            port: 0,
            username: "me".into(),
            password: None,
        };
        assert_eq!(message(account(input)), "Enter a port between 1 and 65535");

        let mut input = imap_input();
        input.folders = vec![];
        assert_eq!(message(account(input)), "Add at least one folder");

        let mut input = imap_input();
        input.initial_sync_days = 400;
        assert_eq!(message(account(input)), "Enter between 1 and 365 days");
    }

    fn settings_input() -> SettingsInput {
        SettingsInput {
            ai_provider: AiProviderKind::Jev,
            jev_model: " jev-latest ".into(),
            ollama_base_url: " http://localhost:11434 ".into(),
            ollama_model: " qwen3:8b ".into(),
            microsoft_client_id: Some("  ".into()),
            blocked_senders: vec![" @uv.es ".into(), " ".into()],
        }
    }

    #[test]
    fn settings_are_normalized() {
        let settings = settings(settings_input()).unwrap();
        assert_eq!(settings.ai_provider, AiProviderKind::Jev);
        assert_eq!(settings.jev_model, "jev-latest");
        assert_eq!(settings.ollama_base_url, "http://localhost:11434");
        assert_eq!(settings.ollama_model, "qwen3:8b");
        assert_eq!(settings.microsoft_client_id, None);
        assert_eq!(settings.blocked_senders, vec!["@uv.es"]);
    }

    #[test]
    fn settings_reject_invalid_url_and_blank_models() {
        let mut input = settings_input();
        input.ollama_base_url = "localhost".into();
        assert_eq!(message(settings(input)), "Enter a valid URL");

        let mut input = settings_input();
        input.ollama_model = " ".into();
        assert_eq!(message(settings(input)), "Enter a model name");

        let mut input = settings_input();
        input.jev_model = "".into();
        assert_eq!(message(settings(input)), "Enter a model name");
    }
}
