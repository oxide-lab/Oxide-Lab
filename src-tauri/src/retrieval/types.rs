use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetrievalWebRequest {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetrievalLocalRequest {
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetrievalRequest {
    #[serde(default)]
    pub web: Option<RetrievalWebRequest>,
    #[serde(default)]
    pub local: Option<RetrievalLocalRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpRequest {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub max_tool_rounds: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalSource {
    pub source_type: String,
    pub title: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub snippet: String,
    #[serde(default)]
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetrievalContextEvent {
    pub sources: Vec<RetrievalSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RetrievalUrlCandidatesEvent {
    pub urls: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalWarningEvent {
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct RetrievalCandidate {
    pub source: RetrievalSource,
    pub estimated_tokens: usize,
}
