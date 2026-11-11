//! Composition root for the Academic Assistant desktop app: builds the Tauri
//! application, wires the `academic-infra` adapters to `academic-core` ports and
//! registers the Tauri commands and events consumed by the React frontend.

#[cfg(not(target_os = "android"))]
mod account_status;
#[cfg(target_os = "android")]
mod android_secrets;
mod commands;
mod dto;
mod error;
mod lms_auth;
mod lms_observer;
#[cfg(not(target_os = "android"))]
mod observer;
mod setup;
mod state;
#[cfg(test)]
mod test_support;
mod validation;

use tauri::Manager;

#[cfg(target_os = "android")]
use commands::android;
#[cfg(not(target_os = "android"))]
use commands::{accounts, catalog, results, runs, settings};
use commands::{personal, tracker, universities};

/// The commands every target registers (LMS, tracker and personal tasks), with
/// the desktop-only email and AI commands passed in first.
macro_rules! lms_handler {
    ($($email:tt)*) => {
        tauri::generate_handler![
            $($email)*
            universities::lms_probe,
            universities::get_universities,
            universities::add_university,
            universities::rename_university,
            universities::remove_university,
            universities::lms_sign_in,
            universities::lms_sign_out,
            universities::lms_check_session,
            universities::lms_fetch_courses,
            universities::get_courses,
            universities::set_tracked_courses,
            tracker::lms_sync,
            tracker::get_tracker,
            tracker::set_item_done,
            tracker::set_item_hidden,
            tracker::set_item_note,
            tracker::dismiss_item_change,
            tracker::dismiss_all_changes,
            personal::create_personal_task,
            personal::update_personal_task,
            personal::set_personal_task_done,
            personal::delete_personal_task,
        ]
    };
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default().plugin(tauri_plugin_opener::init());
    #[cfg(target_os = "android")]
    let builder = builder.plugin(tauri_plugin_academic_android::init());
    let builder = builder
        .setup(|app| setup::initialize(app))
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                lms_auth::close_all_login_windows(window.app_handle());
            }
        });
    #[cfg(not(target_os = "android"))]
    let builder = builder.invoke_handler(lms_handler!(
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
        settings::get_settings,
        settings::update_settings,
        settings::set_jev_api_key,
        settings::clear_jev_api_key,
        settings::test_ai_provider,
    ));
    #[cfg(target_os = "android")]
    let builder = builder.invoke_handler(lms_handler!(android::android_leave_app,));
    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
