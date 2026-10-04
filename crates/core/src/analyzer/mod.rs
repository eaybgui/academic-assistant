//! Email analyzer: builds AI questions from an email and assembles the model's answers into an `AIAnalysis` (Task T5).
//!
//! [`build_request`] turns an [`AnalysisInput`] into the state and [`QuestionSet`] a
//! [`StructuredModel`] is asked; [`assemble`] validates the model's [`AnswerSet`] against
//! that same question set and produces an [`AnalysisOutcome`]. [`analyze`] does both in
//! one call, asking the model exactly once. See `docs/core/email-analyzer.md` for the
//! full behavior (state shape, question set, thresholds, validation rules).
//!
//! This module never calls into [`crate::dates`]: date candidates are extracted
//! upstream and handed in through [`AnalysisInput::date_candidates`].

mod assemble;
mod questions;
#[cfg(test)]
mod test_support;

use crate::domain::{Account, AnalysisOutcome, Catalog, DateCandidate, Email};
use crate::ports::{ModelError, QuestionSet, StructuredModel};

pub use assemble::assemble;

/// `relevant.p_yes` strictly below this is judged [`AnalysisOutcome::Irrelevant`].
pub const RELEVANCE_THRESHOLD: f32 = 0.4;
/// `p_yes` at or above this flags `action_required`/`possible_change` as true.
pub const FLAG_THRESHOLD: f32 = 0.5;
/// Maximum number of course options offered in the `course` question (plus one more
/// for the fixed `none` option), so the total stays within `Question::Choice`'s 255-option limit.
pub const MAX_COURSE_OPTIONS: usize = 254;

/// Everything [`build_request`]/[`assemble`] need to analyze one email.
pub struct AnalysisInput<'a> {
    pub email: &'a Email,
    pub normalized_body: &'a str,
    pub date_candidates: &'a [DateCandidate],
    pub account: &'a Account,
    pub catalog: &'a Catalog,
}

/// The state and question set built for one [`AnalysisInput`], ready to send to a
/// [`StructuredModel::ask`].
pub struct AnalysisRequest {
    pub state: serde_json::Value,
    pub questions: QuestionSet,
}

/// Builds the state and question set for `input` without calling any model.
pub fn build_request(input: &AnalysisInput) -> AnalysisRequest {
    AnalysisRequest {
        state: questions::build_state(input),
        questions: questions::build_questions(input),
    }
}

/// Builds the request for `input`, asks `model` exactly once, and assembles the
/// validated answers into an [`AnalysisOutcome`].
pub async fn analyze(
    input: &AnalysisInput<'_>,
    model: &dyn StructuredModel,
) -> Result<AnalysisOutcome, ModelError> {
    let request = build_request(input);
    let answers = model.ask(&request.state, &request.questions).await?;
    assemble(input, &answers, model.descriptor())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;

    use crate::domain::ModelDescriptor;
    use crate::ports::{Answer, AnswerSet};

    use super::*;

    struct FakeModel {
        calls: AtomicUsize,
        answers: AnswerSet,
    }

    #[async_trait]
    impl StructuredModel for FakeModel {
        fn descriptor(&self) -> ModelDescriptor {
            ModelDescriptor {
                provider: "fake".to_string(),
                model: "fake-1".to_string(),
            }
        }

        async fn ask(
            &self,
            _state: &serde_json::Value,
            _questions: &QuestionSet,
        ) -> Result<AnswerSet, ModelError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.answers.clone())
        }
    }

    fn minimal_valid_answers() -> AnswerSet {
        HashMap::from([
            (
                "relevant".to_string(),
                Answer::Noul {
                    p_yes: 0.9,
                    confidence: Some(0.9),
                },
            ),
            (
                "category".to_string(),
                Answer::Choice {
                    choice: "exam".to_string(),
                    confidence: Some(0.9),
                },
            ),
            (
                "action_required".to_string(),
                Answer::Noul {
                    p_yes: 0.1,
                    confidence: Some(0.9),
                },
            ),
            (
                "possible_change".to_string(),
                Answer::Noul {
                    p_yes: 0.1,
                    confidence: Some(0.9),
                },
            ),
            (
                "priority".to_string(),
                Answer::Score {
                    level: 1,
                    confidence: Some(0.9),
                },
            ),
        ])
    }

    #[test]
    fn build_request_bundles_state_and_questions() {
        let email = test_support::email();
        let account = test_support::account(None);
        let catalog = test_support::catalog(vec![], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "Normalized body",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };

        let request = build_request(&input);

        assert!(request.questions.contains_key("relevant"));
        assert_eq!(request.state["email"]["body"], "Normalized body");
    }

    #[tokio::test]
    async fn analyze_calls_the_model_exactly_once() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "Normalized body",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let model = FakeModel {
            calls: AtomicUsize::new(0),
            answers: minimal_valid_answers(),
        };

        let outcome = analyze(&input, &model).await.unwrap();

        assert_eq!(model.calls.load(Ordering::SeqCst), 1);
        assert!(matches!(outcome, AnalysisOutcome::Relevant(_)));
    }
}
