//! Integration tests for `academic_infra::ollama::OllamaModel`, against `wiremock`.

use academic_core::domain::ModelDescriptor;
use academic_core::ports::{Answer, ModelError, Question, QuestionSet, StructuredModel};
use academic_infra::ollama::OllamaModel;
use indexmap::IndexMap;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn model_for(server: &MockServer) -> OllamaModel {
    OllamaModel::new(reqwest::Client::new(), server.uri(), "qwen3:8b".to_string())
}

fn sample_questions() -> QuestionSet {
    let mut questions = IndexMap::new();
    questions.insert(
        "relevant".to_string(),
        Question::Noul {
            instructions: "Is this email relevant to the student?".to_string(),
        },
    );
    let mut options = IndexMap::new();
    options.insert("task".to_string(), "An assignment or task.".to_string());
    options.insert("exam".to_string(), "An exam.".to_string());
    questions.insert(
        "category".to_string(),
        Question::Choice {
            instructions: "Categorize this email.".to_string(),
            options,
        },
    );
    questions.insert(
        "priority".to_string(),
        Question::Score {
            instructions: "How urgent is this email?".to_string(),
            levels: vec![
                "low".to_string(),
                "medium".to_string(),
                "high".to_string(),
                "urgent".to_string(),
            ],
        },
    );
    questions
}

fn sample_state() -> Value {
    json!({ "subject": "Exam date change", "body": "The exam moved to next week." })
}

fn chat_response(content: &Value) -> Value {
    json!({
        "message": { "role": "assistant", "content": content.to_string() },
        "done": true,
    })
}

/// Finds a `127.0.0.1` port with nothing listening on it, for connection-refused tests.
fn unused_base_url() -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    format!("http://127.0.0.1:{port}")
}

#[tokio::test]
async fn descriptor_reports_ollama_and_the_configured_model() {
    let server = MockServer::start().await;
    let model = model_for(&server);
    assert_eq!(
        model.descriptor(),
        ModelDescriptor {
            provider: "ollama".to_string(),
            model: "qwen3:8b".to_string(),
        }
    );
}

#[tokio::test]
async fn request_shape_has_the_expected_options_and_schema() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_response(&json!({
                "relevant": true,
                "category": "task",
                "priority": 1
            }))),
        )
        .expect(1)
        .mount(&server)
        .await;

    model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: Value = serde_json::from_slice(&requests[0].body).unwrap();

    assert_eq!(body["stream"], false);
    assert_eq!(body["think"], false);
    assert_eq!(body["options"]["temperature"], 0);
    assert_eq!(body["messages"][0]["role"], "system");
    assert_eq!(body["messages"][1]["role"], "user");

    let format = &body["format"];
    assert_eq!(format["properties"]["relevant"]["type"], "boolean");
    assert_eq!(format["properties"]["category"]["type"], "string");
    assert_eq!(
        format["properties"]["category"]["enum"],
        json!(["task", "exam"])
    );
    assert_eq!(format["properties"]["priority"]["type"], "integer");
    assert_eq!(
        format["properties"]["priority"]["enum"],
        json!([0, 1, 2, 3])
    );
    let required = format["required"].as_array().unwrap();
    assert!(required.contains(&json!("relevant")));
    assert!(required.contains(&json!("category")));
    assert!(required.contains(&json!("priority")));
}

#[tokio::test]
async fn maps_noul_true_and_false_to_1_and_0() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_response(&json!({
                "relevant": true,
            }))),
        )
        .mount(&server)
        .await;

    let mut questions = IndexMap::new();
    questions.insert(
        "relevant".to_string(),
        Question::Noul {
            instructions: "Is this relevant?".to_string(),
        },
    );

    let answers = model_for(&server)
        .ask(&sample_state(), &questions)
        .await
        .unwrap();
    assert_eq!(
        answers.get("relevant"),
        Some(&Answer::Noul {
            p_yes: 1.0,
            confidence: None,
        })
    );
}

#[tokio::test]
async fn maps_choice_and_score_answers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_response(&json!({
                "relevant": false,
                "category": "exam",
                "priority": 2
            }))),
        )
        .mount(&server)
        .await;

    let answers = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap();

    assert_eq!(
        answers.get("relevant"),
        Some(&Answer::Noul {
            p_yes: 0.0,
            confidence: None,
        })
    );
    assert_eq!(
        answers.get("category"),
        Some(&Answer::Choice {
            choice: "exam".to_string(),
            confidence: None,
        })
    );
    assert_eq!(
        answers.get("priority"),
        Some(&Answer::Score {
            level: 2,
            confidence: None,
        })
    );
}

#[tokio::test]
async fn missing_answer_key_is_simply_absent_from_the_result() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_response(&json!({
                "relevant": true,
            }))),
        )
        .mount(&server)
        .await;

    let answers = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap();

    assert_eq!(answers.len(), 1);
    assert!(answers.contains_key("relevant"));
    assert!(!answers.contains_key("category"));
}

#[tokio::test]
async fn connection_refused_maps_to_unavailable() {
    let model = OllamaModel::new(
        reqwest::Client::new(),
        unused_base_url(),
        "qwen3:8b".to_string(),
    );

    let err = model
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::Unavailable(_)));
}

#[tokio::test]
async fn server_error_status_maps_to_unavailable() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::Unavailable(_)));
}

#[tokio::test]
async fn content_not_valid_json_maps_to_invalid_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": { "role": "assistant", "content": "not json" },
            "done": true,
        })))
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidResponse(_)));
}

#[tokio::test]
async fn content_not_matching_schema_maps_to_invalid_response() {
    let server = MockServer::start().await;
    // "relevant" is asked as Noul (boolean) but the model replies with a string.
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_response(&json!({
                "relevant": "yes",
            }))),
        )
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidResponse(_)));
}

#[tokio::test]
async fn choice_outside_known_options_maps_to_invalid_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_response(&json!({
                "category": "not-an-option",
            }))),
        )
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidResponse(_)));
}

#[tokio::test]
async fn score_out_of_range_maps_to_invalid_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(chat_response(&json!({
                "priority": 99,
            }))),
        )
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidResponse(_)));
}

#[tokio::test]
async fn malformed_top_level_body_maps_to_invalid_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidResponse(_)));
}

/// Hits a real local Ollama server. Run explicitly with `OLLAMA_MODEL=<model>
/// [OLLAMA_BASE_URL=<url>] cargo test -p academic-infra --test ollama -- --ignored`.
#[tokio::test]
#[ignore]
async fn live_ollama_answers_a_simple_noul_question() {
    let model_name = match std::env::var("OLLAMA_MODEL") {
        Ok(m) => m,
        Err(_) => return,
    };
    let base_url =
        std::env::var("OLLAMA_BASE_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

    let model = OllamaModel::new(reqwest::Client::new(), base_url, model_name);

    let mut questions = IndexMap::new();
    questions.insert(
        "relevant".to_string(),
        Question::Noul {
            instructions: "Is the sky blue on a clear day? Answer true.".to_string(),
        },
    );

    let answers = model
        .ask(&json!({ "note": "sanity check" }), &questions)
        .await
        .unwrap();
    assert!(answers.contains_key("relevant"));
}
