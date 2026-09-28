use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;

use crate::domain::{ModelDescriptor, Settings};

/// The set of questions sent to a [`StructuredModel`] for one email, keyed by
/// question id (e.g. `relevant`, `u{id}`, `date_0`). Order matters for prompt building.
pub type QuestionSet = IndexMap<String, Question>;
/// The model's answers, keyed by the same question ids as the [`QuestionSet`] it answered.
pub type AnswerSet = HashMap<String, Answer>;

/// A single question posed to the AI model.
#[derive(Debug, Clone, PartialEq)]
pub enum Question {
    /// A yes/no probability question ("noul" = probability of yes).
    Noul { instructions: String },
    /// A single choice among named options.
    Choice {
        instructions: String,
        options: IndexMap<String, String>,
    },
    /// A single choice among an ordered list of levels (e.g. priority).
    Score {
        instructions: String,
        levels: Vec<String>,
    },
}

/// The model's answer to one [`Question`].
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Noul {
        p_yes: f32,
        confidence: Option<f32>,
    },
    Choice {
        choice: String,
        confidence: Option<f32>,
    },
    Score {
        level: u8,
        confidence: Option<f32>,
    },
}

/// Error returned by a [`StructuredModel`] operation.
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("The AI provider rejected the credentials")]
    Auth,
    #[error("The AI provider is rate limiting requests")]
    RateLimited,
    #[error("The AI provider is unavailable: {0}")]
    Unavailable(String),
    #[error("The AI provider returned an invalid response: {0}")]
    InvalidResponse(String),
}

impl ModelError {
    /// True for errors worth retrying later without user intervention (`Auth`,
    /// `RateLimited`, `Unavailable`); false for `InvalidResponse`, which indicates
    /// a malformed reply rather than a transient provider issue.
    pub fn is_transient(&self) -> bool {
        matches!(
            self,
            ModelError::Auth | ModelError::RateLimited | ModelError::Unavailable(_)
        )
    }
}

/// A structured AI model that answers a [`QuestionSet`] about an email's state.
#[async_trait]
pub trait StructuredModel: Send + Sync {
    fn descriptor(&self) -> ModelDescriptor;

    async fn ask(
        &self,
        state: &serde_json::Value,
        questions: &QuestionSet,
    ) -> Result<AnswerSet, ModelError>;
}

/// Selects and builds the currently configured [`StructuredModel`].
pub trait ModelProvider: Send + Sync {
    /// Builds the model selected in `settings`. Err(message) when it cannot be used
    /// (e.g. jev selected without API key); the pipeline maps it to `RunError::AiNotConfigured`.
    fn current(&self, settings: &Settings) -> Result<Arc<dyn StructuredModel>, String>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_error_is_transient_matches_spec() {
        assert!(ModelError::Auth.is_transient());
        assert!(ModelError::RateLimited.is_transient());
        assert!(ModelError::Unavailable("down".into()).is_transient());
        assert!(!ModelError::InvalidResponse("bad json".into()).is_transient());
    }
}
