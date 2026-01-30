//! DeepSeek2 Model wrapper
//!
//! Compatibility wrapper over candle-transformers deepseek2.

pub use super::config::*;
use candle::{Result, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::deepseek2::DeepSeekV2;

/// Обертка для DeepSeekV2 для обеспечения совместимости с нашим API.
pub struct ModelForCausalLM {
    inner: DeepSeekV2,
}

impl ModelForCausalLM {
    /// Создаёт новую модель
    pub fn new(cfg: &DeepSeekV2Config, vb: VarBuilder) -> Result<Self> {
        // Конвертируем наш конфиг в конфиг candle-transformers через JSON
        // Это необходимо, так как поля в оригинальном конфиге приватные
        let json = serde_json::to_value(cfg).map_err(|e| candle::Error::Msg(e.to_string()))?;
        let lib_cfg: candle_transformers::models::deepseek2::DeepSeekV2Config =
            serde_json::from_value(json).map_err(|e| candle::Error::Msg(e.to_string()))?;

        let inner = DeepSeekV2::new(&lib_cfg, vb)?;
        Ok(Self { inner })
    }

    /// Forward pass
    pub fn forward(&mut self, input: &Tensor, pos: usize) -> Result<Tensor> {
        self.inner.forward(input, pos)
    }

    /// Очищает KV-кэш
    pub fn clear_kv_cache(&mut self) {
        self.inner.clear_kv_cache();
    }
}
