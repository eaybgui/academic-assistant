//! The shared HTTP client and, on desktop, the email and AI provider factories.
//!
//! See `docs/infra/factories.md` for the provider factories' contract.

use std::time::Duration;

#[cfg(not(target_os = "android"))]
mod providers;

#[cfg(not(target_os = "android"))]
pub use providers::{DefaultEmailProviderFactory, DefaultModelProvider};

/// The `User-Agent` sent on every request made by [`http_client`]'s client.
const USER_AGENT: &str = concat!("academic-assistant/", env!("CARGO_PKG_VERSION"));
/// Time allowed to establish the TCP+TLS connection before giving up. Per-request
/// timeouts (waiting for a response once connected) are set by each adapter.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Builds the `reqwest::Client` shared by every adapter this crate constructs:
/// rustls TLS, this app's `User-Agent`, and a 10 second connect timeout.
pub fn http_client() -> reqwest::Client {
    let builder = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .connect_timeout(CONNECT_TIMEOUT);
    #[cfg(target_os = "android")]
    let builder = builder.use_preconfigured_tls(android_tls_config());
    builder
        .build()
        .expect("http client configuration is static and always valid")
}

/// Android has no system trust store rustls can read without JNI, so TLS verifies
/// against the bundled Mozilla roots, as the IMAP adapter does on desktop.
#[cfg(target_os = "android")]
fn android_tls_config() -> tokio_rustls::rustls::ClientConfig {
    let mut roots = tokio_rustls::rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    tokio_rustls::rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth()
}
