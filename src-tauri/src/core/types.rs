use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StreamMessage {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thinking: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub content: String,
}

impl StreamMessage {
    pub fn is_empty(&self) -> bool {
        self.thinking.is_empty() && self.content.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum DevicePreference {
    Auto,
    Cpu,
    Cuda { index: usize },
    Metal,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackendPreference {
    #[default]
    Auto,
    Llamacpp,
    Candle,
}

impl BackendPreference {
    pub fn normalized(self) -> Self {
        BackendPreference::Llamacpp
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ActiveBackend {
    #[default]
    None,
    Llamacpp,
    Candle,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum LlamaSessionKind {
    Chat,
    Embedding,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum LlamaEmbeddingsStrategy {
    #[default]
    SeparateSession,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaSchedulerConfig {
    #[serde(default = "default_keep_alive_secs")]
    pub keep_alive_secs: u64,
    #[serde(default = "default_max_loaded_models")]
    pub max_loaded_models: u32,
    #[serde(default = "default_max_queue")]
    pub max_queue: u32,
    #[serde(default = "default_queue_wait_timeout_ms")]
    pub queue_wait_timeout_ms: u64,
    #[serde(default = "default_vram_recovery_timeout_ms")]
    pub vram_recovery_timeout_ms: u64,
    #[serde(default = "default_vram_recovery_poll_ms")]
    pub vram_recovery_poll_ms: u64,
    #[serde(default = "default_vram_recovery_threshold")]
    pub vram_recovery_threshold: f32,
    #[serde(default = "default_expiration_tick_ms")]
    pub expiration_tick_ms: u64,
}

impl Default for LlamaSchedulerConfig {
    fn default() -> Self {
        Self {
            keep_alive_secs: default_keep_alive_secs(),
            max_loaded_models: default_max_loaded_models(),
            max_queue: default_max_queue(),
            queue_wait_timeout_ms: default_queue_wait_timeout_ms(),
            vram_recovery_timeout_ms: default_vram_recovery_timeout_ms(),
            vram_recovery_poll_ms: default_vram_recovery_poll_ms(),
            vram_recovery_threshold: default_vram_recovery_threshold(),
            expiration_tick_ms: default_expiration_tick_ms(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaRuntimeConfig {
    #[serde(default)]
    pub server_path: Option<String>,
    #[serde(default)]
    pub selected_backend: Option<String>,
    #[serde(default = "default_ngl")]
    pub n_gpu_layers: i32,
    #[serde(default = "default_threads")]
    pub threads: i32,
    #[serde(default)]
    pub threads_batch: i32,
    #[serde(default = "default_ctx_size")]
    pub ctx_size: i32,
    #[serde(default = "default_batch_size")]
    pub batch_size: i32,
    #[serde(default = "default_ubatch_size")]
    pub ubatch_size: i32,
    #[serde(default = "default_n_predict")]
    pub n_predict: i32,
    #[serde(default = "default_flash_attn")]
    pub flash_attn: String,
    #[serde(default)]
    pub extra_env: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub embeddings_strategy: LlamaEmbeddingsStrategy,
    #[serde(default)]
    pub scheduler: LlamaSchedulerConfig,
}

const fn default_ngl() -> i32 {
    100
}
const fn default_threads() -> i32 {
    0
}
const fn default_ctx_size() -> i32 {
    4096
}
const fn default_batch_size() -> i32 {
    512
}
const fn default_ubatch_size() -> i32 {
    512
}
const fn default_n_predict() -> i32 {
    -1
}
fn default_flash_attn() -> String {
    "auto".to_string()
}
const fn default_keep_alive_secs() -> u64 {
    300
}
const fn default_max_loaded_models() -> u32 {
    0
}
const fn default_max_queue() -> u32 {
    128
}
const fn default_queue_wait_timeout_ms() -> u64 {
    15_000
}
const fn default_vram_recovery_timeout_ms() -> u64 {
    5_000
}
const fn default_vram_recovery_poll_ms() -> u64 {
    250
}
const fn default_vram_recovery_threshold() -> f32 {
    0.75
}
const fn default_expiration_tick_ms() -> u64 {
    1_000
}

impl Default for LlamaRuntimeConfig {
    fn default() -> Self {
        Self {
            server_path: None,
            selected_backend: None,
            n_gpu_layers: default_ngl(),
            threads: default_threads(),
            threads_batch: 0,
            ctx_size: default_ctx_size(),
            batch_size: default_batch_size(),
            ubatch_size: default_ubatch_size(),
            n_predict: default_n_predict(),
            flash_attn: default_flash_attn(),
            extra_env: std::collections::HashMap::new(),
            embeddings_strategy: LlamaEmbeddingsStrategy::SeparateSession,
            scheduler: LlamaSchedulerConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlamaSessionSnapshot {
    pub pid: i32,
    pub port: u16,
    pub model_id: String,
    pub api_key: String,
    pub kind: LlamaSessionKind,
    pub created_at: u64,
    pub last_health_ok_at: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "format", rename_all = "lowercase")]
pub enum LoadRequest {
    Gguf {
        model_path: String,
        tokenizer_path: Option<String>,
        context_length: usize,
        device: Option<DevicePreference>,
    },
    #[serde(rename = "hub_gguf")]
    HubGguf {
        repo_id: String,
        revision: Option<String>,
        filename: String,
        context_length: usize,
        device: Option<DevicePreference>,
    },
    #[serde(rename = "hub_safetensors")]
    HubSafetensors {
        repo_id: String,
        revision: Option<String>,
        context_length: usize,
        device: Option<DevicePreference>,
    },
    #[serde(rename = "local_safetensors")]
    LocalSafetensors {
        model_path: String,
        context_length: usize,
        device: Option<DevicePreference>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateRequest {
    pub prompt: String,
    #[serde(default)]
    pub messages: Option<Vec<ChatMessage>>,
    #[serde(default)]
    pub attachments: Option<Vec<Attachment>>,
    #[serde(default)]
    pub max_new_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<usize>,
    pub min_p: Option<f64>,
    pub repeat_penalty: Option<f32>,
    pub repeat_last_n: usize,
    #[serde(default)]
    pub use_custom_params: bool,
    #[serde(default)]
    pub seed: Option<u64>,
    #[serde(default)]
    pub split_prompt: Option<bool>,
    #[serde(default)]
    pub verbose_prompt: Option<bool>,
    #[serde(default)]
    pub tracing: Option<bool>,
    #[serde(default)]
    pub edit_index: Option<usize>,
    #[serde(default)]
    pub format: Option<crate::generate::grammar::OutputFormat>,
    #[serde(default)]
    pub tools: Option<Vec<crate::generate::tool_call_parser::Tool>>,
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(default)]
    pub tool_choice: Option<ToolChoice>,
    #[serde(default)]
    pub retrieval: Option<crate::retrieval::types::RetrievalRequest>,
    #[serde(default)]
    pub mcp: Option<crate::retrieval::types::McpRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Mode(String),
    Function {
        r#type: String,
        function: ToolChoiceFunction,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolChoiceFunction {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub kind: Option<String>,
    pub mime: Option<String>,
    pub name: Option<String>,
    pub path: Option<String>,
    pub bytes_b64: Option<String>,
}
