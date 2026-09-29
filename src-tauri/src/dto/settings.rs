//! Settings DTOs.

use academic_core::domain::{AiProviderKind, Settings};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub ai_provider: AiProviderKind,
    pub jev_model: String,
    pub jev_api_key_configured: bool,
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub microsoft_client_id: Option<String>,
    pub blocked_senders: Vec<String>,
}

impl SettingsDto {
    pub fn new(settings: &Settings, jev_api_key_configured: bool) -> Self {
        Self {
            ai_provider: settings.ai_provider,
            jev_model: settings.jev_model.clone(),
            jev_api_key_configured,
            ollama_base_url: settings.ollama_base_url.clone(),
            ollama_model: settings.ollama_model.clone(),
            microsoft_client_id: settings.microsoft_client_id.clone(),
            blocked_senders: settings.blocked_senders.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsInput {
    pub ai_provider: AiProviderKind,
    pub jev_model: String,
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub microsoft_client_id: Option<String>,
    pub blocked_senders: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn settings_dto_matches_the_typescript_shape() {
        let dto = SettingsDto::new(&Settings::default(), true);
        assert_eq!(
            serde_json::to_value(&dto).unwrap(),
            json!({
                "aiProvider": "ollama",
                "jevModel": "jev-latest",
                "jevApiKeyConfigured": true,
                "ollamaBaseUrl": "http://localhost:11434",
                "ollamaModel": "qwen3:8b",
                "microsoftClientId": null,
                "blockedSenders": []
            })
        );
    }

    #[test]
    fn settings_input_deserializes_what_the_client_sends() {
        let input: SettingsInput = serde_json::from_value(json!({
            "aiProvider": "jev",
            "jevModel": "jev-latest",
            "ollamaBaseUrl": "http://localhost:11434",
            "ollamaModel": "qwen3:8b",
            "microsoftClientId": "abc",
            "blockedSenders": ["@uv.es", "spam@x.com"]
        }))
        .unwrap();
        assert_eq!(input.ai_provider, AiProviderKind::Jev);
        assert_eq!(input.microsoft_client_id.as_deref(), Some("abc"));
        assert_eq!(input.blocked_senders, vec!["@uv.es", "spam@x.com"]);
    }

    #[test]
    fn settings_input_accepts_null_client_id() {
        let input: SettingsInput = serde_json::from_value(json!({
            "aiProvider": "ollama",
            "jevModel": "m",
            "ollamaBaseUrl": "http://x",
            "ollamaModel": "m",
            "microsoftClientId": null,
            "blockedSenders": []
        }))
        .unwrap();
        assert_eq!(input.microsoft_client_id, None);
    }
}
