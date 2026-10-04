//! Shared HTTP helpers for the infra adapters.

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

/// Encodes `pairs` as an `application/x-www-form-urlencoded` body. Used instead of
/// `reqwest`'s `form` feature, which is not enabled for this workspace (only `json`
/// and `rustls` are); `url`'s encoder produces the same result.
pub(crate) fn encode_form(pairs: &[(&str, &str)]) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    for (key, value) in pairs {
        serializer.append_pair(key, value);
    }
    serializer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_form_url_encodes_keys_and_values() {
        let body = encode_form(&[("a", "1 2"), ("b", "x&y")]);
        assert_eq!(body, "a=1+2&b=x%26y");
    }
}
