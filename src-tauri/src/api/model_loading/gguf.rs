use super::{LoadDebugCtx, emit_load_progress_debug};
use crate::core::device::{device_label, select_device};
use crate::core::performance::ModelLoadTracker;
use crate::core::state::ModelState;
use crate::core::tokenizer::{
    extract_chat_template, find_chat_template_in_metadata, mark_special_chat_tokens,
    tokenizer_from_gguf_metadata,
};
use crate::generate::cancel::CANCEL_LOADING;

use crate::models::registry::{detect_arch, get_model_factory};
use crate::{log_load, log_template, log_template_error};
use candle::quantized::{gguf_file, GgmlDType};
use std::collections::HashSet;
use std::fs::File;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use tauri::Emitter;

pub fn load_gguf_model(
    app: &tauri::AppHandle,
    guard: &mut ModelState,
    model_path: String,
    request_context_length: usize, // Shadowed later
    device_pref: Option<crate::core::types::DevicePreference>,
) -> Result<(), String> {
    let dbg = LoadDebugCtx::new();
    // Создаём трекер загрузки модели
    let tracker_result = tokio::runtime::Runtime::new()
        .map_err(|e| e.to_string())?
        .block_on(async { ModelLoadTracker::new(guard.performance_monitor.clone()).await });
    let mut tracker = tracker_result;

    emit_load_progress_debug(
        &dbg,
        app,
        "start",
        0,
        Some("Начало загрузки GGUF"),
        false,
        None,
    );
    tracker.start_stage("device_selection");

    let dev = select_device(device_pref);
    guard.device = dev;
    {
        // Use default GPU kernel config (BF16 reduced precision enabled)
        let kcfg = crate::core::precision::GpuKernelConfig::default();
        kcfg.apply_for_device(&guard.device);
    }
    log_load!("device selected: {}", device_label(&guard.device));
    emit_load_progress_debug(
        &dbg,
        app,
        "device",
        5,
        Some(device_label(&guard.device)),
        false,
        None,
    );

    let mut file = File::open(&model_path).map_err(|e| {
        emit_load_progress_debug(&dbg, app, "open_file", 8, None, false, Some(&e.to_string()));
        e.to_string()
    })?;
    tracker.start_stage("file_opening");
    emit_load_progress_debug(&dbg, app, "open_file", 10, Some("Файл открыт"), false, None);
    if CANCEL_LOADING.load(Ordering::SeqCst) {
        emit_load_progress_debug(
            &dbg,
            app,
            "cancel",
            12,
            Some("Отменено"),
            true,
            Some("cancelled"),
        );
        return Err("cancelled".into());
    }

    tracker.start_stage("read_header");
    dbg.stage_begin("read_header");
    let read_header_start = std::time::Instant::now();
    let content = gguf_file::Content::read(&mut file).map_err(|e| {
        let error_msg = e.with_path(PathBuf::from(model_path.clone())).to_string();

        // Улучшаем сообщение об ошибке для пользователя
        let enhanced_msg = if error_msg.contains("unknown dtype") {
            format!("{} - This GGUF file contains quantization types that are not supported by the current version of Candle. Try updating Candle to the latest version or use a model with different quantization (Q4_K, Q8_0, etc.)", error_msg)
        } else {
            error_msg
        };

        emit_load_progress_debug(&dbg, app, "read_header", 20, None, false, Some(&enhanced_msg));
        enhanced_msg
    })?;
    dbg.stage_end("read_header", read_header_start.elapsed());

    let rope_metadata: Vec<_> = content
        .metadata
        .iter()
        .filter(|(k, _)| k.contains("rope") || k.contains("freq"))
        .collect();
    log::info!("GGUF RoPE Metadata: {:?}", rope_metadata);

    emit_load_progress_debug(
        &dbg,
        app,
        "read_header",
        25,
        Some("GGUF заголовок прочитан"),
        false,
        None,
    );
    if CANCEL_LOADING.load(Ordering::SeqCst) {
        emit_load_progress_debug(
            &dbg,
            app,
            "cancel",
            28,
            Some("Отменено"),
            true,
            Some("cancelled"),
        );
        return Err("cancelled".into());
    }

    tracker.start_stage("tokenizer_init");
    // Токенизатор обязан быть в метаданных GGUF. Никакого внешнего tokenizer.json не допускается.
    let (mut tokenizer, tokenizer_source): (tokenizers::Tokenizer, &'static str) =
        match tokenizer_from_gguf_metadata(&content.metadata) {
            Ok(tk) => (tk, "embedded"),
            Err(e) => {
                return Err(format!(
                    "Tokenizer must be embedded in GGUF metadata: {}",
                    e
                ));
            }
        };
    mark_special_chat_tokens(&mut tokenizer);
    let chat_tpl = extract_chat_template(&tokenizer)
        .or_else(|| find_chat_template_in_metadata(&content.metadata));
    // Не отключаем шаблон даже при ошибке — логируем, но сохраняем сырой вариант (рендер сам уйдёт в fallback)
    let chat_tpl = match chat_tpl {
        Some(raw) => {
            // Fuzzy match against known registry to fix broken templates
            if let Some(entry) = crate::core::template_registry::match_template(&raw) {
                log::info!(
                    "Replaced GGUF template with registry version: {}",
                    entry.name
                );
                Some(entry.template.to_string())
            } else {
                if let Err(e) = crate::core::prompt::normalize_and_validate(&raw) {
                    log_template_error!(
                        "chat_template validation failed; keeping raw. reason={}; head=<<<{}>>>",
                        e,
                        raw.chars().take(180).collect::<String>()
                    );
                }
                Some(raw)
            }
        }
        None => None,
    };
    match &chat_tpl {
        Some(tpl) => {
            let head: String = tpl.chars().take(120).collect();
            log_template!("detected: len={}, head=<<<{}>>>", tpl.len(), head);
        }
        None => log_template!("not found in tokenizer.json"),
    }
    emit_load_progress_debug(
        &dbg,
        app,
        "tokenizer",
        35,
        Some("Инициализирован"),
        false,
        None,
    );

    // Модальности теперь определяются строго по архитектуре
    let arch = detect_arch(&content.metadata).ok_or_else(|| {
        let err = "Unsupported GGUF architecture".to_string();
        emit_load_progress_debug(&dbg, app, "detect_arch", 38, None, false, Some(&err));
        err
    })?;

    // Проверяем наличие неподдерживаемых типов данных в тензорах
    check_supported_dtypes(&content).map_err(|dtype_error| {
        let error_msg = format!("Unsupported quantization types: {}", dtype_error);
        emit_load_progress_debug(&dbg, app, "dtype_check", 35, None, false, Some(&error_msg));
        log::error!("Model loading blocked: {}", dtype_error);
        error_msg
    })?;
    emit_load_progress_debug(
        &dbg,
        app,
        "detect_arch",
        40,
        Some(&format!("{:?}", arch)),
        false,
        None,
    );
    // Модальная индикация удалена: единая обработка вложений реализуется на уровне проекта.
    // Persist detected architecture in state
    guard.arch = Some(arch);
    if CANCEL_LOADING.load(Ordering::SeqCst) {
        emit_load_progress_debug(
            &dbg,
            app,
            "cancel",
            42,
            Some("Отменено"),
            true,
            Some("cancelled"),
        );
        return Err("cancelled".into());
    }

    if let Some(gg) = content
        .metadata
        .get("config.json")
        .and_then(|v| v.to_string().ok())
        .cloned()
        .or_else(|| {
            content
                .metadata
                .get("tokenizer.ggml.config")
                .and_then(|v| v.to_string().ok())
                .cloned()
        })
        .or_else(|| {
            content
                .metadata
                .get("general.config_json")
                .and_then(|v| v.to_string().ok())
                .cloned()
        })
    {
        guard.model_config_json = Some(gg);
    }

    // --- Dynamic Context Autotuning ---
    // Extract metadata for cache estimation
    let arch_str = format!("{:?}", arch);
    let arch_str_lower = arch_str.to_lowercase();

    // Helper to get u32 from metadata with fallback to lowercase arch
    let get_u32 = |key_suffix: &str| -> u32 {
        content
            .metadata
            .get(&format!("{}.{}", arch_str, key_suffix))
            .or_else(|| {
                content
                    .metadata
                    .get(&format!("{}.{}", arch_str_lower, key_suffix))
            })
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(0)
    };

    let n_layer = get_u32("block_count") as usize;
    let n_embd = get_u32("embedding_length") as usize;
    let n_head = get_u32("attention.head_count") as usize;
    let mut n_kv_head = get_u32("attention.head_count_kv") as usize;
    if n_kv_head == 0 {
        n_kv_head = n_head;
    } // Fallback

    // Estimate head_dim
    let head_dim = if n_head > 0 { n_embd / n_head } else { 0 };

    // Determine final context length
    let context_length = if n_layer > 0 && n_embd > 0 && n_head > 0 {
        use crate::api::model_loading::context_algo::{ModelCacheParams, estimate_best_context};
        use crate::api::model_loading::context_settings::{
            ContextSettingsManager, ContextSource, ModelContextSettings,
        };

        let settings_manager = ContextSettingsManager::new(app);
        // Use filename as ID
        let model_id = std::path::Path::new(&model_path)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown_model".to_string());

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
            // Run Autotune
            log::info!("No context settings for {}. Running Autotune...", model_id);
            let cache_params = ModelCacheParams {
                n_layer,
                n_kv_head,
                head_dim,
                dtype_size: 2, // Assuming F16/Q8 equivalent cache size (safe upper estimation)
            };

            // Candidates: 8192, 16384, 32768, 65536
            // We start small (4096 fallback handled in algo)
            let candidates = vec![4096, 8192, 16384, 24576, 32768, 49152, 65536];

            let best_ctx = estimate_best_context(&guard.device, &cache_params, &candidates);

            log::info!("Autotune selected context: {}", best_ctx);

            // Save it
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
        log::warn!(
            "Could not extract GGUF params for architecture {}. Using requested context.",
            arch_str
        );
        request_context_length
    };

    // Warn if mismatch
    if context_length != request_context_length && request_context_length > 0 {
        log::info!(
            "Context adjusted from requested {} to {}",
            request_context_length,
            context_length
        );
    }
    // ----------------------------------

    tracker.start_stage("model_building");
    // Use the model factory to build the model
    emit_load_progress_debug(&dbg, app, "build_model", 50, None, false, None);
    if CANCEL_LOADING.load(Ordering::SeqCst) {
        emit_load_progress_debug(
            &dbg,
            app,
            "cancel",
            50,
            Some("Отменено"),
            true,
            Some("cancelled"),
        );
        return Err("cancelled".into());
    }
    // Если в метаданных присутствует конфигурация модели, попробуем распарсить и применить её
    let config_json_opt = content
        .metadata
        .get("config.json")
        .and_then(|v| v.to_string().ok())
        .or_else(|| {
            content
                .metadata
                .get("general.config_json")
                .and_then(|v| v.to_string().ok())
        });
    let config_value: Option<serde_json::Value> = match config_json_opt {
        Some(s) => match serde_json::from_str(s) {
            Ok(v) => Some(v),
            Err(e) => {
                emit_load_progress_debug(
                    &dbg,
                    app,
                    "build_model",
                    55,
                    None,
                    false,
                    Some(&format!(
                        "Failed to parse config.json from GGUF metadata: {}",
                        e
                    )),
                );
                None
            }
        },
        None => None,
    };

    dbg.stage_begin("build_model_backend");
    let build_start = std::time::Instant::now();
    let mut model_backend = get_model_factory()
        .build_from_gguf(
            arch,
            content,
            &mut file,
            &guard.device,
            context_length,
            false,
        )
        .map_err(|e| {
            emit_load_progress_debug(&dbg, app, "build_model", 60, None, false, Some(&e));
            format!("Failed to build model: {}", e)
        })?;
    dbg.stage_end("build_model_backend", build_start.elapsed());

    // Если модель предоставляет возможность применения конфигурации - применим
    if let Some(cfg) = config_value.as_ref() {
        if let Err(e) = model_backend.apply_config(cfg) {
            emit_load_progress_debug(
                &dbg,
                app,
                "apply_config",
                70,
                None,
                false,
                Some(&format!("Model apply_config failed: {}", e)),
            );
        } else {
            emit_load_progress_debug(
                &dbg,
                app,
                "apply_config",
                70,
                None,
                false,
                Some("Model config applied"),
            );
        }
    }
    emit_load_progress_debug(
        &dbg,
        app,
        "build_model_done",
        85,
        Some("Модель сконструирована"),
        false,
        None,
    );

    if CANCEL_LOADING.load(Ordering::SeqCst) {
        emit_load_progress_debug(
            &dbg,
            app,
            "cancel",
            90,
            Some("Отменено"),
            true,
            Some("cancelled"),
        );
        return Err("cancelled".into());
    }
    guard
        .scheduler
        .load_model(model_backend, model_path.clone());
    // gguf_file удалён из ModelState
    guard.tokenizer = Some(tokenizer);
    guard.chat_template = chat_tpl;
    let ctx = if context_length == 0 {
        1
    } else {
        context_length
    };
    guard.context_length = ctx;
    guard.model_path = Some(model_path);
    guard.tokenizer_path = None;
    log_load!(
        "gguf loaded, context_length={}, tokenizer_source={}",
        guard.context_length,
        tokenizer_source
    );
    emit_load_progress_debug(
        &dbg,
        app,
        "finalize",
        95,
        Some("Состояние обновлено"),
        false,
        None,
    );

    // Финализируем метрики загрузки
    let model_size_mb = std::fs::metadata(guard.model_path.as_ref().unwrap())
        .map(|m| m.len() as f64 / (1024.0 * 1024.0))
        .unwrap_or(0.0);

    let metrics = tokio::runtime::Runtime::new()
        .map_err(|e| e.to_string())?
        .block_on(async { tracker.finish(model_size_mb).await });

    log_load!(
        "Метрики загрузки: total_time={}ms, memory_delta={:.2}MB, stages={:?}",
        metrics.total_duration_ms,
        metrics.memory_delta_mb,
        metrics
            .stages
            .iter()
            .map(|s| format!("{}:{}ms", s.name, s.duration_ms))
            .collect::<Vec<_>>()
    );

    // Отправляем метрики на фронтенд
    let _ = app.emit("model_load_metrics", &metrics);

    emit_load_progress_debug(&dbg, app, "complete", 100, Some("Готово"), true, None);

    Ok(())
}

/// Проверяет наличие поддерживаемых типов данных в GGUF файле
/// Возвращает ошибку, если найдены неподдерживаемые типы данных
fn check_supported_dtypes(content: &gguf_file::Content) -> Result<(), String> {
    let mut found_unsupported = Vec::new();
    let mut haq_quantized_tensors = false;

    for tensor_info in content.tensor_infos.values() {
        match tensor_info.ggml_dtype {
            GgmlDType::F32
            | GgmlDType::F16
            | GgmlDType::BF16
            | GgmlDType::Q4_0
            | GgmlDType::Q4_1
            | GgmlDType::Q5_0
            | GgmlDType::Q5_1
            | GgmlDType::Q8_0
            | GgmlDType::Q8_1
            | GgmlDType::Q2K
            | GgmlDType::Q3K
            | GgmlDType::Q4K
            | GgmlDType::Q5K
            | GgmlDType::Q6K
            | GgmlDType::Q8K => {
                // Supported
                if tensor_info.ggml_dtype != GgmlDType::F32 
                    && tensor_info.ggml_dtype != GgmlDType::F16 
                    && tensor_info.ggml_dtype != GgmlDType::BF16 {
                    haq_quantized_tensors = true;
                }
            }
            other => {
                // Collect unique unsupported types
                 let dtype_str = format!("{:?}", other);
                 if !found_unsupported.contains(&dtype_str) {
                     found_unsupported.push(dtype_str);
                 }
            }
        }
    }

    // High-precision check
    if !haq_quantized_tensors {
         // Allow high precision if explicit override or just warn?
         // Current logic blocks it. Keeping consistent with previous logic.
         let error_msg = "Pure high-precision GGUF models (F32, F16, BF16) are currently disabled. \
                          These models produce incorrect output on CUDA in the current version of Candle. \
                          Please use a quantized model instead (Q4_K_M, Q5_K_M, Q8_0, etc.)."
            .to_string();
         log::error!(
            "Model loading blocked: Model appears to be high-precision (no quantized tensors found)"
         );
         return Err(error_msg);
    }

    if !found_unsupported.is_empty() {
        let error_msg = format!(
            "Model loading blocked. Found unsupported quantization types: {:?}. \
            Please use a model with supported quantizations (Q4_K_M, Q5_K_M, Q8_0, etc.).",
            found_unsupported
        );
        return Err(error_msg);
    }

    Ok(())
}
