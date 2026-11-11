//! Validation and normalization of command inputs.
//!
//! Messages are user-facing: sentence case, no trailing period. Every function
//! returns the trimmed/normalized value that the commands then hand to the domain.

use academic_core::domain::PersonalTaskDraft;
use chrono::NaiveDate;

use crate::dto::PersonalTaskInput;
use crate::error::CommandError;

#[cfg(not(target_os = "android"))]
mod email_ai;

#[cfg(not(target_os = "android"))]
pub use email_ai::*;

const MAX_NOTE_CHARS: usize = 2000;
const MAX_TITLE_CHARS: usize = 200;

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

/// Trims a personal task and checks its title, day and note.
pub fn personal_task(input: PersonalTaskInput) -> Result<PersonalTaskDraft, CommandError> {
    let title = required_text(&input.title, "Enter a title")?;
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(CommandError::validation(
            "Titles can have at most 200 characters",
        ));
    }
    let date = NaiveDate::parse_from_str(&input.date, "%Y-%m-%d")
        .map_err(|_| CommandError::validation("Enter a valid day"))?;
    Ok(PersonalTaskDraft {
        title,
        date,
        note: item_note(input.note)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::{AccountInput, ProviderInput, ResultQueryInput, SettingsInput};
    use academic_core::domain::{AiProviderKind, ProviderConfig, ResultSort};

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
