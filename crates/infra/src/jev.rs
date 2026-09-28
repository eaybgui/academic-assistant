//! `JevModel`: `StructuredModel` adapter for the jev (TypeSafe) AI provider (Task T7).
//!
//! See `docs/infra/ai-providers.md` for the request/response shapes and the retry policy.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Map, Value};

use academic_core::domain::ModelDescriptor;
use academic_core::ports::{Answer, AnswerSet, ModelError, Question, QuestionSet, StructuredModel};

use crate::http_util::describe_transport_error;

/// Base URL of the jev API.
pub const JEV_BASE_URL: &str = "https://api.typesafe.ai";

const ENDPOINT_PATH: &str = "/v1/systemone";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// Number of retries attempted after the initial request (so up to 4 requests total).
const MAX_RETRIES: u32 = 3;
const DEFAULT_RETRY_BASE: Duration = Duration::from_secs(1);

/// `StructuredModel` adapter for the jev API (`POST {base_url}/v1/systemone`).
pub struct JevModel {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    retry_base: Duration,
}

impl JevModel {
    pub fn new(http: reqwest::Client, base_url: String, api_key: String, model: String) -> Self {
        Self {
            http,
            base_url,
            api_key,
            model,
            retry_base: DEFAULT_RETRY_BASE,
        }
    }

    /// Overrides the exponential backoff base between retries (default: 1 second, giving
    /// 1 s/2 s/4 s waits). Intended for tests, so they don't wait for real seconds between
    /// retried requests; not part of the public contract used by production callers.
    #[doc(hidden)]
    pub fn with_retry_base(mut self, base: Duration) -> Self {
        self.retry_base = base;
        self
    }

    fn endpoint(&self) -> String {
        format!("{}{ENDPOINT_PATH}", self.base_url)
    }
}

#[async_trait]
impl StructuredModel for JevModel {
    fn descriptor(&self) -> ModelDescriptor {
        ModelDescriptor {
            provider: "jev".to_string(),
            model: self.model.clone(),
        }
    }

