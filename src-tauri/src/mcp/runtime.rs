use crate::core::settings_v2::{McpPermissionMode, McpServerConfig, McpSettings, McpTransportType};
use crate::mcp::types::{McpPermissionDecision, McpToolDescriptor};
use rmcp::model::{CallToolRequestParam, CallToolResult, Tool};
use rmcp::service::{RunningService, ServiceError};
use rmcp::transport::{
    TokioChildProcess, streamable_http_client::StreamableHttpClientTransportConfig,
};
use rmcp::{
    RoleClient, ServiceExt,
    model::{ClientCapabilities, ClientInfo, Implementation, InitializeRequestParam},
    transport::StreamableHttpClientTransport,
};
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::process::Command;
use tokio::sync::{Mutex, oneshot};
use tokio::time::timeout;
use uuid::Uuid;

const MCP_TOOL_DISCOVERY_TIMEOUT_SECS: u64 = 3;

pub enum RunningServiceEnum {
    NoInit(RunningService<RoleClient, ()>),
    WithInit(RunningService<RoleClient, InitializeRequestParam>),
}

impl RunningServiceEnum {
    async fn list_all_tools(&self) -> Result<Vec<Tool>, ServiceError> {
        match self {
            Self::NoInit(service) => service.list_all_tools().await,
            Self::WithInit(service) => service.list_all_tools().await,
        }
    }

    async fn call_tool(
        &self,
        params: CallToolRequestParam,
    ) -> Result<CallToolResult, ServiceError> {
        match self {
            Self::NoInit(service) => service.call_tool(params).await,
            Self::WithInit(service) => service.call_tool(params).await,
        }
    }

    async fn cancel(self) -> Result<(), String> {
        match self {
            Self::NoInit(service) => service
                .cancel()
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
            Self::WithInit(service) => service
                .cancel()
                .await
                .map(|_| ())
                .map_err(|e| e.to_string()),
        }
    }
}

struct PendingPermission {
    sender: oneshot::Sender<McpPermissionDecision>,
    server_id: String,
    tool_name: String,
}

#[derive(Clone, Default)]
pub struct McpRuntimeState {
    servers: Arc<Mutex<HashMap<String, RunningServiceEnum>>>,
    pending_permissions: Arc<Mutex<HashMap<String, PendingPermission>>>,
    session_allowed_tools: Arc<Mutex<HashSet<String>>>,
    server_allowed: Arc<Mutex<HashSet<String>>>,
    tool_call_cancellations: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
}

fn permission_key(server_id: &str, tool_name: &str) -> String {
    format!("{server_id}:{tool_name}")
}

fn parse_headers(headers: &HashMap<String, String>) -> Result<reqwest::header::HeaderMap, String> {
    let mut out = reqwest::header::HeaderMap::new();
    for (name, value) in headers {
        let normalized = name.trim().to_ascii_lowercase();
        let canonical_name: &str = match normalized.as_str() {
            // User-facing alias used in UI; convert to actual HTTP header expected by providers.
            "context7_api_key" | "x_context7_api_key" => "Context7-API-Key",
            _ => name.trim(),
        };
        let header_name = reqwest::header::HeaderName::from_bytes(canonical_name.as_bytes())
            .map_err(|e| format!("invalid MCP header name {canonical_name}: {e}"))?;
        let header_value = reqwest::header::HeaderValue::from_str(value)
            .map_err(|e| format!("invalid MCP header value for {canonical_name}: {e}"))?;
        out.insert(header_name, header_value);
    }
    Ok(out)
}

fn has_context7_auth(headers: &HashMap<String, String>) -> bool {
    headers.keys().any(|key| {
        let normalized = key.trim().to_ascii_lowercase();
        normalized == "context7_api_key"
            || normalized == "authorization"
            || normalized == "context7-api-key"
            || normalized == "x-context7-api-key"
            || normalized == "x-api-key"
    })
}

