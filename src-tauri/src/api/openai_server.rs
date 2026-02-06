//! OpenAI-compatible HTTP API server.
//! Proxy-only implementation for external process-host engines.

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{StatusCode, header},
    response::Response,
    routing::{get, post},
};
use futures_util::StreamExt;
use serde::Serialize;
use std::{
    net::SocketAddr,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::broadcast;
use tower_http::cors::{Any, CorsLayer};

use crate::core::state::SharedState;
use crate::core::types::ActiveBackend;
use crate::inference::engine::{self, EngineSessionKind, ResolvedModelSource};
use crate::inference::llamacpp::state::LlamaCppState;

pub const OPENAI_PORT: u16 = 11434;

#[derive(Serialize)]
pub struct ServerConfig {
    pub port: u16,
    pub running: bool,
}

#[tauri::command]
pub fn get_server_config() -> ServerConfig {
    ServerConfig {
        port: OPENAI_PORT,
        running: true,
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelList {
    pub object: String,
    pub data: Vec<Model>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Model {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub owned_by: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    pub code: Option<String>,
}

pub struct OpenAIServerState {
    pub app_handle: tauri::AppHandle,
    pub model_state: SharedState,
    pub llama_state: LlamaCppState,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn server_error(msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: ApiError {
                message: msg.into(),
                error_type: "server_error".into(),
                code: None,
            },
        }),
    )
}

fn bad_request(msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: ApiError {
                message: msg.into(),
                error_type: "invalid_request_error".into(),
                code: None,
            },
        }),
    )
}

fn active_source(state: &Arc<OpenAIServerState>) -> Result<ResolvedModelSource, String> {
    let guard = state.model_state.lock().map_err(|e| e.to_string())?;
    if guard.active_backend != ActiveBackend::Llamacpp {
        return Err("No active llama backend".to_string());
    }

    let model_id = guard
        .active_model_id
        .clone()
        .ok_or_else(|| "No active model id".to_string())?;
    let model_path = guard
        .model_path
        .clone()
        .ok_or_else(|| "No active model path".to_string())?;

    Ok(ResolvedModelSource {
        model_id,
        model_path,
        context_length: guard.context_length,
    })
}

async fn ensure_session(
    state: &Arc<OpenAIServerState>,
    kind: EngineSessionKind,
) -> Result<crate::inference::engine::EngineSessionInfo, String> {
    let source = active_source(state)?;
    let runtime_cfg = {
        let guard = state.model_state.lock().map_err(|e| e.to_string())?;
        guard.llama_runtime.clone()
    };

    let manager =
        engine::default_session_manager(state.app_handle.clone(), state.llama_state.clone());
    let session = manager.start_session(kind, &source, &runtime_cfg).await?;
    manager.ensure_health(session, &runtime_cfg).await
}

async fn proxy_to_llama(
    session: &crate::inference::engine::EngineSessionInfo,
    path: &str,
    payload: serde_json::Value,
    stream: bool,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| server_error(&e.to_string()))?;

    let upstream_url = format!("http://127.0.0.1:{}{}", session.port, path);
    let resp = client
        .post(upstream_url)
        .bearer_auth(&session.api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| server_error(&e.to_string()))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err((
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(ErrorResponse {
                error: ApiError {
                    message: format!("Upstream llama-server error: {}", text),
                    error_type: "upstream_error".to_string(),
                    code: None,
                },
            }),
        ));
    }

    if stream {
        let mapped = resp.bytes_stream().map(|chunk| {
            chunk.map_err(|e| std::io::Error::other(format!("SSE proxy chunk error: {}", e)))
        });
        let mut out = Response::new(Body::from_stream(mapped));
        *out.status_mut() = StatusCode::OK;
        out.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("text/event-stream"),
        );
        out.headers_mut().insert(
            header::CACHE_CONTROL,
            header::HeaderValue::from_static("no-cache"),
        );
        out.headers_mut().insert(
            header::CONNECTION,
            header::HeaderValue::from_static("keep-alive"),
        );
        return Ok(out);
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| server_error(&e.to_string()))?;
    let mut out = Response::new(Body::from(bytes));
    *out.status_mut() = StatusCode::OK;
    out.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    Ok(out)
}

async fn models_handler(
    State(state): State<Arc<OpenAIServerState>>,
) -> Result<Json<ModelList>, (StatusCode, Json<ErrorResponse>)> {
    let guard = state
        .model_state
        .lock()
        .map_err(|_| server_error("Lock failed"))?;

    let mut models = Vec::new();
    if guard.active_backend == ActiveBackend::Llamacpp
        && let Some(model_id) = guard.active_model_id.clone()
    {
        models.push(Model {
            id: model_id,
            object: "model".to_string(),
            created: now_unix(),
            owned_by: "oxide-lab".to_string(),
        });
    }

    models.push(Model {
        id: "local-model".to_string(),
        object: "model".to_string(),
        created: now_unix(),
        owned_by: "oxide-lab".to_string(),
    });

    Ok(Json(ModelList {
        object: "list".to_string(),
        data: models,
    }))
}

async fn chat_completions_handler(
    State(state): State<Arc<OpenAIServerState>>,
    Json(req): Json<serde_json::Value>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if active_source(&state).is_err() {
        return Err(bad_request("No model loaded"));
    }

    let stream = req.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let session = ensure_session(&state, EngineSessionKind::Chat)
        .await
        .map_err(|e| server_error(&e))?;

    proxy_to_llama(&session, "/v1/chat/completions", req, stream).await
}

async fn completions_handler(
    State(state): State<Arc<OpenAIServerState>>,
    Json(req): Json<serde_json::Value>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if active_source(&state).is_err() {
        return Err(bad_request("No model loaded"));
    }

    let stream = req.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);
    let session = ensure_session(&state, EngineSessionKind::Chat)
        .await
        .map_err(|e| server_error(&e))?;

    proxy_to_llama(&session, "/v1/completions", req, stream).await
}

async fn embeddings_handler(
    State(state): State<Arc<OpenAIServerState>>,
    Json(req): Json<serde_json::Value>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if active_source(&state).is_err() {
        return Err(bad_request("No model loaded"));
    }

    let session = ensure_session(&state, EngineSessionKind::Embedding)
        .await
        .map_err(|e| server_error(&e))?;

    proxy_to_llama(&session, "/v1/embeddings", req, false).await
}

pub fn create_router(state: Arc<OpenAIServerState>) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/v1/models", get(models_handler))
        .route("/v1/chat/completions", post(chat_completions_handler))
        .route("/v1/completions", post(completions_handler))
        .route("/v1/embeddings", post(embeddings_handler))
        .layer(cors)
        .with_state(state)
}

pub async fn start_server(
    app_handle: tauri::AppHandle,
    model_state: SharedState,
    llama_state: LlamaCppState,
    port: u16,
) -> Result<broadcast::Sender<()>, std::io::Error> {
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    let state = Arc::new(OpenAIServerState {
        app_handle,
        model_state,
        llama_state,
    });

    let app = create_router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], port));

    log::info!("OpenAI API server starting on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;

    let shutdown_rx = shutdown_tx.subscribe();

    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let mut rx = shutdown_rx;
                let _ = rx.recv().await;
                log::info!("OpenAI API server shutting down");
            })
            .await
            .ok();
    });

    Ok(shutdown_tx)
}