    async fn ask(&self, state: &Value, questions: &QuestionSet) -> Result<AnswerSet, ModelError> {
        let body = build_request_body(&self.model, state, questions);
        let url = self.endpoint();

        let mut attempt: u32 = 0;
        loop {
            attempt += 1;
            match send_once(&self.http, &url, &self.api_key, &body, questions).await {
                Ok(answers) => return Ok(answers),
                Err(Outcome::Fatal(err)) => return Err(err),
                Err(Outcome::Retryable(err)) => {
                    if attempt > MAX_RETRIES {
                        return Err(err);
                    }
                    let backoff = self.retry_base * 2u32.pow(attempt - 1);
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }
}

/// Outcome of a single HTTP attempt: `Fatal` errors are returned immediately, `Retryable`
/// ones are retried (up to `MAX_RETRIES` times) before being surfaced to the caller.
enum Outcome {
    Fatal(ModelError),
    Retryable(ModelError),
}

async fn send_once(
    http: &reqwest::Client,
    url: &str,
    api_key: &str,
    body: &Value,
    questions: &QuestionSet,
) -> Result<AnswerSet, Outcome> {
    let response = http
        .post(url)
        .bearer_auth(api_key)
        .timeout(REQUEST_TIMEOUT)
        .json(body)
        .send()
        .await
        .map_err(|err| {
            Outcome::Retryable(ModelError::Unavailable(describe_transport_error(&err)))
        })?;

    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err(Outcome::Fatal(ModelError::Auth));
    }
    if status == 429 {
        return Err(Outcome::Retryable(ModelError::RateLimited));
    }
    if (400..500).contains(&status) {
        // 422 and any other 4xx (400, 404, ...) are treated as the provider rejecting
        // the request as invalid and are not retried. 401/403/429 were already handled
        // above.
        return Err(Outcome::Fatal(ModelError::InvalidResponse(format!(
            "the provider rejected the request as invalid (status {status})"
        ))));
    }
    if !(200..300).contains(&status) {
        // 529 (overloaded), other 5xx and any other unexpected status are treated as a
        // transient availability problem and retried.
        return Err(Outcome::Retryable(ModelError::Unavailable(format!(
            "provider returned status {status}"
        ))));
    }

    let text = response.text().await.map_err(|err| {
        Outcome::Retryable(ModelError::Unavailable(describe_transport_error(&err)))
    })?;

    let parsed: JevResponse = serde_json::from_str(&text).map_err(|_| {
        Outcome::Fatal(ModelError::InvalidResponse(
            "the provider returned a malformed response body".to_string(),
        ))
    })?;

    convert_answers(parsed.answers, questions).map_err(Outcome::Fatal)
}

fn build_request_body(model: &str, state: &Value, questions: &QuestionSet) -> Value {
    let mut questions_json = Map::new();
    for (id, question) in questions {
        questions_json.insert(id.clone(), question_to_json(question));
    }
    serde_json::json!({
        "model": model,
        "state": state,
        "questions": Value::Object(questions_json),
    })
}

fn question_to_json(question: &Question) -> Value {
    match question {
        Question::Noul { instructions } => serde_json::json!({
            "type": "noul",
            "instructions": instructions,
        }),
        Question::Choice {
            instructions,
            options,
        } => serde_json::json!({
            "type": "choice",
            "instructions": instructions,
            "criteria": options,
        }),
        Question::Score {
            instructions,
            levels,
        } => serde_json::json!({
            "type": "score",
            "instructions": instructions,
            "criteria": levels,
        }),
    }
}

#[derive(Debug, Deserialize)]
struct JevResponse {
    #[serde(default)]
    answers: HashMap<String, Value>,
}

/// Shape of a `noul` answer's fields, decoded without relying on a `"type"` tag.
#[derive(Debug, Deserialize)]
struct NoulRaw {
    noul: f32,
    confidence: Option<f32>,
}

/// Shape of a `choice` answer's fields, decoded without relying on a `"type"` tag.
#[derive(Debug, Deserialize)]
struct ChoiceRaw {
    choice: String,
    confidence: Option<f32>,
}

/// Shape of a `score` answer's fields, decoded without relying on a `"type"` tag.
#[derive(Debug, Deserialize)]
struct ScoreRaw {
    #[serde(default)]
    probabilities: HashMap<String, f32>,
    confidence: Option<f32>,
}

/// Decodes the raw `answers` object into an [`AnswerSet`]. Each answer is decoded
/// according to the type of the question that was asked with the same id — the jev
/// response is not trusted to carry a reliable `"type"` field of its own. An id absent
/// from `questions` is ignored; a malformed answer for a known id fails the whole call
/// with `InvalidResponse` naming that id.
fn convert_answers(
    raw_answers: HashMap<String, Value>,
    questions: &QuestionSet,
) -> Result<AnswerSet, ModelError> {
    let mut answers = AnswerSet::new();
    for (id, raw) in raw_answers {
        let Some(question) = questions.get(&id) else {
            continue;
        };
        match decode_answer(question, raw) {
            Ok(Some(answer)) => {
                answers.insert(id, answer);
            }
            Ok(None) => {}
            Err(()) => {
                return Err(ModelError::InvalidResponse(format!(
                    "answer for question \"{id}\" did not match the requested schema"
                )));
            }
        }
    }
    Ok(answers)
}

/// Decodes one raw answer according to `question`'s type. Returns `Ok(None)` only for a
/// `Score` answer whose `probabilities` map has no parseable index (malformed/empty) —
/// nothing sensible to report, so that answer is dropped rather than failing the whole
/// call. Any other shape mismatch (missing/wrong-typed field) is `Err(())`, surfaced by
/// the caller as `InvalidResponse`.
fn decode_answer(question: &Question, raw: Value) -> Result<Option<Answer>, ()> {
    match question {
        Question::Noul { .. } => {
            let parsed: NoulRaw = serde_json::from_value(raw).map_err(|_| ())?;
            Ok(Some(Answer::Noul {
                p_yes: parsed.noul,
                confidence: parsed.confidence,
            }))
        }
        Question::Choice { .. } => {
            let parsed: ChoiceRaw = serde_json::from_value(raw).map_err(|_| ())?;
            Ok(Some(Answer::Choice {
                choice: parsed.choice,
                confidence: parsed.confidence,
            }))
        }
        Question::Score { .. } => {
            let parsed: ScoreRaw = serde_json::from_value(raw).map_err(|_| ())?;
            Ok(
                argmax_level(&parsed.probabilities).map(|level| Answer::Score {
                    level,
                    confidence: parsed.confidence,
                }),
            )
        }
    }
}

/// The level (string index key) with the highest probability. Ties are broken by the
/// lowest level, independent of `HashMap` iteration order.
fn argmax_level(probabilities: &HashMap<String, f32>) -> Option<u8> {
    let mut best: Option<(u8, f32)> = None;
    for (key, &value) in probabilities {
        let Ok(level) = key.parse::<u8>() else {
            continue;
        };
        best = match best {
            None => Some((level, value)),
            Some((best_level, best_value)) => {
                if value > best_value || (value == best_value && level < best_level) {
                    Some((level, value))
                } else {
                    Some((best_level, best_value))
                }
            }
        };
    }
    best.map(|(level, _)| level)
}
