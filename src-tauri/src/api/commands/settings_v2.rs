use crate::api::commands::threads::apply_rayon_thread_limit;
use crate::api::openai::OpenAiServerController;
use crate::core::settings_v2::{
    AppSettingsPatch, AppSettingsV2, ClearDataResult, ClearDataScope, DataLocations, ExportResult,
    OpenAiServerConfig, OpenAiServerStatus, SettingsApplyResult, SettingsScope, SettingsV2State,
};
use crate::core::state::{ModelState, SharedState};
use crate::i18n::Locale;
use serde::Serialize;
use simsearch::SimSearch;
use std::path::PathBuf;
use std::str::FromStr;

fn apply_runtime_side_effects(
    app: &tauri::AppHandle,
    shared_state: &SharedState,
    settings: &AppSettingsV2,
) -> Result<(), String> {
    if let Ok(locale) = Locale::from_str(&settings.general.locale) {
        crate::i18n::set_locale(locale);
    }

    apply_rayon_thread_limit(settings.performance.manual_thread_limit);

    {
        let mut guard = shared_state.lock().map_err(|e| e.to_string())?;
        guard.rayon_thread_limit = settings.performance.manual_thread_limit;
        guard.llama_runtime = settings.performance.llama_runtime.clone();
    }

    ModelState::save_thread_limit(app, settings.performance.manual_thread_limit)?;
    ModelState::save_llama_runtime(app, &settings.performance.llama_runtime)?;
    crate::api::commands::experimental::set_experimental_features_enabled(
        app.clone(),
        settings.general.developer_mode,
    )?;

    Ok(())
}

#[tauri::command]
pub fn get_app_settings_v2(
    state: tauri::State<'_, SettingsV2State>,
) -> Result<AppSettingsV2, String> {
    let guard = state.inner.lock().map_err(|e| e.to_string())?;
    Ok(guard.get())
}

#[tauri::command]
pub async fn patch_app_settings_v2(
    app: tauri::AppHandle,
    shared_state: tauri::State<'_, SharedState>,
    settings_state: tauri::State<'_, SettingsV2State>,
    openai_controller: tauri::State<'_, OpenAiServerController>,
    patch: AppSettingsPatch,
) -> Result<SettingsApplyResult, String> {
    let result = {
        let mut guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.apply_patch(patch)?
    };
    apply_runtime_side_effects(&app, shared_state.inner(), &result.settings)?;
    openai_controller
        .apply_config(result.settings.developer.openai_server.clone())
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn reset_app_settings_v2(
    app: tauri::AppHandle,
    shared_state: tauri::State<'_, SharedState>,
    settings_state: tauri::State<'_, SettingsV2State>,
    openai_controller: tauri::State<'_, OpenAiServerController>,
    scope: Option<SettingsScope>,
) -> Result<AppSettingsV2, String> {
    let settings = {
        let mut guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.reset(scope)?
    };
    apply_runtime_side_effects(&app, shared_state.inner(), &settings)?;
    openai_controller
        .apply_config(settings.developer.openai_server.clone())
        .await?;
    Ok(settings)
}

#[tauri::command]
pub fn get_data_locations(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
) -> Result<DataLocations, String> {
    let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
    guard.data_locations(&app)
}

#[tauri::command]
pub fn export_user_data(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
    target_path: String,
) -> Result<ExportResult, String> {
    let target = PathBuf::from(target_path);
    let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
    guard.export_data(&app, &target)
}

#[tauri::command]
pub fn clear_user_data(
    app: tauri::AppHandle,
    settings_state: tauri::State<'_, SettingsV2State>,
    scope: ClearDataScope,
    confirm_token: String,
) -> Result<ClearDataResult, String> {
    let mut guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
    guard.clear_data(&app, scope, &confirm_token)
}

#[tauri::command]
pub async fn get_openai_server_status(
    settings_state: tauri::State<'_, SettingsV2State>,
    openai_controller: tauri::State<'_, OpenAiServerController>,
) -> Result<OpenAiServerStatus, String> {
    let cfg = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.get_ref().developer.openai_server.clone()
    };
    Ok(openai_controller.status_for(&cfg).await)
}

#[tauri::command]
pub async fn set_openai_server_config(
    settings_state: tauri::State<'_, SettingsV2State>,
    openai_controller: tauri::State<'_, OpenAiServerController>,
    config: OpenAiServerConfig,
) -> Result<SettingsApplyResult, String> {
    let result = {
        let mut guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.update_openai_config(config)?
    };
    openai_controller
        .apply_config(result.settings.developer.openai_server.clone())
        .await?;
    Ok(result)
}

