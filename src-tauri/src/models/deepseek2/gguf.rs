//! DeepSeek2 GGUF loading
//!
//! Загрузка квантизированных DeepSeek-V2 моделей из GGUF формата.

use candle::quantized::gguf_file;
use candle::{DType, Device};
use std::fs::File;

use super::DeepSeek2Backend;
use super::quantized_model::GGUFDeepSeek2;

impl DeepSeek2Backend {
    /// Создаёт бекенд из GGUF Content
    pub fn from_gguf(
        content: gguf_file::Content,
        file: &mut File,
        device: &Device,
        dtype: DType,
    ) -> candle::Result<Self> {
        // Извлекаем метаданные из GGUF
        let arch_val = content
            .metadata
            .get("general.architecture")
            .and_then(|v| match v {
                gguf_file::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "deepseek2".to_string());
        let arch = arch_val.as_str();

        let vocab_size = content
            .metadata
            .get(&format!("{arch}.vocab_size"))
            .or_else(|| content.metadata.get("tokenizer.vocab_size"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(102400) as usize;

        let max_seq_len = content
            .metadata
            .get(&format!("{arch}.context_length"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(163840) as usize;

        log::info!(
            "Loading DeepSeek2 GGUF: arch={}, vocab_size={}, max_seq_len={}, dtype={:?}",
            arch,
            vocab_size,
            max_seq_len,
            dtype
        );

        // Создаём модель
        let inner = GGUFDeepSeek2::from_gguf(content, file, device, dtype).map_err(|e| {
            candle::Error::Msg(format!("Failed to load DeepSeek2 GGUF model: {}", e))
        })?;

        Ok(Self::new_quantized(
            inner,
            device.clone(),
            vocab_size,
            max_seq_len,
        ))
    }

    /// Создаёт бекенд из пути к GGUF файлу
    pub fn from_gguf_path(
        path: &std::path::Path,
        device: &Device,
        dtype: DType,
    ) -> candle::Result<Self> {
        let mut file = File::open(path)
            .map_err(|e| candle::Error::Msg(format!("Failed to open GGUF file: {}", e)))?;

        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| candle::Error::Msg(format!("Failed to read GGUF header: {}", e)))?;

        Self::from_gguf(content, &mut file, device, dtype)
    }
}
