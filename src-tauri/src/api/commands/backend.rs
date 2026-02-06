use crate::core::state::SharedState;
use crate::core::types::{ActiveBackend, BackendPreference, LlamaRuntimeConfig};

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
    Ok(())
}

