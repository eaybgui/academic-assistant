//! Composition root for the Academic Assistant desktop app: builds the Tauri
//! application, wires the `academic-infra` adapters to `academic-core` ports and
//! registers the Tauri commands and events consumed by the React frontend.

mod account_status;
mod commands;
mod dto;
mod error;
mod lms_auth;
mod observer;
mod setup;
mod state;
#[cfg(test)]
mod test_support;
mod validation;

use tauri::Manager;

use commands::{accounts, catalog, results, runs, settings, universities};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| setup::initialize(app))
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                lms_auth::close_all_login_windows(window.app_handle());
            }
        })
        .invoke_handler(tauri::generate_handler![
            runs::check_now,
            runs::get_dashboard,
            runs::get_run_history,
            results::get_results,
            results::set_result_seen,
            results::mark_all_seen,
            accounts::get_accounts,
            accounts::add_account,
            accounts::update_account,
            accounts::remove_account,
            accounts::test_account,
            accounts::connect_microsoft_account,
            catalog::get_catalog,
            universities::lms_probe,
            universities::get_universities,
            universities::add_university,
            universities::rename_university,
            universities::remove_university,
            universities::lms_sign_in,
            universities::lms_sign_out,
            universities::lms_check_session,
            settings::get_settings,
            settings::update_settings,
            settings::set_jev_api_key,
            settings::clear_jev_api_key,
            settings::test_ai_provider,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
