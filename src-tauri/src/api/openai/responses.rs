use super::error::ApiError;
use super::types::ExtraFields;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponsesReasoningConfig {
    #[serde(default)]
    pub effort: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub generate_summary: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponsesTextFormat {
    #[serde(rename = "type")]
    pub format_type: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub schema: Option<Value>,
    #[serde(default)]
    pub strict: Option<bool>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponsesTextConfig {
    #[serde(default)]
    pub format: Option<ResponsesTextFormat>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResponsesTool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub strict: Option<bool>,
    #[serde(default)]
    pub parameters: Option<Value>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResponsesRequest {
    pub model: String,
    #[serde(default)]
    pub background: Option<bool>,
    #[serde(default)]
    pub conversation: Option<Value>,
    #[serde(default)]
    pub include: Option<Vec<String>>,
    #[serde(default)]
    pub input: Value,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub max_output_tokens: Option<usize>,
    #[serde(default)]
    pub reasoning: Option<ResponsesReasoningConfig>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub text: Option<ResponsesTextConfig>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub truncation: Option<String>,
    #[serde(default)]
    pub tools: Option<Vec<ResponsesTool>>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

impl ResponsesRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.model.trim().is_empty() {
            return Err(ApiError::invalid_request("model is required", "model"));
        }

        if matches!(self.input, Value::Null) {
            return Err(ApiError::invalid_request("input is required", "input"));
        }

        if let Some(reasoning) = &self.reasoning
            && let Some(effort) = reasoning.effort.as_deref()
        {
            let allowed = ["high", "medium", "low", "none"];
            if !allowed.contains(&effort) {
                return Err(ApiError::invalid_request(
                    "reasoning.effort must be one of: high, medium, low, none",
                    "reasoning.effort",
                ));
            }
        }

        if let Some(text_cfg) = &self.text
            && let Some(fmt) = &text_cfg.format
        {
            let t = fmt.format_type.as_str();
            if t != "text" && t != "json_schema" {
                return Err(ApiError::invalid_request(
                    "text.format.type must be either 'text' or 'json_schema'",
                    "text.format.type",
                ));
            }
        }

        Ok(())
    }

    pub fn stream_enabled(&self) -> bool {
        self.stream.unwrap_or(false)
    }

    pub fn to_chat_payload(&self, stream: bool) -> Result<Value, ApiError> {
        let mut map = Map::<String, Value>::new();
        map.insert("model".to_string(), Value::String(self.model.clone()));
        map.insert("stream".to_string(), Value::Bool(stream));
        map.insert("messages".to_string(), Value::Array(build_messages(self)?));

        if let Some(v) = self.max_output_tokens {
            map.insert("max_tokens".to_string(), json!(v));
        }
        if let Some(v) = self.temperature {
            map.insert("temperature".to_string(), json!(v));
        }
        if let Some(v) = self.top_p {
            map.insert("top_p".to_string(), json!(v));
        }

        if let Some(reasoning) = &self.reasoning {
            if let Some(effort) = &reasoning.effort {
                map.insert(
                    "reasoning_effort".to_string(),
                    Value::String(effort.clone()),
                );
            }
            if !reasoning.extra.is_empty() {
                let keys: Vec<&str> = reasoning.extra.keys().map(|k| k.as_str()).collect();
                log::warn!(
                    "OpenAI responses.reasoning includes unknown fields; forwarding best-effort: {}",
                    keys.join(", ")
                );
            }
        }

        if let Some(text_cfg) = &self.text {
            if let Some(fmt) = &text_cfg.format {
                match fmt.format_type.as_str() {
                    "json_schema" => {
                        let mut rs = Map::<String, Value>::new();
                        rs.insert("type".to_string(), Value::String("json_schema".to_string()));
                        let mut js = Map::<String, Value>::new();
                        if let Some(name) = &fmt.name {
                            js.insert("name".to_string(), Value::String(name.clone()));
                        }
                        if let Some(schema) = &fmt.schema {
                            js.insert("schema".to_string(), schema.clone());
                        }
                        if let Some(strict) = fmt.strict {
                            js.insert("strict".to_string(), Value::Bool(strict));
                        }
                        rs.insert("json_schema".to_string(), Value::Object(js));
                        map.insert("response_format".to_string(), Value::Object(rs));
                    }
                    "text" => {
                        map.insert(
                            "response_format".to_string(),
                            json!({
                                "type": "text"
                            }),
                        );
                    }
                    _ => {}
                }
                if !fmt.extra.is_empty() {
                    let keys: Vec<&str> = fmt.extra.keys().map(|k| k.as_str()).collect();
                    log::warn!(
                        "OpenAI responses.text.format includes unknown fields; forwarding best-effort: {}",
                        keys.join(", ")
                    );
                }
            }
            if !text_cfg.extra.is_empty() {
                let keys: Vec<&str> = text_cfg.extra.keys().map(|k| k.as_str()).collect();
                log::warn!(
                    "OpenAI responses.text includes unknown fields; forwarding best-effort: {}",
                    keys.join(", ")
                );
            }
        }

        if let Some(tools) = &self.tools {
            let mut mapped = Vec::<Value>::new();
            for tool in tools {
                if tool.tool_type != "function" {
                    return Err(ApiError::not_implemented(
                        "only function tools are supported in responses",
                        "tools.type",
                    ));
                }
                let function = json!({
                    "name": tool.name,
                    "description": tool.description,
                    "parameters": tool.parameters.clone().unwrap_or_else(|| json!({})),
                    "strict": tool.strict,
                });
                mapped.push(json!({
                    "type": "function",
                    "function": function
                }));

                if !tool.extra.is_empty() {
                    let keys: Vec<&str> = tool.extra.keys().map(|k| k.as_str()).collect();
                    log::warn!(
                        "OpenAI responses.tool includes unknown fields; forwarding best-effort: {}",
                        keys.join(", ")
                    );
                }
            }
            map.insert("tools".to_string(), Value::Array(mapped));
        }

        for (k, v) in &self.extra {
            map.insert(k.clone(), v.clone());
        }

        Ok(Value::Object(map))
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ResponsesStreamEvent {
    pub event: String,
    pub data: Value,
}

pub struct ResponsesStreamConverter {
    response_id: String,
    item_id: String,
    model: String,
    created_at: u64,
    started: bool,
    completed: bool,
    accumulated: String,
}

impl ResponsesStreamConverter {
    pub fn new(response_id: String, item_id: String, model: String) -> Self {
        Self {
            response_id,
            item_id,
            model,
            created_at: now_unix(),
            started: false,
            completed: false,
            accumulated: String::new(),
        }
    }

    pub fn start_events(&self) -> Vec<ResponsesStreamEvent> {
        vec![
            ResponsesStreamEvent {
                event: "response.created".to_string(),
                data: json!({
                    "id": self.response_id,
                    "object": "response",
                    "created_at": self.created_at,
                    "model": self.model,
                    "status": "created",
                }),
            },
            ResponsesStreamEvent {
                event: "response.in_progress".to_string(),
                data: json!({
                    "id": self.response_id,
                    "object": "response",
                    "created_at": self.created_at,
                    "model": self.model,
                    "status": "in_progress",
                }),
            },
        ]
    }

    pub fn process_chat_chunk(&mut self, chunk: &Value) -> Vec<ResponsesStreamEvent> {
        if self.completed {
            return Vec::new();
        }

        let mut out = Vec::<ResponsesStreamEvent>::new();
        let choice = chunk
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|c| c.first());

        let delta_text = choice
            .and_then(|c| c.get("delta"))
            .and_then(|d| d.get("content"))
            .and_then(Value::as_str)
            .unwrap_or_default();

        let finish_reason = choice
            .and_then(|c| c.get("finish_reason"))
            .and_then(Value::as_str);

        if !delta_text.is_empty() {
            if !self.started {
                self.started = true;
                out.push(ResponsesStreamEvent {
                    event: "response.output_item.added".to_string(),
                    data: json!({
                        "response_id": self.response_id,
                        "output_index": 0,
                        "item": {
                            "id": self.item_id,
                            "type": "message",
                            "status": "in_progress",
                            "role": "assistant",
                            "content": []
                        }
                    }),
                });
            }

            self.accumulated.push_str(delta_text);
            out.push(ResponsesStreamEvent {
                event: "response.output_text.delta".to_string(),
                data: json!({
                    "response_id": self.response_id,
                    "item_id": self.item_id,
                    "output_index": 0,
                    "content_index": 0,
                    "delta": delta_text,
                }),
            });
        }

        if finish_reason.is_some() {
            out.extend(self.finish_events());
        }

        out
    }

    pub fn finish_events(&mut self) -> Vec<ResponsesStreamEvent> {
        if self.completed {
            return Vec::new();
        }
        self.completed = true;

        let mut out = Vec::<ResponsesStreamEvent>::new();
        if self.started {
            out.push(ResponsesStreamEvent {
                event: "response.output_text.done".to_string(),
                data: json!({
                    "response_id": self.response_id,
                    "item_id": self.item_id,
                    "output_index": 0,
                    "content_index": 0,
                    "text": self.accumulated,
                }),
            });
            out.push(ResponsesStreamEvent {
                event: "response.output_item.done".to_string(),
                data: json!({
                    "response_id": self.response_id,
                    "output_index": 0,
                    "item": {
                        "id": self.item_id,
                        "type": "message",
                        "status": "completed",
                        "role": "assistant",
                        "content": [{
                            "type": "output_text",
                            "text": self.accumulated,
                            "annotations": []
                        }]
                    }
                }),
            });
        }

        out.push(ResponsesStreamEvent {
            event: "response.completed".to_string(),
            data: json!({
                "id": self.response_id,
                "object": "response",
                "created_at": self.created_at,
                "model": self.model,
                "status": "completed",
                "output": if self.started {
                    json!([{
                        "id": self.item_id,
                        "type": "message",
                        "status": "completed",
                        "role": "assistant",
                        "content": [{
                            "type": "output_text",
                            "text": self.accumulated,
                            "annotations": []
                        }]
                    }])
                } else {
                    json!([])
                }
            }),
        });

        out
    }

    pub fn failed_event(&self, message: &str) -> ResponsesStreamEvent {
        ResponsesStreamEvent {
            event: "response.failed".to_string(),
            data: json!({
                "id": self.response_id,
                "object": "response",
                "created_at": self.created_at,
                "model": self.model,
                "status": "failed",
                "error": {
                    "message": message,
                    "type": "upstream_error",
                    "param": Value::Null,
                    "code": "upstream_error",
                }
            }),
        }
    }
}

pub fn to_non_stream_response(
    request: &ResponsesRequest,
    response_id: &str,
    item_id: &str,
    chat_payload: &Value,
) -> Value {
    let created = now_unix();
    let choice = chat_payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|v| v.first())
        .cloned()
        .unwrap_or(Value::Null);

    let assistant = choice
        .get("message")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let content = assistant
        .get("content")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut output = Vec::<Value>::new();
    output.push(json!({
        "id": item_id,
        "type": "message",
        "status": "completed",
        "role": "assistant",
        "content": [{
            "type": "output_text",
            "text": content,
            "annotations": []
        }]
    }));

    json!({
        "id": response_id,
        "object": "response",
        "created_at": created,
        "model": request.model,
        "status": "completed",
        "output": output,
        "parallel_tool_calls": true,
        "tools": request.tools.clone().unwrap_or_default(),
        "top_p": request.top_p.unwrap_or(1.0),
        "temperature": request.temperature.unwrap_or(1.0),
        "usage": chat_payload.get("usage").cloned().unwrap_or_else(|| json!({})),
    })
}

fn build_messages(req: &ResponsesRequest) -> Result<Vec<Value>, ApiError> {
    let mut messages = Vec::<Value>::new();
    if let Some(instructions) = &req.instructions
        && !instructions.trim().is_empty()
    {
        messages.push(json!({
            "role": "system",
            "content": instructions
        }));
    }

    match &req.input {
        Value::String(s) => {
            messages.push(json!({ "role": "user", "content": s }));
        }
        Value::Array(items) => {
            for item in items {
                let obj = item.as_object().ok_or_else(|| {
                    ApiError::invalid_request("input[] items must be objects", "input")
                })?;
                let item_type = obj
                    .get("type")
                    .and_then(Value::as_str)
                    .or_else(|| obj.get("role").map(|_| "message"))
                    .unwrap_or_default();

                match item_type {
                    "message" => {
                        let role = obj
                            .get("role")
                            .and_then(Value::as_str)
                            .unwrap_or("user")
                            .to_string();
                        let content = obj
                            .get("content")
                            .map(extract_text_content)
                            .transpose()?
                            .unwrap_or_default();
                        messages.push(json!({
                            "role": role,
                            "content": content
                        }));
                    }
                    "function_call_output" => {
                        let output = obj
                            .get("output")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        let call_id = obj
                            .get("call_id")
                            .and_then(Value::as_str)
                            .unwrap_or_default();
                        messages.push(json!({
                            "role": "tool",
                            "content": output,
                            "tool_call_id": call_id
                        }));
                    }
                    "function_call" => {
                        let call_id = obj.get("call_id").and_then(Value::as_str).unwrap_or("");
                        let name = obj.get("name").and_then(Value::as_str).unwrap_or("");
                        let args = obj.get("arguments").and_then(Value::as_str).unwrap_or("{}");
                        messages.push(json!({
                            "role": "assistant",
                            "tool_calls": [{
                                "id": call_id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": args
                                }
                            }]
                        }));
                    }
                    "reasoning" => {}
                    other => {
                        return Err(ApiError::invalid_request(
                            format!("unsupported input item type: {other}"),
                            "input.type",
                        ));
                    }
                }
            }
        }
        _ => {
            return Err(ApiError::invalid_request(
                "input must be a string or array",
                "input",
            ));
        }
    }

    if messages.is_empty() {
        return Err(ApiError::invalid_request(
            "input produced no chat messages",
            "input",
        ));
    }
    Ok(messages)
}

fn extract_text_content(content: &Value) -> Result<String, ApiError> {
    match content {
        Value::String(s) => Ok(s.clone()),
        Value::Array(parts) => {
            let mut out = String::new();
            for p in parts {
                let obj = p.as_object().ok_or_else(|| {
                    ApiError::invalid_request("content items must be objects", "input.content")
                })?;
                let part_type = obj.get("type").and_then(Value::as_str).unwrap_or_default();
                match part_type {
                    "input_text" | "output_text" | "text" => {
                        if let Some(t) = obj.get("text").and_then(Value::as_str) {
                            out.push_str(t);
                        }
                    }
                    "input_image" => {
                        return Err(ApiError::not_implemented(
                            "input_image in /v1/responses is not supported in current backend",
                            "input.content",
                        ));
                    }
                    _ => {}
                }
            }
            Ok(out)
        }
        _ => Err(ApiError::invalid_request(
            "content must be string or array",
            "input.content",
        )),
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn responses_stream_event_order() {
        let mut c = ResponsesStreamConverter::new(
            "resp_1".to_string(),
            "msg_1".to_string(),
            "m".to_string(),
        );
        let mut events = c.start_events();
        events.extend(c.process_chat_chunk(&json!({
            "choices": [{ "delta": { "content": "hel" } }]
        })));
        events.extend(c.process_chat_chunk(&json!({
            "choices": [{ "delta": { "content": "lo" }, "finish_reason": "stop" }]
        })));

        let names: Vec<String> = events.into_iter().map(|e| e.event).collect();
        assert_eq!(
            names,
            vec![
                "response.created",
                "response.in_progress",
                "response.output_item.added",
                "response.output_text.delta",
                "response.output_text.delta",
                "response.output_text.done",
                "response.output_item.done",
                "response.completed",
            ]
        );
    }
}
