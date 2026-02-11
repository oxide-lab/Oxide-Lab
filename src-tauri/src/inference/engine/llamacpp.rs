use super::adapter::EngineAdapter;
use super::types::{EngineId, EngineSessionInfo, EngineSessionKind, ResolvedModelSource};
use crate::core::settings_v2::SettingsV2State;
use crate::core::types::{GenerateRequest, LlamaRuntimeConfig, LlamaSessionKind, LoadRequest};
use crate::inference::llamacpp::{http_client, state::LlamaCppState};
use oxide_llamacpp::args::LlamacppConfig;
use oxide_llamacpp::commands;
use oxide_llamacpp::gguf::model_planner::{self, ModelMode, ModelPlan};
use oxide_llamacpp::state::SessionInfo as PluginSessionInfo;
use std::collections::HashMap;
use std::env;
use std::fs;
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
            mmproj_path: info.mmproj_path,
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

    fn parse_version_backend(value: &str) -> Option<(&str, &str)> {
        let (version, backend) = value.split_once('/')?;
        let version = version.trim();
        let backend = backend.trim();
        if version.is_empty() || backend.is_empty() {
            return None;
        }
        Some((version, backend))
    }

    fn normalize_backend_name(value: &str) -> String {
        let normalized = value.to_ascii_lowercase();

        if normalized.contains("win-cpu-x64") {
            return "win-common_cpus-x64".to_string();
        }
        if normalized.contains("win-cpu-arm64") {
            return "win-arm64".to_string();
        }
        if normalized.contains("win-vulkan-x64") {
            return "win-vulkan-common_cpus-x64".to_string();
        }
        if normalized.contains("win-cuda-11") {
            return "win-cuda-11-common_cpus-x64".to_string();
        }
        if normalized.contains("win-cuda-12") {
            return "win-cuda-12-common_cpus-x64".to_string();
        }
        if normalized.contains("win-cuda-13") {
            return "win-cuda-13-common_cpus-x64".to_string();
        }
        if normalized == "ubuntu-x64" || normalized == "linux-x64" {
            return "linux-common_cpus-x64".to_string();
        }
        if normalized.contains("ubuntu-vulkan-x64") || normalized.contains("linux-vulkan-x64") {
            return "linux-vulkan-common_cpus-x64".to_string();
        }
        if normalized.contains("ubuntu-cuda-11") || normalized.contains("linux-cuda-11") {
            return "linux-cuda-11-common_cpus-x64".to_string();
        }
        if normalized.contains("ubuntu-cuda-12") || normalized.contains("linux-cuda-12") {
            return "linux-cuda-12-common_cpus-x64".to_string();
        }
        if normalized.contains("ubuntu-cuda-13") || normalized.contains("linux-cuda-13") {
            return "linux-cuda-13-common_cpus-x64".to_string();
        }
        if normalized == "macos-x64" || normalized == "darwin-x64" {
            return "macos-x64".to_string();
        }
        if normalized == "macos-arm64" || normalized == "darwin-arm64" {
            return "macos-arm64".to_string();
        }

        normalized
    }

    fn is_build_version(version: &str) -> bool {
        version.starts_with('b')
            && version.len() > 1
            && version[1..].chars().all(|ch| ch.is_ascii_digit())
    }

    fn infer_backend_from_server_path(server_path: &str) -> Option<String> {
        let normalized = server_path.replace('\\', "/");
        let segments: Vec<&str> = normalized
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        if segments.is_empty() {
            return None;
        }

        let file_name = segments.last()?.to_ascii_lowercase();
        if file_name != "llama-server" && file_name != "llama-server.exe" {
            return None;
        }

        if segments.len() >= 2 {
            let bundle_dir = segments[segments.len() - 2];
            let lower_bundle_dir = bundle_dir.to_ascii_lowercase();
            if lower_bundle_dir.starts_with("llama-")
                && let Some(rest) = bundle_dir.strip_prefix("llama-")
                && let Some((version, backend)) = rest.split_once("-bin-")
                && Self::is_build_version(version)
            {
                return Some(format!("{}/{}", version, backend));
            }
        }

        if segments.len() >= 5 {
            let maybe_bin = segments[segments.len() - 2];
            let maybe_build = segments[segments.len() - 3];
            if maybe_bin.eq_ignore_ascii_case("bin") && maybe_build.eq_ignore_ascii_case("build") {
                let backend = segments[segments.len() - 4];
                let version = segments[segments.len() - 5];
                if Self::is_build_version(version) {
                    return Some(format!("{}/{}", version, backend));
                }
            }
        }

        if segments.len() >= 3 {
            let backend = segments[segments.len() - 2];
            let version = segments[segments.len() - 3];
            if Self::is_build_version(version) {
                return Some(format!("{}/{}", version, backend));
            }
        }

        None
    }

    fn backend_selection_matches(inferred_backend: &str, selected_backend: &str) -> bool {
        let Some((inferred_version, inferred_name)) = Self::parse_version_backend(inferred_backend)
        else {
            return false;
        };
        let Some((selected_version, selected_name)) = Self::parse_version_backend(selected_backend)
        else {
            return false;
        };

        inferred_version.eq_ignore_ascii_case(selected_version)
            && Self::normalize_backend_name(inferred_name)
                .eq_ignore_ascii_case(&Self::normalize_backend_name(selected_name))
    }

    fn should_use_explicit_server_path(
        explicit_path: &str,
        selected_backend: Option<&str>,
    ) -> bool {
        let Some(selected_backend) = selected_backend else {
            return true;
        };
        let Some(inferred_backend) = Self::infer_backend_from_server_path(explicit_path) else {
            return true;
        };
        Self::backend_selection_matches(&inferred_backend, selected_backend)
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
        if let Some((_, selected_backend)) = runtime_cfg
            .selected_backend
            .as_deref()
            .and_then(Self::parse_version_backend)
            && name.contains(&selected_backend.to_ascii_lowercase())
        {
            return 450;
        }
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

    fn resolve_selected_backend_binary(
        &self,
        runtime_cfg: &LlamaRuntimeConfig,
        selected_backend: &str,
    ) -> Option<PathBuf> {
        if let Some((version, backend)) = Self::parse_version_backend(selected_backend)
            && let Some(path) = self.resolve_installed_backend_binary_path(version, backend)
        {
            return Some(path);
        }

        self.find_bundled_binaries(runtime_cfg)
            .into_iter()
            .find(|path| {
                let candidate = path.to_string_lossy();
                let Some(inferred) = Self::infer_backend_from_server_path(&candidate) else {
                    return false;
                };
                Self::backend_selection_matches(&inferred, selected_backend)
            })
    }

    fn resolve_installed_backend_binary_path(
        &self,
        version: &str,
        backend: &str,
    ) -> Option<PathBuf> {
        let root = self
            .app_handle
            .path()
            .app_local_data_dir()
            .ok()?
            .join("oxide-lab")
            .join("llamacpp")
            .join("backends")
            .join(version)
            .join(backend);

        let exe_name = if cfg!(windows) {
            "llama-server.exe"
        } else {
            "llama-server"
        };

        let build = root.join("build").join("bin").join(exe_name);
        if build.exists() && build.is_file() {
            return Some(build);
        }

        let direct = root.join(exe_name);
        if direct.exists() && direct.is_file() {
            return Some(direct);
        }

        None
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
            && Self::should_use_explicit_server_path(
                &explicit.to_string_lossy(),
                runtime_cfg.selected_backend.as_deref(),
            )
        {
            return Ok(explicit.to_string_lossy().to_string());
        }

        if let Ok(v) = env::var("OXIDE_LLAMA_SERVER_PATH")
            && let Some(env_path) = Self::resolve_binary_from_path(&v)
        {
            return Ok(env_path.to_string_lossy().to_string());
        }

        if let Some(selected_backend) = runtime_cfg.selected_backend.as_deref()
            && let Some(path) = self.resolve_selected_backend_binary(runtime_cfg, selected_backend)
        {
            return Ok(path.to_string_lossy().to_string());
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
            offload_mmproj: runtime_cfg.offload_mmproj,
            cpu_moe: runtime_cfg.cpu_moe,
            n_cpu_moe: runtime_cfg.n_cpu_moe,
            override_tensor_buffer_t: runtime_cfg.override_tensor_buffer_t.clone(),
            ctx_size: runtime_cfg.ctx_size,
            threads: runtime_cfg.threads,
            threads_batch: runtime_cfg.threads_batch,
            n_predict: runtime_cfg.n_predict,
            batch_size: runtime_cfg.batch_size,
            ubatch_size: runtime_cfg.ubatch_size,
            device: String::new(),
            split_mode: runtime_cfg.split_mode.clone(),
            main_gpu: runtime_cfg.main_gpu,
            n_parallel: runtime_cfg.n_parallel,
            flash_attn: runtime_cfg.flash_attn.clone(),
            cont_batching: runtime_cfg.cont_batching,
            no_mmap: Self::should_disable_mmap(&runtime_cfg.extra_env),
            mlock: runtime_cfg.mlock,
            no_kv_offload: runtime_cfg.no_kv_offload,
            cache_type_k: runtime_cfg.cache_type_k.clone(),
            cache_type_v: runtime_cfg.cache_type_v.clone(),
            defrag_thold: runtime_cfg.defrag_thold,
            rope_scaling: runtime_cfg.rope_scaling.clone(),
            rope_scale: runtime_cfg.rope_scale,
            rope_freq_base: runtime_cfg.rope_freq_base,
            rope_freq_scale: runtime_cfg.rope_freq_scale,
            ctx_shift: runtime_cfg.ctx_shift,
        }
    }

    fn current_memory_mode(&self) -> String {
        if let Some(state) = self.app_handle.try_state::<SettingsV2State>()
            && let Ok(guard) = state.inner.lock()
        {
            return guard.get_ref().performance.memory_mode.clone();
        }
        "medium".to_string()
    }

    fn saturating_u64_to_i32(value: u64) -> i32 {
        value.min(i32::MAX as u64) as i32
    }

    fn apply_plan_to_config(config: &mut LlamacppConfig, plan: &ModelPlan, has_mmproj: bool) {
        if plan.max_context_length > 0 {
            let planned_ctx = Self::saturating_u64_to_i32(plan.max_context_length).max(512);
            config.ctx_size = config.ctx_size.min(planned_ctx).max(512);
        }

        let planned_batch = Self::saturating_u64_to_i32(plan.batch_size).max(1);
        config.batch_size = config.batch_size.min(planned_batch).max(1);
        config.ubatch_size = config.ubatch_size.min(config.batch_size).max(1);

        // Preserve explicit runtime GPU preferences for multimodal models.
        // If planner can only produce CPU mode, force CPU-safe fallback.
        if matches!(plan.mode, ModelMode::CPU) {
            config.n_gpu_layers = 0;
            config.no_kv_offload = true;
        }

        // Never offload projector to GPU if projector is missing or run is effectively CPU-only.
        if !has_mmproj || config.n_gpu_layers <= 0 || config.no_kv_offload {
            config.offload_mmproj = false;
        }
    }

    async fn apply_multimodal_plan(
        &self,
        source: &ResolvedModelSource,
        runtime_cfg: &LlamaRuntimeConfig,
        config: &mut LlamacppConfig,
    ) -> Result<(), String> {
        if source.mmproj_path.is_none() {
            return Ok(());
        }

        let requested_ctx = Some(runtime_cfg.ctx_size.max(512) as u64);
        let memory_mode = self.current_memory_mode();
        let plan = model_planner::plan_model_load(
            source.model_path.clone(),
            memory_mode.clone(),
            source.mmproj_path.clone(),
            requested_ctx,
        )
        .await
        .map_err(|e| format!("failed to plan multimodal load: {e}"))?;

        if matches!(plan.mode, ModelMode::Unsupported) || plan.max_context_length == 0 {
            return Err(format!(
                "multimodal model cannot fit current hardware budget (mode: {:?}, memory_mode: {})",
                plan.mode, memory_mode
            ));
        }

        Self::apply_plan_to_config(config, &plan, source.mmproj_path.is_some());
        log::info!(
            "Applied multimodal load plan: mode={:?}, ngl={}, ctx={}, batch={}, no_kv_offload={}, offload_mmproj={}",
            plan.mode,
            config.n_gpu_layers,
            config.ctx_size,
            config.batch_size,
            config.no_kv_offload,
            config.offload_mmproj
        );
        Ok(())
    }

    fn should_disable_mmap(extra_env: &HashMap<String, String>) -> bool {
        extra_env
            .get("OXIDE_MEMORY_MAPPING")
            .map(|value| value.eq_ignore_ascii_case("ram"))
            .unwrap_or(false)
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

    fn is_mmproj_filename(name: &str) -> bool {
        let lower = name.to_ascii_lowercase();
        if !lower.ends_with(".gguf") {
            return false;
        }
        lower.contains("mmproj") || (lower.contains("vision") && lower.contains("proj"))
    }

    fn is_quant_token(token: &str) -> bool {
        let lower = token.to_ascii_lowercase();
        if lower.is_empty() {
            return false;
        }
        if matches!(
            lower.as_str(),
            "f16"
                | "f32"
                | "bf16"
                | "fp16"
                | "fp32"
                | "int8"
                | "int4"
                | "k"
                | "m"
                | "s"
                | "xs"
                | "xxs"
                | "xxxs"
        ) {
            return true;
        }
        if lower.starts_with('q') && lower.len() > 1 {
            return lower[1..].chars().all(|ch| ch.is_ascii_alphanumeric());
        }
        if lower.starts_with("iq") && lower.len() > 2 {
            return lower[2..].chars().all(|ch| ch.is_ascii_alphanumeric());
        }
        false
    }

    fn normalized_model_family(raw: &str, strip_projector_markers: bool) -> String {
        let stem = Path::new(raw)
            .file_stem()
            .and_then(|v| v.to_str())
            .unwrap_or(raw)
            .to_ascii_lowercase();

        let mut tokens: Vec<String> = stem
            .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.'))
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(|v| v.to_string())
            .collect();

        if strip_projector_markers {
            tokens.retain(|token| {
                !matches!(
                    token.as_str(),
                    "mmproj" | "projector" | "proj" | "vision" | "visual"
                )
            });
        }

        let mut trimmed_quant = false;
        while let Some(last) = tokens.last() {
            let numeric = last.chars().all(|ch| ch.is_ascii_digit());
            if Self::is_quant_token(last) || (trimmed_quant && numeric) {
                tokens.pop();
                trimmed_quant = true;
                continue;
            }
            break;
        }

        tokens.join("-")
    }

    fn is_subsequence(small: &[&str], large: &[&str]) -> bool {
        if small.is_empty() {
            return false;
        }
        let mut i = 0usize;
        for value in large {
            if i < small.len() && small[i] == *value {
                i += 1;
            }
        }
        i == small.len()
    }

    fn mmproj_candidate_rank(model_family: &str, candidate: &str) -> Option<(u8, usize)> {
        if model_family.is_empty() {
            return None;
        }
        let candidate_family = Self::normalized_model_family(candidate, true);
        if candidate_family.is_empty() {
            return None;
        }

        if candidate_family == model_family {
            return Some((0, 0));
        }

        if candidate_family.starts_with(model_family) || model_family.starts_with(&candidate_family)
        {
            return Some((1, model_family.len().abs_diff(candidate_family.len())));
        }

        let model_tokens: Vec<&str> = model_family.split('-').filter(|v| !v.is_empty()).collect();
        let cand_tokens: Vec<&str> = candidate_family
            .split('-')
            .filter(|v| !v.is_empty())
            .collect();
        if Self::is_subsequence(&model_tokens, &cand_tokens)
            || Self::is_subsequence(&cand_tokens, &model_tokens)
        {
            return Some((2, model_tokens.len().abs_diff(cand_tokens.len())));
        }

        None
    }

    fn local_mmproj_candidates(model_path: &str) -> Vec<String> {
        let model = Path::new(model_path);
        let model_family = Self::normalized_model_family(model_path, false);
        let Some(dir) = model.parent() else {
            return Vec::new();
        };
        let mut candidates = Vec::new();
        let entries = match fs::read_dir(dir) {
            Ok(v) => v,
            Err(_) => return Vec::new(),
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|v| v.to_str()) else {
                continue;
            };
            if path == model {
                continue;
            }
            if Self::is_mmproj_filename(name) {
                candidates.push(path.to_string_lossy().to_string());
            }
        }

        let mut ranked: Vec<((u8, usize), String)> = candidates
            .into_iter()
            .filter_map(|path| {
                Self::mmproj_candidate_rank(&model_family, &path).map(|rank| (rank, path))
            })
            .collect();
        ranked.sort_by(|(ra, pa), (rb, pb)| ra.cmp(rb).then_with(|| pa.cmp(pb)));
        ranked.into_iter().map(|(_, path)| path).collect()
    }

    fn validate_manual_mmproj_path(path: &str) -> Result<String, String> {
        let candidate = PathBuf::from(path);
        if !candidate.exists() || !candidate.is_file() {
            return Err(format!(
                "mmproj file does not exist: {}",
                candidate.display()
            ));
        }
        Ok(candidate.to_string_lossy().to_string())
    }

    fn hf_mmproj_siblings(repo_id: &str, revision: &str) -> Result<Vec<String>, String> {
        #[derive(serde::Deserialize)]
        struct HfSibling {
            rfilename: String,
        }
        #[derive(serde::Deserialize)]
        struct HfModelEntry {
            #[serde(default)]
            siblings: Vec<HfSibling>,
        }

        let encoded = urlencoding::encode(repo_id);
        let url = format!("https://huggingface.co/api/models/{encoded}");
        let client = reqwest::blocking::Client::builder()
            .user_agent("oxide-lab/0.15")
            .build()
            .map_err(|e| e.to_string())?;
        let response = client
            .get(url)
            .query(&[("revision", revision)])
            .send()
            .map_err(|e| e.to_string())?;
        if !response.status().is_success() {
            return Ok(Vec::new());
        }
        let entry: HfModelEntry = response.json().map_err(|e| e.to_string())?;
        let mut candidates: Vec<String> = entry
            .siblings
            .into_iter()
            .map(|s| s.rfilename)
            .filter(|name| Self::is_mmproj_filename(name))
            .collect();
        candidates.sort_unstable();
        Ok(candidates)
    }

    fn maybe_download_hub_file(
        repo_id: &str,
        revision: &str,
        filename: &str,
    ) -> Result<Option<String>, String> {
        let api = hf_hub::api::sync::Api::new().map_err(|e| e.to_string())?;
        let repo = hf_hub::Repo::with_revision(
            repo_id.to_string(),
            hf_hub::RepoType::Model,
            revision.to_string(),
        );
        match api.repo(repo).get(filename) {
            Ok(path) => Ok(Some(path.to_string_lossy().to_string())),
            Err(_) => Ok(None),
        }
    }

    fn resolve_local_mmproj(
        _model_id: &str,
        model_path: &str,
        manual_mmproj: Option<&String>,
    ) -> Result<Option<String>, String> {
        if let Some(manual) = manual_mmproj
            && !manual.trim().is_empty()
        {
            return Self::validate_manual_mmproj_path(manual).map(Some);
        }

        Ok(Self::local_mmproj_candidates(model_path).into_iter().next())
    }

    fn resolve_hub_mmproj(
        repo_id: &str,
        revision: &str,
        model_path: &str,
        manual_mmproj: Option<&String>,
    ) -> Result<Option<String>, String> {
        if let Some(manual) = manual_mmproj
            && !manual.trim().is_empty()
        {
            if let Ok(valid) = Self::validate_manual_mmproj_path(manual) {
                return Ok(Some(valid));
            }
            if let Some(downloaded) = Self::maybe_download_hub_file(repo_id, revision, manual)? {
                return Ok(Some(downloaded));
            }
            return Err(format!(
                "mmproj not found. Tried local path and HF file '{}'",
                manual
            ));
        }

        let model_family = Self::normalized_model_family(model_path, false);
        for sibling in Self::hf_mmproj_siblings(repo_id, revision)? {
            if Self::mmproj_candidate_rank(&model_family, &sibling).is_none() {
                continue;
            }
            if let Some(downloaded) = Self::maybe_download_hub_file(repo_id, revision, &sibling)? {
                return Ok(Some(downloaded));
            }
        }

        Ok(Self::local_mmproj_candidates(model_path).into_iter().next())
    }

    async fn start_new_session(
        &self,
        kind: EngineSessionKind,
        source: &ResolvedModelSource,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<PluginSessionInfo, String> {
        let backend_path = self.resolve_server_binary(runtime_cfg)?;
        let timeout = Duration::from_secs(DEFAULT_START_TIMEOUT_SECS).as_secs();
        let is_embedding = Self::is_embedding(kind);
        let mut config = Self::build_config(runtime_cfg);

        if !is_embedding {
            self.apply_multimodal_plan(source, runtime_cfg, &mut config)
                .await?;
        }

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
                source.mmproj_path.clone(),
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
                mmproj_path,
                context_length,
                ..
            } => {
                let model_id = Self::model_id_from_path(model_path);
                let resolved_mmproj =
                    Self::resolve_local_mmproj(&model_id, model_path, mmproj_path.as_ref())?;
                Ok(ResolvedModelSource {
                    model_id,
                    model_path: model_path.clone(),
                    mmproj_path: resolved_mmproj,
                    context_length: *context_length,
                })
            }
            LoadRequest::HubGguf {
                repo_id,
                revision,
                filename,
                mmproj_path,
                context_length,
                ..
            } => {
                let revision = revision.clone().unwrap_or_else(|| "main".to_string());
                let api = hf_hub::api::sync::Api::new().map_err(|e| e.to_string())?;
                let repo = hf_hub::Repo::with_revision(
                    repo_id.clone(),
                    hf_hub::RepoType::Model,
                    revision.clone(),
                );
                let path = api
                    .repo(repo)
                    .get(filename)
                    .map_err(|e| format!("hf_hub get {} failed: {}", filename, e))?;
                let model_path = path.to_string_lossy().to_string();
                let resolved_mmproj = Self::resolve_hub_mmproj(
                    repo_id,
                    &revision,
                    &model_path,
                    mmproj_path.as_ref(),
                )?;
                Ok(ResolvedModelSource {
                    model_id: repo_id.clone(),
                    model_path,
                    mmproj_path: resolved_mmproj,
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
            mmproj_path: session.mmproj_path.clone(),
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

#[cfg(test)]
mod tests {
    use super::LlamaCppAdapter;
    use crate::core::types::LlamaRuntimeConfig;
    use oxide_llamacpp::gguf::model_planner::{ModelMode, ModelPlan};
    use std::fs;
    use std::path::PathBuf;

    fn runtime_with_mapping(mapping: Option<&str>) -> LlamaRuntimeConfig {
        let mut cfg = LlamaRuntimeConfig::default();
        if let Some(value) = mapping {
            cfg.extra_env
                .insert("OXIDE_MEMORY_MAPPING".to_string(), value.to_string());
        }
        cfg
    }

    #[test]
    fn build_config_enables_no_mmap_when_mapping_is_ram() {
        let cfg = runtime_with_mapping(Some("ram"));
        let built = LlamaCppAdapter::build_config(&cfg);
        assert!(built.no_mmap);
    }

    #[test]
    fn build_config_keeps_mmap_when_mapping_is_mmap() {
        let cfg = runtime_with_mapping(Some("mmap"));
        let built = LlamaCppAdapter::build_config(&cfg);
        assert!(!built.no_mmap);
    }

    #[test]
    fn build_config_keeps_mmap_when_mapping_is_missing() {
        let cfg = runtime_with_mapping(None);
        let built = LlamaCppAdapter::build_config(&cfg);
        assert!(!built.no_mmap);
    }

    #[test]
    fn build_config_enables_no_mmap_for_uppercase_ram_value() {
        let cfg = runtime_with_mapping(Some("RAM"));
        let built = LlamaCppAdapter::build_config(&cfg);
        assert!(built.no_mmap);
    }

    #[test]
    fn build_config_maps_advanced_runtime_fields() {
        let mut cfg = LlamaRuntimeConfig::default();
        cfg.offload_mmproj = false;
        cfg.cpu_moe = true;
        cfg.n_cpu_moe = 6;
        cfg.override_tensor_buffer_t = "blk.0.attn_q=CPU".to_string();
        cfg.split_mode = "row".to_string();
        cfg.main_gpu = 2;
        cfg.n_parallel = 4;
        cfg.flash_attn = "on".to_string();
        cfg.cont_batching = false;
        cfg.mlock = true;
        cfg.no_kv_offload = true;
        cfg.cache_type_k = "q8_0".to_string();
        cfg.cache_type_v = "q4_1".to_string();
        cfg.defrag_thold = 0.25;
        cfg.rope_scaling = "linear".to_string();
        cfg.rope_scale = 1.2;
        cfg.rope_freq_base = 10000.0;
        cfg.rope_freq_scale = 0.8;
        cfg.ctx_shift = true;

        let built = LlamaCppAdapter::build_config(&cfg);
        assert!(!built.offload_mmproj);
        assert!(built.cpu_moe);
        assert_eq!(built.n_cpu_moe, 6);
        assert_eq!(built.override_tensor_buffer_t, "blk.0.attn_q=CPU");
        assert_eq!(built.split_mode, "row");
        assert_eq!(built.main_gpu, 2);
        assert_eq!(built.n_parallel, 4);
        assert_eq!(built.flash_attn, "on");
        assert!(!built.cont_batching);
        assert!(built.mlock);
        assert!(built.no_kv_offload);
        assert_eq!(built.cache_type_k, "q8_0");
        assert_eq!(built.cache_type_v, "q4_1");
        assert!((built.defrag_thold - 0.25).abs() < f32::EPSILON);
        assert_eq!(built.rope_scaling, "linear");
        assert!((built.rope_scale - 1.2).abs() < f32::EPSILON);
        assert!((built.rope_freq_base - 10000.0).abs() < f32::EPSILON);
        assert!((built.rope_freq_scale - 0.8).abs() < f32::EPSILON);
        assert!(built.ctx_shift);
    }

    #[test]
    fn infer_backend_from_server_path_handles_bundled_layout() {
        let inferred = LlamaCppAdapter::infer_backend_from_server_path(
            r"C:\app\bin\llama-b7951-bin-win-cuda-12-common_cpus-x64\llama-server.exe",
        );
        assert_eq!(
            inferred.as_deref(),
            Some("b7951/win-cuda-12-common_cpus-x64")
        );
    }

    #[test]
    fn infer_backend_from_server_path_handles_installed_layout() {
        let inferred = LlamaCppAdapter::infer_backend_from_server_path(
            r"C:\Users\me\AppData\Local\oxide-lab\llamacpp\backends\b7951\win-cuda-12-common_cpus-x64\build\bin\llama-server.exe",
        );
        assert_eq!(
            inferred.as_deref(),
            Some("b7951/win-cuda-12-common_cpus-x64")
        );
    }

    #[test]
    fn explicit_path_is_rejected_when_selected_backend_mismatches() {
        let should_use = LlamaCppAdapter::should_use_explicit_server_path(
            r"C:\app\bin\llama-b7951-bin-win-cpu-x64\llama-server.exe",
            Some("b7951/win-cuda-12-common_cpus-x64"),
        );
        assert!(!should_use);
    }

    #[test]
    fn explicit_path_is_accepted_when_selected_backend_matches() {
        let should_use = LlamaCppAdapter::should_use_explicit_server_path(
            r"C:\app\bin\llama-b7951-bin-win-cuda-12-common_cpus-x64\llama-server.exe",
            Some("b7951/win-cuda-12-common_cpus-x64"),
        );
        assert!(should_use);
    }

    fn make_temp_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("oxide-mmproj-tests-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn local_mmproj_candidates_find_expected_patterns() {
        let dir = make_temp_dir();
        let model = dir.join("qwen3-vl-8b.gguf");
        let mmproj = dir.join("qwen3-vl-mmproj-f16.gguf");
        let vision_proj = dir.join("qwen3-vl-vision-proj.gguf");
        let ignored = dir.join("readme.txt");
        fs::write(&model, b"model").expect("model");
        fs::write(&mmproj, b"mmproj").expect("mmproj");
        fs::write(&vision_proj, b"vision proj").expect("vision proj");
        fs::write(&ignored, b"ignore").expect("ignored");

        let candidates = LlamaCppAdapter::local_mmproj_candidates(&model.to_string_lossy());
        assert!(
            candidates
                .iter()
                .any(|p| p.ends_with("qwen3-vl-mmproj-f16.gguf"))
        );
        assert!(
            candidates
                .iter()
                .any(|p| p.ends_with("qwen3-vl-vision-proj.gguf"))
        );
        assert!(!candidates.iter().any(|p| p.ends_with("readme.txt")));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolve_local_mmproj_prefers_manual_over_auto() {
        let dir = make_temp_dir();
        let model = dir.join("qwen3-vl-8b.gguf");
        let auto = dir.join("qwen3-vl-mmproj-f16.gguf");
        let manual = dir.join("manual-mmproj.gguf");
        fs::write(&model, b"model").expect("model");
        fs::write(&auto, b"auto").expect("auto");
        fs::write(&manual, b"manual").expect("manual");

        let manual_str = manual.to_string_lossy().to_string();
        let resolved = LlamaCppAdapter::resolve_local_mmproj(
            "qwen3-vl-8b",
            &model.to_string_lossy(),
            Some(&manual_str),
        )
        .expect("resolved");

        assert_eq!(resolved.as_deref(), Some(manual_str.as_str()));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolve_local_mmproj_without_companion_returns_none() {
        let dir = make_temp_dir();
        let model = dir.join("qwen3-vl-8b.gguf");
        fs::write(&model, b"model").expect("model");

        let resolved =
            LlamaCppAdapter::resolve_local_mmproj("qwen3-vl-8b", &model.to_string_lossy(), None)
                .expect("resolved");
        assert!(resolved.is_none());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn local_mmproj_candidates_match_human_readable_model_name() {
        let dir = make_temp_dir();
        let model = dir.join("GLM-4.6V-Flash-Q4_K_M.gguf");
        let mmproj = dir.join("mmproj-GLM-4.6V-Flash-F16.gguf");
        let unrelated = dir.join("mmproj-Qwen2.5-VL-7B-F16.gguf");
        fs::write(&model, b"model").expect("model");
        fs::write(&mmproj, b"mmproj").expect("mmproj");
        fs::write(&unrelated, b"unrelated").expect("unrelated");

        let candidates = LlamaCppAdapter::local_mmproj_candidates(&model.to_string_lossy());
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].ends_with("mmproj-GLM-4.6V-Flash-F16.gguf"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn local_mmproj_candidates_do_not_fallback_to_unrelated_neighbor() {
        let dir = make_temp_dir();
        let model = dir.join("MyModel-Q4_K_M.gguf");
        let unrelated = dir.join("OtherModel-Q8_0.gguf");
        fs::write(&model, b"model").expect("model");
        fs::write(&unrelated, b"unrelated").expect("unrelated");

        let candidates = LlamaCppAdapter::local_mmproj_candidates(&model.to_string_lossy());
        assert!(candidates.is_empty());

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn apply_plan_to_config_updates_gpu_ctx_and_batch_for_multimodal() {
        let runtime = LlamaRuntimeConfig::default();
        let mut config = LlamaCppAdapter::build_config(&runtime);
        let plan = ModelPlan {
            gpu_layers: 37,
            max_context_length: 2048,
            no_offload_kv_cache: false,
            offload_mmproj: true,
            batch_size: 256,
            mode: ModelMode::GPU,
        };

        LlamaCppAdapter::apply_plan_to_config(&mut config, &plan, true);
        assert_eq!(config.n_gpu_layers, runtime.n_gpu_layers);
        assert_eq!(config.ctx_size, 2048);
        assert_eq!(config.batch_size, 256);
        assert_eq!(config.ubatch_size, 256);
        assert!(!config.no_kv_offload);
        assert!(config.offload_mmproj);
    }

    #[test]
    fn apply_plan_to_config_disables_mmproj_offload_when_mmproj_missing() {
        let runtime = LlamaRuntimeConfig::default();
        let mut config = LlamaCppAdapter::build_config(&runtime);
        let plan = ModelPlan {
            gpu_layers: 10,
            max_context_length: 1024,
            no_offload_kv_cache: true,
            offload_mmproj: true,
            batch_size: 64,
            mode: ModelMode::Hybrid,
        };

        LlamaCppAdapter::apply_plan_to_config(&mut config, &plan, false);
        assert_eq!(config.n_gpu_layers, runtime.n_gpu_layers);
        assert_eq!(config.ctx_size, 1024);
        assert_eq!(config.batch_size, 64);
        assert_eq!(config.ubatch_size, 64);
        assert!(!config.no_kv_offload);
        assert!(!config.offload_mmproj);
    }

    #[test]
    fn apply_plan_to_config_forces_cpu_safe_mmproj_mode_when_plan_is_cpu() {
        let runtime = LlamaRuntimeConfig::default();
        let mut config = LlamaCppAdapter::build_config(&runtime);
        let plan = ModelPlan {
            gpu_layers: 0,
            max_context_length: 2048,
            no_offload_kv_cache: true,
            offload_mmproj: false,
            batch_size: 64,
            mode: ModelMode::CPU,
        };

        LlamaCppAdapter::apply_plan_to_config(&mut config, &plan, true);
        assert_eq!(config.n_gpu_layers, 0);
        assert!(config.no_kv_offload);
        assert!(!config.offload_mmproj);
    }
}
