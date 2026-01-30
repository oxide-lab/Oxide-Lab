//! Qwen2-MoE model backend
//!
//! Mixture of Experts версия Qwen2.
//! Поддерживает SafeTensors и GGUF форматы.
//!
//! Ключевые особенности Qwen2-MoE:
//! - Sparse Mixture of Experts (разреженные эксперты)
//! - Shared expert (общий эксперт для всех токенов)
//! - Нет per-head RMSNorm (отличие от Qwen3-MoE)

mod fused_moe;
mod gguf;
mod quantized_model;
mod safetensors;

use candle::{Device, Tensor};
use candle_transformers::models::qwen2_moe::Model;
use quantized_model::GGUFQwen2Moe;

use crate::models::ModelBackend;
use crate::models::api::optimization::{OptimizationConfig, WeightFormat};

/// Внутреннее представление модели
enum Qwen2MoeInner {
    /// Квантизированная модель из GGUF
    Quantized(GGUFQwen2Moe),
    /// Полная модель из SafeTensors
    Full(Model),
}

/// Qwen2-MoE бекенд
///
/// Поддерживает полные SafeTensors модели.
/// Квантизированные GGUF модели пока не поддерживаются.
pub struct Qwen2MoeBackend {
    inner: Qwen2MoeInner,
    device: Device,
    vocab_size: usize,
    max_seq_len: usize,
    optimization: OptimizationConfig,
}

impl Qwen2MoeBackend {
    /// Создаёт квантизированный бекенд (используется из gguf.rs)
    pub(crate) fn new_quantized(
        model: GGUFQwen2Moe,
        device: Device,
        vocab_size: usize,
        max_seq_len: usize,
    ) -> Self {
        Self {
            inner: Qwen2MoeInner::Quantized(model),
            device,
            vocab_size,
            max_seq_len,
            optimization: OptimizationConfig::for_gguf(),
        }
    }

    /// Создаёт полный бекенд (используется из safetensors.rs)
    pub(crate) fn new(
        model: Model,
        device: Device,
        vocab_size: usize,
        max_seq_len: usize,
        optimization: OptimizationConfig,
    ) -> Self {
        Self {
            inner: Qwen2MoeInner::Full(model),
            device,
            vocab_size,
            max_seq_len,
            optimization,
        }
    }

    /// Возвращает устройство
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// Проверяет, квантизирована ли модель (всегда false для Qwen2-MoE)
    pub fn is_quantized(&self) -> bool {
        matches!(self.inner, Qwen2MoeInner::Quantized(_))
    }

    /// Возвращает конфигурацию оптимизаций
    pub fn optimization(&self) -> &OptimizationConfig {
        &self.optimization
    }
}

impl ModelBackend for Qwen2MoeBackend {
    fn forward(&mut self, input: &Tensor, pos: usize) -> candle::Result<Tensor> {
        match &mut self.inner {
            // GGUF модель возвращает [batch, vocab_size] - только последний токен
            Qwen2MoeInner::Quantized(model) => model.forward(input, pos),
            // SafeTensors модель возвращает [batch, 1, vocab_size]
            // Извлекаем последнее измерение для совместимости с генерацией
            Qwen2MoeInner::Full(model) => {
                let logits = model.forward(input, pos)?;
                // Model.forward уже делает narrow(1, seq_len - 1, 1)
                // Возвращает [batch, 1, vocab_size], squeeze до [batch, vocab_size]
                logits.squeeze(1)
            }
        }
    }

    fn clear_kv_cache(&mut self) {
        match &mut self.inner {
            Qwen2MoeInner::Quantized(model) => model.clear_kv_cache(),
            Qwen2MoeInner::Full(model) => model.clear_kv_cache(),
        }
    }

    fn model_type(&self) -> &str {
        match self.optimization.weight_format() {
            WeightFormat::Gguf => "qwen2-moe-gguf",
            WeightFormat::SafeTensors => "qwen2-moe",
        }
    }

    fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    fn max_seq_len(&self) -> usize {
        self.max_seq_len
    }

    fn supports_flash_attn(&self) -> bool {
        // qwen2_moe в candle-transformers не использует flash attention
        false
    }
}
