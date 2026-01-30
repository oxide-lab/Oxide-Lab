//! Qwen2-MoE GGUF loading
//!
//! Loads quantized Qwen2-MoE models from GGUF format.

use candle::quantized::gguf_file;
use candle::{DType, Device};
use std::fs::File;

use super::Qwen2MoeBackend;
use super::quantized_model::GGUFQwen2Moe;

impl Qwen2MoeBackend {
    /// Creates a backend from GGUF content.
    pub fn from_gguf(
        content: gguf_file::Content,
        file: &mut File,
        device: &Device,
        dtype: DType,
    ) -> Result<Self, String> {
        let vocab_size = content
            .metadata
            .get("qwen2moe.vocab_size")
            .or_else(|| content.metadata.get("qwen2_moe.vocab_size"))
            .or_else(|| content.metadata.get("qwen2.vocab_size"))
            .or_else(|| content.metadata.get("tokenizer.vocab_size"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(151936) as usize;

        let max_seq_len = content
            .metadata
            .get("qwen2moe.context_length")
            .or_else(|| content.metadata.get("qwen2_moe.context_length"))
            .or_else(|| content.metadata.get("qwen2.context_length"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(32768) as usize;

        log::info!(
            "Loading Qwen2-MoE GGUF: vocab_size={}, max_seq_len={}, dtype={:?}",
            vocab_size,
            max_seq_len,
            dtype
        );

        let inner = GGUFQwen2Moe::from_gguf(content, file, device, dtype)
            .map_err(|e| format!("Failed to load Qwen2-MoE GGUF model: {}", e))?;

        Ok(Self::new_quantized(
            inner,
            device.clone(),
            vocab_size,
            max_seq_len,
        ))
    }

    /// Creates a backend from a GGUF file path.
    pub fn from_gguf_path(
        path: &std::path::Path,
        device: &Device,
        dtype: DType,
    ) -> Result<Self, String> {
        let mut file = File::open(path).map_err(|e| format!("Failed to open GGUF file: {}", e))?;

        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| format!("Failed to read GGUF header: {}", e))?;

        Self::from_gguf(content, &mut file, device, dtype)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_qwen2_moe_gguf_loader_invalid_file() {
        let path = std::env::temp_dir().join("oxide_qwen2_moe_invalid.gguf");
        let mut file = std::fs::File::create(&path).expect("create temp file");
        file.write_all(b"not a gguf").expect("write temp file");

        let res = Qwen2MoeBackend::from_gguf_path(&path, &Device::Cpu, DType::F32);
        assert!(res.is_err());

        let _ = std::fs::remove_file(&path);
    }
}
