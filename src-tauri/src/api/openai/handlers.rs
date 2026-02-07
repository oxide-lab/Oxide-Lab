use super::OpenAIServerState;
use super::error::{ApiError, error_response};
use super::responses::{ResponsesRequest, ResponsesStreamConverter, to_non_stream_response};
use super::types::{
    ChatCompletionRequest, CompletionRequest, EmbeddingsRequest, Model, ModelList, now_unix,
};
use super::upstream;
use async_stream::stream;
use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::Response;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde_json::Value;
use std::sync::Arc;
use tauri::Manager;

use crate::core::types::ActiveBackend;
use crate::inference::engine::{EngineSessionKind, ResolvedModelSource};
use crate::inference::scheduler::{AcquireError, AcquireResult, RequestPriority, VramScheduler};

#[allow(clippy::result_large_err)]
fn active_source(state: &Arc<OpenAIServerState>) -> Result<ResolvedModelSource, Response> {
    let guard = state.model_state.lock().map_err(|_| {
        error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::new(
                "Lock failed",
                "server_error",
                None,
                Some("lock".to_string()),
            ),
        )
    })?;
    if guard.active_backend != ActiveBackend::Llamacpp {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            ApiError::new(
                "No model loaded",
                "invalid_request_error",
                Some("model".to_string()),
                Some("no_model".to_string()),
            ),
        ));
    }

    let model_id = guard.active_model_id.clone().ok_or_else(|| {
        error_response(
            StatusCode::BAD_REQUEST,
            ApiError::new(
                "No active model id",
                "invalid_request_error",
                Some("model".to_string()),
                Some("no_model_id".to_string()),
            ),
        )
    })?;
    let model_path = guard.model_path.clone().ok_or_else(|| {
        error_response(
            StatusCode::BAD_REQUEST,
            ApiError::new(
                "No active model path",
                "invalid_request_error",
                Some("model".to_string()),
                Some("no_model_path".to_string()),
            ),
        )
    })?;

    Ok(ResolvedModelSource {
        model_id,
        model_path,
        context_length: guard.context_length,
    })
}

#[allow(clippy::result_large_err)]
fn scheduler_from_state(state: &Arc<OpenAIServerState>) -> Result<VramScheduler, Response> {
    state
        .app_handle
        .try_state::<VramScheduler>()
        .map(|s| s.inner().clone())
        .ok_or_else(|| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiError::new(
                    "Scheduler state is not initialized",
                    "server_error",
                    None,
                    Some("scheduler_missing".to_string()),
                ),
            )
        })
}

fn map_acquire_error(err: AcquireError) -> Response {
    match err {
        AcquireError::Busy => {
            let mut resp = error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiError::new(
                    "server busy, please retry",
                    "server_busy",
                    None,
                    Some("queue_full".to_string()),
                ),
            );
            resp.headers_mut()
                .insert("Retry-After", HeaderValue::from_static("2"));
            resp
        }
        AcquireError::Timeout { queue_position } => {
            let mut resp = error_response(
                StatusCode::GATEWAY_TIMEOUT,
                ApiError::new(
                    "queue wait timeout",
                    "timeout_error",
                    None,
                    Some("queue_timeout".to_string()),
                ),
            );
            resp.headers_mut()
                .insert("Retry-After", HeaderValue::from_static("1"));
            if let Ok(v) = HeaderValue::from_str(&queue_position.to_string()) {
                resp.headers_mut().insert("X-Queue-Position", v);
            }
            resp
        }
        AcquireError::Shutdown => error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiError::new(
                "scheduler is shutting down",
                "server_shutdown",
                None,
                Some("shutdown".to_string()),
            ),
        ),
        AcquireError::Internal(msg) => error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::new(msg, "server_error", None, Some("internal".to_string())),
        ),
    }
}

async fn acquire_lease(
    state: &Arc<OpenAIServerState>,
    kind: EngineSessionKind,
    priority: RequestPriority,
) -> Result<AcquireResult, Response> {
    let source = active_source(state)?;
    let runtime_cfg = {
        let guard = state.model_state.lock().map_err(|_| {
            error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiError::new(
                    "Lock failed",
                    "server_error",
                    None,
                    Some("lock".to_string()),
                ),
            )
        })?;
        guard.llama_runtime.clone()
    };

    let scheduler = scheduler_from_state(state)?;
    scheduler
        .acquire(kind, source, runtime_cfg, priority)
        .await
        .map_err(map_acquire_error)
}

fn apply_queue_headers(resp: &mut Response, waited_ms: u64, queue_position: Option<usize>) {
    if waited_ms > 1_000 {
        if let Some(pos) = queue_position
            && let Ok(v) = HeaderValue::from_str(&pos.to_string())
        {
            resp.headers_mut().insert("X-Queue-Position", v);
        }
        if let Ok(v) = HeaderValue::from_str(&waited_ms.to_string()) {
            resp.headers_mut().insert("X-Queue-Wait-Ms", v);
        }
    }
}

