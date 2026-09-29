//! Commands: settings, the jev API key and the AI provider test.

use academic_core::domain::{ErrorKind, Settings, JEV_API_KEY};
use academic_core::ports::{Question, QuestionSet};
use tauri::State;

use super::logged;
use crate::dto::{ConnectionTestDto, SettingsDto, SettingsInput};
use crate::error::CommandError;
use crate::state::AppState;
use crate::validation;

const TEST_QUESTION_ID: &str = "greeting";
const TEST_QUESTION: &str = "This text is a greeting.";
const TEST_STATE: &str = "Hello";
const NO_ANSWER_MESSAGE: &str = "The AI provider did not answer the test question";

#[tauri::command]
pub async fn get_settings(state: State<'_, AppState>) -> Result<SettingsDto, CommandError> {
    logged("get_settings", current_settings(&state))
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, AppState>,
    input: SettingsInput,
) -> Result<SettingsDto, CommandError> {
    logged("update_settings", save_settings(&state, input))
}

#[tauri::command]
pub async fn set_jev_api_key(state: State<'_, AppState>, key: String) -> Result<(), CommandError> {
    logged("set_jev_api_key", store_jev_key(&state, &key))
}

#[tauri::command]
pub async fn clear_jev_api_key(state: State<'_, AppState>) -> Result<(), CommandError> {
    logged("clear_jev_api_key", remove_jev_key(&state))
}

#[tauri::command]
pub async fn test_ai_provider(
    state: State<'_, AppState>,
) -> Result<ConnectionTestDto, CommandError> {
    logged("test_ai_provider", ask_test_question(&state).await)
}

fn settings_dto(state: &AppState, settings: &Settings) -> Result<SettingsDto, CommandError> {
    let jev_configured = state.secrets.get(JEV_API_KEY)?.is_some();
    Ok(SettingsDto::new(settings, jev_configured))
}

fn current_settings(state: &AppState) -> Result<SettingsDto, CommandError> {
    settings_dto(state, &state.settings.load()?)
}

fn save_settings(state: &AppState, input: SettingsInput) -> Result<SettingsDto, CommandError> {
    let settings = validation::settings(input)?;
    state.settings.save(&settings)?;
    settings_dto(state, &settings)
}

fn store_jev_key(state: &AppState, key: &str) -> Result<(), CommandError> {
    let key = validation::api_key(key)?;
    Ok(state.secrets.set(JEV_API_KEY, &key)?)
}

fn remove_jev_key(state: &AppState) -> Result<(), CommandError> {
    Ok(state.secrets.delete(JEV_API_KEY)?)
}

