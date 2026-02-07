use serde::{Deserialize, Serialize};

/// Unified sampling options for text generation
///
/// This struct consolidates all sampling parameters used in text generation
/// with sensible defaults and consistent behavior across different model types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SamplingOptions {
    /// Temperature for sampling (higher values = more random, lower values = more deterministic)
    ///
    /// - Values <= 0.0: Use argmax sampling (always pick the most probable token)
    /// - Values > 0.0: Apply temperature scaling to logits before sampling
    /// - Default: 0.7
    pub temperature: f64,

    /// Top-p (nucleus) sampling parameter
    ///
    /// Limits sampling to the smallest set of tokens whose cumulative probability exceeds this value
    /// - None: Disable top-p sampling
    /// - Some(p) where 0.0 < p < 1.0: Enable top-p sampling with threshold p
    /// - Default: Some(0.9)
    pub top_p: Option<f64>,

    /// Top-k sampling parameter
    ///
    /// Limits sampling to the top k most probable tokens
    /// - None: Disable top-k sampling
    /// - Some(k) where k > 0: Enable top-k sampling with k tokens
    /// - Default: Some(20)
    pub top_k: Option<usize>,

    /// Min-p sampling parameter
    ///
    /// Filters out tokens that are less than p times as probable as the most probable token
    /// - None: Disable min-p filtering
    /// - Some(p) where 0.0 <= p <= 1.0: Enable min-p filtering with threshold p
    /// - Default: Some(0.0)
    pub min_p: Option<f64>,

    /// Random seed for sampling reproducibility
    ///
    /// - None: Use default seed (42)
    /// - Some(seed): Use specified seed
    /// - Default: None
    pub seed: Option<u64>,

    /// Repeat penalty factor for controlling token repetition
    ///
    /// Penalizes repeated tokens to reduce degenerate outputs
    /// - None: Disable repeat penalty
    /// - Some(penalty) where penalty >= 1.0: Enable repeat penalty with multiplier
    /// - Default: Some(1.1)
    pub repeat_penalty: Option<f32>,

    /// Number of previous tokens to consider for repeat penalty
    ///
    /// - 0: Consider all tokens in context
    /// - n > 0: Consider only the last n tokens for penalty calculation
    /// - Default: 64
    pub repeat_last_n: usize,
}

impl SamplingOptions {
    /// Create SamplingOptions with default values suitable for most use cases
    pub fn new() -> Self {
        Self {
            temperature: 0.7,
            top_p: Some(0.9),
            top_k: Some(20),
            min_p: Some(0.0),
            seed: None,
            repeat_penalty: Some(1.1),
            repeat_last_n: 64,
        }
    }

    /// Create SamplingOptions with conservative settings for more deterministic outputs
    pub fn conservative() -> Self {
        Self {
            temperature: 0.2,
            top_p: Some(0.8),
            top_k: Some(10),
            min_p: Some(0.0),
            seed: None,
            repeat_penalty: Some(1.2),
            repeat_last_n: 64,
        }
    }

    /// Create SamplingOptions with creative settings for more diverse outputs
    pub fn creative() -> Self {
        Self {
            temperature: 0.9,
            top_p: Some(0.95),
            top_k: Some(50),
            min_p: Some(0.0),
            seed: None,
            repeat_penalty: Some(1.05),
            repeat_last_n: 64,
        }
    }

    /// Create SamplingOptions that mimic argmax sampling (most deterministic)
    pub fn argmax() -> Self {
        Self {
            temperature: 0.0,
            top_p: None,
            top_k: None,
            min_p: None,
            seed: None,
            repeat_penalty: Some(1.1),
            repeat_last_n: 64,
        }
    }

    /// Update options with custom values from a GenerateRequest
    pub fn with_request_options(mut self, req: &crate::core::types::GenerateRequest) -> Self {
        if req.use_custom_params {
            if let Some(temp) = req.temperature {
                self.temperature = temp;
            }
            if let Some(top_p) = req.top_p {
                self.top_p = Some(top_p);
            }
            if let Some(top_k) = req.top_k {
                self.top_k = Some(top_k);
            }
            if let Some(min_p) = req.min_p {
                self.min_p = Some(min_p);
            }
            if let Some(seed) = req.seed {
                self.seed = Some(seed);
            }
            if let Some(repeat_penalty) = req.repeat_penalty {
                self.repeat_penalty = Some(repeat_penalty);
            }
            self.repeat_last_n = req.repeat_last_n;
        }
        self
    }

    /// Get effective seed value (using default if none specified)
    pub fn effective_seed(&self) -> u64 {
        self.seed.unwrap_or(42)
    }

    /// Check if repeat penalty should be applied
    pub fn should_apply_repeat_penalty(&self) -> bool {
        self.repeat_penalty
            .is_some_and(|rp| (rp - 1.0).abs() > f32::EPSILON)
    }
}

impl Default for SamplingOptions {
    fn default() -> Self {
        Self::new()
    }
}
