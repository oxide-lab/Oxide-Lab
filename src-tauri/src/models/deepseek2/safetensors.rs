//! DeepSeek2 SafeTensors loading
//!
//! Загрузка DeepSeek-V2 моделей из SafeTensors формата.

use super::DeepSeek2Backend;
use super::model::{DeepSeekV2Config, ModelForCausalLM};
use crate::models::api::optimization::OptimizationConfig;
use candle::{DType, Device};
use candle_nn::VarBuilder;
use std::path::{Path, PathBuf};

impl DeepSeek2Backend {
    /// Создаёт бекенд из SafeTensors файлов
    pub fn from_safetensors(
        filenames: &[PathBuf],
        config_path: &Path,
        device: &Device,
        dtype: DType,
    ) -> candle::Result<Self> {
        // Загружаем конфигурацию
        let config_data = std::fs::read(config_path)
            .map_err(|e| candle::Error::Msg(format!("Failed to read config.json: {}", e)))?;
        let config: DeepSeekV2Config = serde_json::from_slice(&config_data)
            .map_err(|e| candle::Error::Msg(format!("Failed to parse config.json: {}", e)))?;

        // Создаём VarBuilder из SafeTensors
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(filenames, dtype, device)
                .map_err(|e| candle::Error::Msg(format!("Failed to load SafeTensors: {}", e)))?
        };

        // Создаём модель
        let inner = ModelForCausalLM::new(&config, vb)
            .map_err(|e| candle::Error::Msg(format!("Failed to build DeepSeek2 model: {}", e)))?;

        // Flash Attention автоматически включается для bf16/f16 на CUDA
        let optimization = OptimizationConfig::for_safetensors(dtype);

        Ok(Self::new(
            inner,
            device.clone(),
            config.vocab_size,
            config.max_position_embeddings,
            optimization,
        ))
    }

    /// Создаёт бекенд из директории модели
    pub fn from_safetensors_dir(
        model_dir: &Path,
        device: &Device,
        dtype: DType,
    ) -> candle::Result<Self> {
        let config_path = model_dir.join("config.json");
        if !config_path.exists() {
            return Err(candle::Error::Msg(
                "config.json not found in model directory".to_string(),
            ));
        }

        let filenames = Self::find_weight_files(model_dir)?;

        Self::from_safetensors(&filenames, &config_path, device, dtype)
    }

    /// Находит файлы весов в директории
    fn find_weight_files(model_dir: &Path) -> candle::Result<Vec<PathBuf>> {
        // Проверяем model.safetensors.index.json (для sharded моделей)
        let index_path = model_dir.join("model.safetensors.index.json");
        if index_path.exists() {
            return Self::load_indexed_files(model_dir, &index_path);
        }

        // Проверяем единственный model.safetensors
        let single_file = model_dir.join("model.safetensors");
        if single_file.exists() {
            return Ok(vec![single_file]);
        }

        Err(candle::Error::Msg(
            "No model.safetensors or model.safetensors.index.json found".to_string(),
        ))
    }

    /// Загружает список файлов из index.json
    fn load_indexed_files(model_dir: &Path, index_path: &Path) -> candle::Result<Vec<PathBuf>> {
        let content = std::fs::read_to_string(index_path)
            .map_err(|e| candle::Error::Msg(format!("Failed to read index: {}", e)))?;

        let index: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| candle::Error::Msg(format!("Failed to parse index: {}", e)))?;

        let weight_map = index
            .get("weight_map")
            .and_then(|v| v.as_object())
            .ok_or_else(|| candle::Error::Msg("weight_map not found in index".to_string()))?;

        let mut file_set: std::collections::HashSet<String> = std::collections::HashSet::new();
        for filename in weight_map.values() {
            if let Some(name) = filename.as_str() {
                file_set.insert(name.to_string());
            }
        }

        let files: Vec<PathBuf> = file_set
            .into_iter()
            .map(|name| model_dir.join(name))
            .filter(|path| path.exists())
            .collect();

        if files.is_empty() {
            return Err(candle::Error::Msg(
                "No SafeTensors files found from index".to_string(),
            ));
        }

        Ok(files)
    }
}