fn build_models(state: &Arc<OpenAIServerState>) -> Vec<Model> {
    let mut ids = std::collections::BTreeSet::<String>::new();
    if let Some(scheduler) = state.app_handle.try_state::<VramScheduler>() {
        for id in scheduler.snapshot().loaded_models {
            ids.insert(id);
        }
    }
    if let Ok(guard) = state.model_state.lock()
        && let Some(id) = guard.active_model_id.clone()
    {
        ids.insert(id);
    }
    if ids.is_empty() {
        ids.insert("local-model".to_string());
    }
    ids.into_iter()
        .map(|id| Model {
            id,
            object: "model".to_string(),
            created: now_unix(),
            owned_by: "oxide-lab".to_string(),
        })
        .collect()
}

pub async fn models_handler(
    State(state): State<Arc<OpenAIServerState>>,
) -> Result<Json<ModelList>, Response> {
    Ok(Json(ModelList {
        object: "list".to_string(),
        data: build_models(&state),
    }))
}

pub async fn model_by_id_handler(
    State(state): State<Arc<OpenAIServerState>>,
    Path(model): Path<String>,
) -> Result<Json<Model>, Response> {
    let models = build_models(&state);
    if let Some(m) = models.into_iter().find(|m| m.id == model) {
        return Ok(Json(m));
    }
    Err(error_response(
        StatusCode::NOT_FOUND,
        ApiError::new(
            format!("model '{}' not found", model),
            "not_found_error",
            Some("model".to_string()),
            Some("model_not_found".to_string()),
        ),
    ))
}

