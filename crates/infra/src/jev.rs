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
            match send_once(&self.http, &url, &self.api_key, &body).await {
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
    if status == 401 {
        return Err(Outcome::Fatal(ModelError::Auth));
    }
    if status == 422 {
        return Err(Outcome::Fatal(ModelError::InvalidResponse(
            "the provider rejected the request as invalid".to_string(),
        )));
    }
    if status == 429 {
        return Err(Outcome::Retryable(ModelError::RateLimited));
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

    Ok(parsed
        .answers
        .into_iter()
        .filter_map(|(id, raw)| convert_answer(raw).map(|answer| (id, answer)))
        .collect())
}

fn describe_transport_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "request timed out".to_string()
    } else if err.is_connect() {
        "could not connect to the provider".to_string()
    } else {
        "request failed".to_string()
    }
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
    answers: HashMap<String, RawAnswer>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RawAnswer {
    Noul {
        noul: f32,
        confidence: Option<f32>,
    },
    Choice {
        choice: String,
        confidence: Option<f32>,
    },
    Score {
        #[serde(default)]
        probabilities: HashMap<String, f32>,
        confidence: Option<f32>,
    },
}

/// Converts one raw answer into the domain `Answer`. Returns `None` when a `Score`
/// answer's `probabilities` map has no parseable index (nothing sensible to report);
/// the id is then simply absent from the returned `AnswerSet`, same as a missing key.
fn convert_answer(raw: RawAnswer) -> Option<Answer> {
    match raw {
        RawAnswer::Noul { noul, confidence } => Some(Answer::Noul {
            p_yes: noul,
            confidence,
        }),
        RawAnswer::Choice { choice, confidence } => Some(Answer::Choice { choice, confidence }),
        RawAnswer::Score {
            probabilities,
            confidence,
        } => argmax_level(&probabilities).map(|level| Answer::Score { level, confidence }),
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
