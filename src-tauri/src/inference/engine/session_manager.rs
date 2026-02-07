use super::registry::EngineRegistry;
use super::types::{EngineId, EngineSessionInfo, EngineSessionKind, ResolvedModelSource};
use crate::core::types::{GenerateRequest, LlamaRuntimeConfig, LoadRequest};

#[derive(Clone)]
pub struct EngineSessionManager {
    registry: EngineRegistry,
    engine_id: EngineId,
}

impl EngineSessionManager {
    pub fn new(registry: EngineRegistry, engine_id: EngineId) -> Self {
        Self {
            registry,
            engine_id,
        }
    }

    fn adapter(&self) -> Result<std::sync::Arc<dyn super::adapter::EngineAdapter>, String> {
        self.registry
            .get(self.engine_id)
            .ok_or_else(|| format!("Engine adapter not registered: {:?}", self.engine_id))
    }

    pub fn resolve_model_source(&self, req: &LoadRequest) -> Result<ResolvedModelSource, String> {
        self.adapter()?.resolve_model_source(req)
    }

    pub async fn start_session(
        &self,
        kind: EngineSessionKind,
        source: &ResolvedModelSource,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String> {
        self.adapter()?
            .start_session(kind, source, runtime_cfg)
            .await
    }

    pub async fn ensure_health(
        &self,
        session: EngineSessionInfo,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String> {
        self.adapter()?.ensure_health(session, runtime_cfg).await
    }

    pub async fn stop_model_sessions(&self, model_id: &str) -> Result<(), String> {
        self.adapter()?.stop_session(Some(model_id), None).await
    }

    pub async fn stop_session_kind(
        &self,
        model_id: &str,
        kind: EngineSessionKind,
    ) -> Result<(), String> {
        self.adapter()?
            .stop_session(Some(model_id), Some(kind))
            .await
    }

    pub async fn stop_all_sessions(&self, model_id: Option<&str>) -> Result<(), String> {
        self.adapter()?.stop_session(model_id, None).await
    }

    pub async fn chat_stream(
        &self,
        app: &tauri::AppHandle,
        session: &EngineSessionInfo,
        req: GenerateRequest,
    ) -> Result<(), String> {
        self.adapter()?.chat_stream(app, session, req).await
    }

    pub async fn embeddings(
        &self,
        session: &EngineSessionInfo,
        model: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        self.adapter()?.embeddings(session, model, input).await
    }
}
