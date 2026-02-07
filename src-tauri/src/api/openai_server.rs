use crate::api::openai::OpenAiServerController;
use crate::core::settings_v2::SettingsV2State;

#[derive(serde::Serialize)]
pub struct ServerConfig {
    pub port: u16,
    pub running: bool,
}

#[tauri::command]
pub async fn get_server_config(
    settings_state: tauri::State<'_, SettingsV2State>,
    controller: tauri::State<'_, OpenAiServerController>,
) -> Result<ServerConfig, String> {
    let cfg = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.get_ref().developer.openai_server.clone()
    };
    let running = controller.is_running().await;
    Ok(ServerConfig {
        port: cfg.port,
        running,
    })
}
