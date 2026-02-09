use crate::core::state::ModelState;
use crate::core::types::LlamaRuntimeConfig;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::AppHandle;
use tauri::Manager;

pub const SETTINGS_V2_SCHEMA_VERSION: u32 = 3;
pub const DEFAULT_OPENAI_PORT: u16 = 11434;
pub const CLEAR_DATA_CONFIRM_TOKEN: &str = "CONFIRM_CLEAR_DATA";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CorsMode {
    SameOrigin,
    Allowlist,
    Any,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenAiServerConfig {
    pub enabled: bool,
    pub bind_host: String,
    pub port: u16,
    pub auth_required: bool,
    pub api_keys_hashed: Vec<String>,
    pub cors_mode: CorsMode,
    pub cors_allowlist: Vec<String>,
}

impl Default for OpenAiServerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            bind_host: "127.0.0.1".to_string(),
            port: DEFAULT_OPENAI_PORT,
            auth_required: false,
            api_keys_hashed: Vec::new(),
            cors_mode: CorsMode::SameOrigin,
            cors_allowlist: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSamplingSettings {
    pub temperature: f64,
    pub top_p: f64,
    pub top_k: u32,
    pub min_p: f64,
    pub repeat_penalty: f32,
    pub max_tokens: u32,
    pub seed: Option<u64>,
    pub stop_sequences: Vec<String>,
}

impl Default for ChatSamplingSettings {
    fn default() -> Self {
        Self {
            temperature: 0.7,
            top_p: 0.9,
            top_k: 20,
            min_p: 0.0,
            repeat_penalty: 1.1,
            max_tokens: 1024,
            seed: None,
            stop_sequences: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPreset {
    pub id: String,
    pub name: String,
    pub system_prompt: String,
    pub sampling: ChatSamplingSettings,
    pub context: u32,
    pub builtin: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatPresetSettings {
    pub default_preset_id: String,
    pub presets: Vec<ChatPreset>,
    pub default_system_prompt: String,
}

impl Default for ChatPresetSettings {
    fn default() -> Self {
        let creativity = ChatPreset {
            id: "creativity".to_string(),
            name: "Creativity".to_string(),
            system_prompt: "You are a creative assistant. Be expressive but clear.".to_string(),
            sampling: ChatSamplingSettings {
                temperature: 0.95,
                top_p: 0.95,
                top_k: 50,
                min_p: 0.0,
                repeat_penalty: 1.05,
                max_tokens: 2048,
                seed: None,
                stop_sequences: Vec::new(),
            },
            context: 8192,
            builtin: true,
        };
        let code = ChatPreset {
            id: "code".to_string(),
            name: "Code".to_string(),
            system_prompt: "You are an expert software engineer. Prioritize correctness and concise explanations.".to_string(),
            sampling: ChatSamplingSettings {
                temperature: 0.2,
                top_p: 0.8,
                top_k: 20,
                min_p: 0.0,
                repeat_penalty: 1.1,
                max_tokens: 2048,
                seed: None,
                stop_sequences: Vec::new(),
            },
            context: 8192,
            builtin: true,
        };
        let precision = ChatPreset {
            id: "precision".to_string(),
            name: "Precision".to_string(),
            system_prompt: "You are a precise assistant. State assumptions and avoid speculation."
                .to_string(),
            sampling: ChatSamplingSettings {
                temperature: 0.1,
                top_p: 0.7,
                top_k: 10,
                min_p: 0.0,
                repeat_penalty: 1.15,
                max_tokens: 1536,
                seed: None,
                stop_sequences: Vec::new(),
            },
            context: 8192,
            builtin: true,
        };

        Self {
            default_preset_id: "code".to_string(),
            presets: vec![creativity, code, precision],
            default_system_prompt: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralSettings {
    pub locale: String,
    pub theme: String,
    pub auto_update: bool,
    pub launch_on_startup: bool,
    pub expert_mode: bool,
    pub developer_mode: bool,
    pub search_history_enabled: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            locale: "en".to_string(),
            theme: "system".to_string(),
            auto_update: true,
            launch_on_startup: false,
            expert_mode: false,
            developer_mode: false,
            search_history_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsStorageSettings {
    pub models_dir: Option<String>,
    pub cache_dir: Option<String>,
    pub model_selector_search: bool,
}

impl Default for ModelsStorageSettings {
    fn default() -> Self {
        Self {
            models_dir: None,
            cache_dir: None,
            model_selector_search: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceSettings {
    pub manual_thread_limit: Option<usize>,
    pub llama_runtime: LlamaRuntimeConfig,
    pub memory_mode: String,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            manual_thread_limit: None,
            llama_runtime: LlamaRuntimeConfig::default(),
            memory_mode: "medium".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PrivacyDataSettings {
    pub telemetry_enabled: bool,
    pub crash_reports_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DeveloperSettings {
    pub openai_server: OpenAiServerConfig,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum WebRetrievalDefaultMode {
    #[default]
    Lite,
    Off,
    Pro,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchSettings {
    pub default_mode: WebRetrievalDefaultMode,
    pub max_snippets: usize,
    pub max_snippet_chars: usize,
    pub max_retrieval_tokens: usize,
    pub max_pages: usize,
    pub pro_beta_enabled: bool,
}

impl Default for WebSearchSettings {
    fn default() -> Self {
        Self {
            default_mode: WebRetrievalDefaultMode::Lite,
            max_snippets: 8,
            max_snippet_chars: 420,
            max_retrieval_tokens: 900,
            max_pages: 5,
            pro_beta_enabled: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRagSettings {
    pub beta_enabled: bool,
    pub chunk_size_chars: usize,
    pub chunk_overlap_chars: usize,
    pub top_k: usize,
    pub max_file_size_mb: usize,
    pub max_context_chunks: usize,
}

impl Default for LocalRagSettings {
    fn default() -> Self {
        Self {
            beta_enabled: false,
            chunk_size_chars: 1200,
            chunk_overlap_chars: 180,
            top_k: 6,
            max_file_size_mb: 20,
            max_context_chunks: 5,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingsProviderSettings {
    pub base_url: String,
    pub api_key: Option<String>,
    pub model: String,
    pub timeout_ms: u64,
}

impl Default for EmbeddingsProviderSettings {
    fn default() -> Self {
        Self {
            base_url: "http://127.0.0.1:11434/v1".to_string(),
            api_key: None,
            model: String::new(),
            timeout_ms: 20_000,
        }
    }
}

impl EmbeddingsProviderSettings {
    pub fn is_configured(&self) -> bool {
        !self.base_url.trim().is_empty() && !self.model.trim().is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WebRagSettings {
    pub web_search: WebSearchSettings,
    pub local_rag: LocalRagSettings,
    pub embeddings_provider: EmbeddingsProviderSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsV2 {
    pub schema_version: u32,
    pub general: GeneralSettings,
    pub models_storage: ModelsStorageSettings,
    pub performance: PerformanceSettings,
    pub chat_presets: ChatPresetSettings,
    pub privacy_data: PrivacyDataSettings,
    pub developer: DeveloperSettings,
    #[serde(default)]
    pub web_rag: WebRagSettings,
}

impl Default for AppSettingsV2 {
    fn default() -> Self {
        Self {
            schema_version: SETTINGS_V2_SCHEMA_VERSION,
            general: GeneralSettings::default(),
            models_storage: ModelsStorageSettings::default(),
            performance: PerformanceSettings::default(),
            chat_presets: ChatPresetSettings::default(),
            privacy_data: PrivacyDataSettings::default(),
            developer: DeveloperSettings::default(),
            web_rag: WebRagSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppSettingsPatch {
    pub general: Option<GeneralSettings>,
    pub models_storage: Option<ModelsStorageSettings>,
    pub performance: Option<PerformanceSettings>,
    pub chat_presets: Option<ChatPresetSettings>,
    pub privacy_data: Option<PrivacyDataSettings>,
    pub developer: Option<DeveloperSettings>,
    pub web_rag: Option<WebRagSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingsScope {
    All,
    General,
    ModelsStorage,
    Performance,
    ChatPresets,
    PrivacyData,
    Developer,
    WebRag,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsApplyResult {
    pub applied: bool,
    pub requires_restart: bool,
    pub warnings: Vec<String>,
    pub settings: AppSettingsV2,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataLocations {
    pub app_data_dir: String,
    pub profile_dir: String,
    pub settings_file: String,
    pub settings_backup_file: String,
    pub chat_db: String,
    pub rag_db: String,
    pub legacy_thread_limit_file: String,
    pub legacy_runtime_file: String,
    pub legacy_experimental_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportResult {
    pub success: bool,
    pub target_path: String,
    pub exported_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClearDataScope {
    All,
    Chats,
    Downloads,
    Settings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClearDataResult {
    pub success: bool,
    pub cleared_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiServerStatus {
    pub running: bool,
    pub enabled: bool,
    pub bind_host: String,
    pub port: u16,
    pub endpoint: String,
    pub auth_required: bool,
    pub cors_mode: CorsMode,
    pub cors_allowlist: Vec<String>,
    pub api_keys_count: usize,
    pub warnings: Vec<String>,
}

pub struct SettingsV2Store {
    path: PathBuf,
    backup_path: PathBuf,
    settings: AppSettingsV2,
}

impl SettingsV2Store {
    pub fn load(app: &AppHandle) -> Result<Self, String> {
        let profile_dir = profile_dir(app)?;
        fs::create_dir_all(&profile_dir)
            .map_err(|e| format!("Failed to create profile dir: {e}"))?;

        let path = profile_dir.join("settings_v2.json");
        let backup_path = profile_dir.join("settings_v2.bak");

        let mut settings = if path.exists() {
            match fs::read_to_string(&path) {
                Ok(raw) => match serde_json::from_str::<AppSettingsV2>(&raw) {
                    Ok(parsed) => parsed,
                    Err(err) => {
                        let _ = fs::copy(&path, &backup_path);
                        log::warn!(
                            "settings_v2.json parse error; backing up to {:?}: {}",
                            backup_path,
                            err
                        );
                        Self::migrate_from_legacy(app)?
                    }
                },
                Err(err) => {
                    log::warn!("Failed to read settings_v2.json: {}", err);
                    Self::migrate_from_legacy(app)?
                }
            }
        } else {
            Self::migrate_from_legacy(app)?
        };

        settings.schema_version = SETTINGS_V2_SCHEMA_VERSION;
        ensure_builtin_presets(&mut settings.chat_presets);

        let mut store = Self {
            path,
            backup_path,
            settings,
        };
        store.save()?;
        Ok(store)
    }

    pub fn get(&self) -> AppSettingsV2 {
        self.settings.clone()
    }

    pub fn get_ref(&self) -> &AppSettingsV2 {
        &self.settings
    }

    pub fn set(&mut self, settings: AppSettingsV2) -> Result<(), String> {
        self.settings = settings;
        self.settings.schema_version = SETTINGS_V2_SCHEMA_VERSION;
        ensure_builtin_presets(&mut self.settings.chat_presets);
        self.validate()?;
        self.save()
    }

    pub fn apply_patch(&mut self, patch: AppSettingsPatch) -> Result<SettingsApplyResult, String> {
        if let Some(general) = patch.general {
            self.settings.general = general;
        }
        if let Some(models_storage) = patch.models_storage {
            self.settings.models_storage = models_storage;
        }
        if let Some(performance) = patch.performance {
            self.settings.performance = performance;
        }
        if let Some(chat_presets) = patch.chat_presets {
            self.settings.chat_presets = chat_presets;
        }
        if let Some(privacy_data) = patch.privacy_data {
            self.settings.privacy_data = privacy_data;
        }
        if let Some(developer) = patch.developer {
            self.settings.developer = developer;
        }
        if let Some(web_rag) = patch.web_rag {
            self.settings.web_rag = web_rag;
        }

        self.settings.schema_version = SETTINGS_V2_SCHEMA_VERSION;
        ensure_builtin_presets(&mut self.settings.chat_presets);

        let warnings = self.validate()?;
        self.save()?;

        Ok(SettingsApplyResult {
            applied: true,
            requires_restart: false,
            warnings,
            settings: self.settings.clone(),
        })
    }

    pub fn reset(&mut self, scope: Option<SettingsScope>) -> Result<AppSettingsV2, String> {
        let defaults = AppSettingsV2::default();
        match scope.unwrap_or(SettingsScope::All) {
            SettingsScope::All => {
                self.settings = defaults;
            }
            SettingsScope::General => self.settings.general = defaults.general,
            SettingsScope::ModelsStorage => self.settings.models_storage = defaults.models_storage,
            SettingsScope::Performance => self.settings.performance = defaults.performance,
            SettingsScope::ChatPresets => self.settings.chat_presets = defaults.chat_presets,
            SettingsScope::PrivacyData => self.settings.privacy_data = defaults.privacy_data,
            SettingsScope::Developer => self.settings.developer = defaults.developer,
            SettingsScope::WebRag => self.settings.web_rag = defaults.web_rag,
        }
        self.settings.schema_version = SETTINGS_V2_SCHEMA_VERSION;
        self.save()?;
        Ok(self.settings.clone())
    }

    pub fn update_openai_config(
        &mut self,
        config: OpenAiServerConfig,
    ) -> Result<SettingsApplyResult, String> {
        self.settings.developer.openai_server = config;
        let warnings = self.validate()?;
        self.save()?;
        Ok(SettingsApplyResult {
            applied: true,
            requires_restart: false,
            warnings,
            settings: self.settings.clone(),
        })
    }

    pub fn data_locations(&self, app: &AppHandle) -> Result<DataLocations, String> {
        let app_data_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| format!("Failed to get app data dir: {e}"))?;
        let profile = profile_dir(app)?;
        Ok(DataLocations {
            app_data_dir: app_data_dir.to_string_lossy().to_string(),
            profile_dir: profile.to_string_lossy().to_string(),
            settings_file: self.path.to_string_lossy().to_string(),
            settings_backup_file: self.backup_path.to_string_lossy().to_string(),
            chat_db: profile
                .join("chat_history.db")
                .to_string_lossy()
                .to_string(),
            rag_db: profile.join("rag_index.db").to_string_lossy().to_string(),
            legacy_thread_limit_file: profile
                .join("thread_limit.json")
                .to_string_lossy()
                .to_string(),
            legacy_runtime_file: profile
                .join("llama_runtime.json")
                .to_string_lossy()
                .to_string(),
            legacy_experimental_file: profile
                .join("experimental_features.json")
                .to_string_lossy()
                .to_string(),
        })
    }

    pub fn save(&mut self) -> Result<(), String> {
        self.validate()?;
        let raw = serde_json::to_string_pretty(&self.settings)
            .map_err(|e| format!("Failed to serialize settings_v2: {e}"))?;
        fs::write(&self.path, raw).map_err(|e| format!("Failed to write settings_v2: {e}"))
    }

    pub fn export_data(&self, app: &AppHandle, target_path: &Path) -> Result<ExportResult, String> {
        if !target_path.exists() {
            fs::create_dir_all(target_path)
                .map_err(|e| format!("Failed to create export target directory: {e}"))?;
        }

        let locations = self.data_locations(app)?;
        let mut exported_files = Vec::new();
        let files = vec![
            locations.settings_file,
            locations.chat_db,
            locations.legacy_runtime_file,
            locations.legacy_thread_limit_file,
            locations.legacy_experimental_file,
        ];

        for src in files {
            let src_path = PathBuf::from(&src);
            if !src_path.exists() {
                continue;
            }
            let file_name = match src_path.file_name().and_then(|v| v.to_str()) {
                Some(v) => v,
                None => continue,
            };
            let dst = target_path.join(file_name);
            fs::copy(&src_path, &dst).map_err(|e| {
                format!(
                    "Failed to export {} to {}: {}",
                    src_path.display(),
                    dst.display(),
                    e
                )
            })?;
            exported_files.push(dst.to_string_lossy().to_string());
        }

        Ok(ExportResult {
            success: true,
            target_path: target_path.to_string_lossy().to_string(),
            exported_files,
        })
    }

    pub fn clear_data(
        &mut self,
        app: &AppHandle,
        scope: ClearDataScope,
        confirm_token: &str,
    ) -> Result<ClearDataResult, String> {
        if confirm_token != CLEAR_DATA_CONFIRM_TOKEN {
            return Err("Invalid confirmation token".to_string());
        }

        let locations = self.data_locations(app)?;
        let mut cleared_files = Vec::new();
        let app_data = PathBuf::from(&locations.app_data_dir);
        let downloads_history = app_data
            .join("oxide-lab")
            .join("downloads")
            .join("history.json");
        let legacy_download_manifest = app_data.join("oxide-lab").join("downloads_manifest.json");

        match scope {
            ClearDataScope::All => {
                for p in [
                    locations.chat_db.clone(),
                    locations.settings_file.clone(),
                    locations.legacy_runtime_file.clone(),
                    locations.legacy_thread_limit_file.clone(),
                    locations.legacy_experimental_file.clone(),
                ] {
                    remove_file_if_exists(Path::new(&p), &mut cleared_files)?;
                }
                remove_file_if_exists(&downloads_history, &mut cleared_files)?;
                remove_file_if_exists(&legacy_download_manifest, &mut cleared_files)?;
                self.settings = AppSettingsV2::default();
                self.save()?;
            }
            ClearDataScope::Chats => {
                let chat_db = PathBuf::from(&locations.chat_db);
                remove_file_if_exists(&chat_db, &mut cleared_files)?;
            }
            ClearDataScope::Downloads => {
                remove_file_if_exists(&downloads_history, &mut cleared_files)?;
                remove_file_if_exists(&legacy_download_manifest, &mut cleared_files)?;
            }
            ClearDataScope::Settings => {
                let settings_file = PathBuf::from(&locations.settings_file);
                remove_file_if_exists(&settings_file, &mut cleared_files)?;
                self.settings = AppSettingsV2::default();
                self.save()?;
            }
        }

        Ok(ClearDataResult {
            success: true,
            cleared_files,
        })
    }

    fn validate(&self) -> Result<Vec<String>, String> {
        validate_settings(&self.settings)
    }

    fn migrate_from_legacy(app: &AppHandle) -> Result<AppSettingsV2, String> {
        let mut settings = AppSettingsV2::default();

        settings.general.locale = crate::i18n::get_locale().as_str().to_string();

        if let Ok(limit) = ModelState::load_thread_limit(app) {
            settings.performance.manual_thread_limit = limit;
        }

        if let Ok(Some(runtime)) = ModelState::load_llama_runtime(app) {
            settings.performance.llama_runtime = runtime;
        }

        if let Ok(profile) = profile_dir(app) {
            let experimental_path = profile.join("experimental_features.json");
            if experimental_path.exists()
                && let Ok(raw) = fs::read_to_string(&experimental_path)
                && let Ok(enabled) = serde_json::from_str::<bool>(&raw)
            {
                settings.general.developer_mode = enabled;
            }
        }

        Ok(settings)
    }
}

fn remove_file_if_exists(path: &Path, cleared_files: &mut Vec<String>) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => {
            cleared_files.push(path.to_string_lossy().to_string());
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!("Failed to remove {}: {err}", path.display())),
    }
}

pub struct SettingsV2State {
    pub inner: std::sync::Mutex<SettingsV2Store>,
}

impl SettingsV2State {
    pub fn new(store: SettingsV2Store) -> Self {
        Self {
            inner: std::sync::Mutex::new(store),
        }
    }
}

pub fn profile_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to get app data directory: {e}"))?;
    Ok(dir.join("oxide-lab"))
}

pub fn hash_api_key(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    base64::engine::general_purpose::STANDARD_NO_PAD.encode(digest)
}

pub fn verify_api_key(raw: &str, hashed_keys: &[String]) -> bool {
    let hashed = hash_api_key(raw);
    hashed_keys.iter().any(|v| v == &hashed)
}

pub fn openai_status_from_config(config: &OpenAiServerConfig, running: bool) -> OpenAiServerStatus {
    let endpoint = format!("http://{}:{}/v1", config.bind_host, config.port);
    let mut warnings = Vec::new();
    if config.bind_host == "0.0.0.0" && !config.auth_required {
        warnings.push("LAN mode without auth is blocked".to_string());
    }
    if config.auth_required && config.api_keys_hashed.is_empty() {
        warnings.push("Auth enabled but API keys are empty".to_string());
    }

    OpenAiServerStatus {
        running,
        enabled: config.enabled,
        bind_host: config.bind_host.clone(),
        port: config.port,
        endpoint,
        auth_required: config.auth_required,
        cors_mode: config.cors_mode.clone(),
        cors_allowlist: config.cors_allowlist.clone(),
        api_keys_count: config.api_keys_hashed.len(),
        warnings,
    }
}

pub fn validate_settings(settings: &AppSettingsV2) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();
    let cfg = &settings.developer.openai_server;
    let web = &settings.web_rag.web_search;
    let local = &settings.web_rag.local_rag;
    let embeddings = &settings.web_rag.embeddings_provider;

    if cfg.port < 1024 {
        return Err("OpenAI server port must be in range 1024..65535".to_string());
    }

    if cfg.bind_host != "127.0.0.1" && cfg.bind_host != "0.0.0.0" {
        return Err("OpenAI bind_host must be either 127.0.0.1 or 0.0.0.0".to_string());
    }

    if cfg.bind_host == "0.0.0.0" && !cfg.auth_required {
        return Err("LAN mode requires auth_required=true".to_string());
    }

    if cfg.cors_mode == CorsMode::Any && !settings.general.developer_mode {
        return Err("cors_mode=any is only allowed in developer mode".to_string());
    }

    if cfg.auth_required && cfg.api_keys_hashed.is_empty() {
        warnings.push("Auth is enabled but no API keys are configured".to_string());
    }

    if settings.chat_presets.presets.is_empty() {
        warnings.push("No presets configured; restoring built-in presets".to_string());
    }

    if web.max_snippets == 0 || web.max_snippets > 25 {
        return Err("web_rag.web_search.max_snippets must be between 1 and 25".to_string());
    }
    if web.max_pages == 0 || web.max_pages > 10 {
        return Err("web_rag.web_search.max_pages must be between 1 and 10".to_string());
    }
    if web.max_retrieval_tokens < 64 {
        return Err("web_rag.web_search.max_retrieval_tokens must be >= 64".to_string());
    }
    if local.chunk_overlap_chars >= local.chunk_size_chars {
        return Err(
            "web_rag.local_rag.chunk_overlap_chars must be smaller than chunk_size_chars"
                .to_string(),
        );
    }
    if local.top_k == 0 || local.top_k > 30 {
        return Err("web_rag.local_rag.top_k must be between 1 and 30".to_string());
    }
    if embeddings.timeout_ms < 1000 || embeddings.timeout_ms > 180_000 {
        return Err(
            "web_rag.embeddings_provider.timeout_ms must be between 1000 and 180000".to_string(),
        );
    }
    if !embeddings.base_url.trim().is_empty() {
        let parsed = reqwest::Url::parse(&embeddings.base_url)
            .map_err(|e| format!("Invalid embeddings base_url: {e}"))?;
        match parsed.scheme() {
            "http" | "https" => {}
            _ => return Err("Embeddings base_url must use http or https".to_string()),
        }
    }
    if (web.pro_beta_enabled || local.beta_enabled) && embeddings.model.trim().is_empty() {
        warnings.push(
            "Embeddings model is not configured; Search Pro/Local RAG will be unavailable"
                .to_string(),
        );
    }

    Ok(warnings)
}

fn ensure_builtin_presets(presets: &mut ChatPresetSettings) {
    let defaults = ChatPresetSettings::default();
    let mut existing_ids: std::collections::HashSet<String> =
        presets.presets.iter().map(|p| p.id.clone()).collect();

    for builtin in defaults.presets {
        if existing_ids.insert(builtin.id.clone()) {
            presets.presets.push(builtin);
        }
    }

    if presets.default_preset_id.is_empty()
        || !presets
            .presets
            .iter()
            .any(|p| p.id == presets.default_preset_id)
    {
        presets.default_preset_id = defaults.default_preset_id;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_lan_requires_auth() {
        let mut settings = AppSettingsV2::default();
        settings.developer.openai_server.bind_host = "0.0.0.0".to_string();
        settings.developer.openai_server.auth_required = false;
        let err = validate_settings(&settings).expect_err("LAN without auth must be rejected");
        assert!(err.contains("LAN mode requires auth_required=true"));
    }

    #[test]
    fn validates_cors_any_requires_developer_mode() {
        let mut settings = AppSettingsV2::default();
        settings.general.developer_mode = false;
        settings.developer.openai_server.cors_mode = CorsMode::Any;
        let err =
            validate_settings(&settings).expect_err("cors any outside developer mode must fail");
        assert!(err.contains("cors_mode=any"));
    }

    #[test]
    fn hashes_and_verifies_api_key() {
        let hashed = hash_api_key("test-key");
        assert!(verify_api_key("test-key", &[hashed]));
        assert!(!verify_api_key("other-key", &[]));
    }
}
