use serde::{Deserialize, Serialize};

use super::enum_str;

/// Which AI provider is configured to run structured analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AiProviderKind {
    Ollama,
    Jev,
}

enum_str!(AiProviderKind {
    Ollama => "ollama",
    Jev => "jev",
});

/// User-configurable application settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub ai_provider: AiProviderKind,
    pub jev_model: String,
    pub ollama_base_url: String,
    pub ollama_model: String,
    pub microsoft_client_id: Option<String>,
    pub blocked_senders: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            ai_provider: AiProviderKind::Ollama,
            jev_model: "jev-latest".to_string(),
            ollama_base_url: "http://localhost:11434".to_string(),
            ollama_model: "qwen3:8b".to_string(),
            microsoft_client_id: None,
            blocked_senders: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn ai_provider_kind_round_trips() {
        for k in [AiProviderKind::Ollama, AiProviderKind::Jev] {
            assert_eq!(AiProviderKind::from_str(k.as_str()).unwrap(), k);
        }
    }

    #[test]
    fn settings_default_matches_spec() {
        let settings = Settings::default();
        assert_eq!(settings.ai_provider, AiProviderKind::Ollama);
        assert_eq!(settings.jev_model, "jev-latest");
        assert_eq!(settings.ollama_base_url, "http://localhost:11434");
        assert_eq!(settings.ollama_model, "qwen3:8b");
        assert_eq!(settings.microsoft_client_id, None);
        assert!(settings.blocked_senders.is_empty());
    }
}
