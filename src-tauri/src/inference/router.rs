use crate::api::model_loading::emit_load_progress;
use crate::core::settings_v2::SettingsV2State;
use crate::core::state::SharedState;
use crate::core::types::{
    ActiveBackend, GenerateRequest, LlamaSessionKind, LlamaSessionSnapshot, LoadRequest,
};
use crate::inference::engine::{self, EngineSessionKind, ResolvedModelSource};
use crate::inference::llamacpp::http_client::preflight_chat_messages;
use crate::inference::llamacpp::state::LlamaCppState;
use crate::inference::scheduler::{RequestPriority, VramScheduler};
use tauri::{Emitter, Manager};

fn to_snapshot(info: &crate::inference::engine::EngineSessionInfo) -> LlamaSessionSnapshot {
    LlamaSessionSnapshot {
        pid: info.pid,
        port: info.port,
        model_id: info.model_id.clone(),
        api_key: info.api_key.clone(),
        kind: match info.kind {
            EngineSessionKind::Chat => LlamaSessionKind::Chat,
            EngineSessionKind::Embedding => LlamaSessionKind::Embedding,
        },
        created_at: info.created_at,
        last_health_ok_at: info.last_health_ok_at,
    }
}

fn source_from_state(
    model_id: String,
    model_path: String,
    context_length: usize,
) -> ResolvedModelSource {
    ResolvedModelSource {
        model_id,
        model_path,
        context_length,
    }
}

pub async fn load_model(
    app: tauri::AppHandle,
    state_arc: SharedState,
    llama_state: LlamaCppState,
    req: LoadRequest,
) -> Result<(), String> {
    emit_load_progress(&app, "start", 5, Some("Resolving model"), false, None);

    let runtime_cfg = {
        let guard = state_arc.lock().map_err(|e| e.to_string())?;
        guard.llama_runtime.clone()
    };

    let manager = engine::default_session_manager(app.clone(), llama_state);
    let source = manager.resolve_model_source(&req)?;
    let scheduler = app.state::<VramScheduler>().inner().clone();

    emit_load_progress(
        &app,
        "llama_start",
        25,
        Some("Starting llama-server"),
        false,
        None,
    );

    let chat_session = scheduler
        .preload_chat(source.clone(), runtime_cfg.clone())
        .await?;

    emit_load_progress(
        &app,
        "llama_ready",
        80,
        Some("llama-server is ready"),
        false,
        None,
    );

    let mut guard = state_arc.lock().map_err(|e| e.to_string())?;
    guard.context_length = source.context_length.max(1);
    guard.model_path = Some(source.model_path.clone());
    guard.hub_repo_id = match &req {
        LoadRequest::HubGguf { repo_id, .. } => Some(repo_id.clone()),
        _ => None,
    };
    guard.hub_revision = match &req {
        LoadRequest::HubGguf { revision, .. } => revision.clone(),
        _ => None,
    };
    guard.chat_template = None;
    guard.active_backend = ActiveBackend::Llamacpp;
    guard.active_model_id = Some(source.model_id);
    guard.active_llama_session = Some(to_snapshot(&chat_session));
    guard.backend_preference = crate::core::types::BackendPreference::Llamacpp;

    emit_load_progress(&app, "complete", 100, Some("Ready"), true, None);
    Ok(())
}