pub async fn chat_completions_handler(
    State(state): State<Arc<OpenAIServerState>>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Response, Response> {
    req.validate()
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;
    super::types::log_unknown_fields("chat.completions", &req.extra);

    let acquired = acquire_lease(&state, EngineSessionKind::Chat, RequestPriority::High).await?;
    let waited_ms = acquired.waited_ms;
    let queue_position = acquired.queue_position;
    let session = acquired.lease.session().clone();
    let payload = req.to_upstream_payload(None);

    if req.stream {
        let resp = upstream::post_stream(&session, "/v1/chat/completions", &payload)
            .await
            .map_err(|e| error_response(StatusCode::BAD_GATEWAY, e))?;
        let lease = acquired.lease;
        let mapped = resp.bytes_stream().map(move |chunk| {
            let _keep = &lease;
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
        apply_queue_headers(&mut out, waited_ms, queue_position);
        return Ok(out);
    }

    let json = upstream::post_json(&session, "/v1/chat/completions", &payload)
        .await
        .map_err(|e| error_response(StatusCode::BAD_GATEWAY, e))?;
    drop(acquired.lease);

    let mut out = Response::new(Body::from(json.to_string()));
    *out.status_mut() = StatusCode::OK;
    out.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    apply_queue_headers(&mut out, waited_ms, queue_position);
    Ok(out)
}

pub async fn completions_handler(
    State(state): State<Arc<OpenAIServerState>>,
    Json(req): Json<CompletionRequest>,
) -> Result<Response, Response> {
    req.validate()
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;
    super::types::log_unknown_fields("completions", &req.extra);

    let acquired = acquire_lease(&state, EngineSessionKind::Chat, RequestPriority::High).await?;
    let waited_ms = acquired.waited_ms;
    let queue_position = acquired.queue_position;
    let session = acquired.lease.session().clone();
    let payload = req.to_upstream_payload(None);

    if req.stream {
        let resp = upstream::post_stream(&session, "/v1/completions", &payload)
            .await
            .map_err(|e| error_response(StatusCode::BAD_GATEWAY, e))?;
        let lease = acquired.lease;
        let mapped = resp.bytes_stream().map(move |chunk| {
            let _keep = &lease;
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
        apply_queue_headers(&mut out, waited_ms, queue_position);
        return Ok(out);
    }

    let json = upstream::post_json(&session, "/v1/completions", &payload)
        .await
        .map_err(|e| error_response(StatusCode::BAD_GATEWAY, e))?;
    drop(acquired.lease);

    let mut out = Response::new(Body::from(json.to_string()));
    *out.status_mut() = StatusCode::OK;
    out.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    apply_queue_headers(&mut out, waited_ms, queue_position);
    Ok(out)
}

pub async fn embeddings_handler(
    State(state): State<Arc<OpenAIServerState>>,
    Json(req): Json<EmbeddingsRequest>,
) -> Result<Response, Response> {
    req.validate()
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;
    super::types::log_unknown_fields("embeddings", &req.extra);

    let acquired = acquire_lease(
        &state,
        EngineSessionKind::Embedding,
        RequestPriority::Normal,
    )
    .await?;
    let waited_ms = acquired.waited_ms;
    let queue_position = acquired.queue_position;
    let session = acquired.lease.session().clone();

    let mut json = upstream::post_json(&session, "/v1/embeddings", &req.to_upstream_payload())
        .await
        .map_err(|e| error_response(StatusCode::BAD_GATEWAY, e))?;
    if req.wants_base64() {
        super::types::embed_float_to_base64(&mut json)
            .map_err(|e| error_response(StatusCode::BAD_GATEWAY, e))?;
    }
    drop(acquired.lease);

    let mut out = Response::new(Body::from(json.to_string()));
    *out.status_mut() = StatusCode::OK;
    out.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("application/json"),
    );
    apply_queue_headers(&mut out, waited_ms, queue_position);
    Ok(out)
}

pub async fn responses_handler(
    State(state): State<Arc<OpenAIServerState>>,
    Json(req): Json<ResponsesRequest>,
) -> Result<Response, Response> {
    req.validate()
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;
    super::types::log_unknown_fields("responses", &req.extra);

    let is_stream = req.stream_enabled();
    let acquired = acquire_lease(&state, EngineSessionKind::Chat, RequestPriority::High).await?;
    let waited_ms = acquired.waited_ms;
    let queue_position = acquired.queue_position;
    let session = acquired.lease.session().clone();

    let response_id = format!("resp_{}", rand::random::<u32>());
    let item_id = format!("msg_{}", rand::random::<u32>());
    let payload = req
        .to_chat_payload(is_stream)
        .map_err(|e| error_response(StatusCode::BAD_REQUEST, e))?;

    if !is_stream {
        let chat_json = upstream::post_json(&session, "/v1/chat/completions", &payload)
            .await
            .map_err(|e| error_response(StatusCode::BAD_GATEWAY, e))?;
        let out_json = to_non_stream_response(&req, &response_id, &item_id, &chat_json);
        drop(acquired.lease);
        let mut out = Response::new(Body::from(out_json.to_string()));
        *out.status_mut() = StatusCode::OK;
        out.headers_mut().insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );
        apply_queue_headers(&mut out, waited_ms, queue_position);
        return Ok(out);
    }

    let upstream_stream = upstream::post_stream(&session, "/v1/chat/completions", &payload)
        .await
        .map_err(|e| error_response(StatusCode::BAD_GATEWAY, e))?;

    let lease = acquired.lease;
    let body_stream = stream! {
        let mut converter = ResponsesStreamConverter::new(response_id, item_id, req.model.clone());
        for event in converter.start_events() {
            match serde_json::to_string(&event.data) {
                Ok(payload) => {
                    let line = format!("event: {}\ndata: {}\n\n", event.event, payload);
                    yield Ok::<bytes::Bytes, std::io::Error>(bytes::Bytes::from(line));
                }
                Err(e) => {
                    yield Err(std::io::Error::other(e.to_string()));
                    break;
                }
            }
        }

        let mut events = upstream_stream.bytes_stream().eventsource();
        while let Some(item) = events.next().await {
            let _keep = &lease;
            let sse = match item {
                Ok(v) => v,
                Err(e) => {
                    let failed = converter.failed_event(&format!("upstream SSE parse error: {}", e));
                    let payload = serde_json::to_string(&failed.data)
                        .unwrap_or_else(|_| "{\"error\":\"stream_failure\"}".to_string());
                    let line = format!("event: {}\ndata: {}\n\n", failed.event, payload);
                    yield Ok(bytes::Bytes::from(line));
                    break;
                }
            };

            let data = sse.data.trim();
            if data.is_empty() {
                continue;
            }
            if data == "[DONE]" {
                for event in converter.finish_events() {
                    let payload = serde_json::to_string(&event.data)
                        .unwrap_or_else(|_| "{\"error\":\"stream_failure\"}".to_string());
                    let line = format!("event: {}\ndata: {}\n\n", event.event, payload);
                    yield Ok(bytes::Bytes::from(line));
                }
                break;
            }

            let parsed: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(e) => {
                    let failed = converter.failed_event(&format!("invalid upstream JSON chunk: {}", e));
                    let payload = serde_json::to_string(&failed.data)
                        .unwrap_or_else(|_| "{\"error\":\"stream_failure\"}".to_string());
                    let line = format!("event: {}\ndata: {}\n\n", failed.event, payload);
                    yield Ok(bytes::Bytes::from(line));
                    break;
                }
            };

            for event in converter.process_chat_chunk(&parsed) {
                let payload = serde_json::to_string(&event.data)
                    .unwrap_or_else(|_| "{\"error\":\"stream_failure\"}".to_string());
                let line = format!("event: {}\ndata: {}\n\n", event.event, payload);
                yield Ok(bytes::Bytes::from(line));
            }
        }
    };

    let mut out = Response::new(Body::from_stream(body_stream));
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
    apply_queue_headers(&mut out, waited_ms, queue_position);
    Ok(out)
}

pub async fn image_generations_handler() -> Result<Response, Response> {
    Err(error_response(
        StatusCode::NOT_IMPLEMENTED,
        ApiError::not_implemented(
            "/v1/images/generations is not supported in current backend",
            "images",
        ),
    ))
}

pub async fn image_edits_handler() -> Result<Response, Response> {
    Err(error_response(
        StatusCode::NOT_IMPLEMENTED,
        ApiError::not_implemented(
            "/v1/images/edits is not supported in current backend",
            "images",
        ),
    ))
}
