use crate::core::performance::PerformanceMonitor;
use crate::core::precision::{Precision, PrecisionPolicy};
use crate::core::prefix_cache::{PrefixCache, PrefixCacheConfig};
use crate::core::types::{
    ActiveBackend, BackendPreference, DevicePreference, LlamaRuntimeConfig, LlamaSessionSnapshot,
};
use std::fs::File;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tauri::Manager;

pub struct ModelState {
    pub(crate) device_pref: DevicePreference,
    pub(crate) context_length: usize,
    pub(crate) model_path: Option<String>,
    pub(crate) chat_template: Option<String>,
    pub(crate) hub_repo_id: Option<String>,
    pub(crate) hub_revision: Option<String>,
    pub(crate) precision_policy: PrecisionPolicy,
    pub(crate) rayon_thread_limit: Option<usize>,
    pub(crate) performance_monitor: Arc<PerformanceMonitor>,
    pub(crate) prefix_cache: PrefixCache,
    pub(crate) backend_preference: BackendPreference,
    pub(crate) active_backend: ActiveBackend,
    pub(crate) active_model_id: Option<String>,
    pub(crate) active_mmproj_path: Option<String>,
    pub(crate) active_llama_session: Option<LlamaSessionSnapshot>,
    pub(crate) llama_runtime: LlamaRuntimeConfig,
}

impl ModelState {
    pub fn new(device_pref: DevicePreference) -> Self {
        Self {
            device_pref,
            context_length: 4096,
            model_path: None,
            chat_template: None,
            hub_repo_id: None,
            hub_revision: None,
            precision_policy: PrecisionPolicy::Default,
            rayon_thread_limit: None,
            performance_monitor: Arc::new(PerformanceMonitor::new(1000)),
            prefix_cache: PrefixCache::new(PrefixCacheConfig::enabled(32)),
            backend_preference: BackendPreference::Llamacpp,
            active_backend: ActiveBackend::None,
            active_model_id: None,
            active_mmproj_path: None,
            active_llama_session: None,
            llama_runtime: LlamaRuntimeConfig::default(),
        }
    }

    pub fn save_precision(&self, app: &AppHandle) -> Result<(), String> {
        let profile_dir = Self::ensure_profile_dir(app)?;
        let path = profile_dir.join("precision.json");
        let file =
            File::create(&path).map_err(|e| format!("Failed to create precision file: {}", e))?;
        serde_json::to_writer(file, &self.precision_policy)
            .map_err(|e| format!("Failed to serialize precision: {}", e))?;
        Ok(())
    }

    pub fn load_precision(app: &AppHandle) -> Result<Precision, String> {
        let profile_dir = Self::profile_dir(app)?;
        let path = profile_dir.join("precision.json");
        if path.exists() {
            let file =
                File::open(&path).map_err(|e| format!("Failed to open precision file: {}", e))?;
            let _policy: PrecisionPolicy = serde_json::from_reader(file)
                .map_err(|e| format!("Failed to deserialize precision: {}", e))?;
            Ok(Precision::default())
        } else {
            Ok(Precision::default())
        }
    }

    fn profile_dir(app: &AppHandle) -> Result<PathBuf, String> {
        let dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| format!("Failed to get app data directory: {}", e))?;
        Ok(dir.join("oxide-lab"))
    }

    fn ensure_profile_dir(app: &AppHandle) -> Result<PathBuf, String> {
        let profile_dir = Self::profile_dir(app)?;
        create_dir_all(&profile_dir)
            .map_err(|e| format!("Failed to create profile directory: {}", e))?;
        Ok(profile_dir)
    }

    pub fn save_thread_limit(app: &AppHandle, limit: Option<usize>) -> Result<(), String> {
        let profile_dir = Self::ensure_profile_dir(app)?;
        let path = profile_dir.join("thread_limit.json");
        let file = File::create(&path)
            .map_err(|e| format!("Failed to create thread limit file: {}", e))?;
        serde_json::to_writer(file, &limit)
            .map_err(|e| format!("Failed to serialize thread limit: {}", e))?;
        Ok(())
    }

    pub fn load_thread_limit(app: &AppHandle) -> Result<Option<usize>, String> {
        let profile_dir = Self::profile_dir(app)?;
        let path = profile_dir.join("thread_limit.json");
        if path.exists() {
            let file = File::open(&path)
                .map_err(|e| format!("Failed to open thread limit file: {}", e))?;
            let limit: Option<usize> = serde_json::from_reader(file)
                .map_err(|e| format!("Failed to deserialize thread limit: {}", e))?;
            Ok(limit)
        } else {
            Ok(None)
        }
    }

    pub fn save_llama_runtime(app: &AppHandle, config: &LlamaRuntimeConfig) -> Result<(), String> {
        let profile_dir = Self::ensure_profile_dir(app)?;
        let path = profile_dir.join("llama_runtime.json");
        let file = File::create(&path)
            .map_err(|e| format!("Failed to create llama runtime file: {}", e))?;
        serde_json::to_writer(file, config)
            .map_err(|e| format!("Failed to serialize llama runtime config: {}", e))?;
        Ok(())
    }

    pub fn load_llama_runtime(app: &AppHandle) -> Result<Option<LlamaRuntimeConfig>, String> {
        let profile_dir = Self::profile_dir(app)?;
        let path = profile_dir.join("llama_runtime.json");
        if path.exists() {
            let file = File::open(&path)
                .map_err(|e| format!("Failed to open llama runtime file: {}", e))?;
            let cfg: LlamaRuntimeConfig = serde_json::from_reader(file)
                .map_err(|e| format!("Failed to deserialize llama runtime config: {}", e))?;
            Ok(Some(cfg))
        } else {
            Ok(None)
        }
    }
}

pub type SharedState = Arc<Mutex<ModelState>>;