async fn start_service(config: &McpServerConfig) -> Result<RunningServiceEnum, String> {
    match config.transport {
        McpTransportType::Stdio => {
            let command = config
                .command
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    format!(
                        "MCP server '{}' requires command for stdio transport",
                        config.id
                    )
                })?;

            let mut cmd = Command::new(command);
            cmd.kill_on_drop(true);
            cmd.stdin(Stdio::piped());
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());

            for arg in &config.args {
                cmd.arg(arg);
            }
            for (key, value) in &config.env {
                cmd.env(key, value);
            }

            #[cfg(windows)]
            {
                cmd.creation_flags(0x08000000);
            }

            let (process, _stderr) = TokioChildProcess::builder(cmd)
                .spawn()
                .map_err(|e| format!("failed to spawn MCP server '{}': {e}", config.id))?;
            let service = ()
                .serve(process)
                .await
                .map_err(|e| format!("failed to initialize MCP stdio '{}': {e}", config.id))?;
            Ok(RunningServiceEnum::NoInit(service))
        }
        McpTransportType::StreamableHttp => {
            let url = config
                .url
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    format!(
                        "MCP server '{}' requires url for streamable_http transport",
                        config.id
                    )
                })?;
            if url.contains("mcp.context7.com") && !has_context7_auth(&config.headers) {
                return Err(format!(
                    "MCP server '{}' (Context7) requires auth header (CONTEXT7_API_KEY or Authorization). \
Set header in MCP server config or use stdio with @upstash/context7-mcp --api-key",
                    config.id
                ));
            }
            let headers = parse_headers(&config.headers)?;
            let client = reqwest::Client::builder()
                .user_agent(format!("Oxide-Lab/{}", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(8))
                .timeout(Duration::from_secs(20))
                .default_headers(headers)
                .build()
                .map_err(|e| format!("failed to build MCP http client '{}': {e}", config.id))?;

            let transport = StreamableHttpClientTransport::with_client(
                client,
                StreamableHttpClientTransportConfig {
                    uri: url.to_string().into(),
                    ..Default::default()
                },
            );
            let client_info = ClientInfo {
                protocol_version: Default::default(),
                capabilities: ClientCapabilities::default(),
                client_info: Implementation {
                    name: "Oxide-Lab MCP Client".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    title: None,
                    website_url: None,
                    icons: None,
                },
            };
            let service = client_info.serve(transport).await.map_err(|e| {
                format!("failed to connect MCP streamable-http '{}': {e}", config.id)
            })?;
            Ok(RunningServiceEnum::WithInit(service))
        }
    }
}

pub async fn activate_server(
    app: &AppHandle,
    state: &McpRuntimeState,
    config: &McpServerConfig,
) -> Result<(), String> {
    let service = start_service(config).await?;
    let mut servers = state.servers.lock().await;
    servers.insert(config.id.clone(), service);
    let _ = app.emit(
        "tooling_log",
        serde_json::json!({
            "category": "MCP_DEBUG",
            "message": "MCP server activated",
            "details": { "server_id": config.id }
        }),
    );
    Ok(())
}

pub async fn deactivate_server(state: &McpRuntimeState, server_id: &str) -> Result<(), String> {
    let maybe = state.servers.lock().await.remove(server_id);
    if let Some(service) = maybe {
        service
            .cancel()
            .await
            .map_err(|e| format!("failed to stop MCP server '{server_id}': {e}"))?;
    }
    Ok(())
}

pub async fn restart_servers(
    app: &AppHandle,
    state: &McpRuntimeState,
    settings: &McpSettings,
) -> Result<(), String> {
    shutdown(state).await?;
    let enabled: Vec<&McpServerConfig> =
        settings.servers.iter().filter(|cfg| cfg.enabled).collect();
    if enabled.is_empty() {
        return Ok(());
    }

    let mut activated = 0usize;
    let mut errors: Vec<String> = Vec::new();
    for config in enabled {
        match activate_server(app, state, config).await {
            Ok(()) => {
                activated += 1;
            }
            Err(err) => {
                let msg = format!("{}: {}", config.id, err);
                log::warn!("MCP_DEBUG failed to activate server: {}", msg);
                errors.push(msg);
            }
        }
    }

    if activated == 0 {
        return Err(format!(
            "failed to activate all configured MCP servers: {}",
            errors.join(" | ")
        ));
    }

    if !errors.is_empty() {
        let _ = app.emit(
            "tooling_log",
            serde_json::json!({
                "category": "MCP_DEBUG",
                "message": "Some MCP servers failed to activate; continuing with active subset",
                "details": {
                    "activated": activated,
                    "failed": errors,
                }
            }),
        );
    }

    Ok(())
}

pub async fn connected_servers(state: &McpRuntimeState) -> Vec<String> {
    state.servers.lock().await.keys().cloned().collect()
}

pub async fn list_tools(state: &McpRuntimeState) -> Result<Vec<McpToolDescriptor>, String> {
    let mut out = Vec::new();
    let servers = state.servers.lock().await;
    for (server_id, service) in servers.iter() {
        let tools = match timeout(
            Duration::from_secs(MCP_TOOL_DISCOVERY_TIMEOUT_SECS),
            service.list_all_tools(),
        )
        .await
        {
            Ok(Ok(tools)) => tools,
            Ok(Err(err)) => {
                log::warn!(
                    "MCP_DEBUG failed to list tools for server '{}': {}",
                    server_id,
                    err
                );
                continue;
            }
            Err(_) => {
                log::warn!(
                    "MCP_DEBUG timed out listing tools for server '{}'",
                    server_id
                );
                continue;
            }
        };
        for tool in tools {
            out.push(McpToolDescriptor {
                server_id: server_id.clone(),
                name: tool.name.to_string(),
                description: tool.description.as_ref().map(|v| v.to_string()),
                input_schema: Value::Object((*tool.input_schema).clone()),
            });
        }
    }
    Ok(out)
}

