//! Qwen3-MoE model backend
//!
//! Mixture of Experts версия Qwen3.
//! Основано на примере src-tauri/src/models/qwen/main.rs (строки 13, 26, 35, 344-346)
//! и примере quantized-qwen3-moe для GGUF
//!
//! Поддерживает:
//! - SafeTensors (полные веса, BF16/F16)
//! - GGUF (квантизированные модели)

pub mod fused_moe;
mod gguf;
pub mod model;
pub mod quantized_model;
mod safetensors;

use candle::{Device, Tensor};
// Use local quantized model with clear_kv_cache support
use quantized_model::GGUFQWenMoE;
// Use local model with flash-attn support
use model::ModelForCausalLM;

use crate::models::ModelBackend;
use crate::models::api::optimization::{OptimizationConfig, WeightFormat};

/// Внутреннее представление модели
enum Qwen3MoeInner {
    /// Квантизированная модель из GGUF
    Quantized(GGUFQWenMoE),
    /// Полная модель из SafeTensors (с опциональным Flash Attention)
    Full(ModelForCausalLM),
}

/// Qwen3-MoE бекенд
///
/// Поддерживает как квантизированные (GGUF) так и полные (SafeTensors) модели.
pub struct Qwen3MoeBackend {
    inner: Qwen3MoeInner,
    device: Device,
    vocab_size: usize,
    max_seq_len: usize,
    optimization: OptimizationConfig,
}

impl Qwen3MoeBackend {
    /// Создаёт квантизированный бекенд (используется из gguf.rs)
    pub(crate) fn new_quantized(
        model: GGUFQWenMoE,
        device: Device,
        vocab_size: usize,
        max_seq_len: usize,
    ) -> Self {
        Self {
            inner: Qwen3MoeInner::Quantized(model),
            device,
            vocab_size,
            max_seq_len,
            optimization: OptimizationConfig::for_gguf(),
        }
    }

    /// Создаёт полный бекенд (используется из safetensors.rs)
    pub(crate) fn new(
        model: ModelForCausalLM,
        device: Device,
        vocab_size: usize,
        max_seq_len: usize,
        optimization: OptimizationConfig,
    ) -> Self {
        Self {
            inner: Qwen3MoeInner::Full(model),
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

    /// Проверяет, квантизирована ли модель
    pub fn is_quantized(&self) -> bool {
        matches!(self.inner, Qwen3MoeInner::Quantized(_))
    }

    /// Возвращает конфигурацию оптимизаций
    pub fn optimization(&self) -> &OptimizationConfig {
        &self.optimization
    }
}

impl ModelBackend for Qwen3MoeBackend {
    fn forward(&mut self, input: &Tensor, pos: usize) -> candle::Result<Tensor> {
        match &mut self.inner {
            // GGUF модель возвращает [batch, vocab_size] - только последний токен
            Qwen3MoeInner::Quantized(model) => model.forward(input, pos),
            // SafeTensors модель возвращает [batch, seq_len, vocab_size]
            // Извлекаем только последний токен для совместимости с генерацией
            Qwen3MoeInner::Full(model) => {
                let logits = model.forward(input, pos)?;
                let seq_len = logits.dim(1)?;
                // Берём логиты последнего токена: [batch, vocab_size]
                logits.narrow(1, seq_len - 1, 1)?.squeeze(1)
            }
        }
    }

    fn clear_kv_cache(&mut self) {
        match &mut self.inner {
            Qwen3MoeInner::Quantized(model) => {
                // Use local model with clear_kv_cache support
                model.clear_kv_cache();
            }
            Qwen3MoeInner::Full(model) => model.clear_kv_cache(),
        }
    }

    fn model_type(&self) -> &str {
        match self.optimization.weight_format() {
            WeightFormat::Gguf => "qwen3-moe-gguf",
            WeightFormat::SafeTensors => {
                if self.optimization.uses_flash_attn() {
                    "qwen3-moe-flash"
                } else {
                    "qwen3-moe"
                }
            }
        }
    }

    fn vocab_size(&self) -> usize {
        self.vocab_size
    }

    fn max_seq_len(&self) -> usize {
        self.max_seq_len
    }

    fn supports_flash_attn(&self) -> bool {
        self.optimization.uses_flash_attn()
    }
}
