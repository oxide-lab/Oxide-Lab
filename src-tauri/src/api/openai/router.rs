use super::OpenAIServerState;
use super::error::{ApiError, error_response};
use super::handlers;
use crate::core::settings_v2::{CorsMode, OpenAiServerConfig, verify_api_key};
use axum::extract::State;
use axum::http::HeaderValue;
use axum::http::header;
use axum::http::{Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::{
    Router,
    routing::{get, post},
};
use std::sync::Arc;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};

async fn auth_middleware(
    State(state): State<Arc<OpenAIServerState>>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    if !state.config.auth_required {
        return next.run(req).await;
    }

    let token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|v| !v.is_empty());

    match token {
        Some(token) if verify_api_key(token, &state.config.api_keys_hashed) => next.run(req).await,
        _ => error_response(
            StatusCode::UNAUTHORIZED,
            ApiError::new(
                "Unauthorized request",
                "authentication_error",
                None,
                Some("invalid_api_key".to_string()),
            ),
        ),
    }
}

fn build_cors(config: &OpenAiServerConfig) -> Result<Option<CorsLayer>, String> {
    let methods = [Method::GET, Method::POST, Method::OPTIONS];
    let headers = [header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT];

    match config.cors_mode {
        CorsMode::SameOrigin => Ok(None),
        CorsMode::Any => Ok(Some(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(methods)
                .allow_headers(headers),
        )),
        CorsMode::Allowlist => {
            let mut origins = Vec::new();
            for value in &config.cors_allowlist {
                let origin = HeaderValue::from_str(value)
                    .map_err(|e| format!("Invalid CORS allowlist origin '{value}': {e}"))?;
                origins.push(origin);
            }
            if origins.is_empty() {
                return Ok(None);
            }
            Ok(Some(
                CorsLayer::new()
                    .allow_origin(AllowOrigin::list(origins))
                    .allow_methods(methods)
                    .allow_headers(headers),
            ))
        }
    }
}

pub fn create_router(
    state: Arc<OpenAIServerState>,
    config: &OpenAiServerConfig,
) -> Result<Router, String> {
    let mut router = Router::new()
        .route("/v1/models", get(handlers::models_handler))
        .route("/v1/models/{model}", get(handlers::model_by_id_handler))
        .route(
            "/v1/chat/completions",
            post(handlers::chat_completions_handler),
        )
        .route("/v1/completions", post(handlers::completions_handler))
        .route("/v1/embeddings", post(handlers::embeddings_handler))
        .route("/v1/responses", post(handlers::responses_handler))
        .route(
            "/v1/images/generations",
            post(handlers::image_generations_handler),
        )
        .route("/v1/images/edits", post(handlers::image_edits_handler))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state);

    if let Some(cors) = build_cors(config)? {
        router = router.layer(cors);
    }

    Ok(router)
}
