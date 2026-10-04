//! Live checks against a real platform with the credential stored by the app.
//! Run after signing in through the app:
//!   $env:LMS_LIVE_UNIVERSITY_ID="1"; $env:LMS_LIVE_PLATFORM="moodle"; $env:LMS_LIVE_BASE_URL="https://aulavirtual.uv.es"
//!   cargo test -p academic-infra --test lms_live -- --ignored --nocapture
//! Prints only counts and the display name, never credentials or course data.

use std::sync::Arc;

use academic_core::domain::{LmsPlatform, LmsUniversity, LoginState};
use academic_core::ports::LmsClientFactory;
use academic_infra::factory::http_client;
use academic_infra::lms::factory::DefaultLmsClientFactory;
use academic_infra::secrets::KeyringSecretStore;
use chrono::Utc;

fn university_from_env() -> LmsUniversity {
    let platform = match std::env::var("LMS_LIVE_PLATFORM")
        .expect("LMS_LIVE_PLATFORM")
        .as_str()
    {
        "moodle" => LmsPlatform::Moodle,
        "blackboard" => LmsPlatform::Blackboard,
        other => panic!("unknown platform {other}"),
    };
    LmsUniversity {
        id: std::env::var("LMS_LIVE_UNIVERSITY_ID")
            .expect("LMS_LIVE_UNIVERSITY_ID")
            .parse()
            .unwrap(),
        name: "live".into(),
        platform,
        base_url: std::env::var("LMS_LIVE_BASE_URL").expect("LMS_LIVE_BASE_URL"),
        login_state: LoginState::Ok,
        account_label: None,
        last_sync_at: None,
        last_sync_outcome: None,
        last_sync_error: None,
        created_at: Utc::now(),
    }
}

#[tokio::test]
#[ignore = "needs a real platform and a credential stored by the app"]
async fn live_verify_and_list_courses() {
    let factory = DefaultLmsClientFactory::new(Arc::new(KeyringSecretStore), http_client());
    let client = factory
        .create(&university_from_env())
        .expect("stored credential");
    let identity = client.verify().await.expect("verify");
    println!("verified as: {}", identity.display_name);
    let courses = client.list_courses().await.expect("list_courses");
    println!("courses: {}", courses.len());
    assert!(!courses.is_empty());
}
