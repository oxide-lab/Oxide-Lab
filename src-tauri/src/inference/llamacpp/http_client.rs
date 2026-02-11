use crate::core::attachments_text::{gather_text_from_attachments, read_attachment_bytes};
use crate::core::limits::MAX_ATTACHMENTS_PER_MESSAGE;
use crate::core::modality::{ModalitySupport, detect_modality_support};
use crate::core::types::{Attachment, ChatMessage, GenerateRequest, StreamMessage, ToolChoice};
use crate::generate::cancel::CANCEL_GENERATION;
use crate::generate::thinking_parser::ThinkingParser;
use crate::generate::tool_call_parser::ToolCallParser;
use crate::inference::engine::EngineSessionInfo;
use base64::Engine;
use eventsource_stream::Eventsource;
use futures_util::StreamExt;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::sync::OnceLock;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::Emitter;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAIMessage {
    role: String,
    content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
enum MessageContent {
    Text(String),
    Parts(Vec<ContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum ContentPart {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image_url")]
    ImageUrl { image_url: ContentImageUrl },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ContentImageUrl {
    url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OpenAITool {
    #[serde(rename = "type")]
    tool_type: String,
    function: crate::generate::tool_call_parser::ToolFunction,
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
    tools: Option<Vec<OpenAITool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_forced_open: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chat_template_kwargs: Option<Value>,
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
    #[serde(default)]
    reasoning_content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    #[serde(default)]
    message: Option<ChatCompletionMessage>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning_content: Option<String>,
    #[serde(default)]
    tool_calls: Option<Vec<ChatCompletionToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionToolCall {
    #[serde(default)]
    id: Option<String>,
    function: ChatCompletionToolFunction,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionToolFunction {
    name: String,
    #[serde(default)]
    arguments: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolCallMessage {
    pub id: String,
    pub name: String,
    pub arguments: Map<String, Value>,
}

#[derive(Debug, Clone)]
pub struct ChatCompletionMessageResult {
    pub content: String,
    pub finish_reason: Option<String>,
    pub tool_calls: Vec<ToolCallMessage>,
}

fn should_start_in_implicit_thinking_mode(
    model_id: &str,
    reasoning_start: &str,
    reasoning_end: &str,
) -> bool {
    if reasoning_start != "<think>" || reasoning_end != "</think>" {
        return false;
    }

    let id = model_id.to_ascii_lowercase();

    // Some reasoning models stream CoT without emitting <think>, but still
    // terminate with </think>. Enable implicit mode for those families/variants.
    let explicit_reasoning_variant = id.contains("think") || id.contains("reason");
    let known_implicit_families = id.contains("glm-4.6")
        || id.contains("glm4.6")
        || id.contains("deepseek-r1")
        || id.contains("qwq");

    explicit_reasoning_variant || known_implicit_families
}

fn attachment_hint(att: &Attachment) -> String {
    att.name
        .clone()
        .or_else(|| att.path.clone())
        .unwrap_or_else(|| "attachment".to_string())
}

fn attachment_ext(att: &Attachment) -> Option<String> {
    let candidate = att.name.clone().or_else(|| att.path.clone())?;
    let ext = std::path::Path::new(&candidate)
        .extension()
        .and_then(|v| v.to_str())?;
    Some(ext.to_ascii_lowercase())
}

fn is_image_attachment(att: &Attachment) -> bool {
    if let Some(kind) = att.kind.as_deref()
        && kind.eq_ignore_ascii_case("image")
    {
        return true;
    }
    if let Some(mime) = att.mime.as_deref()
        && mime.to_ascii_lowercase().starts_with("image/")
    {
        return true;
    }
    matches!(
        attachment_ext(att).as_deref(),
        Some("png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "svg")
    )
}

fn is_audio_attachment(att: &Attachment) -> bool {
    if let Some(kind) = att.kind.as_deref()
        && kind.eq_ignore_ascii_case("audio")
    {
        return true;
    }
    if let Some(mime) = att.mime.as_deref()
        && mime.to_ascii_lowercase().starts_with("audio/")
    {
        return true;
    }
    matches!(
        attachment_ext(att).as_deref(),
        Some("wav" | "mp3" | "m4a" | "ogg")
    )
}

fn is_video_attachment(att: &Attachment) -> bool {
    if let Some(kind) = att.kind.as_deref()
        && kind.eq_ignore_ascii_case("video")
    {
        return true;
    }
    if let Some(mime) = att.mime.as_deref()
        && mime.to_ascii_lowercase().starts_with("video/")
    {
        return true;
    }
    matches!(
        attachment_ext(att).as_deref(),
        Some("mp4" | "webm" | "mov" | "mkv")
    )
}

fn image_data_url(att: &Attachment) -> Result<String, String> {
    let bytes = read_attachment_bytes(att)?
        .ok_or_else(|| format!("Image attachment has no content: {}", attachment_hint(att)))?;
    let mime = att
        .mime
        .clone()
        .filter(|v| v.to_ascii_lowercase().starts_with("image/"))
        .unwrap_or_else(|| "image/png".to_string());
    let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    Ok(format!("data:{mime};base64,{b64}"))
}

fn build_user_message_content(
    user_text: String,
    attachments: &[Attachment],
    modality: &ModalitySupport,
) -> Result<MessageContent, String> {
    if attachments.is_empty() {
        return Ok(MessageContent::Text(user_text));
    }
    if attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(format!(
            "Too many attachments: {} (max {})",
            attachments.len(),
            MAX_ATTACHMENTS_PER_MESSAGE
        ));
    }

    let text_from_files = gather_text_from_attachments(attachments)?;
    let merged_text = if text_from_files.is_empty() {
        user_text
    } else if user_text.trim().is_empty() {
        text_from_files
    } else {
        format!("{user_text}\n\n{text_from_files}")
    };

    let mut has_image = false;
    let mut has_audio = false;
    let mut has_video = false;
    for att in attachments {
        if is_image_attachment(att) {
            has_image = true;
        } else if is_audio_attachment(att) {
            has_audio = true;
        } else if is_video_attachment(att) {
            has_video = true;
        }
    }

    if has_audio && !modality.audio {
        return Err("Current model/backend does not support audio attachments".to_string());
    }
    if has_video && !modality.video {
        return Err("Current model/backend does not support video attachments".to_string());
    }
    if has_image && !modality.image {
        return Err("Current model/backend does not support image attachments".to_string());
    }

    let mut parts = Vec::new();
    if !merged_text.trim().is_empty() {
        parts.push(ContentPart::Text { text: merged_text });
    }

    for att in attachments {
        if !is_image_attachment(att) {
            continue;
        }
        parts.push(ContentPart::ImageUrl {
            image_url: ContentImageUrl {
                url: image_data_url(att)?,
            },
        });
    }

    if parts.is_empty() {
        return Ok(MessageContent::Text(String::new()));
    }
    if parts.len() == 1
        && let ContentPart::Text { text } = &parts[0]
    {
        return Ok(MessageContent::Text(text.clone()));
    }
    Ok(MessageContent::Parts(parts))
}

fn to_openai_messages(
    req: &GenerateRequest,
    modality: &ModalitySupport,
) -> Result<Vec<OpenAIMessage>, String> {
    let attachments = req.attachments.clone().unwrap_or_default();
    if let Some(messages) = req.messages.as_ref() {
        let last_user_index = messages.iter().rposition(|m| m.role == "user");
        let mut out = Vec::with_capacity(messages.len());
        for (index, message) in messages.iter().enumerate() {
            let content = if Some(index) == last_user_index {
                build_user_message_content(message.content.clone(), &attachments, modality)?
            } else {
                MessageContent::Text(message.content.clone())
            };
            out.push(OpenAIMessage {
                role: message.role.clone(),
                content,
            });
        }
        return Ok(out);
    }

    Ok(vec![OpenAIMessage {
        role: "user".to_string(),
        content: build_user_message_content(req.prompt.clone(), &attachments, modality)?,
    }])
}

fn to_openai_tools(
    tools: Option<Vec<crate::generate::tool_call_parser::Tool>>,
) -> Option<Vec<OpenAITool>> {
    tools
        .map(|items| {
            items
                .into_iter()
                .map(|tool| OpenAITool {
                    tool_type: "function".to_string(),
                    function: tool.function,
                })
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty())
}

fn parse_arguments_value(value: Option<&Value>) -> Map<String, Value> {
    match value {
        Some(Value::Object(map)) => map.clone(),
        Some(Value::String(raw)) => serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|parsed| parsed.as_object().cloned())
            .unwrap_or_default(),
        _ => Map::new(),
    }
}

fn parse_tool_call_object(
    value: &Value,
    allowed_names: &HashSet<String>,
    auto_index: &mut usize,
) -> Option<ToolCallMessage> {
    let obj = value.as_object()?;

    let (name, arguments, id) =
        if let Some(function) = obj.get("function").and_then(Value::as_object) {
            (
                function.get("name").and_then(Value::as_str)?.to_string(),
                parse_arguments_value(function.get("arguments")),
                obj.get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            )
        } else {
            (
                obj.get("name").and_then(Value::as_str)?.to_string(),
                parse_arguments_value(obj.get("arguments").or(obj.get("parameters"))),
                obj.get("id")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            )
        };

    if !allowed_names.contains(&name) {
        return None;
    }

    let call_id = id.unwrap_or_else(|| {
        let next = *auto_index;
        *auto_index += 1;
        format!("call_parsed_{next}")
    });

    Some(ToolCallMessage {
        id: call_id,
        name,
        arguments,
    })
}

fn push_calls_from_json_snippet(
    snippet: &str,
    allowed_names: &HashSet<String>,
    auto_index: &mut usize,
    out: &mut Vec<ToolCallMessage>,
) {
    let trimmed = snippet.trim();
    if trimmed.is_empty() {
        return;
    }
    let Ok(value) = serde_json::from_str::<Value>(trimmed) else {
        return;
    };
    match value {
        Value::Array(items) => {
            for item in items {
                if let Some(call) = parse_tool_call_object(&item, allowed_names, auto_index) {
                    out.push(call);
                }
            }
        }
        single => {
            if let Some(call) = parse_tool_call_object(&single, allowed_names, auto_index) {
                out.push(call);
            }
        }
    }
}

fn parse_qwen3coder_function_block(
    block: &str,
    allowed_names: &HashSet<String>,
    auto_index: &mut usize,
) -> Option<ToolCallMessage> {
    static FUNCTION_RE: OnceLock<Regex> = OnceLock::new();
    static PARAM_RE: OnceLock<Regex> = OnceLock::new();
    let function_re = FUNCTION_RE.get_or_init(|| {
        Regex::new(r"(?s)<function=([^>]+)>\s*(.*?)\s*</function>").expect("valid function regex")
    });
    let param_re = PARAM_RE.get_or_init(|| {
        Regex::new(r"(?s)<parameter=([^>]+)>\s*(.*?)\s*</parameter>")
            .expect("valid parameter regex")
    });

    let captures = function_re.captures(block)?;
    let name = captures.get(1)?.as_str().trim().to_string();
    if !allowed_names.contains(&name) {
        return None;
    }
    let body = captures.get(2)?.as_str();
    let mut arguments = Map::new();
    for cap in param_re.captures_iter(body) {
        let key = cap.get(1).map(|m| m.as_str().trim()).unwrap_or_default();
        if key.is_empty() {
            continue;
        }
        let raw = cap
            .get(2)
            .map(|m| m.as_str().trim())
            .unwrap_or_default()
            .to_string();
        let value = serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw));
        arguments.insert(key.to_string(), value);
    }
    let call_id = {
        let next = *auto_index;
        *auto_index += 1;
        format!("call_parsed_{next}")
    };
    Some(ToolCallMessage {
        id: call_id,
        name,
        arguments,
    })
}

fn parse_tool_calls_from_content(
    content: &str,
    tools: &[crate::generate::tool_call_parser::Tool],
) -> Vec<ToolCallMessage> {
    static TOOL_CALL_TAG_RE: OnceLock<Regex> = OnceLock::new();
    static DEEPSEEK_TOOL_RE: OnceLock<Regex> = OnceLock::new();
    let tool_call_tag_re = TOOL_CALL_TAG_RE.get_or_init(|| {
        Regex::new(r"(?s)<tool_call>\s*(.*?)\s*</tool_call>").expect("valid tool_call regex")
    });
    let deepseek_tool_re = DEEPSEEK_TOOL_RE.get_or_init(|| {
        Regex::new(r"(?s)<｜tool▁call▁begin｜>function<｜tool▁sep｜>([^\n]+)\n```json\n(.*?)\n```<｜tool▁call▁end｜>")
            .expect("valid deepseek tool regex")
    });

    let allowed_names: HashSet<String> = tools
        .iter()
        .map(|tool| tool.function.name.clone())
        .collect();
    if allowed_names.is_empty() {
        return Vec::new();
    }

    let mut out = Vec::<ToolCallMessage>::new();
    let mut auto_index = 0usize;

    push_calls_from_json_snippet(content, &allowed_names, &mut auto_index, &mut out);

    if let Some(inner) = content
        .trim()
        .strip_prefix("[TOOL_CALLS][")
        .and_then(|raw| raw.strip_suffix(']'))
    {
        push_calls_from_json_snippet(inner, &allowed_names, &mut auto_index, &mut out);
    }

    for captures in tool_call_tag_re.captures_iter(content) {
        let body = captures.get(1).map(|m| m.as_str()).unwrap_or_default();
        let before = out.len();
        push_calls_from_json_snippet(body, &allowed_names, &mut auto_index, &mut out);
        if out.len() == before
            && let Some(call) =
                parse_qwen3coder_function_block(body, &allowed_names, &mut auto_index)
        {
            out.push(call);
        }
    }

    for captures in deepseek_tool_re.captures_iter(content) {
        let name = captures
            .get(1)
            .map(|m| m.as_str().trim())
            .unwrap_or_default()
            .to_string();
        if !allowed_names.contains(&name) {
            continue;
        }
        let args_text = captures.get(2).map(|m| m.as_str().trim()).unwrap_or("{}");
        let arguments = serde_json::from_str::<Value>(args_text)
            .ok()
            .and_then(|v| v.as_object().cloned())
            .unwrap_or_default();
        let call_id = {
            let next = auto_index;
            auto_index += 1;
            format!("call_parsed_{next}")
        };
        out.push(ToolCallMessage {
            id: call_id,
            name,
            arguments,
        });
    }

    let mut seen = HashSet::new();
    out.retain(|call| {
        let key = format!(
            "{}:{}",
            call.name,
            serde_json::to_string(&call.arguments).unwrap_or_default()
        );
        seen.insert(key)
    });
    out
}

fn response_format_from_request(req: &GenerateRequest) -> Option<Value> {
    if req.structured_output_enabled.unwrap_or(false) {
        Some(json!({ "type": "json_object" }))
    } else {
        None
    }
}

fn reasoning_format_from_request(req: &GenerateRequest) -> String {
    if req.reasoning_parse_enabled.unwrap_or(true) {
        "deepseek".to_string()
    } else {
        "none".to_string()
    }
}

fn chat_template_kwargs_from_request(req: &GenerateRequest) -> Value {
    json!({
        "enable_thinking": req.reasoning_parse_enabled.unwrap_or(true)
    })
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
    let modality = detect_modality_support(&session.model_id, session.mmproj_path.as_deref());
    let response_format = response_format_from_request(&req);

    let payload = ChatCompletionRequest {
        model: session.model_id.clone(),
        messages: to_openai_messages(&req, &modality)?,
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
        tools: to_openai_tools(req.tools.clone()),
        stop: req.stop_sequences.clone(),
        tool_choice: req.tool_choice.clone(),
        response_format,
        reasoning_format: Some(reasoning_format_from_request(&req)),
        thinking_forced_open: Some(req.reasoning_parse_enabled.unwrap_or(true)),
        chat_template_kwargs: Some(chat_template_kwargs_from_request(&req)),
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
    let reasoning_enabled = req.reasoning_parse_enabled.unwrap_or(true);
    let reasoning_start = req.reasoning_start_tag.as_deref().unwrap_or("<think>");
    let reasoning_end = req.reasoning_end_tag.as_deref().unwrap_or("</think>");
    let mut thinking_parser = if reasoning_enabled {
        Some(
            if should_start_in_implicit_thinking_mode(
                &session.model_id,
                reasoning_start,
                reasoning_end,
            ) {
                ThinkingParser::new_in_thinking_mode()
            } else if reasoning_start == "<think>" && reasoning_end == "</think>" {
                ThinkingParser::new()
            } else {
                ThinkingParser::with_tags(reasoning_start, reasoning_end)
            },
        )
    } else {
        None
    };
    let mut server_split_reasoning = false;

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
            if let Some(delta) = choice.delta {
                if let Some(reasoning_piece) = delta.reasoning_content
                    && !reasoning_piece.is_empty()
                {
                    if !server_split_reasoning {
                        // If llama.cpp streams reasoning_content separately, disable local
                        // tag-based parser to avoid splitting the same content twice.
                        if let Some(parser) = thinking_parser.as_mut() {
                            let tail = parser.flush();
                            if !tail.thinking.is_empty() || !tail.content.is_empty() {
                                let _ = app.emit(
                                    "message",
                                    StreamMessage {
                                        thinking: tail.thinking,
                                        content: tail.content,
                                    },
                                );
                            }
                        }
                        thinking_parser = None;
                        server_split_reasoning = true;
                    }

                    let _ = app.emit(
                        "message",
                        StreamMessage {
                            thinking: reasoning_piece,
                            content: String::new(),
                        },
                    );
                }

                if let Some(content) = delta.content
                    && !content.is_empty()
                {
                    if server_split_reasoning {
                        let _ = app.emit(
                            "message",
                            StreamMessage {
                                thinking: String::new(),
                                content,
                            },
                        );
                    } else if let Some(parser) = thinking_parser.as_mut() {
                        let chunk = parser.process_token(&content);
                        if !chunk.thinking.is_empty() || !chunk.content.is_empty() {
                            let _ = app.emit(
                                "message",
                                StreamMessage {
                                    thinking: chunk.thinking,
                                    content: chunk.content,
                                },
                            );
                        }
                    } else {
                        let _ = app.emit(
                            "message",
                            StreamMessage {
                                thinking: String::new(),
                                content,
                            },
                        );
                    }
                }
            }
            if choice.finish_reason.is_some() {
                break;
            }
        }
    }

    if let Some(parser) = thinking_parser.as_mut() {
        let tail = parser.flush();
        if !tail.thinking.is_empty() || !tail.content.is_empty() {
            let _ = app.emit(
                "message",
                StreamMessage {
                    thinking: tail.thinking,
                    content: tail.content,
                },
            );
        }
    }

    let _ = app.emit("message_done", ());
    Ok(())
}

pub async fn chat_completion_once(
    session: &EngineSessionInfo,
    req: GenerateRequest,
) -> Result<ChatCompletionMessageResult, String> {
    let modality = detect_modality_support(&session.model_id, session.mmproj_path.as_deref());
    let response_format = response_format_from_request(&req);
    let payload = ChatCompletionRequest {
        model: session.model_id.clone(),
        messages: to_openai_messages(&req, &modality)?,
        stream: false,
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
        tools: to_openai_tools(req.tools.clone()),
        stop: req.stop_sequences.clone(),
        tool_choice: req.tool_choice.clone(),
        response_format,
        reasoning_format: Some(reasoning_format_from_request(&req)),
        thinking_forced_open: Some(req.reasoning_parse_enabled.unwrap_or(true)),
        chat_template_kwargs: Some(chat_template_kwargs_from_request(&req)),
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

    let parsed = response
        .json::<ChatCompletionResponse>()
        .await
        .map_err(|e| format!("failed to parse non-stream chat completion: {e}"))?;
    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| "llama-server returned empty choices".to_string())?;
    let message = choice
        .message
        .ok_or_else(|| "llama-server returned empty assistant message".to_string())?;
    let content = message
        .content
        .or(message.reasoning_content)
        .unwrap_or_default();

    let mut tool_calls = Vec::new();
    if let Some(calls) = message.tool_calls {
        for call in calls {
            let args = call.function.arguments.unwrap_or_else(|| "{}".to_string());
            let map = serde_json::from_str::<Value>(&args)
                .ok()
                .and_then(|v| v.as_object().cloned())
                .unwrap_or_default();
            tool_calls.push(ToolCallMessage {
                id: call
                    .id
                    .unwrap_or_else(|| format!("call_{}", tool_calls.len())),
                name: call.function.name,
                arguments: map,
            });
        }
    }

    if tool_calls.is_empty() && !content.trim().is_empty() && req.tools.is_some() {
        let tool_defs = req.tools.clone().unwrap_or_default();
        let mut parsed = parse_tool_calls_from_content(&content, &tool_defs);
        if parsed.is_empty() {
            let mut parser = ToolCallParser::with_json_tag(tool_defs);
            let parsed_calls = parser.add(&content).calls;
            for call in parsed_calls {
                parsed.push(ToolCallMessage {
                    id: call.id,
                    name: call.function.name,
                    arguments: call.function.arguments.into_iter().collect(),
                });
            }
        }
        tool_calls.extend(parsed);
    }

    Ok(ChatCompletionMessageResult {
        content,
        finish_reason: choice.finish_reason,
        tool_calls,
    })
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

#[cfg(test)]
mod tests {
    use super::parse_tool_calls_from_content;
    use super::should_start_in_implicit_thinking_mode;
    use super::{MessageContent, build_user_message_content};
    use crate::core::modality::ModalitySupport;
    use crate::core::types::{Attachment, GenerateRequest};
    use crate::generate::tool_call_parser::{Tool, ToolFunction};
    use serde_json::{Value, json};

    fn tools() -> Vec<Tool> {
        vec![
            Tool {
                function: ToolFunction {
                    name: "mcp_context7_resolve_library_id".to_string(),
                    description: None,
                    parameters: None,
                },
            },
            Tool {
                function: ToolFunction {
                    name: "mcp_context7_get_docs".to_string(),
                    description: None,
                    parameters: None,
                },
            },
        ]
    }

    fn make_generate_request() -> GenerateRequest {
        GenerateRequest {
            prompt: "hello".to_string(),
            messages: None,
            attachments: None,
            max_new_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            min_p: None,
            repeat_penalty: None,
            repeat_last_n: 64,
            use_custom_params: false,
            seed: None,
            split_prompt: None,
            verbose_prompt: None,
            tracing: None,
            reasoning_parse_enabled: None,
            reasoning_start_tag: None,
            reasoning_end_tag: None,
            structured_output_enabled: None,
            edit_index: None,
            format: None,
            tools: None,
            stop_sequences: None,
            tool_choice: None,
            retrieval: None,
            mcp: None,
        }
    }

    #[test]
    fn parses_qwen_tool_call_tag() {
        let content = r#"<tool_call>
{"name":"mcp_context7_resolve_library_id","arguments":{"query":"svelte"}}
</tool_call>"#;
        let parsed = parse_tool_calls_from_content(content, &tools());
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "mcp_context7_resolve_library_id");
        assert_eq!(parsed[0].arguments.get("query"), Some(&json!("svelte")));
    }

    #[test]
    fn parses_mistral_tool_calls_wrapper() {
        let content = r#"[TOOL_CALLS][{"name":"mcp_context7_get_docs","arguments":{"libraryName":"svelte","topic":"stores"}}]"#;
        let parsed = parse_tool_calls_from_content(content, &tools());
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "mcp_context7_get_docs");
        assert_eq!(
            parsed[0].arguments.get("libraryName"),
            Some(&json!("svelte"))
        );
    }

    #[test]
    fn parses_qwen3coder_function_tag() {
        let content = r#"<tool_call>
<function=mcp_context7_get_docs>
<parameter=libraryName>
svelte
</parameter>
<parameter=topic>
stores
</parameter>
</function>
</tool_call>"#;
        let parsed = parse_tool_calls_from_content(content, &tools());
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "mcp_context7_get_docs");
        assert_eq!(parsed[0].arguments.get("topic"), Some(&json!("stores")));
    }

    #[test]
    fn serializes_openai_tools_with_type_function() {
        let payload = super::ChatCompletionRequest {
            model: "test".to_string(),
            messages: vec![super::OpenAIMessage {
                role: "user".to_string(),
                content: super::MessageContent::Text("hi".to_string()),
            }],
            stream: false,
            max_tokens: None,
            temperature: None,
            top_p: None,
            top_k: None,
            min_p: None,
            repeat_penalty: None,
            repeat_last_n: None,
            tools: super::to_openai_tools(Some(tools())),
            stop: None,
            tool_choice: None,
            response_format: None,
            reasoning_format: None,
            thinking_forced_open: None,
            chat_template_kwargs: None,
        };

        let json = serde_json::to_value(payload).expect("payload should serialize");
        let tools = json
            .get("tools")
            .and_then(Value::as_array)
            .expect("tools should be present");
        assert_eq!(tools[0].get("type"), Some(&json!("function")));
        assert_eq!(
            tools[0].get("function").and_then(|v| v.get("name")),
            Some(&json!("mcp_context7_resolve_library_id"))
        );
    }

    #[test]
    fn qwen3_think_models_start_in_implicit_thinking_mode() {
        assert!(should_start_in_implicit_thinking_mode(
            "Qwen/Qwen3-30B-A3B-Thinking-GGUF",
            "<think>",
            "</think>",
        ));
        assert!(should_start_in_implicit_thinking_mode(
            "qwen3-14b-think",
            "<think>",
            "</think>",
        ));
        assert!(!should_start_in_implicit_thinking_mode(
            "Qwen/Qwen3-8B-GGUF",
            "<think>",
            "</think>",
        ));
        assert!(!should_start_in_implicit_thinking_mode(
            "Qwen/Qwen3-30B-A3B-Thinking-GGUF",
            "<reasoning>",
            "</reasoning>",
        ));
        assert!(should_start_in_implicit_thinking_mode(
            "THUDM/GLM-4.6",
            "<think>",
            "</think>",
        ));
        assert!(should_start_in_implicit_thinking_mode(
            "deepseek-r1-distill-14b",
            "<think>",
            "</think>",
        ));
    }

    #[test]
    fn response_format_enabled_maps_to_json_object() {
        let mut req = make_generate_request();
        req.structured_output_enabled = Some(true);
        let response_format = super::response_format_from_request(&req);
        assert_eq!(response_format, Some(json!({ "type": "json_object" })));
    }

    #[test]
    fn response_format_disabled_is_none() {
        let req = make_generate_request();
        let response_format = super::response_format_from_request(&req);
        assert_eq!(response_format, None);
    }

    #[test]
    fn message_content_becomes_parts_for_image_attachment() {
        let attachment = Attachment {
            kind: Some("image".to_string()),
            mime: Some("image/png".to_string()),
            name: Some("cat.png".to_string()),
            path: None,
            bytes_b64: Some("AAAA".to_string()),
            size: None,
        };
        let modality = ModalitySupport {
            text: true,
            image: true,
            audio: false,
            video: false,
        };
        let content = build_user_message_content("describe".to_string(), &[attachment], &modality)
            .expect("content");
        match content {
            MessageContent::Parts(parts) => assert_eq!(parts.len(), 2),
            MessageContent::Text(_) => panic!("expected parts"),
        }
    }
}
