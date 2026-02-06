use crate::core::types::{ChatMessage, GenerateRequest, StreamMessage, ToolChoice};
use crate::generate::cancel::CANCEL_GENERATION;
use crate::inference::engine::EngineSessionInfo;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<OpenAIMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    min_p: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    repeat_last_n: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<crate::generate::tool_call_parser::Tool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChunk {
    choices: Vec<ChatChunkChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatChunkChoice {
    #[serde(default)]
    delta: Option<ChatDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatDelta {
    #[serde(default)]
    content: Option<String>,
}

fn to_openai_messages(req: &GenerateRequest) -> Vec<OpenAIMessage> {
    if let Some(messages) = req.messages.as_ref() {
        return messages
            .iter()
            .map(|m| OpenAIMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();
    }

    vec![OpenAIMessage {
        role: "user".to_string(),
        content: req.prompt.clone(),
    }]
}

pub async fn health_check(session: &EngineSessionInfo) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(600))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };

    let url = format!("http://127.0.0.1:{}/health", session.port);
    match client.get(url).bearer_auth(&session.api_key).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

pub async fn stream_chat_completion(
    app: &tauri::AppHandle,
    session: &EngineSessionInfo,
    req: GenerateRequest,
) -> Result<(), String> {
    CANCEL_GENERATION.store(false, Ordering::SeqCst);

    let payload = ChatCompletionRequest {
        model: session.model_id.clone(),
        messages: to_openai_messages(&req),
        stream: true,
        max_tokens: req.max_new_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        top_k: req.top_k,
        min_p: req.min_p,
        repeat_penalty: req.repeat_penalty,
        repeat_last_n: if req.repeat_last_n > 0 {
            Some(req.repeat_last_n)
        } else {
            None
        },
        tools: req.tools.clone(),
        stop: req.stop_sequences.clone(),
        tool_choice: req.tool_choice.clone(),
    };

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("http://127.0.0.1:{}/v1/chat/completions", session.port);
    let response = client
        .post(url)
        .bearer_auth(&session.api_key)
        .json(&payload)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "llama-server chat request failed ({status}): {body}"
        ));
    }

    let mut events = response.bytes_stream().eventsource();
    let _ = app.emit("message_start", ());

    while let Some(event) = events.next().await {
        if CANCEL_GENERATION.load(Ordering::SeqCst) {
            break;
        }
        let event = event.map_err(|e| format!("SSE parse error: {}", e))?;
        if event.data.trim() == "[DONE]" {
            break;
        }
        if event.data.trim().is_empty() {
            continue;
        }

        let parsed: ChatChunk = match serde_json::from_str(&event.data) {
            Ok(v) => v,
            Err(_) => {
                continue;
            }
        };

        for choice in parsed.choices {
            if let Some(delta) = choice.delta
                && let Some(content) = delta.content
                && !content.is_empty()
            {
                let _ = app.emit(
                    "message",
                    StreamMessage {
                        thinking: String::new(),
                        content,
                    },
                );
            }
            if choice.finish_reason.is_some() {
                break;
            }
        }
    }

    let _ = app.emit("message_done", ());
    Ok(())
}

pub async fn create_embeddings(
    session: &EngineSessionInfo,
    model: &str,
    input: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let url = format!("http://127.0.0.1:{}/v1/embeddings", session.port);
    let body = json!({
        "model": model,
        "input": input,
        "encoding_format": "float"
    });

    let response = client
        .post(url)
        .bearer_auth(&session.api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "llama-server embeddings request failed ({status}): {body}"
        ));
    }

    response.json().await.map_err(|e| e.to_string())
}

pub fn preflight_chat_messages(req: &GenerateRequest) -> Result<Vec<ChatMessage>, String> {
    if let Some(messages) = req.messages.clone() {
        return Ok(messages);
    }
    if req.prompt.trim().is_empty() {
        return Err("Neither messages nor prompt provided".to_string());
    }
    Ok(vec![ChatMessage {
        role: "user".to_string(),
        content: req.prompt.clone(),
    }])
}

