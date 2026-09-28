//! TLS connection and login for the IMAP provider, and the shared error mapping
//! from `async-imap`/IO errors to [`ProviderError`].

use std::sync::Arc;
use std::time::Duration;

use async_imap::Client;
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::client::TlsStream;
use tokio_rustls::rustls::pki_types::ServerName;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

use academic_core::ports::ProviderError;

/// An authenticated IMAP session over an implicit-TLS connection.
pub(crate) type ImapSession = async_imap::Session<TlsStream<TcpStream>>;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// Opens an implicit-TLS connection to `host:port` and logs in with `username`/`password`.
pub(crate) async fn connect(
    host: &str,
    port: u16,
    username: &str,
    password: &str,
) -> Result<ImapSession, ProviderError> {
    let tcp = timeout(CONNECT_TIMEOUT, TcpStream::connect((host, port)))
        .await
        .map_err(|_| timed_out("TCP connection", host, port))?
        .map_err(|e| {
            ProviderError::Connection(format!("could not connect to {host}:{port}: {e}"))
        })?;

    let tls_stream = timeout(CONNECT_TIMEOUT, tls_connect(host, tcp))
        .await
        .map_err(|_| timed_out("TLS handshake", host, port))??;

    let mut client = Client::new(tls_stream);
    client
        .read_response()
        .await
        .map_err(|e| ProviderError::Connection(format!("no greeting from {host}:{port}: {e}")))?;

    client
        .login(username, password)
        .await
        .map_err(|(err, _client)| map_login_error(err))
}

async fn tls_connect(host: &str, tcp: TcpStream) -> Result<TlsStream<TcpStream>, ProviderError> {
    let mut root_store = RootCertStore::empty();
    root_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    let server_name = ServerName::try_from(host.to_string())
        .map_err(|_| ProviderError::Connection(format!("invalid IMAP host name: {host}")))?;

    connector
        .connect(server_name, tcp)
        .await
        .map_err(|e| ProviderError::Connection(format!("TLS handshake with {host} failed: {e}")))
}

/// Maps an `async-imap` login error. A `NO` response means the server rejected the
/// credentials; everything else (`BAD`, parse/validate failures, I/O) is either a
/// connection or protocol problem, never an authentication one.
fn map_login_error(err: async_imap::error::Error) -> ProviderError {
    use async_imap::error::Error as ImapError;
    match err {
        ImapError::No(message) => ProviderError::Auth(message),
        ImapError::Io(e) => ProviderError::Connection(e.to_string()),
        ImapError::ConnectionLost => ProviderError::Connection("connection lost".into()),
        other => ProviderError::Protocol(other.to_string()),
    }
}

/// Maps an `async-imap` error from any command issued after login (SELECT, SEARCH,
/// FETCH, LOGOUT, ...). None of these carry authentication semantics.
pub(crate) fn map_command_error(err: async_imap::error::Error) -> ProviderError {
    use async_imap::error::Error as ImapError;
    match err {
        ImapError::Io(e) => ProviderError::Connection(e.to_string()),
        ImapError::ConnectionLost => ProviderError::Connection("connection lost".into()),
        other => ProviderError::Protocol(other.to_string()),
    }
}

fn timed_out(step: &str, host: &str, port: u16) -> ProviderError {
    ProviderError::Connection(format!("{step} to {host}:{port} timed out"))
}
