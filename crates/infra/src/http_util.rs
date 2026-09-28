//! Shared HTTP helpers for the AI provider adapters (`jev`, `ollama`).

/// Produces a short, generic description of a `reqwest` transport error (timeout,
/// connection refused, ...). Never includes the request/response body or any secret —
/// see spec §12.
pub(crate) fn describe_transport_error(err: &reqwest::Error) -> String {
    if err.is_timeout() {
        "request timed out".to_string()
    } else if err.is_connect() {
        "could not connect to the provider".to_string()
    } else {
        "request failed".to_string()
    }
}
