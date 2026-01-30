//! Model loading from safetensors files using the ModelBuilder pattern.
//!
//! This module provides functions to load models from safetensors files
//! using the unified ModelBuilder interface.

use hf_hub::{Repo, RepoType, api::sync::Api};
use std::path::Path;

use super::emit_load_progress;
use crate::core::device::{device_label, select_device};
use crate::core::state::ModelState;
use crate::core::template_registry::match_template;
use crate::core::tokenizer::{extract_chat_template, mark_special_chat_tokens};
use crate::core::weights::{
    hub_cache_safetensors, hub_list_safetensors, local_list_safetensors, validate_safetensors_files,
};
use crate::generate::cancel::CANCEL_LOADING;
use crate::models::ModelBackend;
use crate::models::registry::{detect_arch_from_config, get_model_factory};
use crate::{log_hub_error, log_load, log_local_error, log_template};
use std::sync::atomic::Ordering;

/// Load a model from local safetensors files using the ModelBuilder pattern
pub fn load_local_safetensors_model(
    app: &tauri::AppHandle,
    guard: &mut ModelState,
    model_path: String,
    request_context_length: usize, // Shadowed later
    device_pref: Option<crate::core::types::DevicePreference>,
) -> Result<(), String> {
    emit_load_progress(
        app,
        "start",
        0,
        Some("Начало загрузки локальной модели (safetensors)"),
        false,
        None,
    );
    let dev = select_device(device_pref);
    guard.device = dev.clone();
    {
        // Use default GPU kernel config (BF16 reduced precision enabled)
        let kcfg = crate::core::precision::GpuKernelConfig::default();
        kcfg.apply_for_device(&guard.device);
    }
    log_load!("device selected: {}", device_label(&guard.device));
    emit_load_progress(
        app,
        "device",
        5,
        Some(device_label(&guard.device)),
        false,
        None,
    );

    let model_path = Path::new(&model_path);
    if !model_path.exists() {
        return Err(format!(
            "Model path does not exist: {}",
            model_path.display()
        ));
    }

    // Determine if it's a directory or file
    let model_dir = if model_path.is_file() {
        // If it's a file, assume it's in a model directory
        model_path
            .parent()
            .ok_or("Cannot determine parent directory")?
    } else {
        model_path
    };

    // Load tokenizer.json (если есть)
    let tokenizer_path = model_dir.join("tokenizer.json");
    let mut tokenizer_opt = None;
    if tokenizer_path.exists() {
        match std::fs::read(&tokenizer_path) {
            Ok(bytes) => match tokenizers::Tokenizer::from_bytes(&bytes) {
                Ok(mut tk) => {
                    mark_special_chat_tokens(&mut tk);
                    tokenizer_opt = Some(tk);
                }
                Err(e) => log_local_error!("tokenizer.json parse error: {}", e),
            },
            Err(e) => log_local_error!("tokenizer.json read error: {}", e),
        }
    }

    // Load config.json (если есть) и сохраняем как строку
    let config_path = model_dir.join("config.json");
    let config_json = if config_path.exists() {
        match std::fs::read(&config_path) {
            Ok(bytes) => {
                let json_str = String::from_utf8_lossy(&bytes).to_string();
                guard.model_config_json = Some(json_str.clone());
                Some(json_str)
            }
            Err(e) => {
                log_local_error!("config.json read error: {}", e);
                None
            }
        }
    } else {
        None
    };

    // Используем универсальный загрузчик весов для определения списка файлов safetensors
    let filenames = local_list_safetensors(model_dir)
        .map_err(|e| format!("Failed to list safetensors files from local path: {}", e))?;

    // Validate the local safetensors files
    validate_safetensors_files(&filenames)?;
    if CANCEL_LOADING.load(Ordering::SeqCst) {
        emit_load_progress(app, "cancel", 44, Some("Отменено"), true, Some("cancelled"));
        return Err("cancelled".into());
    }
    emit_load_progress(
        app,
        "scan_weights",
        45,
        Some(&format!("{} файлов", filenames.len())),
        false,
        None,
    );

    // Инициализируем tokenizer и chat_template
    let mut chat_tpl = None;
    if let Some(tk) = tokenizer_opt.as_ref() {
        chat_tpl = extract_chat_template(tk);
        if let Some(tpl) = chat_tpl.as_ref() {
            // Fuzzy match against known registry
            if let Some(entry) = match_template(tpl) {
                log::info!(
                    "Replaced SafeTensors template with registry version: {}",
                    entry.name
                );
                chat_tpl = Some(entry.template.to_string());
            }

            let tpl_ref = chat_tpl.as_ref().unwrap();
            let head: String = tpl_ref.chars().take(120).collect();
            log_template!("detected: len={}, head=<<<{}>>>", tpl_ref.len(), head);
        } else {
            // Fallback: загружаем из chat_template.jinja
            let jinja_path = model_dir.join("chat_template.jinja");
            if jinja_path.exists() {
                match std::fs::read_to_string(&jinja_path) {
                    Ok(content) => {
                        let head: String = content.chars().take(120).collect();
                        log_template!(
                            "loaded from chat_template.jinja: len={}, head=<<<{}>>>",
                            content.len(),
                            head
                        );
                        chat_tpl = Some(content);
                    }
                    Err(e) => log_template!("chat_template.jinja read error: {}", e),
                }
            } else {
                log_template!("not found in tokenizer.json or chat_template.jinja");
            }
        }
    }

    // Use ModelBuilder pattern if we have config
    let mut built_model_opt: Option<Box<dyn ModelBackend + Send>> = None;
    let mut context_length = request_context_length;
    if let Some(config_json_str) = config_json {
        // Parse the config JSON
        let config: serde_json::Value = serde_json::from_str(&config_json_str)
            .map_err(|e| format!("Failed to parse config.json: {}", e))?;

        // --- Dynamic Context Autotuning ---
        let n_layer = config
            .get("num_hidden_layers")
            .or_else(|| config.get("n_layer"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let n_embd = config
            .get("hidden_size")
            .or_else(|| config.get("n_embd"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let n_head = config
            .get("num_attention_heads")
            .or_else(|| config.get("n_head"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let n_kv_head = config
            .get("num_key_value_heads")
            .or_else(|| config.get("n_head_kv"))
            .and_then(|v| v.as_u64())
            .unwrap_or(n_head as u64) as usize;
        let head_dim = if n_head > 0 { n_embd / n_head } else { 0 };

        context_length = if n_layer > 0 && n_embd > 0 && n_head > 0 {
            use crate::api::model_loading::context_algo::{
                ModelCacheParams, estimate_best_context,
            };
            use crate::api::model_loading::context_settings::{
                ContextSettingsManager, ContextSource, ModelContextSettings,
            };

            let settings_manager = ContextSettingsManager::new(app);
            let model_id = std::path::Path::new(&model_path)
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| model_path.to_string_lossy().to_string());

            let existing = settings_manager.get_settings(&model_id);
            if let Some(s) = existing {
                log::info!(
                    "Using saved context settings for {}: size={}, source={:?}",
                    model_id,
                    s.size,
                    s.source
                );
                s.size
            } else {
                log::info!("No context settings for {}. Running Autotune...", model_id);
                let cache_params = ModelCacheParams {
                    n_layer,
                    n_kv_head,
                    head_dim,
                    dtype_size: 2,
                };
                let candidates = vec![4096, 8192, 16384, 24576, 32768, 49152, 65536];
                let best_ctx = estimate_best_context(&guard.device, &cache_params, &candidates);
                log::info!("Autotune selected context: {}", best_ctx);
                let new_settings = ModelContextSettings {
                    size: best_ctx,
                    source: ContextSource::Auto,
                    last_autotune: Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    ),
                };
                let _ = settings_manager.save_settings(&model_id, new_settings);
                best_ctx
            }
        } else {
            // Fallback
            request_context_length
        };
        // ----------------------------------
        log::debug!("Autotuned/Calculated context length: {}", context_length);

        // Detect the architecture
        if let Some(arch) = detect_arch_from_config(&config) {
            // Set modality support based on architecture
            // Модальная индикация удалена.
            // Read torch_dtype from config.json, fallback to BF16 on CUDA
            let dtype = config
                .get("torch_dtype")
                .and_then(|v| v.as_str())
                .and_then(|s| match s {
                    "bfloat16" => Some(candle::DType::BF16),
                    "float16" => Some(candle::DType::F16),
                    "float32" => Some(candle::DType::F32),
                    _ => None,
                })
                .unwrap_or_else(|| crate::core::precision::select_dtype_default(&dev));

            log::info!("SafeTensors model dtype: {:?} (from config.json)", dtype);

            // Use the model factory to build the model
            emit_load_progress(app, "build_model", 60, None, false, None);
            match get_model_factory().build_from_safetensors(arch, &filenames, &config, &dev, dtype)
            {
                Ok(model_backend) => {
                    built_model_opt = Some(model_backend);
                    emit_load_progress(
                        app,
                        "build_model_done",
                        85,
                        Some("Модель сконструирована"),
                        false,
                        None,
                    );
                }
                Err(e) => {
                    emit_load_progress(app, "build_model", 65, None, false, Some(&e));
                    log_local_error!("ModelBuilder failed: {}", e)
                }
            }
        }
    }

    if let Some(model) = built_model_opt {
        guard
            .scheduler
            .load_model(model, model_path.to_string_lossy().to_string());
    } else {
        return Err("Failed to build model".into());
    }
    // guard.gguf_file удалено
    guard.tokenizer = tokenizer_opt;
    guard.chat_template = chat_tpl;
    guard.context_length = context_length.max(1);
    guard.model_path = Some(model_path.to_string_lossy().to_string());
    guard.tokenizer_path = tokenizer_path
        .exists()
        .then(|| tokenizer_path.to_string_lossy().to_string());
    guard.hub_repo_id = None;
    guard.hub_revision = None;
    guard.safetensors_files = Some(filenames);
    log_load!(
        "local safetensors loaded with ModelBuilder, context_length={}",
        guard.context_length
    );
    emit_load_progress(
        app,
        "finalize",
        95,
        Some("Состояние обновлено"),
        false,
        None,
    );
    emit_load_progress(app, "complete", 100, Some("Готово"), true, None);

    Ok(())
}

/// Load a model from Hub safetensors files using the ModelBuilder pattern
pub fn load_hub_safetensors_model(
    app: &tauri::AppHandle,
    guard: &mut ModelState,
    repo_id: String,
    revision: Option<String>,
    request_context_length: usize, // Shadowed later
    device_pref: Option<crate::core::types::DevicePreference>,
) -> Result<(), String> {
    emit_load_progress(
        app,
        "start",
        0,
        Some("Начало загрузки из HF Hub (safetensors)"),
        false,
        None,
    );
    let dev = select_device(device_pref);
    guard.device = dev.clone();
    log_load!("device selected: {}", device_label(&guard.device));
    emit_load_progress(
        app,
        "device",
        5,
        Some(device_label(&guard.device)),
        false,
        None,
    );

    // Настраиваем API и репозиторий
    let api = Api::new().map_err(|e| e.to_string())?;
    if !repo_id.contains('/') {
        return Err("repo_id должен быть в формате 'owner/repo'".into());
    }
    let rev = revision.clone().unwrap_or_else(|| "main".to_string());
    let repo = Repo::with_revision(repo_id.clone(), RepoType::Model, rev.clone());
    let api = api.repo(repo);

    // Загружаем tokenizer.json (если есть)
    let tokenizer_path = api.get("tokenizer.json").ok();
    let mut tokenizer_opt = None;
    if let Some(path) = tokenizer_path.as_ref() {
        match std::fs::read(path) {
            Ok(bytes) => match tokenizers::Tokenizer::from_bytes(&bytes) {
                Ok(mut tk) => {
                    mark_special_chat_tokens(&mut tk);
                    tokenizer_opt = Some(tk);
                }
                Err(e) => log_hub_error!("tokenizer.json parse error: {}", e),
            },
            Err(e) => log_hub_error!("tokenizer.json read error: {}", e),
        }
    }

    // Загружаем config.json (если есть) и сохраняем как строку
    let config_json = match api.get("config.json") {
        Ok(cfg_path) => match std::fs::read(&cfg_path) {
            Ok(bytes) => {
                let json_str = String::from_utf8_lossy(&bytes).to_string();
                guard.model_config_json = Some(json_str.clone());
                Some(json_str)
            }
            Err(e) => {
                log_hub_error!("config.json read error: {}", e);
                None
            }
        },
        _ => None,
    };

    // Используем универсальный загрузчик весов для определения списка файлов safetensors
    let filenames = hub_list_safetensors(&api)
        .map_err(|e| format!("Failed to list safetensors files from Hub: {}", e))?;
    emit_load_progress(
        app,
        "hub_list",
        35,
        Some(&format!("{} файлов", filenames.len())),
        false,
        None,
    );

    // Предзагрузим все файлы в кэш (скачать/проверить наличие)
    let cached_filenames = hub_cache_safetensors(&api, &filenames)
        .map_err(|e| format!("Failed to cache safetensors files: {}", e))?;
    emit_load_progress(
        app,
        "hub_cache",
        50,
        Some("Файлы загружены/в кэше"),
        false,
        None,
    );

    // Validate the downloaded safetensors files
    validate_safetensors_files(&cached_filenames)?;

    // Инициализируем tokenizer и chat_template
    let mut chat_tpl = None;
    if let Some(tk) = tokenizer_opt.as_ref() {
        chat_tpl = extract_chat_template(tk);
        if let Some(tpl) = chat_tpl.as_ref() {
            // Fuzzy match against known registry
            if let Some(entry) = match_template(tpl) {
                log::info!(
                    "Replaced SafeTensors template with registry version: {}",
                    entry.name
                );
                chat_tpl = Some(entry.template.to_string());
            }

            let tpl_ref = chat_tpl.as_ref().unwrap();
            let head: String = tpl_ref.chars().take(120).collect();
            log_template!("detected: len={}, head=<<<{}>>>", tpl_ref.len(), head);
        } else {
            // Fallback: загружаем chat_template.jinja из hub
            if let Ok(jinja_path) = api.get("chat_template.jinja") {
                match std::fs::read_to_string(&jinja_path) {
                    Ok(content) => {
                        let head: String = content.chars().take(120).collect();
                        log_template!(
                            "loaded from chat_template.jinja: len={}, head=<<<{}>>>",
                            content.len(),
                            head
                        );
                        chat_tpl = Some(content);
                    }
                    Err(e) => log_template!("chat_template.jinja read error: {}", e),
                }
            } else {
                log_template!("not found in tokenizer.json or chat_template.jinja");
            }
        }
    }

    // Use ModelBuilder pattern if we have config
    let mut built_model_opt: Option<Box<dyn ModelBackend + Send>> = None;
    let mut context_length = request_context_length;

    if let Some(config_json_str) = config_json {
        // Parse the config JSON
        let config: serde_json::Value = serde_json::from_str(&config_json_str)
            .map_err(|e| format!("Failed to parse config.json: {}", e))?;

        // --- Dynamic Context Autotuning ---
        let n_layer = config
            .get("num_hidden_layers")
            .or_else(|| config.get("n_layer"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let n_embd = config
            .get("hidden_size")
            .or_else(|| config.get("n_embd"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let n_head = config
            .get("num_attention_heads")
            .or_else(|| config.get("n_head"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let n_kv_head = config
            .get("num_key_value_heads")
            .or_else(|| config.get("n_head_kv"))
            .and_then(|v| v.as_u64())
            .unwrap_or(n_head as u64) as usize;
        let head_dim = if n_head > 0 { n_embd / n_head } else { 0 };

        context_length = if n_layer > 0 && n_embd > 0 && n_head > 0 {
            use crate::api::model_loading::context_algo::{
                ModelCacheParams, estimate_best_context,
            };
            use crate::api::model_loading::context_settings::{
                ContextSettingsManager, ContextSource, ModelContextSettings,
            };

            let settings_manager = ContextSettingsManager::new(app);
            // Use repo_id as ID
            let model_id = repo_id.clone();

            let existing = settings_manager.get_settings(&model_id);
            if let Some(s) = existing {
                log::info!(
                    "Using saved context settings for {}: size={}, source={:?}",
                    model_id,
                    s.size,
                    s.source
                );
                s.size
            } else {
                log::info!("No context settings for {}. Running Autotune...", model_id);
                let cache_params = ModelCacheParams {
                    n_layer,
                    n_kv_head,
                    head_dim,
                    dtype_size: 2,
                };
                let candidates = vec![4096, 8192, 16384, 24576, 32768, 49152, 65536];
                let best_ctx = estimate_best_context(&guard.device, &cache_params, &candidates);
                log::info!("Autotune selected context: {}", best_ctx);
                let new_settings = ModelContextSettings {
                    size: best_ctx,
                    source: ContextSource::Auto,
                    last_autotune: Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs(),
                    ),
                };
                let _ = settings_manager.save_settings(&model_id, new_settings);
                best_ctx
            }
        } else {
            request_context_length
        };
        // ----------------------------------

        // Detect the architecture
        if let Some(arch) = detect_arch_from_config(&config) {
            // Set modality support based on architecture
            // Модальная индикация удалена.
            // Read torch_dtype from config.json, fallback to BF16 on CUDA
            let dtype = config
                .get("torch_dtype")
                .and_then(|v| v.as_str())
                .and_then(|s| match s {
                    "bfloat16" => Some(candle::DType::BF16),
                    "float16" => Some(candle::DType::F16),
                    "float32" => Some(candle::DType::F32),
                    _ => None,
                })
                .unwrap_or_else(|| crate::core::precision::select_dtype_default(&dev));

            log::info!("SafeTensors model dtype: {:?} (from config.json)", dtype);

            // Use the model factory to build the model
            emit_load_progress(app, "build_model", 70, None, false, None);
            match get_model_factory().build_from_safetensors(
                arch,
                &cached_filenames,
                &config,
                &dev,
                dtype,
            ) {
                Ok(model_backend) => {
                    built_model_opt = Some(model_backend);
                    emit_load_progress(
                        app,
                        "build_model_done",
                        90,
                        Some("Модель сконструирована"),
                        false,
                        None,
                    );
                }
                Err(e) => {
                    emit_load_progress(app, "build_model", 75, None, false, Some(&e));
                    log_hub_error!("ModelBuilder failed: {}", e)
                }
            }
        }
    }

    if let Some(model) = built_model_opt {
        guard.scheduler.load_model(model, repo_id.clone());
    } else {
        return Err("Failed to build model".into());
    }
    // guard.gguf_file удалено
    guard.tokenizer = tokenizer_opt;
    guard.chat_template = chat_tpl;
    guard.context_length = context_length.max(1);
    guard.model_path = None;
    guard.tokenizer_path = tokenizer_path.map(|p| p.to_string_lossy().to_string());
    guard.hub_repo_id = Some(repo_id);
    guard.hub_revision = Some(rev);
    guard.safetensors_files = Some(cached_filenames);
    log_load!(
        "hub safetensors loaded with ModelBuilder, context_length={}",
        guard.context_length
    );
    emit_load_progress(
        app,
        "finalize",
        95,
        Some("Состояние обновлено"),
        false,
        None,
    );
    emit_load_progress(app, "complete", 100, Some("Готово"), true, None);

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_safetensors_loading_function_exists() {
        // This is just a basic test to ensure the function is properly defined
        // Actual testing would require model files and a test environment
    }
}
