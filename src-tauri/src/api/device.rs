use crate::core::device::device_label;
use crate::core::state::ModelState;
use crate::core::tokenizer::{
    extract_chat_template, find_chat_template_in_metadata, mark_special_chat_tokens,
    tokenizer_from_gguf_metadata,
};

use crate::models::registry::detect_arch;
use crate::models::registry::detect_arch_from_config;
use crate::models::registry::get_model_factory;
use crate::{log_device, log_device_error, log_load};
use candle::quantized::gguf_file;
use candle::utils::{cuda_is_available, metal_is_available};
use std::fs::File;
use std::path::PathBuf;

pub fn set_device(
    guard: &mut ModelState,
    pref: crate::core::types::DevicePreference,
) -> Result<(), String> {
    // Явно проверяем запрос CUDA и возвращаем ошибку, если инициализация не удалась
    match pref {
        crate::core::types::DevicePreference::Cuda { index } => {
            match candle::Device::new_cuda(index) {
                Ok(dev) => {
                    guard.device = dev;
                }
                Err(e) => {
                    return Err(format!("CUDA init failed (index={}): {}", index, e));
                }
            }
        }
        crate::core::types::DevicePreference::Cpu => {
            guard.device = candle::Device::Cpu;
        }
        crate::core::types::DevicePreference::Metal => match candle::Device::new_metal(0) {
            Ok(dev) => {
                guard.device = dev;
            }
            Err(e) => {
                return Err(format!("Metal init failed: {}", e));
            }
        },
        crate::core::types::DevicePreference::Auto => {
            // Авто-выбор с предпочтением CUDA → Metal → CPU
            if cuda_is_available() {
                match candle::Device::new_cuda(0) {
                    Ok(dev) => {
                        guard.device = dev;
                        log_device!("auto -> CUDA");
                    }
                    Err(e) => {
                        log_device_error!("CUDA init failed: {}, fallback to CPU", e);
                        guard.device = candle::Device::Cpu;
                    }
                }
            } else if metal_is_available() {
                match candle::Device::new_metal(0) {
                    Ok(dev) => {
                        guard.device = dev;
                        log_device!("auto -> Metal");
                    }
                    Err(e) => {
                        log_device_error!("Metal init failed: {}, fallback to CPU", e);
                        guard.device = candle::Device::Cpu;
                    }
                }
            } else {
                guard.device = candle::Device::Cpu;
                log_device!("auto -> CPU");
            }
        }
    }
    let label = device_label(&guard.device);
    log_device!("switched -> {}", label);
    {
        // Use default GPU kernel config (BF16 reduced precision enabled)
        let kcfg = crate::core::precision::GpuKernelConfig::default();
        kcfg.apply_for_device(&guard.device);
    }
    log_device!(
        "hw caps: avx={}, neon={}, simd128={}, f16c={}",
        candle::utils::with_avx(),
        candle::utils::with_neon(),
        candle::utils::with_simd128(),
        candle::utils::with_f16c()
    );
    // Если модель загружена — перезагрузим её под выбранное устройство
    // TODO: Поддержка перезагрузки safetensors моделей
    if guard.scheduler.has_model() {
        // Перечитываем с диска по сохранённому пути
        let model_path = match guard.model_path.clone() {
            Some(p) => p,
            None => return Ok(()),
        };

        // Простая проверка: если это GGUF файл
        if model_path.ends_with(".gguf") {
            let ctx_len = guard.context_length.max(1);
            let mut file = File::open(&model_path).map_err(|e| e.to_string())?;
            let content = gguf_file::Content::read(&mut file)
                .map_err(|e| format!("{}", e.with_path(PathBuf::from(model_path.clone()))))?;

            // Токенизатор и шаблон чата
            let mut tokenizer = tokenizer_from_gguf_metadata(&content.metadata)?;
            mark_special_chat_tokens(&mut tokenizer);
            let chat_tpl = extract_chat_template(&tokenizer)
                .or_else(|| find_chat_template_in_metadata(&content.metadata));

            // Архитектура
            let arch = detect_arch(&content.metadata)
                .ok_or_else(|| "Unsupported GGUF architecture".to_string())?;

            // Универсальное создание модели через фабрику (под выбранное устройство)
            let model_backend = get_model_factory()
                .build_from_gguf(arch, content, &mut file, &guard.device, ctx_len, false)
                .map_err(|e| format!("Failed to rebuild model for new device: {}", e))?;

            guard.scheduler.load_model(model_backend, model_path);
            guard.tokenizer = Some(tokenizer);
            guard.chat_template = chat_tpl;
            log_load!("model reloaded for {}", label);
        } else if let Some(files) = guard.safetensors_files.clone() {
            // For safetensors models, we reload from recorded files/config on device switch.
            if files.is_empty() {
                return Err("Cannot reload safetensors: no files recorded".to_string());
            }
            let config_json = guard
                .model_config_json
                .clone()
                .ok_or_else(|| "Cannot reload safetensors: config.json is missing".to_string())?;
            let config: serde_json::Value = serde_json::from_str(&config_json)
                .map_err(|e| format!("Cannot reload safetensors: invalid config.json: {}", e))?;
            let arch = detect_arch_from_config(&config)
                .ok_or_else(|| "Cannot reload safetensors: unsupported architecture".to_string())?;

            let dtype = config
                .get("torch_dtype")
                .and_then(|v| v.as_str())
                .and_then(|s| match s {
                    "bfloat16" => Some(candle::DType::BF16),
                    "float16" => Some(candle::DType::F16),
                    "float32" => Some(candle::DType::F32),
                    _ => None,
                })
                .unwrap_or_else(|| crate::core::precision::select_dtype_default(&guard.device));

            let filenames: Vec<std::path::PathBuf> =
                files.iter().map(std::path::PathBuf::from).collect();

            let model_backend = get_model_factory()
                .build_from_safetensors(arch, &filenames, &config, &guard.device, dtype)
                .map_err(|e| format!("Failed to rebuild safetensors model: {}", e))?;

            let mut tokenizer_opt = None;
            let mut chat_tpl = None;
            if let Some(tokenizer_path) = guard.tokenizer_path.clone()
                && let Ok(bytes) = std::fs::read(&tokenizer_path)
                && let Ok(mut tk) = tokenizers::Tokenizer::from_bytes(&bytes)
            {
                mark_special_chat_tokens(&mut tk);
                chat_tpl = extract_chat_template(&tk);
                tokenizer_opt = Some(tk);
            }
            if chat_tpl.is_none()
                && let Some(model_path) = guard.model_path.clone()
            {
                let model_dir = std::path::Path::new(&model_path);
                let model_dir = if model_dir.is_file() {
                    model_dir.parent().unwrap_or(model_dir)
                } else {
                    model_dir
                };
                let jinja_path = model_dir.join("chat_template.jinja");
                if let Ok(content) = std::fs::read_to_string(&jinja_path) {
                    chat_tpl = Some(content);
                }
            }

            let model_id = guard
                .model_path
                .clone()
                .or_else(|| guard.hub_repo_id.clone())
                .unwrap_or_else(|| "safetensors".to_string());
            guard.scheduler.load_model(model_backend, model_id);
            guard.tokenizer = tokenizer_opt;
            guard.chat_template = chat_tpl;
            guard.arch = Some(arch);
            log_load!("safetensors model reloaded for {}", label);
        } else {
            return Err("Cannot reload model: unknown format or missing metadata".to_string());
        }
    }
    Ok(())
}
