use crate::core::settings_v2::SettingsV2State;
use crate::core::state::{ModelState, SharedState};

use std::sync::atomic::{AtomicUsize, Ordering};
use tauri::AppHandle;
use tauri::Manager;

const NO_THREAD_LIMIT: usize = 0;
static RAYON_THREAD_LIMIT_HINT: AtomicUsize = AtomicUsize::new(NO_THREAD_LIMIT);

pub(crate) fn default_rayon_thread_limit() -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    cpus.saturating_sub(1).max(1)
}

pub(crate) fn apply_rayon_thread_limit(limit: Option<usize>) {
    // Runtime changes are persisted in settings/state and applied on next startup.
    // Avoid mutating process environment in multithreaded context.
    let stored = limit.unwrap_or(NO_THREAD_LIMIT);
    RAYON_THREAD_LIMIT_HINT.store(stored, Ordering::Relaxed);
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
