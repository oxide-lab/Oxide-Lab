use super::adapter::EngineAdapter;
use super::types::{EngineId, EngineSessionInfo, EngineSessionKind, ResolvedModelSource};
use crate::core::types::{GenerateRequest, LlamaRuntimeConfig, LlamaSessionKind, LoadRequest};
use crate::inference::llamacpp::{http_client, state::LlamaCppState};
use oxide_llamacpp::args::LlamacppConfig;
use oxide_llamacpp::commands;
use oxide_llamacpp::state::SessionInfo as PluginSessionInfo;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tauri::Manager;

const START_RETRIES: usize = 5;
const DEFAULT_START_TIMEOUT_SECS: u64 = 120;

#[derive(Clone)]
pub struct LlamaCppAdapter {
    app_handle: tauri::AppHandle,
    state: LlamaCppState,
}

impl LlamaCppAdapter {
    pub fn new(app_handle: tauri::AppHandle, state: LlamaCppState) -> Self {
        Self { app_handle, state }
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

    fn is_embedding(kind: EngineSessionKind) -> bool {
        matches!(kind, EngineSessionKind::Embedding)
    }

    fn to_port(port: i32) -> Result<u16, String> {
        u16::try_from(port).map_err(|_| format!("invalid llama-server port: {}", port))
    }

    fn now_unix() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    }

    fn to_engine_session(
        &self,
        kind: EngineSessionKind,
        info: PluginSessionInfo,
    ) -> Result<EngineSessionInfo, String> {
        let llama_kind = Self::to_llama_kind(kind);
        let meta = self.state.ensure_meta(&info.model_id, llama_kind)?;
        Ok(EngineSessionInfo {
            engine_id: EngineId::Llamacpp,
            model_id: info.model_id,
            model_path: info.model_path,
            pid: info.pid,
            port: Self::to_port(info.port)?,
            api_key: info.api_key,
            kind,
            created_at: meta.created_at,
            last_health_ok_at: meta.last_health_ok_at,
        })
    }

    async fn get_all_plugin_sessions(&self) -> Result<Vec<PluginSessionInfo>, String> {
        commands::get_all_sessions(self.app_handle.clone())
            .await
            .map_err(|e| e.to_string())
    }

    async fn find_plugin_session(
        &self,
        model_id: &str,
        kind: EngineSessionKind,
    ) -> Result<Option<PluginSessionInfo>, String> {
        let is_embedding = Self::is_embedding(kind);
        let sessions = self.get_all_plugin_sessions().await?;
        Ok(sessions
            .into_iter()
            .find(|s| s.model_id == model_id && s.is_embedding == is_embedding))
    }

    fn resolve_binary_from_path(value: &str) -> Option<PathBuf> {
        let path = PathBuf::from(value);
        if path.exists() && path.is_file() {
            Some(path)
        } else {
            None
        }
    }

