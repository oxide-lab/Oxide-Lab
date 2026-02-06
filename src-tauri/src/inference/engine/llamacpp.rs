use super::adapter::EngineAdapter;
use super::types::{EngineId, EngineSessionInfo, EngineSessionKind, ResolvedModelSource};
use crate::core::types::{GenerateRequest, LlamaRuntimeConfig, LoadRequest, LlamaSessionKind};
use crate::inference::llamacpp::{http_client, process, state::LlamaCppState};
use std::path::Path;
use std::time::Duration;

#[derive(Clone)]
pub struct LlamaCppAdapter {
    state: LlamaCppState,
}

impl LlamaCppAdapter {
    pub fn new(state: LlamaCppState) -> Self {
        Self { state }
    }

    fn model_id_from_path(path: &str) -> String {
        Path::new(path)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "loaded-model".to_string())
    }

    fn to_llama_kind(kind: EngineSessionKind) -> LlamaSessionKind {
        match kind {
            EngineSessionKind::Chat => LlamaSessionKind::Chat,
            EngineSessionKind::Embedding => LlamaSessionKind::Embedding,
        }
    }

    fn to_engine_kind(kind: LlamaSessionKind) -> EngineSessionKind {
        match kind {
            LlamaSessionKind::Chat => EngineSessionKind::Chat,
            LlamaSessionKind::Embedding => EngineSessionKind::Embedding,
        }
    }

    fn to_engine_session(info: crate::inference::llamacpp::state::SessionInfo) -> EngineSessionInfo {
        EngineSessionInfo {
            engine_id: EngineId::Llamacpp,
            model_id: info.model_id,
            model_path: info.model_path,
            pid: info.pid,
            port: info.port,
            api_key: info.api_key,
            kind: Self::to_engine_kind(info.kind),
            created_at: info.created_at,
            last_health_ok_at: info.last_health_ok_at,
        }
    }

    fn to_llama_session(info: &EngineSessionInfo) -> crate::inference::llamacpp::state::SessionInfo {
        crate::inference::llamacpp::state::SessionInfo {
            pid: info.pid,
            port: info.port,
            model_id: info.model_id.clone(),
            model_path: info.model_path.clone(),
            api_key: info.api_key.clone(),
            kind: Self::to_llama_kind(info.kind),
            created_at: info.created_at,
            last_health_ok_at: info.last_health_ok_at,
        }
    }
}

#[async_trait::async_trait]
impl EngineAdapter for LlamaCppAdapter {
    fn id(&self) -> EngineId {
        EngineId::Llamacpp
    }

    fn resolve_model_source(&self, req: &LoadRequest) -> Result<ResolvedModelSource, String> {
        match req {
            LoadRequest::Gguf {
                model_path,
                context_length,
                ..
            } => Ok(ResolvedModelSource {
                model_id: Self::model_id_from_path(model_path),
                model_path: model_path.clone(),
                context_length: *context_length,
            }),
            LoadRequest::HubGguf {
                repo_id,
                revision,
                filename,
                context_length,
                ..
            } => {
                let revision = revision.clone().unwrap_or_else(|| "main".to_string());
                let api = hf_hub::api::sync::Api::new().map_err(|e| e.to_string())?;
                let repo =
                    hf_hub::Repo::with_revision(repo_id.clone(), hf_hub::RepoType::Model, revision);
                let path = api
                    .repo(repo)
                    .get(filename)
                    .map_err(|e| format!("hf_hub get {} failed: {}", filename, e))?;
                Ok(ResolvedModelSource {
                    model_id: repo_id.clone(),
                    model_path: path.to_string_lossy().to_string(),
                    context_length: *context_length,
                })
            }
            LoadRequest::HubSafetensors { .. } | LoadRequest::LocalSafetensors { .. } => {
                Err("unsupported format: only gguf and hub_gguf are supported".to_string())
            }
        }
    }

    fn start_session(
        &self,
        kind: EngineSessionKind,
        source: &ResolvedModelSource,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String> {
        let info = process::get_or_start_session(
            &self.state,
            &source.model_id,
            &source.model_path,
            Self::to_llama_kind(kind),
            runtime_cfg,
            Duration::from_secs(120),
        )?;
        Ok(Self::to_engine_session(info))
    }

    async fn ensure_health(
        &self,
        session: EngineSessionInfo,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String> {
        let llama_session = Self::to_llama_session(&session);
        if http_client::health_check(&llama_session).await {
            process::mark_session_health_ok(
                &self.state,
                &session.model_id,
                Self::to_llama_kind(session.kind),
            )?;
            let refreshed = process::find_session_info(
                &self.state,
                &session.model_id,
                Self::to_llama_kind(session.kind),
            )?
            .unwrap_or(llama_session);
            return Ok(Self::to_engine_session(refreshed));
        }

        process::unload_session(
            &self.state,
            &session.model_id,
            Self::to_llama_kind(session.kind),
        )?;
        let restarted = process::get_or_start_session(
            &self.state,
            &session.model_id,
            &session.model_path,
            Self::to_llama_kind(session.kind),
            runtime_cfg,
            Duration::from_secs(120),
        )?;
        Ok(Self::to_engine_session(restarted))
    }

    fn stop_session(
        &self,
        model_id: Option<&str>,
        kind: Option<EngineSessionKind>,
    ) -> Result<(), String> {
        match model_id {
            None => process::unload_all_sessions(&self.state),
            Some(id) => match kind {
                Some(k) => process::unload_session(&self.state, id, Self::to_llama_kind(k)),
                None => process::unload_model_sessions(&self.state, id),
            },
        }
    }

    async fn chat_stream(
        &self,
        app: &tauri::AppHandle,
        session: &EngineSessionInfo,
        req: GenerateRequest,
    ) -> Result<(), String> {
        let info = Self::to_llama_session(session);
        http_client::stream_chat_completion(app, &info, req).await
    }

    async fn embeddings(
        &self,
        session: &EngineSessionInfo,
        model: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let info = Self::to_llama_session(session);
        http_client::create_embeddings(&info, model, input).await
    }
}
