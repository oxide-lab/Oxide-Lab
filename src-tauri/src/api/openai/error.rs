use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    pub message: String,
    #[serde(rename = "type")]
    pub error_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorResponse {
    pub error: ApiError,
}

impl ApiError {
    pub fn new(
        message: impl Into<String>,
        error_type: impl Into<String>,
        param: Option<String>,
        code: Option<String>,
    ) -> Self {
        Self {
            message: message.into(),
            error_type: error_type.into(),
            param,
            code,
        }
    }

    pub fn invalid_request(message: impl Into<String>, param: impl Into<String>) -> Self {
        Self::new(
            message,
            "invalid_request_error",
            Some(param.into()),
            Some("invalid_request".to_string()),
        )
    }

    pub fn upstream(message: impl Into<String>) -> Self {
        Self::new(
            message,
            "upstream_error",
            None,
            Some("upstream_error".to_string()),
        )
    }

    pub fn not_implemented(message: impl Into<String>, param: impl Into<String>) -> Self {
        Self::new(
            message,
            "not_implemented",
            Some(param.into()),
            Some("not_implemented".to_string()),
        )
    }
}

pub fn error_response(status: StatusCode, error: ApiError) -> Response {
    (status, Json(ErrorResponse { error })).into_response()
}
