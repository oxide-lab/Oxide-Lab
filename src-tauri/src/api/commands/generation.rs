use crate::core::state::SharedState;
use crate::core::types::GenerateRequest;
use crate::log_template;

#[tauri::command]
pub async fn generate_stream(
    app: tauri::AppHandle,
    state: tauri::State<'_, SharedState>,
    llama_state: tauri::State<'_, crate::inference::llamacpp::state::LlamaCppState>,
    req: GenerateRequest,
) -> Result<(), String> {
    if let Ok(guard) = state.lock() {
        log_template!(
            "present_at_generate={}",
            guard.chat_template.as_ref().map(|_| true).unwrap_or(false)
        );
    }
    crate::inference::router::generate_stream(
        app,
        state.inner().clone(),
        llama_state.inner().clone(),
        req,
    )
    .await
}

#[tauri::command]
pub fn cancel_generation() -> Result<(), String> {
    crate::generate::cancel_generation_cmd()
}
