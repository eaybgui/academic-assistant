//! Android-only Tauri plugin. The Kotlin class `AcademicPlugin` keeps secrets
//! encrypted under an Android Keystore key; Rust reaches it through the
//! [`PluginHandle`] returned by [`handle`]. On other targets the plugin is an
//! empty shell so the crate still builds as a workspace member.

use tauri::plugin::{Builder, TauriPlugin};
use tauri::Runtime;
#[cfg(target_os = "android")]
use tauri::{plugin::PluginHandle, Manager};

const PLUGIN_NAME: &str = "academic-android";
#[cfg(target_os = "android")]
const ANDROID_PACKAGE: &str = "dev.eaybar.academicassistant.plugin";
#[cfg(target_os = "android")]
const ANDROID_CLASS: &str = "AcademicPlugin";

#[cfg(target_os = "android")]
struct AcademicHandle<R: Runtime>(PluginHandle<R>);

/// Registers the plugin (and, on Android, its Kotlin class).
pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new(PLUGIN_NAME)
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            _app.manage(AcademicHandle(
                _api.register_android_plugin(ANDROID_PACKAGE, ANDROID_CLASS)?,
            ));
            Ok(())
        })
        .build()
}

/// Handle to the Kotlin plugin. The plugin must have been added with [`init`].
#[cfg(target_os = "android")]
pub fn handle<R: Runtime>(app: &impl Manager<R>) -> PluginHandle<R> {
    app.state::<AcademicHandle<R>>().0.clone()
}