/// Builds the configured model and asks it one fixed yes/no question. The test
/// passes when an answer for that question comes back.
async fn ask_test_question(state: &AppState) -> Result<ConnectionTestDto, CommandError> {
    let settings = state.settings.load()?;
    let model = match state.models.current(&settings) {
        Ok(model) => model,
        Err(message) => return Ok(ConnectionTestDto::failure(ErrorKind::Ai, message)),
    };

    let mut questions = QuestionSet::new();
    questions.insert(
        TEST_QUESTION_ID.to_string(),
        Question::Noul {
            instructions: TEST_QUESTION.to_string(),
        },
    );
    Ok(
        match model.ask(&serde_json::json!(TEST_STATE), &questions).await {
            Ok(answers) if answers.contains_key(TEST_QUESTION_ID) => ConnectionTestDto::success(),
            Ok(_) => ConnectionTestDto::failure(ErrorKind::Ai, NO_ANSWER_MESSAGE),
            Err(error) => ConnectionTestDto::failure(ErrorKind::Ai, error.to_string()),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CommandErrorKind;
    use crate::test_support::{harness, harness_with, FakeModel, FakeProviderBehavior};
    use academic_core::domain::AiProviderKind;
    use academic_core::ports::{AnswerSet, ModelError, SecretStore};
    use std::sync::Arc;

    fn input() -> SettingsInput {
        SettingsInput {
            ai_provider: AiProviderKind::Jev,
            jev_model: "jev-latest".into(),
            ollama_base_url: "http://localhost:11434".into(),
            ollama_model: "qwen3:8b".into(),
            microsoft_client_id: Some(" client-id ".into()),
            blocked_senders: vec![" @uv.es ".into(), "".into()],
        }
    }

    #[test]
    fn defaults_are_returned_before_anything_is_saved() {
        let h = harness();
        let dto = current_settings(&h.state).unwrap();
        assert_eq!(dto.ai_provider, AiProviderKind::Ollama);
        assert!(!dto.jev_api_key_configured);
    }

    #[test]
    fn saved_settings_are_normalized_and_persisted() {
        let h = harness();
        let dto = save_settings(&h.state, input()).unwrap();
        assert_eq!(dto.microsoft_client_id.as_deref(), Some("client-id"));
        assert_eq!(dto.blocked_senders, vec!["@uv.es"]);
        assert_eq!(current_settings(&h.state).unwrap(), dto);
    }

    #[test]
    fn invalid_settings_are_rejected_without_saving() {
        let h = harness();
        let mut bad = input();
        bad.ollama_base_url = "nope".into();
        let error = save_settings(&h.state, bad).unwrap_err();
        assert_eq!(error.message, "Enter a valid URL");
        assert_eq!(
            current_settings(&h.state).unwrap().ai_provider,
            AiProviderKind::Ollama
        );
    }

    #[test]
    fn jev_key_is_stored_trimmed_and_never_returned() {
        let h = harness();
        store_jev_key(&h.state, "  test-key ").unwrap();
        assert_eq!(
            h.secrets.get(JEV_API_KEY).unwrap().as_deref(),
            Some("test-key")
        );

        let dto = current_settings(&h.state).unwrap();
        assert!(dto.jev_api_key_configured);
        assert!(!serde_json::to_string(&dto).unwrap().contains("test-key"));
    }

    #[test]
    fn blank_jev_key_is_a_validation_error() {
        let h = harness();
        let error = store_jev_key(&h.state, "   ").unwrap_err();
        assert_eq!(error.kind, CommandErrorKind::Validation);
        assert_eq!(error.message, "Enter an API key");
    }

    #[test]
    fn clearing_the_jev_key_is_idempotent() {
        let h = harness();
        remove_jev_key(&h.state).unwrap();
        store_jev_key(&h.state, "test-key").unwrap();
        remove_jev_key(&h.state).unwrap();
        remove_jev_key(&h.state).unwrap();
        assert!(!current_settings(&h.state).unwrap().jev_api_key_configured);
    }

    #[tokio::test]
    async fn provider_test_passes_when_the_greeting_is_answered() {
        let h = harness();
        assert_eq!(
            ask_test_question(&h.state).await.unwrap(),
            ConnectionTestDto::success()
        );
    }

    #[tokio::test]
    async fn provider_test_reports_a_missing_configuration() {
        let h = harness_with(
            FakeProviderBehavior::default(),
            Err("Add your jev API key in Settings".into()),
        );
        assert_eq!(
            ask_test_question(&h.state).await.unwrap(),
            ConnectionTestDto::failure(ErrorKind::Ai, "Add your jev API key in Settings")
        );
    }

    #[tokio::test]
    async fn provider_test_reports_model_errors() {
        let model = FakeModel(|_| Err(ModelError::Unavailable("connection refused".into())));
        let h = harness_with(FakeProviderBehavior::default(), Ok(Arc::new(model)));
        let dto = ask_test_question(&h.state).await.unwrap();
        assert!(!dto.ok);
        assert_eq!(dto.error_kind, Some(ErrorKind::Ai));
        assert_eq!(
            dto.message.as_deref(),
            Some("The AI provider is unavailable: connection refused")
        );
    }

    #[tokio::test]
    async fn provider_test_fails_when_the_answer_is_missing() {
        let model = FakeModel(|_| Ok(AnswerSet::new()));
        let h = harness_with(FakeProviderBehavior::default(), Ok(Arc::new(model)));
        let dto = ask_test_question(&h.state).await.unwrap();
        assert!(!dto.ok);
        assert_eq!(dto.message.as_deref(), Some(NO_ANSWER_MESSAGE));
    }
}
