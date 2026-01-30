use candle_nn::Activation;
use serde::{Deserialize, Serialize};

#[doc(hidden)]
#[macro_export]
macro_rules! serde_default_fn {
    ($t:ty, $name:ident, $v:expr) => {
        fn $name() -> $t {
            $v
        }
    };
}

serde_default_fn!(f64, routed_scaling_factor, 1.0);
serde_default_fn!(TopkMethod, topk_method, TopkMethod::Greedy);
serde_default_fn!(usize, moe_layer_freq, 1);
serde_default_fn!(usize, first_k_dense_replace, 0);
serde_default_fn!(bool, norm_topk_prob, false);
serde_default_fn!(ScoringFunc, scoring_func, ScoringFunc::Softmax);
serde_default_fn!(Activation, hidden_act, Activation::Silu);
serde_default_fn!(bool, tie_word_embeddings, false);

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum TopkMethod {
    #[serde(rename = "greedy")]
    Greedy,
    #[serde(rename = "group_limited_greedy")]
    GroupLimitedGreedy,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub enum ScoringFunc {
    #[serde(rename = "softmax")]
    Softmax,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
pub struct DeepSeekV2Config {
    pub vocab_size: usize,
    pub hidden_size: usize,
    pub intermediate_size: usize,
    pub moe_intermediate_size: usize,
    pub num_hidden_layers: usize,
    pub num_attention_heads: usize,
    pub n_shared_experts: Option<usize>,
    pub n_routed_experts: Option<usize>,
    #[serde(default = "routed_scaling_factor")]
    pub routed_scaling_factor: f64,
    #[serde(default = "topk_method")]
    pub topk_method: TopkMethod,
    pub num_experts_per_tok: Option<usize>,
    #[serde(default = "moe_layer_freq")]
    pub moe_layer_freq: usize,
    #[serde(default = "first_k_dense_replace")]
    pub first_k_dense_replace: usize,
    #[serde(default = "norm_topk_prob")]
    pub norm_topk_prob: bool,
    #[serde(default = "scoring_func")]
    pub scoring_func: ScoringFunc,
    #[serde(default = "hidden_act")]
    pub hidden_act: Activation,
    pub max_position_embeddings: usize,
    pub rms_norm_eps: f64,
    #[serde(default = "tie_word_embeddings")]
    pub tie_word_embeddings: bool,
    pub rope_theta: f32,
    pub rope_scaling: Option<DeepSeekV2RopeScaling>,
    pub attention_bias: bool,
    pub q_lora_rank: Option<usize>,
    pub qk_rope_head_dim: usize,
    pub kv_lora_rank: usize,
    pub v_head_dim: usize,
    pub qk_nope_head_dim: usize,
    pub n_group: usize,
    pub topk_group: usize,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ScaledRopeType {
    #[serde(alias = "su")]
    #[serde(alias = "longrope")]
    Su,
    #[serde(alias = "yarn")]
    Yarn,
    #[serde(alias = "dynamic")]
    Dynamic,
    #[serde(alias = "linear")]
    Linear,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum DeepSeekV2RopeScaling {
    Yarn {
        original_max_position_embeddings: usize,
        beta_fast: f32,
        beta_slow: f32,
        mscale: f32,
        mscale_all_dim: f32,
        factor: f32,
        #[serde(rename = "type")]
        scaling_type: ScaledRopeType,
    },
    LinearOrDynamic {
        #[serde(rename = "type")]
        scaling_type: ScaledRopeType,
        factor: f64,
    },
}