pub async fn generate_stream(
    app: tauri::AppHandle,
    state_arc: SharedState,
    llama_state: LlamaCppState,
    mut req: GenerateRequest,
) -> Result<(), String> {
    let (active_model_id, model_path, context_length, runtime_cfg) = {
        let guard = state_arc.lock().map_err(|e| e.to_string())?;
        (
            guard.active_model_id.clone(),
            guard.model_path.clone(),
            guard.context_length,
            guard.llama_runtime.clone(),
        )
    };

    let model_id = active_model_id.ok_or_else(|| "No active model loaded".to_string())?;
    let model_path = model_path.ok_or_else(|| "Active model path is missing".to_string())?;

    let scheduler = app.state::<VramScheduler>().inner().clone();
    let manager = engine::default_session_manager(app.clone(), llama_state);
    let source = source_from_state(model_id.clone(), model_path, context_length);
    let acquired = scheduler
        .acquire(
            EngineSessionKind::Chat,
            source,
            runtime_cfg.clone(),
            RequestPriority::High,
        )
        .await
        .map_err(|e| e.to_string())?;
    if acquired.waited_ms > 1_000 {
        let _ = app.emit(
            "scheduler_queue_wait",
            serde_json::json!({
                "waited_ms": acquired.waited_ms,
                "queue_position": acquired.queue_position,
            }),
        );
    }
    let chat_session = acquired.lease.session().clone();

    if let Ok(mut guard) = state_arc.lock() {
        guard.active_backend = ActiveBackend::Llamacpp;
        guard.active_model_id = Some(model_id);
        guard.active_llama_session = Some(to_snapshot(&chat_session));
    }

    let msgs = preflight_chat_messages(&req)?;
    req.messages = Some(msgs);
    if let Err(err) =
        crate::retrieval::orchestrator::apply_retrieval(&app, &state_arc, &mut req).await
    {
        let _ = app.emit(
            "retrieval_warning",
            crate::retrieval::types::RetrievalWarningEvent {
                message: format!("Retrieval pipeline failed: {err}"),
            },
        );
    }
    let mcp_requested = req.mcp.as_ref().map(|m| m.enabled).unwrap_or(false);
    if mcp_requested
        && let (Some(settings_state), Some(mcp_state)) = (
            app.try_state::<SettingsV2State>(),
            app.try_state::<crate::mcp::McpRuntimeState>(),
        )
    {
        let mcp_settings = {
            let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
            guard.get_ref().web_rag.mcp.clone()
        };
        if mcp_settings.enabled {
            match crate::mcp::runtime::list_tools(mcp_state.inner()).await {
                Ok(tools) if tools.is_empty() => {
                    let warning =
                        "MCP requested, but no active MCP tools are available for this request.";
                    let _ = app.emit(
                        "tooling_log",
                        serde_json::json!({
                            "category": "MCP_DEBUG",
                            "message": "MCP requested but no active tools are available; using standard streaming",
                            "details": {},
                        }),
                    );
                    let _ = app.emit(
                        "retrieval_warning",
                        crate::retrieval::types::RetrievalWarningEvent {
                            message: warning.to_string(),
                        },
                    );
                }
                Ok(_) => {
                    match crate::mcp::agent::run_agent_loop(
                        &app,
                        &chat_session,
                        req.clone(),
                        mcp_state.inner(),
                        &mcp_settings,
                    )
                    .await
                    {
                        Ok(()) => {
                            drop(acquired.lease);
                            return Ok(());
                        }
                        Err(err) => {
                            let _ = app.emit(
                                "tooling_log",
                                serde_json::json!({
                                    "category": "MCP_DEBUG",
                                    "message": "MCP agent loop failed; falling back to standard streaming",
                                    "details": { "error": err },
                                }),
                            );
                        }
                    }
                }
                Err(err) => {
                    let warning =
                        format!("MCP tool discovery failed; using standard streaming: {err}");
                    let _ = app.emit(
                        "tooling_log",
                        serde_json::json!({
                            "category": "MCP_DEBUG",
                            "message": "MCP tool discovery failed; using standard streaming",
                            "details": { "error": err },
                        }),
                    );
                    let _ = app.emit(
                        "retrieval_warning",
                        crate::retrieval::types::RetrievalWarningEvent { message: warning },
                    );
                }
            }
        } else {
            let _ = app.emit(
                "tooling_log",
                serde_json::json!({
                    "category": "MCP_DEBUG",
                    "message": "MCP was requested but disabled in settings",
                    "details": {},
                }),
            );
        }
    }
    let result = manager.chat_stream(&app, &chat_session, req).await;
    drop(acquired.lease);
    result
}

pub async fn unload_model(
    app: tauri::AppHandle,
    state_arc: SharedState,
    llama_state: LlamaCppState,
) -> Result<(), String> {
    emit_load_progress(&app, "unload_start", 0, None, false, None);

    let scheduler = app.state::<VramScheduler>().inner().clone();
    let active_model_id = {
        let guard = state_arc.lock().map_err(|e| e.to_string())?;
        guard.active_model_id.clone()
    };

    let _ = llama_state;
    if let Some(model_id) = active_model_id.as_deref() {
        scheduler.force_unload_model(model_id).await?;
    } else {
        scheduler.force_unload_all().await?;
    }

    let mut guard = state_arc.lock().map_err(|e| e.to_string())?;
    guard.context_length = 4096;
    guard.model_path = None;
    guard.hub_repo_id = None;
    guard.hub_revision = None;
    guard.chat_template = None;
    guard.active_backend = ActiveBackend::None;
    guard.active_model_id = None;
    guard.active_llama_session = None;

    emit_load_progress(&app, "unload_complete", 100, Some("complete"), true, None);
    Ok(())
}
