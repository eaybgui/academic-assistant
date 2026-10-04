//! A minimal local HTTP/1.1 listener used as the OAuth redirect target during
//! interactive sign-in. Binds `127.0.0.1:0` (a random free port), serves a small
//! confirmation page for the one request that carries the redirect, and answers
//! everything else with 404 while it keeps waiting (browsers commonly issue a
//! `/favicon.ico` request alongside the redirect).

use std::collections::HashMap;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::Instant;

use academic_core::ports::ProviderError;

const CONFIRMATION_PAGE: &str = "<html><body>You can close this window.</body></html>";

/// Upper bound on the bytes read from a single connection (request line + headers)
/// before reading is cut off, so a client that never sends a terminating newline
/// cannot make this listener buffer unbounded data.
const MAX_REQUEST_BYTES: u64 = 8 * 1024;

/// Upper bound on how long a single connection may take to send its request before
/// it is dropped and the listener goes back to `accept()`. Kept short (well under
/// the overall sign-in `timeout`) so one idle or stalled connection — e.g. a stray
/// browser pre-connect that never sends anything — cannot itself block a
/// subsequent, legitimate redirect from being accepted and handled.
const CONNECTION_TIMEOUT: Duration = Duration::from_secs(2);

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

    /// Waits for a `GET /?code=...&state=...` (or `?error=...`) request at the root
    /// path, ignoring any other request or path (which get a 404), until `timeout`
    /// elapses overall. Each individual connection is additionally bounded by
    /// [`CONNECTION_TIMEOUT`] so one idle connection cannot itself consume the whole
    /// `timeout` budget and block a later, legitimate one from being handled.
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

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(ProviderError::Auth("Sign-in timed out. Try again".into()));
            }
            let connection_budget = remaining.min(CONNECTION_TIMEOUT);

            match tokio::time::timeout(connection_budget, handle_connection(stream)).await {
                Ok(Some(params)) => return Ok(params),
                Ok(None) => continue,
                // This connection didn't send a complete, matching request within its
                // budget (idle, slow, or simply not the redirect) — drop it and keep
                // waiting for one that does, up to the overall deadline checked above.
                Err(_) => continue,
            }
        }
    }
}

async fn handle_connection(stream: TcpStream) -> Option<RedirectParams> {
    let mut reader = BufReader::new(stream.take(MAX_REQUEST_BYTES));
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

    // Only the redirect target's own path is ever served; anything else (favicons,
    // probes, ...) gets a 404 while the listener keeps waiting.
    let is_root_path = target.split('?').next() == Some("/");
    let params = if is_root_path {
        redirect_params(&target)
    } else {
        None
    };
    let matched = params
        .as_ref()
        .map(|p| p.code.is_some() || p.error.is_some())
        .unwrap_or(false);

    let mut stream = reader.into_inner().into_inner();
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

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpStream;
    use tokio::time::Instant;

    use super::*;

    async fn connect_to(port: u16) -> TcpStream {
        TcpStream::connect(("127.0.0.1", port)).await.unwrap()
    }

    async fn read_response(stream: &mut TcpStream) -> String {
        let mut buf = [0u8; 1024];
        let n = stream.read(&mut buf).await.unwrap();
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }

    #[tokio::test]
    async fn requests_to_paths_other_than_root_get_404_and_listener_keeps_waiting() {
        let server = LoopbackServer::bind().await.unwrap();
        let port = server.port;
        let wait = tokio::spawn(server.wait_for_redirect(Duration::from_secs(5)));

        // An unrelated request (e.g. a browser favicon probe): must be answered 404
        // and must not satisfy `wait_for_redirect`.
        let mut other = connect_to(port).await;
        other
            .write_all(b"GET /favicon.ico HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let response = read_response(&mut other).await;
        assert!(response.starts_with("HTTP/1.1 404"), "got: {response}");

        // The real redirect, on a fresh connection, must still be picked up.
        let mut redirect = connect_to(port).await;
        redirect
            .write_all(b"GET /?code=test-code&state=test-state HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();
        let response = read_response(&mut redirect).await;
        assert!(response.starts_with("HTTP/1.1 200"), "got: {response}");

        let params = wait.await.unwrap().unwrap();
        assert_eq!(params.code.as_deref(), Some("test-code"));
        assert_eq!(params.state.as_deref(), Some("test-state"));
    }

    #[tokio::test]
    async fn an_idle_connection_does_not_block_a_subsequent_valid_redirect() {
        let server = LoopbackServer::bind().await.unwrap();
        let port = server.port;
        let wait = tokio::spawn(server.wait_for_redirect(Duration::from_secs(10)));

        // Open a connection and never send anything on it (simulates a stray
        // pre-connect); it must be dropped after CONNECTION_TIMEOUT rather than
        // blocking the whole 10s overall deadline.
        let idle = connect_to(port).await;

        // Give the listener a moment to accept the idle connection first.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let mut redirect = connect_to(port).await;
        redirect
            .write_all(b"GET /?code=abc&state=xyz HTTP/1.1\r\nHost: localhost\r\n\r\n")
            .await
            .unwrap();

        let started = Instant::now();
        let params = tokio::time::timeout(Duration::from_secs(8), wait)
            .await
            .expect("wait_for_redirect must not block for the full 10s overall timeout")
            .unwrap()
            .unwrap();

        assert_eq!(params.code.as_deref(), Some("abc"));
        assert!(
            started.elapsed() < Duration::from_secs(8),
            "took too long: {:?}",
            started.elapsed()
        );

        drop(idle);
    }
}
