use crate::core::settings_v2::EmbeddingsProviderSettings;
use serde_json::json;
use std::time::Duration;

pub async fn test_provider(cfg: &EmbeddingsProviderSettings) -> Result<(), String> {
    if !cfg.is_configured() {
        return Err("Embeddings provider is not configured".to_string());
    }
    let _ = create_embeddings(cfg, &["health check".to_string()]).await?;
    Ok(())
}

pub async fn create_embeddings(
    cfg: &EmbeddingsProviderSettings,
    inputs: &[String],
) -> Result<Vec<Vec<f32>>, String> {
    if inputs.is_empty() {
        return Ok(Vec::new());
    }
    let base = cfg.base_url.trim().trim_end_matches('/');
    let url = format!("{base}/embeddings");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(cfg.timeout_ms))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = client.post(url).json(&json!({
        "model": cfg.model,
        "input": inputs,
        "encoding_format": "float"
    }));
    if let Some(key) = cfg.api_key.as_deref()
        && !key.trim().is_empty()
    {
        req = req.bearer_auth(key);
    }

    let response = req.send().await.map_err(|e| e.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("embeddings request failed ({status}): {body}"));
    }

    let payload = response
        .json::<serde_json::Value>()
        .await
        .map_err(|e| e.to_string())?;
    let data = payload
        .get("data")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| "invalid embeddings payload: missing data".to_string())?;

    let mut out = Vec::with_capacity(data.len());
    for row in data {
        let emb = row
            .get("embedding")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| "invalid embeddings payload: missing embedding array".to_string())?;
        let mut vec = Vec::with_capacity(emb.len());
        for item in emb {
            let Some(v) = item.as_f64() else {
                return Err("invalid embeddings payload: non-numeric value".to_string());
            };
            vec.push(v as f32);
        }
        out.push(vec);
    }
    Ok(out)
}
