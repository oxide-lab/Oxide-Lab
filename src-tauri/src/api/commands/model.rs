use crate::core::state::SharedState;
use crate::core::types::{ActiveBackend, LoadRequest};
use crate::generate::cancel::{CANCEL_LOADING, cancel_model_loading_cmd};

#[tauri::command]
pub async fn load_model(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedState>,
    llama_state: tauri::State<'_, crate::inference::llamacpp::state::LlamaCppState>,
    req: LoadRequest,
) -> Result<(), String> {
    CANCEL_LOADING.store(false, std::sync::atomic::Ordering::SeqCst);
    let app_clone = app.clone();
    let state_arc = state.inner().clone();
    let llama_state = llama_state.inner().clone();

    tauri::async_runtime::spawn(async move {
        let result =
            crate::inference::router::load_model(app_clone.clone(), state_arc, llama_state, req)
                .await;
        if let Err(e) = result {
            crate::api::model_loading::emit_load_progress(
                &app_clone,
                "error",
                100,
                None,
                true,
                Some(&e),
            );
        }
    });

    Ok(())
}

#[tauri::command]
pub fn cancel_model_loading() -> Result<(), String> {
    cancel_model_loading_cmd()
}

#[tauri::command]
pub async fn unload_model(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedState>,
    llama_state: tauri::State<'_, crate::inference::llamacpp::state::LlamaCppState>,
) -> Result<(), String> {
    crate::inference::router::unload_model(app, state.inner().clone(), llama_state.inner().clone())
        .await
}

#[tauri::command]
pub fn is_model_loaded(state: tauri::State<'_, SharedState>) -> Result<bool, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(matches!(guard.active_backend, ActiveBackend::Llamacpp) && guard.active_model_id.is_some())
}

