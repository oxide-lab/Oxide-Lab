//! Model registry - регистрация и автоопределение архитектур моделей

use candle::quantized::gguf_file::Value;
use std::collections::HashMap;

/// Поддерживаемые архитектуры моделей
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum ArchKind {
    llama,     // Llama 1/2/3, Mistral, Mixtral, Yi, DeepSeek, SmolLM, CodeLlama, etc.
    qwen2,     // Qwen 2, Qwen 2.5
    qwen2moe,  // Qwen 2 MoE
    qwen3,     // Qwen 3
    qwen3moe,  // Qwen 3 MoE (30B-A3B)
    deepseek2, // DeepSeek-V2
}

impl ArchKind {
    /// Возвращает человекочитаемое название
    pub fn display_name(&self) -> &'static str {
        match self {
            ArchKind::llama => "llama",
            ArchKind::qwen2 => "qwen2",
            ArchKind::qwen2moe => "qwen2moe",
            ArchKind::qwen3 => "qwen3",
            ArchKind::qwen3moe => "qwen3moe",
            ArchKind::deepseek2 => "deepseek2",
        }
    }

    /// Проверяет, поддерживается ли GGUF формат
    pub fn supports_gguf(&self) -> bool {
        match self {
            ArchKind::llama
            | ArchKind::qwen2
            | ArchKind::qwen2moe
            | ArchKind::qwen3
            | ArchKind::qwen3moe
            | ArchKind::deepseek2 => true,
        }
    }

    /// Проверяет, поддерживается ли SafeTensors формат
    pub fn supports_safetensors(&self) -> bool {
        true // Все архитектуры поддерживают SafeTensors
    }
}

/// Определяет архитектуру из GGUF метаданных
pub fn detect_arch(metadata: &HashMap<String, Value>) -> Option<ArchKind> {
    // Проверяем поле general.architecture
    let arch_str = metadata.get("general.architecture").and_then(|v| match v {
        Value::String(s) => Some(s.as_str()),
        _ => None,
    })?;

    detect_arch_from_string(arch_str)
}

/// Определяет архитектуру из config.json
pub fn detect_arch_from_config(config: &serde_json::Value) -> Option<ArchKind> {
    // Проверяем model_type
    let model_type = config.get("model_type")?.as_str()?;

    detect_arch_from_string(model_type)
}

/// Определяет архитектуру из строки
pub fn detect_arch_from_string(s: &str) -> Option<ArchKind> {
    let s_lower = s.to_lowercase();

    // Порядок важен - более специфичные первыми
    if s_lower == "qwen3moe" || s_lower == "qwen3_moe" {
        Some(ArchKind::qwen3moe)
    } else if s_lower == "qwen3" {
        Some(ArchKind::qwen3)
    } else if s_lower == "qwen2moe" || s_lower == "qwen2_moe" {
        Some(ArchKind::qwen2moe)
    } else if s_lower == "qwen2" {
        Some(ArchKind::qwen2)
    } else if s_lower.contains("llama") {
        Some(ArchKind::llama)
    } else if s_lower == "deepseek2" || s_lower == "deepseek_v2" {
        Some(ArchKind::deepseek2)
    } else {
        None
    }
}

/// Информация о модели из GGUF
#[derive(Debug, Clone)]
pub struct GgufModelInfo {
    pub arch: Option<ArchKind>,
    pub name: Option<String>,
    pub context_length: Option<usize>,
    pub vocab_size: Option<usize>,
    pub hidden_size: Option<usize>,
    pub num_layers: Option<usize>,
    pub num_heads: Option<usize>,
}

