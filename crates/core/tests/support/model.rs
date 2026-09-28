use std::collections::VecDeque;
use std::sync::Mutex;

use academic_core::domain::{ModelDescriptor, Settings};
use academic_core::ports::{AnswerSet, ModelError, ModelProvider, QuestionSet, StructuredModel};
use async_trait::async_trait;

/// A [`ModelError`]-shaped value that can be cloned and queued ahead of time.
#[derive(Clone)]
pub enum FakeModelError {
    Auth,
    RateLimited,
    Unavailable(String),
    InvalidResponse(String),
}

impl FakeModelError {
    fn into_model_error(self) -> ModelError {
        match self {
            FakeModelError::Auth => ModelError::Auth,
            FakeModelError::RateLimited => ModelError::RateLimited,
            FakeModelError::Unavailable(m) => ModelError::Unavailable(m),
            FakeModelError::InvalidResponse(m) => ModelError::InvalidResponse(m),
        }
    }
}

/// A [`StructuredModel`] whose `ask` responses are queued ahead of time, one per
/// call, consumed in order (the pipeline calls `ask` at most once per message).
pub struct FakeStructuredModel {
    responses: Mutex<VecDeque<Result<AnswerSet, FakeModelError>>>,
    calls: Mutex<Vec<QuestionSet>>,
}

impl FakeStructuredModel {
    pub fn new(responses: Vec<Result<AnswerSet, FakeModelError>>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
}

#[async_trait]
impl StructuredModel for FakeStructuredModel {
    fn descriptor(&self) -> ModelDescriptor {
        ModelDescriptor {
            provider: "fake".to_string(),
            model: "fake-1".to_string(),
        }
    }

    async fn ask(
        &self,
        _state: &serde_json::Value,
        questions: &QuestionSet,
    ) -> Result<AnswerSet, ModelError> {
        self.calls.lock().unwrap().push(questions.clone());
        match self
            .responses
            .lock()
            .unwrap()
            .pop_front()
            .expect("FakeStructuredModel: no queued response for this call")
        {
            Ok(answers) => Ok(answers),
            Err(error) => Err(error.into_model_error()),
        }
    }
}

/// A [`ModelProvider`] that always returns the same [`FakeStructuredModel`], or
/// always fails with a fixed message when built via [`FakeModelProvider::failing`].
pub struct FakeModelProvider {
    outcome: Result<std::sync::Arc<dyn StructuredModel>, String>,
}

impl FakeModelProvider {
    pub fn new(model: std::sync::Arc<dyn StructuredModel>) -> Self {
        Self { outcome: Ok(model) }
    }

    pub fn failing(message: &str) -> Self {
        Self {
            outcome: Err(message.to_string()),
        }
    }
}

impl ModelProvider for FakeModelProvider {
    fn current(&self, _settings: &Settings) -> Result<std::sync::Arc<dyn StructuredModel>, String> {
        self.outcome.clone()
    }
}
