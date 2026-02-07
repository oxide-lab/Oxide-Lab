use super::error::ApiError;
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

pub type ExtraFields = HashMap<String, Value>;

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct StreamOptions {
    #[serde(default)]
    pub include_usage: bool,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Value,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Value>,
    #[serde(default)]
    pub reasoning: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub stream_options: Option<StreamOptions>,
    #[serde(default)]
    pub max_tokens: Option<usize>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub stop: Option<Value>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub frequency_penalty: Option<f64>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub response_format: Option<Value>,
    #[serde(default)]
    pub tools: Option<Value>,
    #[serde(default)]
    pub tool_choice: Option<Value>,
    #[serde(default)]
    pub reasoning: Option<Value>,
    #[serde(default)]
    pub reasoning_effort: Option<String>,
    #[serde(default)]
    pub logprobs: Option<bool>,
    #[serde(default)]
    pub top_logprobs: Option<i32>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

impl ChatCompletionRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        if self.messages.is_empty() {
            return Err(ApiError::invalid_request(
                "[] is too short - 'messages'",
                "messages",
            ));
        }

        if let Some(top) = self.top_logprobs
            && !(0..=20).contains(&top)
        {
            return Err(ApiError::invalid_request(
                "top_logprobs must be between 0 and 20",
                "top_logprobs",
            ));
        }

        if let Some(effort) = self.reasoning_effort.as_deref() {
            let allowed = ["high", "medium", "low", "none"];
            if !allowed.contains(&effort) {
                return Err(ApiError::invalid_request(
                    "reasoning_effort must be one of: high, medium, low, none",
                    "reasoning_effort",
                ));
            }
        }

        Ok(())
    }

    pub fn to_upstream_payload(&self, stream_override: Option<bool>) -> Value {
        let mut map = Map::<String, Value>::new();
        map.insert("model".to_string(), Value::String(self.model.clone()));
        map.insert(
            "messages".to_string(),
            serde_json::to_value(&self.messages).unwrap_or_else(|_| Value::Array(Vec::new())),
        );
        map.insert(
            "stream".to_string(),
            Value::Bool(stream_override.unwrap_or(self.stream)),
        );

        if let Some(v) = &self.stream_options {
            map.insert(
                "stream_options".to_string(),
                serde_json::to_value(v).unwrap_or_default(),
            );
        }
        insert_opt(&mut map, "max_tokens", self.max_tokens.map(|v| json!(v)));
        insert_opt(&mut map, "seed", self.seed.map(|v| json!(v)));
        insert_opt(&mut map, "stop", self.stop.clone());
        insert_opt(&mut map, "temperature", self.temperature.map(|v| json!(v)));
        insert_opt(
            &mut map,
            "frequency_penalty",
            self.frequency_penalty.map(|v| json!(v)),
        );
        insert_opt(
            &mut map,
            "presence_penalty",
            self.presence_penalty.map(|v| json!(v)),
        );
        insert_opt(&mut map, "top_p", self.top_p.map(|v| json!(v)));
        insert_opt(&mut map, "response_format", self.response_format.clone());
        insert_opt(&mut map, "tools", self.tools.clone());
        insert_opt(&mut map, "tool_choice", self.tool_choice.clone());
        insert_opt(&mut map, "reasoning", self.reasoning.clone());
        insert_opt(
            &mut map,
            "reasoning_effort",
            self.reasoning_effort.clone().map(Value::String),
        );
        insert_opt(&mut map, "logprobs", self.logprobs.map(Value::Bool));
        insert_opt(
            &mut map,
            "top_logprobs",
            self.top_logprobs.map(|v| json!(v)),
        );
        insert_opt(&mut map, "user", self.user.clone().map(Value::String));
        merge_extras(&mut map, &self.extra);
        Value::Object(map)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CompletionRequest {
    pub model: String,
    pub prompt: Value,
    #[serde(default)]
    pub frequency_penalty: Option<f64>,
    #[serde(default)]
    pub max_tokens: Option<usize>,
    #[serde(default)]
    pub presence_penalty: Option<f64>,
    #[serde(default)]
    pub seed: Option<i64>,
    #[serde(default)]
    pub stop: Option<Value>,
    #[serde(default)]
    pub stream: bool,
    #[serde(default)]
    pub stream_options: Option<StreamOptions>,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub top_p: Option<f64>,
    #[serde(default)]
    pub suffix: Option<String>,
    #[serde(default)]
    pub logprobs: Option<i32>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

impl CompletionRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        if matches!(self.prompt, Value::Null) {
            return Err(ApiError::invalid_request("prompt is required", "prompt"));
        }
        if let Some(v) = self.logprobs
            && !(0..=20).contains(&v)
        {
            return Err(ApiError::invalid_request(
                "logprobs must be between 0 and 20",
                "logprobs",
            ));
        }
        Ok(())
    }

    pub fn to_upstream_payload(&self, stream_override: Option<bool>) -> Value {
        let mut map = Map::<String, Value>::new();
        map.insert("model".to_string(), Value::String(self.model.clone()));
        map.insert("prompt".to_string(), self.prompt.clone());
        map.insert(
            "stream".to_string(),
            Value::Bool(stream_override.unwrap_or(self.stream)),
        );
        insert_opt(
            &mut map,
            "frequency_penalty",
            self.frequency_penalty.map(|v| json!(v)),
        );
        insert_opt(&mut map, "max_tokens", self.max_tokens.map(|v| json!(v)));
        insert_opt(
            &mut map,
            "presence_penalty",
            self.presence_penalty.map(|v| json!(v)),
        );
        insert_opt(&mut map, "seed", self.seed.map(|v| json!(v)));
        insert_opt(&mut map, "stop", self.stop.clone());
        insert_opt(
            &mut map,
            "stream_options",
            self.stream_options
                .as_ref()
                .and_then(|v| serde_json::to_value(v).ok()),
        );
        insert_opt(&mut map, "temperature", self.temperature.map(|v| json!(v)));
        insert_opt(&mut map, "top_p", self.top_p.map(|v| json!(v)));
        insert_opt(&mut map, "suffix", self.suffix.clone().map(Value::String));
        insert_opt(&mut map, "logprobs", self.logprobs.map(|v| json!(v)));
        insert_opt(&mut map, "user", self.user.clone().map(Value::String));
        merge_extras(&mut map, &self.extra);
        Value::Object(map)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingsRequest {
    pub model: String,
    pub input: Value,
    #[serde(default)]
    pub dimensions: Option<usize>,
    #[serde(default)]
    pub encoding_format: Option<String>,
    #[serde(default)]
    pub user: Option<String>,
    #[serde(flatten, default)]
    pub extra: ExtraFields,
}

impl EmbeddingsRequest {
    pub fn validate(&self) -> Result<(), ApiError> {
        if matches!(self.input, Value::Null) {
            return Err(ApiError::invalid_request("input is required", "input"));
        }
        if let Value::Array(v) = &self.input
            && v.is_empty()
        {
            return Err(ApiError::invalid_request(
                "input array must not be empty",
                "input",
            ));
        }
        if let Some(fmt) = self.encoding_format.as_deref() {
            let lower = fmt.to_ascii_lowercase();
            if lower != "float" && lower != "base64" {
                return Err(ApiError::invalid_request(
                    "encoding_format must be either 'float' or 'base64'",
                    "encoding_format",
                ));
            }
        }
        Ok(())
    }

    pub fn wants_base64(&self) -> bool {
        self.encoding_format
            .as_deref()
            .map(|v| v.eq_ignore_ascii_case("base64"))
            .unwrap_or(false)
    }

    pub fn to_upstream_payload(&self) -> Value {
        let mut map = Map::<String, Value>::new();
        map.insert("model".to_string(), Value::String(self.model.clone()));
        map.insert("input".to_string(), self.input.clone());
        map.insert(
            "encoding_format".to_string(),
            Value::String("float".to_string()),
        );
        insert_opt(&mut map, "dimensions", self.dimensions.map(|v| json!(v)));
        insert_opt(&mut map, "user", self.user.clone().map(Value::String));
        merge_extras(&mut map, &self.extra);
        Value::Object(map)
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

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn log_unknown_fields(endpoint: &str, extras: &ExtraFields) {
    if extras.is_empty() {
        return;
    }
    let keys: Vec<&str> = extras.keys().map(|k| k.as_str()).collect();
    log::warn!(
        "OpenAI {} request includes unknown fields; forwarding best-effort: {}",
        endpoint,
        keys.join(", ")
    );
}

pub fn embed_float_to_base64(payload: &mut Value) -> Result<(), ApiError> {
    let data = payload
        .get_mut("data")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| ApiError::upstream("invalid embeddings payload: missing data array"))?;

    for item in data {
        let emb = item
            .get("embedding")
            .cloned()
            .ok_or_else(|| ApiError::upstream("invalid embeddings payload: missing embedding"))?;

        let arr = emb.as_array().ok_or_else(|| {
            ApiError::upstream("invalid embeddings payload: embedding is not array")
        })?;

        let mut bytes = Vec::with_capacity(arr.len() * 4);
        for v in arr {
            let f = v.as_f64().ok_or_else(|| {
                ApiError::upstream("invalid embeddings payload: non-numeric value")
            })? as f32;
            bytes.extend_from_slice(&f.to_le_bytes());
        }

        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
        if let Some(obj) = item.as_object_mut() {
            obj.insert("embedding".to_string(), Value::String(encoded));
        }
    }

    Ok(())
}

fn insert_opt(map: &mut Map<String, Value>, key: &str, value: Option<Value>) {
    if let Some(v) = value {
        map.insert(key.to_string(), v);
    }
}

fn merge_extras(map: &mut Map<String, Value>, extras: &ExtraFields) {
    for (k, v) in extras {
        map.insert(k.clone(), v.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeddings_base64_encoding_is_little_endian_f32() {
        let mut payload = json!({
            "data": [{
                "embedding": [1.0, -2.5, 0.25]
            }]
        });

        embed_float_to_base64(&mut payload).expect("base64");
        let got = payload["data"][0]["embedding"]
            .as_str()
            .expect("string")
            .to_string();

        let mut expected = Vec::new();
        for f in [1.0_f32, -2.5_f32, 0.25_f32] {
            expected.extend_from_slice(&f.to_le_bytes());
        }
        let want = base64::engine::general_purpose::STANDARD.encode(expected);
        assert_eq!(got, want);
    }
}
