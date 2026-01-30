//! Models module
//!
//! Содержит реализации моделей и унифицированное API.

pub mod api;
pub mod common;
pub mod registry;

// Model backends
pub mod deepseek2;
pub mod llama;
pub mod qwen2;
pub mod qwen2_moe;
pub mod qwen3;
pub mod qwen3_moe;
// TODO: Add more models
// pub mod gemma3;
// pub mod phi3;

// Re-exports
pub use api::error::{Error as ApiError, Result as ApiResult};
pub use api::{
    GenerationConfig, HubDownloader, LogitsProcessorBuilder, ModelBackend, OptimizationConfig,
    SamplingStrategy, SimdCapabilities, TextGenerationPipeline, TokenizerWrapper,
};
