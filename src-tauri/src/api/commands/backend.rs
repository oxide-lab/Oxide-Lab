use crate::core::state::SharedState;
use crate::core::settings_v2::SettingsV2State;
use crate::core::types::{ActiveBackend, BackendPreference, LlamaRuntimeConfig};
use crate::inference::scheduler::{SchedulerStats, VramScheduler};
use tauri::Manager;

#[tauri::command]
pub fn get_active_backend(state: tauri::State<'_, SharedState>) -> Result<ActiveBackend, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(match guard.active_backend {
        ActiveBackend::Llamacpp => ActiveBackend::Llamacpp,
        _ => ActiveBackend::None,
    })
}

#[tauri::command]
pub fn get_backend_preference(
    _state: tauri::State<'_, SharedState>,
) -> Result<BackendPreference, String> {
    Ok(BackendPreference::Llamacpp)
}

#[tauri::command]
pub fn set_backend_preference(
    state: tauri::State<'_, SharedState>,
    _preference: BackendPreference,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.backend_preference = BackendPreference::Llamacpp;
    Ok(())
}

#[tauri::command]
pub fn get_llama_runtime_config(
    state: tauri::State<'_, SharedState>,
) -> Result<LlamaRuntimeConfig, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.llama_runtime.clone())
}

#[tauri::command]
pub fn set_llama_runtime_config(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedState>,
    config: LlamaRuntimeConfig,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.llama_runtime = config.clone();
    crate::core::state::ModelState::save_llama_runtime(&app, &config)?;

    if let Some(settings_state) = app.try_state::<SettingsV2State>() {
        let mut settings_guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        let mut settings = settings_guard.get();
        settings.performance.llama_runtime = config;
        settings_guard.set(settings)?;
    }

    Ok(())
}

#[tauri::command]
pub fn get_loaded_models(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    let scheduler = app
        .try_state::<VramScheduler>()
        .ok_or_else(|| "scheduler is not initialized".to_string())?;
    Ok(scheduler.snapshot().loaded_models)
}

#[tauri::command]
pub fn get_scheduler_stats(app: tauri::AppHandle) -> Result<SchedulerStats, String> {
    let scheduler = app
        .try_state::<VramScheduler>()
        .ok_or_else(|| "scheduler is not initialized".to_string())?;
    Ok(scheduler.stats())
}
