//! Integration tests for `academic_infra::jev::JevModel`, against `wiremock`.

use std::time::Duration;

use academic_core::domain::ModelDescriptor;
use academic_core::ports::{Answer, ModelError, Question, QuestionSet, StructuredModel};
use academic_infra::jev::{JevModel, JEV_BASE_URL};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Fast retry base so tests that exhaust retries don't wait for real seconds.
const FAST_RETRY_BASE: Duration = Duration::from_millis(1);

fn model_for(server: &MockServer) -> JevModel {
    JevModel::new(
        reqwest::Client::new(),
        server.uri(),
        "test-key".to_string(),
        "jev-latest".to_string(),
    )
    .with_retry_base(FAST_RETRY_BASE)
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

#[test]
fn jev_base_url_is_the_typesafe_api() {
    assert_eq!(JEV_BASE_URL, "https://api.typesafe.ai");
}

#[tokio::test]
async fn descriptor_reports_jev_and_the_configured_model() {
    let server = MockServer::start().await;
    let model = model_for(&server);
    let descriptor = model.descriptor();
    assert_eq!(
        descriptor,
        ModelDescriptor {
            provider: "jev".to_string(),
            model: "jev-latest".to_string(),
        }
    );
}

#[tokio::test]
async fn request_body_and_auth_header_match_each_question_type() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "jev-1.13.0",
            "answers": {},
            "usage": { "input_tokens": 1, "output_tokens": 1 },
        })))
        .expect(1)
        .mount(&server)
        .await;

    model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let request = &requests[0];

    assert_eq!(
        request.headers.get("authorization").unwrap(),
        "Bearer test-key"
    );

    let body: Value = serde_json::from_slice(&request.body).unwrap();
    assert_eq!(body["model"], "jev-latest");
    assert_eq!(body["state"], sample_state());

    let relevant = &body["questions"]["relevant"];
    assert_eq!(relevant["type"], "noul");
    assert!(relevant.get("criteria").is_none());

    let category = &body["questions"]["category"];
    assert_eq!(category["type"], "choice");
    assert_eq!(category["criteria"]["task"], "An assignment or task.");
    assert_eq!(category["criteria"]["exam"], "An exam.");

    let priority = &body["questions"]["priority"];
    assert_eq!(priority["type"], "score");
    assert_eq!(
        priority["criteria"],
        json!(["low", "medium", "high", "urgent"])
    );
}

#[tokio::test]
async fn maps_noul_choice_and_score_answers_with_confidence() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "jev-1.13.0",
            "answers": {
                "relevant": { "type": "noul", "noul": 0.87, "confidence": 0.9 },
                "category": {
                    "type": "choice",
                    "choice": "task",
                    "probabilities": { "task": 0.7, "exam": 0.3 },
                    "confidence": 0.6
                },
                "priority": {
                    "type": "score",
                    "score": 1.43,
                    "confidence": 0.35,
                    "legend": { "0": "low", "1": "medium", "2": "high", "3": "urgent" },
                    "probabilities": { "0": 0.0, "1": 0.57, "2": 0.43, "3": 0.0 }
                }
            },
            "usage": { "input_tokens": 10, "output_tokens": 5 },
        })))
        .mount(&server)
        .await;

    let answers = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap();

    assert_eq!(
        answers.get("relevant"),
        Some(&Answer::Noul {
            p_yes: 0.87,
            confidence: Some(0.9),
        })
    );
    assert_eq!(
        answers.get("category"),
        Some(&Answer::Choice {
            choice: "task".to_string(),
            confidence: Some(0.6),
        })
    );
    // argmax of probabilities is index "1" (0.57), not round(score).
    assert_eq!(
        answers.get("priority"),
        Some(&Answer::Score {
            level: 1,
            confidence: Some(0.35),
        })
    );
}

#[tokio::test]
async fn missing_answer_key_is_simply_absent_from_the_result() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "jev-1.13.0",
            "answers": {
                "relevant": { "type": "noul", "noul": 0.9, "confidence": null },
            },
            "usage": { "input_tokens": 1, "output_tokens": 1 },
        })))
        .mount(&server)
        .await;

    let answers = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap();

    assert_eq!(answers.len(), 1);
    assert!(answers.contains_key("relevant"));
    assert!(!answers.contains_key("category"));
    assert!(!answers.contains_key("priority"));
}

#[tokio::test]
async fn status_401_maps_to_auth_without_retrying() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(401))
        .expect(1)
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::Auth));
}

#[tokio::test]
async fn status_422_maps_to_invalid_response_without_retrying() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(422))
        .expect(1)
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidResponse(_)));
}

#[tokio::test]
async fn status_429_then_200_succeeds_after_one_retry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(429))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "model": "jev-1.13.0",
            "answers": {
                "relevant": { "type": "noul", "noul": 0.5, "confidence": null },
            },
            "usage": { "input_tokens": 1, "output_tokens": 1 },
        })))
        .with_priority(2)
        .mount(&server)
        .await;

    let answers = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap();
    assert!(answers.contains_key("relevant"));
}

#[tokio::test]
async fn persistent_429_maps_to_rate_limited_after_retries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(429))
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::RateLimited));
}

#[tokio::test]
async fn status_529_maps_to_unavailable_after_retries() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(529))
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::Unavailable(_)));
}

#[tokio::test]
async fn malformed_json_body_maps_to_invalid_response() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/systemone"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .expect(1)
        .mount(&server)
        .await;

    let err = model_for(&server)
        .ask(&sample_state(), &sample_questions())
        .await
        .unwrap_err();
    assert!(matches!(err, ModelError::InvalidResponse(_)));
}

/// Hits the real jev API. Run explicitly with `JEV_API_KEY=<key> cargo test -p
/// academic-infra --test jev -- --ignored`.
#[tokio::test]
#[ignore]
async fn live_jev_answers_a_simple_noul_question() {
    let api_key = match std::env::var("JEV_API_KEY") {
        Ok(key) => key,
        Err(_) => return,
    };

    let model = JevModel::new(
        reqwest::Client::new(),
        JEV_BASE_URL.to_string(),
        api_key,
        "jev-latest".to_string(),
    );

    let mut questions = IndexMap::new();
    questions.insert(
        "relevant".to_string(),
        Question::Noul {
            instructions: "Is the sky blue on a clear day?".to_string(),
        },
    );

    let answers = model
        .ask(&json!({ "note": "sanity check" }), &questions)
        .await
        .unwrap();
    assert!(answers.contains_key("relevant"));
}
