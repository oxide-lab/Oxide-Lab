use super::error::ApiError;
use crate::inference::engine::EngineSessionInfo;
use reqwest::Response;
use serde_json::Value;
use std::time::Duration;

fn client(timeout_secs: u64) -> Result<reqwest::Client, ApiError> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()
        .map_err(|e| {
            ApiError::new(
                e.to_string(),
                "server_error",
                None,
                Some("client".to_string()),
            )
        })
}

pub async fn post_json(
    session: &EngineSessionInfo,
    path: &str,
    payload: &Value,
) -> Result<Value, ApiError> {
    let client = client(300)?;
    let upstream_url = format!("http://127.0.0.1:{}{}", session.port, path);
    let response = client
        .post(upstream_url)
        .bearer_auth(&session.api_key)
        .json(payload)
        .send()
        .await
        .map_err(|e| ApiError::upstream(e.to_string()))?;

    parse_json_response(response).await
}

pub async fn post_stream(
    session: &EngineSessionInfo,
    path: &str,
    payload: &Value,
) -> Result<Response, ApiError> {
    let client = client(300)?;
    let upstream_url = format!("http://127.0.0.1:{}{}", session.port, path);
    let response = client
        .post(upstream_url)
        .bearer_auth(&session.api_key)
        .json(payload)
        .send()
        .await
        .map_err(|e| ApiError::upstream(e.to_string()))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::new(
            format!("upstream error {}: {}", status, body),
            "upstream_error",
            None,
            Some("upstream_status".to_string()),
        ));
    }

    Ok(response)
}

async fn parse_json_response(response: Response) -> Result<Value, ApiError> {
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ApiError::new(
            format!("upstream error {}: {}", status, body),
            "upstream_error",
            None,
            Some("upstream_status".to_string()),
        ));
    }

    response
        .json::<Value>()
        .await
        .map_err(|e| ApiError::upstream(format!("invalid upstream json: {}", e)))
}
