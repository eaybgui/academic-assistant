//! Unauthenticated check that a URL hosts the chosen platform (spec §5.3).

use academic_core::domain::LmsPlatform;
use serde_json::{json, Value};

use super::REQUEST_TIMEOUT;
use crate::http_util::describe_transport_error;

#[derive(Debug, thiserror::Error)]
pub enum ProbeError {
    #[error("{0}")]
    Mismatch(String),
    #[error("Could not reach the platform: {0}")]
    Connection(String),
}

pub async fn probe(
    http: &reqwest::Client,
    platform: LmsPlatform,
    base_url: &str,
) -> Result<(), ProbeError> {
    match platform {
        LmsPlatform::Moodle => probe_moodle(http, base_url).await,
        LmsPlatform::Blackboard => probe_blackboard(http, base_url).await,
    }
}

async fn probe_moodle(http: &reqwest::Client, base_url: &str) -> Result<(), ProbeError> {
    let not_moodle = || ProbeError::Mismatch("This URL is not a Moodle site".into());
    let response = http
        .post(format!(
            "{base_url}/lib/ajax/service-nologin.php?info=tool_mobile_get_public_config"
        ))
        .json(&json!([{ "index": 0, "methodname": "tool_mobile_get_public_config", "args": {} }]))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| ProbeError::Connection(describe_transport_error(&e)))?;
    if !response.status().is_success() {
        return Err(not_moodle());
    }
    let body: Value = response.json().await.map_err(|_| not_moodle())?;
    let first = body.get(0).ok_or_else(not_moodle)?;
    if first.get("error").and_then(Value::as_bool) != Some(false) {
        return Err(not_moodle());
    }
    let data = first.get("data").ok_or_else(not_moodle)?;
    if is_enabled(data.get("enablewebservices")) && is_enabled(data.get("enablemobilewebservice")) {
        Ok(())
    } else {
        Err(ProbeError::Mismatch(
            "Mobile web services are disabled on this Moodle site".into(),
        ))
    }
}

/// Moodle reports these flags as `1`/`0` (sometimes as booleans).
fn is_enabled(value: Option<&Value>) -> bool {
    matches!(value, Some(v) if v.as_i64() == Some(1) || v.as_bool() == Some(true))
}

async fn probe_blackboard(http: &reqwest::Client, base_url: &str) -> Result<(), ProbeError> {
    let not_blackboard = || ProbeError::Mismatch("This URL is not a Blackboard Learn site".into());
    let response = http
        .get(format!("{base_url}/learn/api/public/v1/system/version"))
        .timeout(REQUEST_TIMEOUT)
        .send()
        .await
        .map_err(|e| ProbeError::Connection(describe_transport_error(&e)))?;
    if !response.status().is_success() {
        return Err(not_blackboard());
    }
    let body: Value = response.json().await.map_err(|_| not_blackboard())?;
    if body.get("learn").is_some_and(Value::is_object) {
        Ok(())
    } else {
        Err(not_blackboard())
    }
}
