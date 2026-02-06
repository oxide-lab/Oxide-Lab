use super::types::{EngineId, EngineSessionInfo, EngineSessionKind, ResolvedModelSource};
use crate::core::types::{GenerateRequest, LlamaRuntimeConfig, LoadRequest};

#[async_trait::async_trait]
pub trait EngineAdapter: Send + Sync {
    fn id(&self) -> EngineId;

    fn resolve_model_source(&self, req: &LoadRequest) -> Result<ResolvedModelSource, String>;

    fn start_session(
        &self,
        kind: EngineSessionKind,
        source: &ResolvedModelSource,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String>;

    async fn ensure_health(
        &self,
        session: EngineSessionInfo,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String>;

    fn stop_session(
        &self,
        model_id: Option<&str>,
        kind: Option<EngineSessionKind>,
    ) -> Result<(), String>;

    async fn chat_stream(
        &self,
        app: &tauri::AppHandle,
        session: &EngineSessionInfo,
        req: GenerateRequest,
    ) -> Result<(), String>;

    async fn embeddings(
        &self,
        session: &EngineSessionInfo,
        model: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, String>;
}
