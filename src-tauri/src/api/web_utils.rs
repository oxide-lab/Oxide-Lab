use serde::Serialize;
use reqwest::Client;
use std::time::Duration;

#[derive(Debug, Serialize)]
pub struct FetchResult {
    pub content: Vec<u8>,
    pub mime_type: String,
    pub filename: String,
}

#[tauri::command]
pub async fn fetch_url(url: String) -> Result<FetchResult, String> {
    if !url.starts_with("https://") {
        return Err("Only HTTPS URLs are allowed for security reasons.".to_string());
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let response = client.get(&url)
        .send()
        .await
        .map_err(|e| format!("Failed to fetch URL: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Server returned error: {}", response.status()));
    }

    // Limit to 10MB
    let content_length = response.content_length().unwrap_or(0);
    if content_length > 10 * 1024 * 1024 {
        return Err("File is too large (max 10MB).".to_string());
    }

    let mime_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("application/octet-stream")
        .to_string();

    let filename = url
        .split('/')
        .next_back()
        .unwrap_or("attachment")
        .split('?')
        .next()
        .unwrap_or("attachment")
        .to_string();

    let content = response
        .bytes()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?
        .to_vec();

    if content.len() > 10 * 1024 * 1024 {
        return Err("File is too large (max 10MB).".to_string());
    }

    Ok(FetchResult {
        content,
        mime_type,
        filename,
    })
}
