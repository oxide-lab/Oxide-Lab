use crate::core::state::{ModelState, SharedState};
use crate::core::settings_v2::SettingsV2State;

use std::env;
use tauri::AppHandle;
use tauri::Manager;

const RAYON_ENV_VAR: &str = "RAYON_NUM_THREADS";

pub(crate) fn default_rayon_thread_limit() -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    cpus.saturating_sub(1).max(1)
}

pub(crate) fn apply_rayon_thread_limit(limit: Option<usize>) {
    unsafe {
        match limit {
            Some(count) => env::set_var(RAYON_ENV_VAR, count.to_string()),
            None => env::remove_var(RAYON_ENV_VAR),
        }
    }
}

#[tauri::command]
pub fn get_rayon_thread_limit(app: AppHandle) -> Result<Option<usize>, String> {
    ModelState::load_thread_limit(&app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_rayon_thread_limit(
    app: AppHandle,
    state: tauri::State<'_, SharedState>,
    limit: Option<usize>,
) -> Result<(), String> {
    apply_rayon_thread_limit(limit);
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.rayon_thread_limit = limit;
    ModelState::save_thread_limit(&app, limit).map_err(|e| e.to_string())?;

    if let Some(settings_state) = app.try_state::<SettingsV2State>() {
        let mut settings_guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        let mut settings = settings_guard.get();
        settings.performance.manual_thread_limit = limit;
        settings_guard.set(settings)?;
    }

    Ok(())
}
