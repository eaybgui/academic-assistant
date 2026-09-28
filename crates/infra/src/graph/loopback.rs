//! A minimal local HTTP/1.1 listener used as the OAuth redirect target during
//! interactive sign-in. Binds `127.0.0.1:0` (a random free port), serves a small
//! confirmation page for the one request that carries the redirect, and answers
//! everything else with 404 while it keeps waiting (browsers commonly issue a
//! `/favicon.ico` request alongside the redirect).

use std::collections::HashMap;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;

use academic_core::ports::ProviderError;

const CONFIRMATION_PAGE: &str = "<html><body>You can close this window.</body></html>";

/// The query parameters Microsoft redirected back with: either `code` (+ `state`) on
/// success, or `error` (+ `error_description`) when the user cancels or sign-in fails.
pub(crate) struct RedirectParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// A short-lived local HTTP server bound to a random `127.0.0.1` port.
pub(crate) struct LoopbackServer {
    listener: TcpListener,
    port: u16,
}

impl LoopbackServer {
    /// Binds a new listener on `127.0.0.1:0`.
    pub async fn bind() -> Result<Self, ProviderError> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.map_err(|e| {
            ProviderError::Connection(format!("Could not start the local sign-in listener: {e}"))
        })?;
        let port = listener
            .local_addr()
            .map_err(|e| {
                ProviderError::Connection(format!(
                    "Could not read the local sign-in listener's port: {e}"
                ))
            })?
            .port();
        Ok(Self { listener, port })
    }

    /// The `redirect_uri` Microsoft should send the browser back to: `http://localhost:{port}`.
    pub fn redirect_uri(&self) -> String {
        format!("http://localhost:{}", self.port)
    }

    /// Waits for a `GET /?code=...&state=...` (or `?error=...`) request, ignoring any
    /// other request, until `timeout` elapses.
    pub async fn wait_for_redirect(
        self,
        timeout: Duration,
    ) -> Result<RedirectParams, ProviderError> {
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ProviderError::Auth("Sign-in timed out. Try again".into()));
            }

            let accepted = tokio::time::timeout(remaining, self.listener.accept()).await;
            let stream = match accepted {
                Ok(Ok((stream, _addr))) => stream,
                Ok(Err(_)) => continue,
                Err(_) => return Err(ProviderError::Auth("Sign-in timed out. Try again".into())),
            };

            if let Some(params) = handle_connection(stream).await {
                return Ok(params);
            }
        }
    }
}

async fn handle_connection(stream: TcpStream) -> Option<RedirectParams> {
    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).await.ok()?;
    let target = request_target(&request_line)?;

    // Drain (and discard) the request headers up to the blank line; this listener
    // never inspects them and the request has no body.
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) if line == "\r\n" || line == "\n" => break,
            Ok(_) => continue,
            Err(_) => break,
        }
    }

    let params = redirect_params(&target);
    let matched = params
        .as_ref()
        .map(|p| p.code.is_some() || p.error.is_some())
        .unwrap_or(false);

    let mut stream = reader.into_inner();
    let response = if matched {
        http_response(200, "OK", CONFIRMATION_PAGE)
    } else {
        http_response(404, "Not Found", "")
    };
    let _ = stream.write_all(response.as_bytes()).await;
    let _ = stream.shutdown().await;

    if matched {
        params
    } else {
        None
    }
}

/// Extracts the request target (path + query) from a `GET /path?query HTTP/1.1` line.
/// Any other method is treated as unmatched (answered 404, listener keeps waiting).
fn request_target(request_line: &str) -> Option<String> {
    let mut parts = request_line.split_whitespace();
    let method = parts.next()?;
    if method != "GET" {
        return None;
    }
    parts.next().map(str::to_string)
}

fn redirect_params(target: &str) -> Option<RedirectParams> {
    let (_, query) = target.split_once('?')?;
    let pairs: HashMap<String, String> = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect();
    Some(RedirectParams {
        code: pairs.get("code").cloned(),
        state: pairs.get("state").cloned(),
        error: pairs.get("error").cloned(),
        error_description: pairs.get("error_description").cloned(),
    })
}

fn http_response(status: u16, reason: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}