async fn resolve_service_for_tool(
    state: &McpRuntimeState,
    server_id: Option<&str>,
    tool_name: &str,
) -> Result<(String, bool), String> {
    let servers = state.servers.lock().await;
    let server_candidates: Vec<(&String, &RunningServiceEnum)> = if let Some(server_id) = server_id
    {
        servers
            .iter()
            .filter(|(id, _)| id.as_str() == server_id)
            .collect()
    } else {
        servers.iter().collect()
    };

    if server_candidates.is_empty() {
        return Err("no active MCP servers found".to_string());
    }

    let total_candidates = server_candidates.len();
    let mut list_errors: Vec<String> = Vec::new();
    for (id, service) in server_candidates {
        let tools = match timeout(
            Duration::from_secs(MCP_TOOL_DISCOVERY_TIMEOUT_SECS),
            service.list_all_tools(),
        )
        .await
        {
            Ok(Ok(tools)) => tools,
            Ok(Err(err)) => {
                list_errors.push(format!("failed to list tools for '{id}': {err}"));
                continue;
            }
            Err(_) => {
                list_errors.push(format!("timed out listing tools for '{id}'"));
                continue;
            }
        };
        if tools.iter().any(|tool| tool.name == tool_name) {
            return Ok((id.clone(), true));
        }
    }
    if !list_errors.is_empty() && list_errors.len() == total_candidates {
        return Err(format!(
            "failed to query MCP servers: {}",
            list_errors.join(" | ")
        ));
    }
    Err(format!(
        "tool '{tool_name}' was not found in active MCP servers"
    ))
}

async fn request_permission(
    app: &AppHandle,
    state: &McpRuntimeState,
    server_id: &str,
    tool_name: &str,
    arguments: &Option<Map<String, Value>>,
) -> Result<McpPermissionDecision, String> {
    let request_id = Uuid::new_v4().to_string();
    let (tx, rx) = oneshot::channel::<McpPermissionDecision>();
    state.pending_permissions.lock().await.insert(
        request_id.clone(),
        PendingPermission {
            sender: tx,
            server_id: server_id.to_string(),
            tool_name: tool_name.to_string(),
        },
    );

    let _ = app.emit(
        "mcp_tool_permission_request",
        serde_json::json!({
            "request_id": request_id.clone(),
            "server_id": server_id,
            "tool_name": tool_name,
            "arguments": arguments,
        }),
    );

    match timeout(Duration::from_secs(120), rx).await {
        Ok(Ok(decision)) => Ok(decision),
        Ok(Err(_)) => {
            state.pending_permissions.lock().await.remove(&request_id);
            Err("MCP tool permission request channel closed".to_string())
        }
        Err(_) => {
            state.pending_permissions.lock().await.remove(&request_id);
            Err("MCP tool permission request timed out".to_string())
        }
    }
}

async fn ensure_permission(
    app: &AppHandle,
    state: &McpRuntimeState,
    settings: &McpSettings,
    server_id: &str,
    tool_name: &str,
    arguments: &Option<Map<String, Value>>,
) -> Result<(), String> {
    let perm_key = permission_key(server_id, tool_name);
    let mode = settings.default_permission_mode;
    match mode {
        McpPermissionMode::PerCall => {}
        McpPermissionMode::AllowThisSession => {
            if state.session_allowed_tools.lock().await.contains(&perm_key) {
                return Ok(());
            }
        }
        McpPermissionMode::AllowThisServer => {
            if state.server_allowed.lock().await.contains(server_id) {
                return Ok(());
            }
        }
    }

    let decision = request_permission(app, state, server_id, tool_name, arguments).await?;
    match decision {
        McpPermissionDecision::AllowOnce => Ok(()),
        McpPermissionDecision::AllowThisSession => {
            state.session_allowed_tools.lock().await.insert(perm_key);
            Ok(())
        }
        McpPermissionDecision::AllowThisServer => {
            state
                .server_allowed
                .lock()
                .await
                .insert(server_id.to_string());
            Ok(())
        }
        McpPermissionDecision::Deny => Err("MCP tool call denied by user".to_string()),
    }
}

