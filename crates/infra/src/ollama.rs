//! `OllamaModel`: `StructuredModel` adapter for a local Ollama server (Task T7).
//!
//! See `docs/infra/ai-providers.md` for the request/response shapes.

use std::fmt::Write as _;
use std::time::Duration;

use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{Map, Value};

use academic_core::domain::ModelDescriptor;
use academic_core::ports::{Answer, AnswerSet, ModelError, Question, QuestionSet, StructuredModel};

const CHAT_PATH: &str = "/api/chat";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);
const SYSTEM_PROMPT: &str =
    "You answer typed questions about the provided state. Reply only with JSON matching the schema.";

/// `StructuredModel` adapter for a local Ollama server (`POST {base_url}/api/chat`).
pub struct OllamaModel {
    http: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaModel {
    pub fn new(http: reqwest::Client, base_url: String, model: String) -> Self {
        Self {
            http,
            base_url,
            model,
        }
    }

    fn endpoint(&self) -> String {
        format!("{}{CHAT_PATH}", self.base_url)
    }
}

#[async_trait]
impl StructuredModel for OllamaModel {
    fn descriptor(&self) -> ModelDescriptor {
        ModelDescriptor {
            provider: "ollama".to_string(),
            model: self.model.clone(),
        }
    }

    async fn ask(&self, state: &Value, questions: &QuestionSet) -> Result<AnswerSet, ModelError> {
        let body = build_request_body(&self.model, state, questions);

        let response = self
            .http
            .post(self.endpoint())
            .timeout(REQUEST_TIMEOUT)
            .json(&body)
            .send()
            .await
            .map_err(|err| ModelError::Unavailable(describe_transport_error(&err)))?;

        let status = response.status();
        if !status.is_success() {
            return Err(ModelError::Unavailable(format!(
                "provider returned status {}",
                status.as_u16()
            )));
        }

        let text = response
            .text()
            .await
            .map_err(|err| ModelError::Unavailable(describe_transport_error(&err)))?;

        let parsed: ChatResponse = serde_json::from_str(&text).map_err(|_| {
            ModelError::InvalidResponse(
                "the provider returned a malformed response body".to_string(),
            )
        })?;

        parse_answers(&parsed.message.content, questions)
    }
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

#[derive(Debug, Deserialize)]
struct ChatResponse {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: String,
}

/// Parses the model's JSON reply (`message.content`) against the questions asked.
/// A question id absent from the reply is simply left out of the result (same
/// contract as the jev adapter); a present value that doesn't match its question's
/// type/options/levels makes the whole reply `InvalidResponse`.
fn parse_answers(content: &str, questions: &QuestionSet) -> Result<AnswerSet, ModelError> {
    let value: Value = serde_json::from_str(content).map_err(|_| {
        ModelError::InvalidResponse("the model reply was not valid JSON".to_string())
    })?;
    let object = value.as_object().ok_or_else(|| {
        ModelError::InvalidResponse("the model reply was not a JSON object".to_string())
    })?;

    let mut answers = AnswerSet::new();
    for (id, question) in questions {
        let Some(raw) = object.get(id) else {
            continue;
        };
        let answer = convert_answer(question, raw).ok_or_else(|| {
            ModelError::InvalidResponse(format!(
                "answer for question \"{id}\" did not match the requested schema"
            ))
        })?;
        answers.insert(id.clone(), answer);
    }
    Ok(answers)
}

fn convert_answer(question: &Question, raw: &Value) -> Option<Answer> {
    match question {
        Question::Noul { .. } => raw.as_bool().map(|value| Answer::Noul {
            p_yes: if value { 1.0 } else { 0.0 },
            confidence: None,
        }),
        Question::Choice { options, .. } => {
            let choice = raw.as_str()?;
            options.contains_key(choice).then(|| Answer::Choice {
                choice: choice.to_string(),
                confidence: None,
            })
        }
        Question::Score { levels, .. } => {
            let level = raw.as_u64()?;
            (level < levels.len() as u64).then_some(Answer::Score {
                level: level as u8,
                confidence: None,
            })
        }
    }
}

fn build_request_body(model: &str, state: &Value, questions: &QuestionSet) -> Value {
    serde_json::json!({
        "model": model,
        "messages": [
            { "role": "system", "content": SYSTEM_PROMPT },
            { "role": "user", "content": build_user_message(state, questions) },
        ],
        "stream": false,
        "think": false,
        "format": build_schema(questions),
        "options": { "temperature": 0 },
    })
}

fn build_schema(questions: &QuestionSet) -> Value {
    let mut properties = Map::new();
    let mut required = Vec::new();

    for (id, question) in questions {
        let schema = match question {
            Question::Noul { .. } => serde_json::json!({ "type": "boolean" }),
            Question::Choice { options, .. } => serde_json::json!({
                "type": "string",
                "enum": options.keys().collect::<Vec<_>>(),
            }),
            Question::Score { levels, .. } => serde_json::json!({
                "type": "integer",
                "enum": (0..levels.len() as u64).collect::<Vec<_>>(),
            }),
        };
        properties.insert(id.clone(), schema);
        required.push(Value::String(id.clone()));
    }

    serde_json::json!({
        "type": "object",
        "properties": Value::Object(properties),
        "required": required,
    })
}

fn build_user_message(state: &Value, questions: &QuestionSet) -> String {
    let mut message = String::new();
    let _ = writeln!(message, "State:\n{state}\n");
    let _ = writeln!(message, "Questions:");
    for (id, question) in questions {
        match question {
            Question::Noul { instructions } => {
                let _ = writeln!(message, "\"{id}\": {instructions}");
            }
            Question::Choice {
                instructions,
                options,
            } => {
                let _ = writeln!(message, "\"{id}\": {instructions}");
                for (key, description) in options {
                    let _ = writeln!(message, "- {key}: {description}");
                }
            }
            Question::Score {
                instructions,
                levels,
            } => {
                let _ = writeln!(message, "\"{id}\": {instructions}");
                for (index, level) in levels.iter().enumerate() {
                    let _ = writeln!(message, "- {index}: {level}");
                }
            }
        }
    }
    message
}
