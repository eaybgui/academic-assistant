//! Composition root for the Academic Assistant desktop app: builds the Tauri
//! application, wires the `academic-infra` adapters to `academic-core` ports and
//! registers Tauri commands and events (filled in by later tasks).

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