pub async fn call_tool(
    app: &AppHandle,
    state: &McpRuntimeState,
    settings: &McpSettings,
    server_id: Option<String>,
    tool_name: String,
    arguments: Option<Map<String, Value>>,
    cancellation_token: Option<String>,
) -> Result<Value, String> {
    let (resolved_server_id, _) =
        resolve_service_for_tool(state, server_id.as_deref(), &tool_name).await?;
    ensure_permission(
        app,
        state,
        settings,
        &resolved_server_id,
        &tool_name,
        &arguments,
    )
    .await?;

    let call_id = cancellation_token
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let _ = app.emit(
        "mcp_tool_call_started",
        serde_json::json!({
            "call_id": call_id,
            "server_id": resolved_server_id,
            "tool_name": tool_name,
            "arguments": arguments.clone(),
        }),
    );

    let (cancel_tx, cancel_rx) = oneshot::channel::<()>();
    if let Some(token) = &cancellation_token {
        state
            .tool_call_cancellations
            .lock()
            .await
            .insert(token.clone(), cancel_tx);
    }

    let timeout_duration = Duration::from_millis(settings.tool_call_timeout_ms.max(1_000));
    let tool_future = async {
        let servers = state.servers.lock().await;
        let service = servers
            .get(&resolved_server_id)
            .ok_or_else(|| format!("MCP server '{}' is no longer active", resolved_server_id))?;
        service
            .call_tool(CallToolRequestParam {
                name: tool_name.clone().into(),
                arguments: arguments.clone(),
            })
            .await
            .map_err(|e| format!("MCP tool call failed: {e}"))
    };

    let result = if cancellation_token.is_some() {
        tokio::select! {
            call_result = timeout(timeout_duration, tool_future) => {
                match call_result {
                    Ok(result) => result,
                    Err(_) => Err(format!("MCP tool call '{}' timed out", tool_name)),
                }
            }
            _ = cancel_rx => Err(format!("MCP tool call '{}' was cancelled", tool_name)),
        }
    } else {
        match timeout(timeout_duration, tool_future).await {
            Ok(result) => result,
            Err(_) => Err(format!("MCP tool call '{}' timed out", tool_name)),
        }
    };

    if let Some(token) = &cancellation_token {
        state.tool_call_cancellations.lock().await.remove(token);
    }

    match result {
        Ok(payload) => {
            let serialized = serde_json::to_value(&payload)
                .map_err(|e| format!("failed to serialize MCP tool result: {e}"))?;
            let _ = app.emit(
                "mcp_tool_call_finished",
                serde_json::json!({
                    "call_id": call_id,
                    "server_id": resolved_server_id,
                    "tool_name": tool_name,
                    "result": serialized,
                }),
            );
            Ok(serialized)
        }
        Err(err) => {
            let _ = app.emit(
                "mcp_tool_call_error",
                serde_json::json!({
                    "call_id": call_id,
                    "server_id": resolved_server_id,
                    "tool_name": tool_name,
                    "error": err,
                }),
            );
            Err(err)
        }
    }
}

pub async fn cancel_tool_call(
    state: &McpRuntimeState,
    cancellation_token: &str,
) -> Result<(), String> {
    let sender = state
        .tool_call_cancellations
        .lock()
        .await
        .remove(cancellation_token)
        .ok_or_else(|| format!("MCP cancellation token '{cancellation_token}' not found"))?;
    let _ = sender.send(());
    Ok(())
}

pub async fn resolve_tool_permission(
    state: &McpRuntimeState,
    request_id: &str,
    decision: McpPermissionDecision,
) -> Result<(), String> {
    let pending = state.pending_permissions.lock().await.remove(request_id);
    let Some(pending) = pending else {
        // The request may already be resolved or timed out. Treat as idempotent success.
        log::warn!(
            "MCP_DEBUG resolve_tool_permission ignored stale request_id={}",
            request_id
        );
        return Ok(());
    };
    let _server_id = pending.server_id;
    let _tool_name = pending.tool_name;
    pending
        .sender
        .send(decision)
        .map_err(|_| format!("MCP permission request '{request_id}' receiver dropped"))?;
    Ok(())
}

pub async fn shutdown(state: &McpRuntimeState) -> Result<(), String> {
    let servers: Vec<RunningServiceEnum> = {
        let mut locked = state.servers.lock().await;
        locked.drain().map(|(_, service)| service).collect()
    };
    for service in servers {
        let _ = timeout(Duration::from_secs(2), service.cancel()).await;
    }
    state.session_allowed_tools.lock().await.clear();
    state.server_allowed.lock().await.clear();
    state.pending_permissions.lock().await.clear();
    state.tool_call_cancellations.lock().await.clear();
    Ok(())
}
