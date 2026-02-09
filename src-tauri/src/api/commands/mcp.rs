use crate::core::settings_v2::{AppSettingsV2, McpSettings, SettingsV2State};
use crate::mcp::runtime::{
    McpRuntimeState, activate_server, call_tool, cancel_tool_call, connected_servers,
    deactivate_server, list_tools, resolve_tool_permission, restart_servers, shutdown,
};
use crate::mcp::types::{McpPermissionDecision, McpToolDescriptor};
use serde_json::{Map, Value};

fn get_settings_snapshot(
    settings_state: &tauri::State<'_, SettingsV2State>,
) -> Result<AppSettingsV2, String> {
    let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
    Ok(guard.get())
}

fn save_settings_snapshot(
    settings_state: &tauri::State<'_, SettingsV2State>,
    settings: AppSettingsV2,
) -> Result<(), String> {
    let mut guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
    guard.set(settings)
}

#[tauri::command]
pub fn mcp_get_config(
    settings_state: tauri::State<'_, SettingsV2State>,
) -> Result<McpSettings, String> {
    let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
    Ok(guard.get_ref().web_rag.mcp.clone())
}

#[tauri::command]
pub async fn mcp_save_config(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
    runtime_state: tauri::State<'_, McpRuntimeState>,
    config: McpSettings,
) -> Result<McpSettings, String> {
    let mut next = get_settings_snapshot(&settings_state)?;
    next.web_rag.mcp = config.clone();
    save_settings_snapshot(&settings_state, next)?;

    if config.enabled {
        restart_servers(&app, runtime_state.inner(), &config).await?;
    } else {
        shutdown(runtime_state.inner()).await?;
    }
    Ok(config)
}

#[tauri::command]
pub async fn mcp_get_connected_servers(
    runtime_state: tauri::State<'_, McpRuntimeState>,
) -> Result<Vec<String>, String> {
    Ok(connected_servers(runtime_state.inner()).await)
}

#[tauri::command]
pub async fn mcp_activate_server(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
    runtime_state: tauri::State<'_, McpRuntimeState>,
    server_id: String,
) -> Result<(), String> {
    let cfg = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard
            .get_ref()
            .web_rag
            .mcp
            .servers
            .iter()
            .find(|server| server.id == server_id)
            .cloned()
            .ok_or_else(|| format!("MCP server '{}' is not configured", server_id))?
    };
    activate_server(&app, runtime_state.inner(), &cfg).await
}

#[tauri::command]
pub async fn mcp_deactivate_server(
    runtime_state: tauri::State<'_, McpRuntimeState>,
    server_id: String,
) -> Result<(), String> {
    deactivate_server(runtime_state.inner(), &server_id).await
}

#[tauri::command]
pub async fn mcp_restart_servers(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
    runtime_state: tauri::State<'_, McpRuntimeState>,
) -> Result<(), String> {
    let settings = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.get_ref().web_rag.mcp.clone()
    };
    if settings.enabled {
        restart_servers(&app, runtime_state.inner(), &settings).await
    } else {
        shutdown(runtime_state.inner()).await
    }
}

#[tauri::command]
pub async fn mcp_list_tools(
    runtime_state: tauri::State<'_, McpRuntimeState>,
) -> Result<Vec<McpToolDescriptor>, String> {
    list_tools(runtime_state.inner()).await
}

#[tauri::command]
pub async fn mcp_call_tool(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
    runtime_state: tauri::State<'_, McpRuntimeState>,
    server_id: Option<String>,
    tool_name: String,
    arguments: Option<Map<String, Value>>,
    cancellation_token: Option<String>,
) -> Result<Value, String> {
    let settings = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.get_ref().web_rag.mcp.clone()
    };
    if !settings.enabled {
        return Err("MCP is disabled in settings".to_string());
    }
    call_tool(
        &app,
        runtime_state.inner(),
        &settings,
        server_id,
        tool_name,
        arguments,
        cancellation_token,
    )
    .await
}

#[tauri::command]
pub async fn mcp_cancel_tool_call(
    runtime_state: tauri::State<'_, McpRuntimeState>,
    cancellation_token: String,
) -> Result<(), String> {
    cancel_tool_call(runtime_state.inner(), &cancellation_token).await
}

#[tauri::command]
pub async fn mcp_resolve_tool_permission(
    runtime_state: tauri::State<'_, McpRuntimeState>,
    request_id: String,
    decision: McpPermissionDecision,
) -> Result<(), String> {
    resolve_tool_permission(runtime_state.inner(), &request_id, decision).await
}
