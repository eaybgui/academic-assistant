//! Live IMAP smoke test against a real server. Ignored by default; run with:
//!
//! ```text
//! IMAP_HOST=imap.example.com IMAP_USER=me@example.com IMAP_PASSWORD=secret \
//!     cargo test -p academic-infra --test imap_live -- --ignored
//! ```
//!
//! `IMAP_PORT` defaults to 993. Credentials are read only from the environment and
//! are never printed.

use academic_core::domain::AccountId;
use academic_core::ports::EmailProvider;
use academic_infra::imap::ImapEmailProvider;
use chrono::{Duration, Utc};

#[tokio::test]
#[ignore = "requires IMAP_HOST/IMAP_USER/IMAP_PASSWORD against a real server"]
async fn connects_and_fetches_recent_messages() {
    let host = std::env::var("IMAP_HOST").expect("IMAP_HOST must be set for this test");
    let username = std::env::var("IMAP_USER").expect("IMAP_USER must be set for this test");
    let password = std::env::var("IMAP_PASSWORD").expect("IMAP_PASSWORD must be set for this test");
    let port: u16 = std::env::var("IMAP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(993);

    let account_id: AccountId = 1;
    let provider = ImapEmailProvider::new(account_id, host, port, username, password);

    provider
        .test_connection()
        .await
        .expect("test_connection should succeed against a real server");

    let since = Utc::now() - Duration::days(2);
    let batch = provider
        .fetch_new("INBOX", None, since)
        .await
        .expect("fetch_new should succeed against a real server");

    // No assertion on content: this just proves the round trip (connect, search,
    // fetch, parse, logout) works end to end against a live mailbox.
    println!(
        "fetched {} message(s); next cursor = {:?}",
        batch.emails.len(),
        batch.next_cursor
    );
}
