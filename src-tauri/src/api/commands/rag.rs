use crate::core::settings_v2::SettingsV2State;
use crate::retrieval::local_rag::{
    LocalRagIndexResult, LocalRagSourceRecord, LocalRagStats, add_source, clear_index,
    list_sources, open_source_folder, remove_source, stats,
};

#[tauri::command]
pub fn rag_list_sources(app: tauri::AppHandle) -> Result<Vec<LocalRagSourceRecord>, String> {
    list_sources(&app)
}

#[tauri::command]
pub fn rag_get_stats(app: tauri::AppHandle) -> Result<LocalRagStats, String> {
    stats(&app)
}

#[tauri::command]
pub async fn rag_add_source(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
    path: String,
) -> Result<LocalRagIndexResult, String> {
    let cfg = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.get_ref().web_rag.clone()
    };
    add_source(&app, &path, &cfg.local_rag, &cfg.embeddings_provider).await
}

#[tauri::command]
pub async fn rag_reindex_source(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
    source_id: String,
) -> Result<LocalRagIndexResult, String> {
    let cfg = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.get_ref().web_rag.clone()
    };
    let path = list_sources(&app)?
        .into_iter()
        .find(|s| s.id == source_id)
        .map(|s| s.path)
        .ok_or_else(|| "Source not found".to_string())?;
    add_source(&app, &path, &cfg.local_rag, &cfg.embeddings_provider).await
}

#[tauri::command]
pub fn rag_remove_source(app: tauri::AppHandle, source_id: String) -> Result<(), String> {
    remove_source(&app, &source_id)
}

#[tauri::command]
pub fn rag_clear_index(app: tauri::AppHandle) -> Result<(), String> {
    clear_index(&app)
}

#[tauri::command]
pub async fn rag_test_embeddings_provider(app: tauri::AppHandle) -> Result<(), String> {
    crate::retrieval::orchestrator::test_embeddings_provider(&app).await
}

#[tauri::command]
pub fn rag_open_source_folder(app: tauri::AppHandle, source_id: String) -> Result<String, String> {
    open_source_folder(&app, &source_id)
}
