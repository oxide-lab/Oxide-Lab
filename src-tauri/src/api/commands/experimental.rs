use crate::core::settings_v2::SettingsV2State;
use std::fs::{File, create_dir_all};
use std::io::{Read, Write};
use tauri::Manager;

#[tauri::command]
pub fn get_experimental_features_enabled(app: tauri::AppHandle) -> Result<bool, String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let profile_dir = dir.join("oxide-lab");
    let path = profile_dir.join("experimental_features.json");

    if !path.exists() {
        return Ok(false);
    }

    let mut file =
        File::open(&path).map_err(|e| format!("Failed to open experimental features file: {e}"))?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)
        .map_err(|e| format!("Failed to read experimental features file: {e}"))?;
    let enabled: bool = serde_json::from_str(&buf)
        .map_err(|e| format!("Failed to parse experimental features file: {e}"))?;
    Ok(enabled)
}

#[tauri::command]
pub fn set_experimental_features_enabled(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<(), String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    let profile_dir = dir.join("oxide-lab");
    create_dir_all(&profile_dir).map_err(|e| format!("Failed to create profile directory: {e}"))?;
    let path = profile_dir.join("experimental_features.json");

    let mut file = File::create(&path)
        .map_err(|e| format!("Failed to create experimental features file: {e}"))?;
    let data = serde_json::to_string(&enabled)
        .map_err(|e| format!("Failed to serialize experimental features: {e}"))?;
    file.write_all(data.as_bytes())
        .map_err(|e| format!("Failed to write experimental features file: {e}"))?;

    if let Some(settings_state) = app.try_state::<SettingsV2State>() {
        let mut guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        let mut settings = guard.get();
        settings.general.developer_mode = enabled;
        guard.set(settings)?;
    }
    Ok(())
}