#[tauri::command]
pub async fn restart_openai_server(
    settings_state: tauri::State<'_, SettingsV2State>,
    openai_controller: tauri::State<'_, OpenAiServerController>,
) -> Result<OpenAiServerStatus, String> {
    let cfg = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.get_ref().developer.openai_server.clone()
    };
    openai_controller.restart().await?;
    Ok(openai_controller.status_for(&cfg).await)
}

#[derive(Debug, Clone, Serialize)]
pub struct SettingsSearchResult {
    pub id: String,
    pub section: String,
    pub title: String,
    pub description: String,
    pub hidden_by_mode: bool,
}

struct SearchEntry {
    id: &'static str,
    section: &'static str,
    title: &'static str,
    description: &'static str,
    keywords: &'static [&'static str],
    synonyms: &'static [&'static str],
    dev_only: bool,
}

const SEARCH_ENTRIES: &[SearchEntry] = &[
    SearchEntry {
        id: "general.locale",
        section: "general",
        title: "Language",
        description: "Application locale",
        keywords: &["locale", "language", "i18n"],
        synonyms: &["idioma", "язык", "lingua", "lang"],
        dev_only: false,
    },
    SearchEntry {
        id: "general.theme",
        section: "general",
        title: "Theme",
        description: "Light, dark, or system mode",
        keywords: &["theme", "appearance", "dark", "light"],
        synonyms: &["ui style", "tema", "тема"],
        dev_only: false,
    },
    SearchEntry {
        id: "models_storage.models_dir",
        section: "models_storage",
        title: "Models Directory",
        description: "Folder where models are stored",
        keywords: &["models", "path", "folder", "directory"],
        synonyms: &["model path", "путь к моделям", "pasta modelos"],
        dev_only: false,
    },
    SearchEntry {
        id: "models_storage.cache_dir",
        section: "models_storage",
        title: "Cache Directory",
        description: "Folder where caches are stored",
        keywords: &["cache", "path", "folder"],
        synonyms: &["kv cache path", "кэш", "cache pasta"],
        dev_only: false,
    },
    SearchEntry {
        id: "performance.ctx_size",
        section: "performance",
        title: "Context Size",
        description: "Maximum context window",
        keywords: &["context", "ctx", "tokens"],
        synonyms: &["window size", "контекст", "contexto"],
        dev_only: false,
    },
    SearchEntry {
        id: "performance.hardware.gpu_offload",
        section: "hardware",
        title: "Hardware GPU Offload",
        description: "GPU offload layers and VRAM estimation",
        keywords: &["hardware", "gpu", "offload", "layers", "vram"],
        synonyms: &["gpu layers", "аппаратное ускорение", "camadas gpu"],
        dev_only: false,
    },
    SearchEntry {
        id: "performance.hardware.gpu_selection",
        section: "hardware",
        title: "Hardware GPU Selection",
        description: "Choose runtime GPU for model execution",
        keywords: &["hardware", "gpu", "device", "selection"],
        synonyms: &["main gpu", "выбор gpu", "selecao gpu"],
        dev_only: false,
    },
    SearchEntry {
        id: "performance.hardware.cpu_threads",
        section: "hardware",
        title: "Hardware CPU Threads",
        description: "CPU thread tuning for llama runtime",
        keywords: &["hardware", "cpu", "threads", "n_threads"],
        synonyms: &["thread limit", "потоки cpu", "threads cpu"],
        dev_only: false,
    },
    SearchEntry {
        id: "performance.hardware.memory_mapping",
        section: "hardware",
        title: "Memory Mapping",
        description: "Load model in RAM or memory-mapped I/O",
        keywords: &["hardware", "memory", "mmap", "ram"],
        synonyms: &["no mmap", "карта памяти", "mapeamento"],
        dev_only: false,
    },
    SearchEntry {
        id: "performance.hardware.split_gpus",
        section: "hardware",
        title: "Split Across GPUs",
        description: "Distribute model layers across detected GPUs",
        keywords: &["hardware", "gpu", "split", "multi-gpu"],
        synonyms: &["split mode", "несколько gpu", "multi gpu"],
        dev_only: false,
    },
    SearchEntry {
        id: "performance.hardware.batch_size",
        section: "hardware",
        title: "Hardware Batch Size",
        description: "Batch size tuning for throughput and latency",
        keywords: &["hardware", "batch", "throughput", "latency"],
        synonyms: &["token batch", "размер батча", "tamanho batch"],
        dev_only: false,
    },
    SearchEntry {
        id: "performance.hardware.memory_mode",
        section: "hardware",
        title: "Hardware Memory Mode",
        description: "Planner strategy for memory pressure",
        keywords: &["hardware", "memory mode", "planner"],
        synonyms: &["memory profile", "режим памяти", "modo memoria"],
        dev_only: false,
    },
    SearchEntry {
        id: "chat_presets.default_preset",
        section: "chat_presets",
        title: "Default Preset",
        description: "Preset applied to new chats",
        keywords: &["preset", "default", "profile"],
        synonyms: &["template", "mode", "профиль", "predefinicao"],
        dev_only: false,
    },
    SearchEntry {
        id: "chat_presets.temperature",
        section: "chat_presets",
        title: "Temperature",
        description: "Sampling creativity level",
        keywords: &["temperature", "sampling", "randomness"],
        synonyms: &["creativity", "температура", "criatividade"],
        dev_only: false,
    },
    SearchEntry {
        id: "models_storage.model_selector_search",
        section: "models_storage",
        title: "Quantization",
        description: "Model quantization metadata",
        keywords: &["quantization", "gguf", "bits"],
        synonyms: &["4bit", "8bit", "квантизация", "quantizacao"],
        dev_only: false,
    },
    SearchEntry {
        id: "general.search_history_enabled",
        section: "general",
        title: "Search History",
        description: "Save search query history",
        keywords: &["search", "history", "privacy"],
        synonyms: &["query log", "история поиска", "historico busca"],
        dev_only: false,
    },
    SearchEntry {
        id: "privacy_data.export",
        section: "privacy_data",
        title: "Export Data",
        description: "Export local user data",
        keywords: &["export", "backup", "data"],
        synonyms: &["archive", "экспорт", "exportar"],
        dev_only: false,
    },
    SearchEntry {
        id: "developer.openai_server",
        section: "developer",
        title: "OpenAI Server",
        description: "Configure local OpenAI-compatible server",
        keywords: &["api server", "openai", "endpoint", "port", "host"],
        synonyms: &["localhost", "lan", "сервер", "servidor"],
        dev_only: true,
    },
    SearchEntry {
        id: "developer.auth_required",
        section: "developer",
        title: "Auth Required",
        description: "Require API key for requests",
        keywords: &["auth", "api key", "security"],
        synonyms: &["token", "авторизация", "autenticacao"],
        dev_only: true,
    },
    SearchEntry {
        id: "developer.cors",
        section: "developer",
        title: "CORS Mode",
        description: "Cross-origin request policy",
        keywords: &["cors", "origin", "allowlist", "any"],
        synonyms: &["cross origin", "источник", "origem"],
        dev_only: true,
    },
];