impl GgufModelInfo {
    /// Извлекает информацию из GGUF метаданных
    pub fn from_metadata(metadata: &HashMap<String, Value>) -> Self {
        let arch = detect_arch(metadata);

        let name = metadata.get("general.name").and_then(|v| match v {
            Value::String(s) => Some(s.clone()),
            _ => None,
        });

        let context_length = metadata
            .get("llama.context_length")
            .or_else(|| metadata.get("qwen2.context_length"))
            .or_else(|| metadata.get("gemma.context_length"))
            .and_then(|v| match v {
                Value::U32(n) => Some(*n as usize),
                Value::U64(n) => Some(*n as usize),
                _ => None,
            });

        let vocab_size = metadata.get("tokenizer.vocab_size").and_then(|v| match v {
            Value::U32(n) => Some(*n as usize),
            Value::U64(n) => Some(*n as usize),
            _ => None,
        });

        Self {
            arch,
            name,
            context_length,
            vocab_size,
            hidden_size: None,
            num_layers: None,
            num_heads: None,
        }
    }
}

use super::ModelBackend;
use candle::Device;
use candle::quantized::gguf_file::Content;
use std::sync::OnceLock;

/// Фабрика моделей - создаёт модели из различных форматов
pub struct ModelFactory {
    // В будущем здесь будут зарегистрированные билдеры
}

impl ModelFactory {
    pub fn new() -> Self {
        Self {}
    }

    /// Создаёт модель из GGUF
    pub fn build_from_gguf(
        &self,
        arch: ArchKind,
        content: Content,
        file: &mut std::fs::File,
        device: &Device,
        _context_length: usize,
        _use_flash_attn: bool,
    ) -> Result<Box<dyn ModelBackend + Send>, String> {
        match arch {
            ArchKind::qwen3 => {
                use super::qwen3::Qwen3Backend;
                let model = Qwen3Backend::from_gguf(content, file, device)?;
                Ok(Box::new(model))
            }
            ArchKind::qwen2 => {
                use super::qwen2::Qwen2Backend;
                let model = Qwen2Backend::from_gguf(content, file, device)?;
                Ok(Box::new(model))
            }
            ArchKind::qwen2moe => {
                use super::qwen2_moe::Qwen2MoeBackend;
                let dtype = if device.is_cuda() || device.is_metal() {
                    candle::DType::BF16
                } else {
                    candle::DType::F32
                };
                let model = Qwen2MoeBackend::from_gguf(content, file, device, dtype)?;
                Ok(Box::new(model))
            }
            ArchKind::qwen3moe => {
                use super::qwen3_moe::Qwen3MoeBackend;
                // Default to BF16 for MoE GGUF models
                let dtype = if device.is_cuda() || device.is_metal() {
                    candle::DType::BF16
                } else {
                    candle::DType::F32
                };
                let model = Qwen3MoeBackend::from_gguf(content, file, device, dtype)?;
                Ok(Box::new(model))
            }
            // Llama-подобные архитектуры (Llama, Mistral, Mixtral, DeepSeek, Yi, SmolLM2)
            // LlamaVariant определяется автоматически из metadata
            ArchKind::llama => {
                use super::llama::LlamaBackend;
                let model = LlamaBackend::from_gguf(content, file, device)?;
                Ok(Box::new(model))
            }
            ArchKind::deepseek2 => {
                use super::deepseek2::DeepSeek2Backend;
                let dtype = if device.is_cuda() || device.is_metal() {
                    candle::DType::BF16
                } else {
                    candle::DType::F32
                };
                let model = DeepSeek2Backend::from_gguf(content, file, device, dtype)
                    .map_err(|e| e.to_string())?;
                Ok(Box::new(model))
            }
        }
    }

