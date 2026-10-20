use tauri::AppHandle;

/// Version of the application, as declared in `tauri.conf.json`.
#[tauri::command]
#[specta::specta]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}