fn normalize_search_query(query: &str) -> String {
    let trimmed = query.trim().to_lowercase();
    match trimmed.as_str() {
        "creativity" | "randomness" => "temperature".to_string(),
        "bits" | "gguf" => "quantization".to_string(),
        _ => trimmed,
    }
}

#[tauri::command]
pub fn search_settings_v2(
    settings_state: tauri::State<'_, SettingsV2State>,
    query: String,
) -> Result<Vec<SettingsSearchResult>, String> {
    let normalized = normalize_search_query(&query);
    if normalized.is_empty() {
        return Ok(Vec::new());
    }

    let developer_mode = {
        let guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        guard.get_ref().general.developer_mode
    };

    let mut index: SimSearch<String> = SimSearch::new();
    for entry in SEARCH_ENTRIES {
        let text = format!(
            "{} {} {} {}",
            entry.title,
            entry.description,
            entry.keywords.join(" "),
            entry.synonyms.join(" ")
        );
        index.insert(entry.id.to_string(), &text);
    }

    let mut out = Vec::new();
    for hit_id in index.search(&normalized).into_iter().take(20) {
        let Some(entry) = SEARCH_ENTRIES
            .iter()
            .find(|entry| entry.id == hit_id.as_str())
        else {
            continue;
        };
        out.push(SettingsSearchResult {
            id: entry.id.to_string(),
            section: entry.section.to_string(),
            title: entry.title.to_string(),
            description: entry.description.to_string(),
            hidden_by_mode: entry.dev_only && !developer_mode,
        });
    }

    Ok(out)
}