    fn candidate_binary_names() -> &'static [&'static str] {
        if cfg!(windows) {
            &["llama-server.exe", "llama-server"]
        } else {
            &["llama-server"]
        }
    }

    fn bin_roots(&self) -> Vec<PathBuf> {
        let mut roots = Vec::new();

        if let Ok(root_var) = env::var("OXIDE_LLAMA_BIN_ROOT") {
            for p in env::split_paths(&root_var) {
                if p.exists() && p.is_dir() {
                    roots.push(p);
                }
            }
        }

        if let Ok(cwd) = env::current_dir() {
            let p = cwd.join("example").join("bin");
            if p.exists() && p.is_dir() {
                roots.push(p);
            }
        }

        if let Ok(resource_dir) = self.app_handle.path().resource_dir() {
            let candidate_a = resource_dir.join("example").join("bin");
            if candidate_a.exists() && candidate_a.is_dir() {
                roots.push(candidate_a);
            }
            let candidate_b = resource_dir.join("bin");
            if candidate_b.exists() && candidate_b.is_dir() {
                roots.push(candidate_b);
            }
        }

        let repo_bin = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("example")
            .join("bin");
        if repo_bin.exists() && repo_bin.is_dir() {
            roots.push(repo_bin);
        }

        let mut uniq = Vec::new();
        for r in roots {
            if !uniq.contains(&r) {
                uniq.push(r);
            }
        }
        uniq
    }

    fn binary_score(runtime_cfg: &LlamaRuntimeConfig, dir_name: &str) -> i32 {
        let name = dir_name.to_ascii_lowercase();
        let prefers_gpu = runtime_cfg.n_gpu_layers > 0;
        if name.contains("cuda") {
            if prefers_gpu { 300 } else { 120 }
        } else if name.contains("vulkan") {
            if prefers_gpu { 260 } else { 170 }
        } else if name.contains("cpu") {
            if prefers_gpu { 140 } else { 280 }
        } else {
            100
        }
    }

    fn find_bundled_binaries(&self, runtime_cfg: &LlamaRuntimeConfig) -> Vec<PathBuf> {
        let mut ranked: Vec<(i32, PathBuf)> = Vec::new();

        for root in self.bin_roots() {
            for bin in Self::candidate_binary_names() {
                let direct = root.join(bin);
                if direct.exists() && direct.is_file() {
                    ranked.push((900, direct));
                }
            }

            let entries = match std::fs::read_dir(&root) {
                Ok(v) => v,
                Err(_) => continue,
            };

            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let dir_name = match path.file_name().and_then(|v| v.to_str()) {
                    Some(v) => v,
                    None => continue,
                };
                let score = Self::binary_score(runtime_cfg, dir_name);
                for bin in Self::candidate_binary_names() {
                    let candidate = path.join(bin);
                    if candidate.exists() && candidate.is_file() {
                        ranked.push((1000 + score, candidate));
                    }
                }
            }
        }

        ranked.sort_by(|(sa, pa), (sb, pb)| sb.cmp(sa).then_with(|| pa.cmp(pb)));
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for (_, p) in ranked {
            let k = p.to_string_lossy().to_ascii_lowercase();
            if seen.insert(k) {
                out.push(p);
            }
        }
        out
    }

    fn find_path_binary() -> Option<PathBuf> {
        let path_var = env::var_os("PATH")?;
        for dir in env::split_paths(&path_var) {
            for bin in Self::candidate_binary_names() {
                let candidate = dir.join(bin);
                if candidate.exists() && candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        None
    }

    fn resolve_server_binary(&self, runtime_cfg: &LlamaRuntimeConfig) -> Result<String, String> {
        if let Some(explicit) = runtime_cfg
            .server_path
            .as_ref()
            .and_then(|v| Self::resolve_binary_from_path(v))
        {
            return Ok(explicit.to_string_lossy().to_string());
        }

        if let Ok(v) = env::var("OXIDE_LLAMA_SERVER_PATH")
            && let Some(env_path) = Self::resolve_binary_from_path(&v)
        {
            return Ok(env_path.to_string_lossy().to_string());
        }

        if let Some(path) = self.find_bundled_binaries(runtime_cfg).into_iter().next() {
            return Ok(path.to_string_lossy().to_string());
        }

        if let Some(path) = Self::find_path_binary() {
            return Ok(path.to_string_lossy().to_string());
        }

        Err(
            "llama-server binary not found (runtime config, env, resources, example/bin, PATH)"
                .to_string(),
        )
    }

    fn build_config(runtime_cfg: &LlamaRuntimeConfig) -> LlamacppConfig {
        LlamacppConfig {
            version_backend: runtime_cfg
                .selected_backend
                .clone()
                .unwrap_or_else(|| "v1.0/standard".to_string()),
            auto_update_engine: false,
            auto_unload: false,
            timeout: DEFAULT_START_TIMEOUT_SECS as i32,
            llamacpp_env: String::new(),
            memory_util: String::new(),
            chat_template: String::new(),
            n_gpu_layers: runtime_cfg.n_gpu_layers,
            offload_mmproj: true,
            cpu_moe: false,
            n_cpu_moe: 0,
            override_tensor_buffer_t: String::new(),
            ctx_size: runtime_cfg.ctx_size,
            threads: runtime_cfg.threads,
            threads_batch: runtime_cfg.threads_batch,
            n_predict: runtime_cfg.n_predict,
            batch_size: runtime_cfg.batch_size,
            ubatch_size: runtime_cfg.ubatch_size,
            device: String::new(),
            split_mode: "layer".to_string(),
            main_gpu: 0,
            flash_attn: runtime_cfg.flash_attn.clone(),
            cont_batching: true,
            no_mmap: false,
            mlock: false,
            no_kv_offload: false,
            cache_type_k: "f16".to_string(),
            cache_type_v: "f16".to_string(),
            defrag_thold: 0.1,
            rope_scaling: "none".to_string(),
            rope_scale: 1.0,
            rope_freq_base: 0.0,
            rope_freq_scale: 1.0,
            ctx_shift: false,
        }
    }

    fn api_key(model_id: &str, kind: EngineSessionKind) -> String {
        let suffix = match kind {
            EngineSessionKind::Chat => "chat",
            EngineSessionKind::Embedding => "embedding",
        };
        format!(
            "oxide-{}-{}-{}",
            model_id.replace('/', "_"),
            suffix,
            Self::now_unix()
        )
    }

    async fn start_new_session(
        &self,
        kind: EngineSessionKind,
        source: &ResolvedModelSource,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<PluginSessionInfo, String> {
        let backend_path = self.resolve_server_binary(runtime_cfg)?;
        let config = Self::build_config(runtime_cfg);
        let timeout = Duration::from_secs(DEFAULT_START_TIMEOUT_SECS).as_secs();
        let is_embedding = Self::is_embedding(kind);

        let mut last_err = String::new();
        for _ in 0..START_RETRIES {
            let port = commands::get_random_port(self.app_handle.clone())
                .await
                .map_err(|e| e.to_string())?;

            let api_key = Self::api_key(&source.model_id, kind);
            let mut envs: HashMap<String, String> = runtime_cfg.extra_env.clone();
            envs.insert("LLAMA_API_KEY".to_string(), api_key);

            match commands::load_llama_model(
                self.app_handle.clone(),
                &backend_path,
                source.model_id.clone(),
                source.model_path.clone(),
                port,
                config.clone(),
                envs,
                None,
                is_embedding,
                timeout,
            )
            .await
            {
                Ok(session) => return Ok(session),
                Err(err) => {
                    last_err = err.to_string();
                    if !(last_err.contains("address already in use")
                        || last_err.contains("in use")
                        || last_err.contains("bind"))
                    {
                        break;
                    }
                }
            }
        }

        Err(format!(
            "failed to start llama.cpp {} session: {}",
            if is_embedding { "embedding" } else { "chat" },
            last_err
        ))
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

    async fn start_session(
        &self,
        kind: EngineSessionKind,
        source: &ResolvedModelSource,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String> {
        if let Some(existing) = self.find_plugin_session(&source.model_id, kind).await? {
            return self.to_engine_session(kind, existing);
        }

        let created = self.start_new_session(kind, source, runtime_cfg).await?;
        self.to_engine_session(kind, created)
    }

    async fn ensure_health(
        &self,
        session: EngineSessionInfo,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String> {
        if http_client::health_check(&session).await {
            self.state
                .mark_health_ok(&session.model_id, Self::to_llama_kind(session.kind))?;
            let meta = self
                .state
                .ensure_meta(&session.model_id, Self::to_llama_kind(session.kind))?;
            return Ok(EngineSessionInfo {
                last_health_ok_at: meta.last_health_ok_at,
                ..session
            });
        }

        self.stop_session(Some(&session.model_id), Some(session.kind))
            .await?;
        let source = ResolvedModelSource {
            model_id: session.model_id.clone(),
            model_path: session.model_path.clone(),
            context_length: runtime_cfg.ctx_size.max(1) as usize,
        };
        self.start_session(session.kind, &source, runtime_cfg).await
    }

    async fn stop_session(
        &self,
        model_id: Option<&str>,
        kind: Option<EngineSessionKind>,
    ) -> Result<(), String> {
        let sessions = self.get_all_plugin_sessions().await?;
        for session in sessions {
            if let Some(id) = model_id
                && session.model_id != id
            {
                continue;
            }
            if let Some(k) = kind
                && session.is_embedding != Self::is_embedding(k)
            {
                continue;
            }
            commands::unload_llama_model(self.app_handle.clone(), session.pid)
                .await
                .map_err(|e| e.to_string())?;
        }

        match (model_id, kind) {
            (Some(id), Some(k)) => self.state.remove_session(id, Self::to_llama_kind(k))?,
            (Some(id), None) => self.state.remove_model(id)?,
            (None, _) => self.state.clear()?,
        }
        Ok(())
    }

    async fn chat_stream(
        &self,
        app: &tauri::AppHandle,
        session: &EngineSessionInfo,
        req: GenerateRequest,
    ) -> Result<(), String> {
        http_client::stream_chat_completion(app, session, req).await
    }

    async fn embeddings(
        &self,
        session: &EngineSessionInfo,
        model: &str,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        http_client::create_embeddings(session, model, input).await
    }
}