    /// Создаёт модель из SafeTensors
    pub fn build_from_safetensors<P: AsRef<std::path::Path>>(
        &self,
        arch: ArchKind,
        files: &[P],
        config: &serde_json::Value,
        device: &Device,
        dtype: candle::DType,
    ) -> Result<Box<dyn ModelBackend + Send>, String> {
        // Получаем config_path из первого файла (ищем config.json в той же директории)
        let config_path = files
            .first()
            .and_then(|f| f.as_ref().parent())
            .map(|p| p.join("config.json"))
            .ok_or("No files provided")?;

        let filenames: Vec<std::path::PathBuf> =
            files.iter().map(|p| p.as_ref().to_path_buf()).collect();

        let _ = config; // config уже загружен, используем config_path

        match arch {
            ArchKind::qwen3 => {
                use super::qwen3::Qwen3Backend;
                let model =
                    Qwen3Backend::from_safetensors(&filenames, &config_path, device, dtype)?;
                Ok(Box::new(model))
            }
            ArchKind::qwen2 => {
                use super::qwen2::Qwen2Backend;
                let model =
                    Qwen2Backend::from_safetensors(&filenames, &config_path, device, dtype)?;
                Ok(Box::new(model))
            }
            ArchKind::qwen3moe => {
                use super::qwen3_moe::Qwen3MoeBackend;
                let model =
                    Qwen3MoeBackend::from_safetensors(&filenames, &config_path, device, dtype)?;
                Ok(Box::new(model))
            }
            ArchKind::qwen2moe => {
                use super::qwen2_moe::Qwen2MoeBackend;
                let model =
                    Qwen2MoeBackend::from_safetensors(&filenames, &config_path, device, dtype)?;
                Ok(Box::new(model))
            }
            ArchKind::llama => {
                use super::llama::LlamaBackend;
                let model =
                    LlamaBackend::from_safetensors(&filenames, &config_path, device, dtype)?;
                Ok(Box::new(model))
            }
            ArchKind::deepseek2 => {
                use super::deepseek2::DeepSeek2Backend;
                let model =
                    DeepSeek2Backend::from_safetensors(&filenames, &config_path, device, dtype)
                        .map_err(|e| e.to_string())?;
                Ok(Box::new(model))
            }
        }
    }

    /// Определяет архитектуру из GGUF метаданных
    pub fn detect_gguf_arch(&self, metadata: &HashMap<String, Value>) -> Option<ArchKind> {
        detect_arch(metadata)
    }

    /// Определяет архитектуру из config.json
    pub fn detect_config_arch(&self, config: &serde_json::Value) -> Option<ArchKind> {
        detect_arch_from_config(config)
    }
}

impl Default for ModelFactory {
    fn default() -> Self {
        Self::new()
    }
}

/// Глобальный экземпляр ModelFactory
static MODEL_FACTORY: OnceLock<ModelFactory> = OnceLock::new();

/// Получает глобальный экземпляр ModelFactory
pub fn get_model_factory() -> &'static ModelFactory {
    MODEL_FACTORY.get_or_init(ModelFactory::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_arch_from_string() {
        assert_eq!(detect_arch_from_string("llama"), Some(ArchKind::Llama));
        assert_eq!(detect_arch_from_string("Qwen3"), Some(ArchKind::Qwen3));
        assert_eq!(
            detect_arch_from_string("qwen2_moe"),
            Some(ArchKind::Qwen2Moe)
        );
        // mistral/mixtral/deepseek имеют architecture="llama" в GGUF
        assert_eq!(detect_arch_from_string("glm4v"), Some(ArchKind::Glm4v));
        assert_eq!(
            detect_arch_from_string("deepseek2"),
            Some(ArchKind::DeepSeek2)
        );
        assert_eq!(
            detect_arch_from_string("glm4_moe_lite"),
            Some(ArchKind::DeepSeek2)
        );
        assert_eq!(detect_arch_from_string("unknown"), None);
    }

    #[test]
    fn test_detect_arch_from_config() {
        let cfg: serde_json::Value = serde_json::json!({ "model_type": "glm4" });
        assert_eq!(detect_arch_from_config(&cfg), Some(ArchKind::Glm4));
        let cfg: serde_json::Value = serde_json::json!({ "model_type": "glm4v" });
        assert_eq!(detect_arch_from_config(&cfg), Some(ArchKind::Glm4v));
        let cfg: serde_json::Value = serde_json::json!({ "model_type": "glm4_moe_lite" });
        assert_eq!(detect_arch_from_config(&cfg), Some(ArchKind::DeepSeek2));
    }

    #[test]
    fn test_supports_gguf_flags() {
        assert!(ArchKind::Glm4.supports_gguf());
        assert!(ArchKind::Qwen2Moe.supports_gguf());
    }
}
