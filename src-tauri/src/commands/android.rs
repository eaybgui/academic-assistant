//! Commands that exist only on Android.

use tauri::AppHandle;

/// Sends the app to the background (Back on the first screen). `async`, so the
/// blocking plugin call runs on the async runtime and not on the main thread.
#[tauri::command]
pub async fn android_leave_app(app: AppHandle) -> Result<(), String> {
    tauri_plugin_academic_android::handle(&app)
        .run_mobile_plugin::<()>("moveTaskToBack", ())
        .map_err(|error| error.to_string())
}
