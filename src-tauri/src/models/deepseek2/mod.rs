//! DeepSeek2 model backend
//!
//! Интеграция DeepSeek-V2 (Lite, LiteChat, CoderLiteChat).
//! Поддерживает SafeTensors (через candle-transformers) и GGUF (локальная реализация).

pub mod config;
pub mod fused_moe;
mod gguf;
pub mod model;
pub mod quantized_model;
mod safetensors;

use self::model::ModelForCausalLM;
use self::quantized_model::GGUFDeepSeek2;
use crate::models::ModelBackend;
use crate::models::api::optimization::{OptimizationConfig, WeightFormat};
use candle::{Device, Tensor};

/// Внутреннее представление модели
enum DeepSeek2Inner {
    /// Полная модель из SafeTensors
    Full(ModelForCausalLM),
    /// Квантизированная модель из GGUF
    Quantized(GGUFDeepSeek2),
}

/// DeepSeek2 бекенд
pub struct DeepSeek2Backend {
    inner: DeepSeek2Inner,
    _device: Device,
    vocab_size: usize,
    max_seq_len: usize,
    optimization: OptimizationConfig,
}

impl DeepSeek2Backend {
    /// Создаёт полный бекенд (SafeTensors)
    pub(crate) fn new(
        model: ModelForCausalLM,
        device: Device,
        vocab_size: usize,
        max_seq_len: usize,
        optimization: OptimizationConfig,
    ) -> Self {
        Self {
            inner: DeepSeek2Inner::Full(model),
            _device: device,
            vocab_size,
            max_seq_len,
            optimization,
        }
    }

    /// Создаёт квантизированный бекенд (GGUF)
    pub(crate) fn new_quantized(
        model: GGUFDeepSeek2,
        device: Device,
        vocab_size: usize,
        max_seq_len: usize,
    ) -> Self {
        Self {
            inner: DeepSeek2Inner::Quantized(model),
            _device: device,
            vocab_size,
            max_seq_len,
            optimization: OptimizationConfig::for_gguf(),
        }
    }
}

impl ModelBackend for DeepSeek2Backend {
    fn forward(&mut self, input: &Tensor, pos: usize) -> candle::Result<Tensor> {
        match &mut self.inner {
            DeepSeek2Inner::Full(model) => {
                let logits = model.forward(input, pos)?;
                let seq_len = logits.dim(1)?;
                // Извлекаем последний токен [batch, vocab_size]
                logits.narrow(1, seq_len - 1, 1)?.squeeze(1)
            }
            DeepSeek2Inner::Quantized(model) => model.forward(input, pos),
        }
    }

    fn clear_kv_cache(&mut self) {
        match &mut self.inner {
            DeepSeek2Inner::Full(model) => model.clear_kv_cache(),
            DeepSeek2Inner::Quantized(model) => model.clear_kv_cache(),
        }
    }

    fn model_type(&self) -> &str {
        match self.optimization.weight_format() {
            WeightFormat::Gguf => "deepseek2-gguf",
            WeightFormat::SafeTensors => {
                if self.optimization.uses_flash_attn() {
                    "deepseek2-flash"
                } else {
                    "deepseek2"
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
