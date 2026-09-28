//! `SettingsRepository` behavior: defaults and round-trip.

use academic_core::domain::{AiProviderKind, Settings};
use academic_core::ports::SettingsRepository;
use academic_infra::db::Database;

#[test]
fn load_with_no_saved_settings_returns_defaults() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.load().unwrap(), Settings::default());
}

#[test]
fn save_then_load_round_trips_every_field() {
    let db = Database::open_in_memory().unwrap();
    let settings = Settings {
        ai_provider: AiProviderKind::Jev,
        jev_model: "jev-2".to_string(),
        ollama_base_url: "http://localhost:12345".to_string(),
        ollama_model: "llama3".to_string(),
        microsoft_client_id: Some("client-id".to_string()),
        blocked_senders: vec!["noreply@spam.test".to_string(), "@blocked.test".to_string()],
    };

    db.save(&settings).unwrap();

    assert_eq!(db.load().unwrap(), settings);
}

#[test]
fn save_overwrites_a_previously_saved_value() {
    let db = Database::open_in_memory().unwrap();
    let mut settings = Settings {
        ollama_model: "first".to_string(),
        ..Settings::default()
    };
    db.save(&settings).unwrap();

    settings.ollama_model = "second".to_string();
    db.save(&settings).unwrap();

    assert_eq!(db.load().unwrap().ollama_model, "second");
}

#[test]
fn save_can_clear_the_optional_microsoft_client_id() {
    let db = Database::open_in_memory().unwrap();
    let mut settings = Settings {
        microsoft_client_id: Some("client-id".to_string()),
        ..Settings::default()
    };
    db.save(&settings).unwrap();
    assert_eq!(
        db.load().unwrap().microsoft_client_id,
        Some("client-id".to_string())
    );

    settings.microsoft_client_id = None;
    db.save(&settings).unwrap();
    assert_eq!(db.load().unwrap().microsoft_client_id, None);
}
