use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EngineId {
    Llamacpp,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum EngineSessionKind {
    Chat,
    Embedding,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineSessionInfo {
    pub engine_id: EngineId,
    pub model_id: String,
    pub model_path: String,
    pub pid: i32,
    pub port: u16,
    pub api_key: String,
    pub kind: EngineSessionKind,
    pub created_at: u64,
    pub last_health_ok_at: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct ResolvedModelSource {
    pub model_id: String,
    pub model_path: String,
    pub context_length: usize,
}
