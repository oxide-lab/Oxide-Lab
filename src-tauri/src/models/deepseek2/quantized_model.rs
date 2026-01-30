//! DeepSeek2 GGUF Quantized Model implementation
//!
//! Based on candle-transformers deepseek2 and qwen3_moe implementation patterns.

use candle::quantized::{GgmlDType, gguf_file};
use candle::{D, DType, Device, IndexOp, Result, Tensor};
use candle_nn::kv_cache::ConcatKvCache;
use candle_nn::{Embedding, Module};
use candle_transformers::models::with_tracing::QMatMul;
use candle_transformers::quantized_nn::RmsNorm;
use std::sync::Arc;

use super::config::*;
use super::fused_moe::{ExpertWeights, FusedMoeGGUF};
use candle_transformers::models::deepseek2::{DeepSeekV2RopeConfig, DeepSeekV2RotaryEmbedding};

#[derive(Debug, Clone)]
struct Mlp {
    up_proj: QMatMul,
    gate_proj: QMatMul,
    down_proj: QMatMul,
}

impl Mlp {
    fn new<R: std::io::Seek + std::io::Read>(gg: &mut Gguf<R>, prefix: &str) -> Result<Self> {
        let up_proj = gg.qmatmul(&format!("{prefix}.ffn_up.weight"))?;
        let gate_proj = gg.qmatmul(&format!("{prefix}.ffn_gate.weight"))?;
        let down_proj = gg.qmatmul(&format!("{prefix}.ffn_down.weight"))?;
        Ok(Self {
            up_proj,
            gate_proj,
            down_proj,
        })
    }
}

impl Module for Mlp {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let up = self.up_proj.forward(xs)?;
        let gate = self.gate_proj.forward(xs)?;
        self.down_proj
            .forward(&(candle_nn::ops::silu(&gate)? * up)?)
    }
}

#[derive(Debug, Clone)]
struct SharedExperts {
    up_proj: QMatMul,
    gate_proj: QMatMul,
    down_proj: QMatMul,
}

impl SharedExperts {
    fn new<R: std::io::Seek + std::io::Read>(gg: &mut Gguf<R>, prefix: &str) -> Result<Self> {
        let up_proj = gg.qmatmul(&format!("{prefix}.ffn_up_shexp.weight"))?;
        let gate_proj = gg.qmatmul(&format!("{prefix}.ffn_gate_shexp.weight"))?;
        let down_proj = gg.qmatmul(&format!("{prefix}.ffn_down_shexp.weight"))?;
        Ok(Self {
            up_proj,
            gate_proj,
            down_proj,
        })
    }
}

impl Module for SharedExperts {
    fn forward(&self, xs: &Tensor) -> Result<Tensor> {
        let up = self.up_proj.forward(xs)?;
        let gate = self.gate_proj.forward(xs)?;
        self.down_proj
            .forward(&(candle_nn::ops::silu(&gate)? * up)?)
    }
}

enum MoeOrMlp {
    FusedMoe(FusedMoeGGUF),
    Mlp(Mlp),
}

impl MoeOrMlp {
    fn forward(&self, xs: &Tensor, is_prefill: bool) -> Result<Tensor> {
        match self {
            Self::Mlp(m) => m.forward(xs),
            Self::FusedMoe(m) => m.forward(xs, is_prefill),
        }
    }
}

pub struct QuantizedAttention {
    kv_a_proj_with_mqa: QMatMul,
    kv_a_layernorm: candle_transformers::quantized_nn::RmsNorm,
    kv_b_proj: QMatMul,
    q_proj: QMatMul,
    q_a_proj: Option<QMatMul>,
    q_a_layernorm: Option<candle_transformers::quantized_nn::RmsNorm>,
    q_b_proj: Option<QMatMul>,
    o_proj: QMatMul,
    rotary_emb: Arc<DeepSeekV2RotaryEmbedding>,
    cfg: DeepSeekV2Config,
    kv_cache: ConcatKvCache,
}

impl QuantizedAttention {
    #[allow(clippy::too_many_arguments)]
    pub fn new<R: std::io::Seek + std::io::Read>(
        gg: &mut Gguf<R>,
        prefix: &str,
        _dtype: DType,
        _device: &Device,
        cfg: &DeepSeekV2Config,
        rotary_emb: Arc<DeepSeekV2RotaryEmbedding>,
    ) -> Result<Self> {
        let kv_a_proj_with_mqa = gg.qmatmul(&format!("{prefix}.attn_kv_a_mqa.weight"))?;
        let kv_a_layernorm = {
            let w = gg.tensor(&format!("{prefix}.attn_kv_a_norm.weight"))?;
            candle_transformers::quantized_nn::RmsNorm::from_qtensor(w, 1e-6)?
        };
        let kv_b_proj = gg.qmatmul(&format!("{prefix}.attn_kv_b.weight"))?;

        let (q_a_proj, q_a_layernorm, q_b_proj, q_proj) = if let Some(_rank) = cfg.q_lora_rank {
            let a = Some(gg.qmatmul(&format!("{prefix}.attn_q_a.weight"))?);
            let w = gg.tensor(&format!("{prefix}.attn_q_a_norm.weight"))?;
            let norm = Some(candle_transformers::quantized_nn::RmsNorm::from_qtensor(
                w, 1e-6,
            )?);
            let b = Some(gg.qmatmul(&format!("{prefix}.attn_q_b.weight"))?);
            (a, norm, b, gg.qmatmul(&format!("{prefix}.attn_q.weight"))?)
        } else {
            (
                None,
                None,
                None,
                gg.qmatmul(&format!("{prefix}.attn_q.weight"))?,
            )
        };

        let o_proj = gg.qmatmul(&format!("{prefix}.attn_output.weight"))?;
        let kv_cache = ConcatKvCache::new(2);

        Ok(Self {
            kv_a_proj_with_mqa,
            kv_a_layernorm,
            kv_b_proj,
            q_proj,
            q_a_proj,
            q_a_layernorm,
            q_b_proj,
            o_proj,
            rotary_emb,
            cfg: cfg.clone(),
            kv_cache,
        })
    }

    pub fn forward(
        &mut self,
        x: &Tensor,
        mask: Option<&Tensor>,
        input_pos: usize,
    ) -> Result<Tensor> {
        let (b_sz, seq_len, _) = x.dims3()?;
        let q = if let (Some(q_a), Some(q_norm), Some(q_b)) =
            (&self.q_a_proj, &self.q_a_layernorm, &self.q_b_proj)
        {
            let x = q_a.forward(x)?;
            let x = q_norm.forward(&x)?;
            q_b.forward(&x)?
        } else {
            self.q_proj.forward(x)?
        };

        let kv = self.kv_a_proj_with_mqa.forward(x)?;
        let kv_lora_rank = self.cfg.kv_lora_rank;
        let qk_rope_head_dim = self.cfg.qk_rope_head_dim;
        let qk_nope_head_dim = self.cfg.qk_nope_head_dim;
        let v_head_dim = self.cfg.v_head_dim;
        let num_attention_heads = self.cfg.num_attention_heads;

        let kv_compressed = kv.narrow(D::Minus1, 0, kv_lora_rank)?.contiguous()?;
        let k_rope = kv
            .narrow(D::Minus1, kv_lora_rank, qk_rope_head_dim)?
            .contiguous()?;

        let kv_compressed = self.kv_a_layernorm.forward(&kv_compressed)?;
        let kv_decompressed = self.kv_b_proj.forward(&kv_compressed)?;

        let q_head_dim = qk_nope_head_dim + qk_rope_head_dim;
        let q = q
            .reshape((b_sz, seq_len, num_attention_heads, q_head_dim))?
            .contiguous()?;
        let q_nope = q.narrow(D::Minus1, 0, qk_nope_head_dim)?.contiguous()?;
        let q_rope = q
            .narrow(D::Minus1, qk_nope_head_dim, qk_rope_head_dim)?
            .contiguous()?;

        let kv_decompressed = kv_decompressed
            .reshape((
                b_sz,
                seq_len,
                num_attention_heads,
                qk_nope_head_dim + v_head_dim,
            ))?
            .contiguous()?;
        let k_nope = kv_decompressed
            .narrow(D::Minus1, 0, qk_nope_head_dim)?
            .contiguous()?;
        let v_base = kv_decompressed
            .narrow(D::Minus1, qk_nope_head_dim, v_head_dim)?
            .contiguous()?;

        // Apply RoPE
        let (q_rope, k_rope) = {
            let original_dtype = q_rope.dtype();
            let q_rope = q_rope.transpose(1, 2)?.contiguous()?.to_dtype(DType::F32)?;
            let k_rope = k_rope
                .unsqueeze(2)?
                .transpose(1, 2)?
                .contiguous()?
                .to_dtype(DType::F32)?;
            log::info!(
                "RoPE forward: q_rope dtype={:?}, k_rope dtype={:?}",
                q_rope.dtype(),
                k_rope.dtype()
            );
            let (q_rope, k_rope) = self.rotary_emb.forward(&q_rope, &k_rope, input_pos)?;
            (
                q_rope
                    .transpose(1, 2)?
                    .contiguous()?
                    .to_dtype(original_dtype)?,
                k_rope
                    .transpose(1, 2)?
                    .squeeze(2)?
                    .contiguous()?
                    .to_dtype(original_dtype)?,
            )
        };

        // Combine
        let q = Tensor::cat(&[q_nope, q_rope], D::Minus1)?.contiguous()?;
        let k = Tensor::cat(
            &[
                k_nope,
                k_rope.unsqueeze(2)?.broadcast_as((
                    b_sz,
                    seq_len,
                    num_attention_heads,
                    qk_rope_head_dim,
                ))?,
            ],
            D::Minus1,
        )?
        .contiguous()?;

        let (k_cached, v_cached) = self.kv_cache.append(&k, &v_base)?;

        let scale = 1.0 / (q_head_dim as f64).sqrt();
        let q = q.transpose(1, 2)?.contiguous()?;
        let k = k_cached.transpose(1, 2)?.contiguous()?;
        let v = v_cached.transpose(1, 2)?.contiguous()?;

        let mut scores = (q.matmul(&k.transpose(2, 3)?.contiguous()?)? * scale)?;
        if let Some(m) = mask {
            scores = scores.broadcast_add(m)?;
        }
        let probs = candle_nn::ops::softmax_last_dim(&scores)?;
        let xs = probs.matmul(&v)?;
        let xs = xs
            .transpose(1, 2)?
            .contiguous()?
            .reshape((b_sz, seq_len, ()))?;

        self.o_proj.forward(&xs)
    }

    pub fn clear_kv_cache(&mut self) {
        self.kv_cache.reset();
    }
}

pub struct Gguf<'a, R: std::io::Read + std::io::Seek> {
    ct: gguf_file::Content,
    reader: &'a mut R,
    device: Device,
}

impl<'a, R: std::io::Read + std::io::Seek> Gguf<'a, R> {
    pub fn new(ct: gguf_file::Content, reader: &'a mut R, device: Device) -> Self {
        Self { ct, reader, device }
    }
    pub fn qmatmul(&mut self, name: &str) -> Result<QMatMul> {
        let tensor = self.ct.tensor(self.reader, name, &self.device)?;
        QMatMul::from_weights(Arc::new(tensor))
    }
    pub fn rms_norm(
        &mut self,
        name: &str,
        eps: f64,
    ) -> Result<candle_transformers::quantized_nn::RmsNorm> {
        let tensor = self.ct.tensor(self.reader, name, &self.device)?;
        candle_transformers::quantized_nn::RmsNorm::from_qtensor(tensor, eps)
    }
    pub fn tensor(&mut self, name: &str) -> Result<candle::quantized::QTensor> {
        self.ct.tensor(self.reader, name, &self.device)
    }
    pub fn metadata(
        &self,
    ) -> &std::collections::HashMap<String, candle::quantized::gguf_file::Value> {
        &self.ct.metadata
    }
    pub fn device(&self) -> &Device {
        &self.device
    }
}

pub struct GGUFDeepSeek2 {
    embed_tokens: Embedding,
    layers: Vec<DecoderLayer>,
    norm: RmsNorm,
    output: QMatMul,
    device: Device,
}

struct DecoderLayer {
    input_layernorm: RmsNorm,
    post_attention_layernorm: RmsNorm,
    attn: QuantizedAttention,
    mlp: MoeOrMlp,
    shared_experts: Option<SharedExperts>,
}

impl GGUFDeepSeek2 {
    pub fn from_gguf<R: std::io::Seek + std::io::Read>(
        ct: gguf_file::Content,
        reader: &mut R,
        device: &Device,
        dtype: DType,
    ) -> Result<Self> {
        let mut gg = Gguf::new(ct, reader, device.clone());

        // Get architecture prefix from metadata
        let arch = gg
            .metadata()
            .get("general.architecture")
            .and_then(|v| match v {
                gguf_file::Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "deepseek2".to_string());

        log::info!("DeepSeek2 GGUF: detected architecture prefix: {}", arch);

        // Helper to get metadata with fallback
        let md_get = |key: &str| -> Option<&gguf_file::Value> { gg.metadata().get(key) };

        // Read configuration from GGUF metadata with fallbacks to DeepSeek-V2-Lite defaults
        let vocab_size = md_get(&format!("{arch}.vocab_size"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(102400) as usize;

        let hidden_size = md_get(&format!("{arch}.embedding_length"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(2048) as usize;

        let intermediate_size = md_get(&format!("{arch}.feed_forward_length"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(10944) as usize;

        let moe_intermediate_size = md_get(&format!("{arch}.expert_feed_forward_length"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(1408) as usize;

        let num_hidden_layers = md_get(&format!("{arch}.block_count"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(27) as usize;

        let num_attention_heads = md_get(&format!("{arch}.attention.head_count"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(16) as usize;

        let n_shared_experts = md_get(&format!("{arch}.expert_shared_count"))
            .and_then(|v| v.to_u32().ok())
            .map(|v| v as usize);

        let n_routed_experts = md_get(&format!("{arch}.expert_count"))
            .and_then(|v| v.to_u32().ok())
            .map(|v| v as usize);

        let num_experts_per_tok = md_get(&format!("{arch}.expert_used_count"))
            .and_then(|v| v.to_u32().ok())
            .map(|v| v as usize);

        let rms_norm_eps = md_get(&format!("{arch}.attention.layer_norm_rms_epsilon"))
            .and_then(|v| v.to_f32().ok())
            .unwrap_or(1e-6) as f64;

        let max_position_embeddings = md_get(&format!("{arch}.context_length"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(163840) as usize;

        let rope_theta = md_get(&format!("{arch}.rope.freq_base"))
            .and_then(|v| v.to_f32().ok())
            .unwrap_or(10000.0) as f64;

        let qk_rope_head_dim = md_get(&format!("{arch}.rope.dimension_count"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(64) as usize;

        let kv_lora_rank = md_get(&format!("{arch}.attention.kv_lora_rank"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(512) as usize;

        let q_lora_rank = md_get(&format!("{arch}.attention.q_lora_rank"))
            .and_then(|v| v.to_u32().ok())
            .map(|v| v as usize);

        let v_head_dim = md_get(&format!("{arch}.attention.value_head_dim"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(128) as usize;

        let qk_nope_head_dim = md_get(&format!("{arch}.attention.qk_nope_head_dim"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(128) as usize;

        let first_k_dense_replace = md_get(&format!("{arch}.leading_dense_block_count"))
            .and_then(|v| v.to_u32().ok())
            .unwrap_or(1) as usize;

        log::info!(
            "DeepSeek2 GGUF config: hidden_size={}, layers={}, heads={}, experts={:?}, kv_lora_rank={}",
            hidden_size,
            num_hidden_layers,
            num_attention_heads,
            n_routed_experts,
            kv_lora_rank
        );

        let cfg = DeepSeekV2Config {
            vocab_size,
            hidden_size,
            intermediate_size,
            moe_intermediate_size,
            num_hidden_layers,
            num_attention_heads,
            n_shared_experts,
            n_routed_experts,
            routed_scaling_factor: 1.0,
            num_experts_per_tok,
            moe_layer_freq: 1,
            first_k_dense_replace,
            norm_topk_prob: false,
            rms_norm_eps,
            max_position_embeddings,
            rope_theta: rope_theta as f32,
            attention_bias: false,
            q_lora_rank,
            qk_rope_head_dim,
            kv_lora_rank,
            v_head_dim,
            qk_nope_head_dim,
            n_group: 1,
            topk_group: 1,
            topk_method: TopkMethod::Greedy,
            scoring_func: ScoringFunc::Softmax,
            rope_scaling: None,
            tie_word_embeddings: false,
            hidden_act: candle_nn::Activation::Silu,
        };

        log::info!("DeepSeek2 GGUF: loading token embeddings...");
        let embed_tokens = gg.tensor("token_embd.weight")?.dequantize(device)?;
        let embed_tokens = Embedding::new(embed_tokens, hidden_size);

        log::info!("DeepSeek2 GGUF: loading output norm and lm_head...");
        let norm = gg.rms_norm("output_norm.weight", rms_norm_eps)?;
        let output = match gg.qmatmul("output.weight") {
            Ok(v) => v,
            _ => {
                log::info!("DeepSeek2 GGUF: output.weight not found, using tie_word_embeddings");
                gg.qmatmul("token_embd.weight")?
            }
        };

        let rope_scaling_json = serde_json::to_value(&cfg.rope_scaling)
            .map_err(|e| candle::Error::Msg(e.to_string()))?;
        let rope_scaling: Option<candle_transformers::models::deepseek2::DeepSeekV2RopeScaling> =
            serde_json::from_value(rope_scaling_json)
                .map_err(|e| candle::Error::Msg(e.to_string()))?;

        let rope_cfg = DeepSeekV2RopeConfig {
            rope_scaling,
            max_position_embeddings: cfg.max_position_embeddings,
            rope_theta: cfg.rope_theta,
            qk_rope_head_dim: cfg.qk_rope_head_dim,
        };
        // RoPE must use F32 for precision even if model uses BF16
        log::info!(
            "DeepSeek2 GGUF: creating RoPE with dtype=F32 (model dtype={:?})",
            dtype
        );
        let rotary_emb = Arc::new(DeepSeekV2RotaryEmbedding::new(
            &rope_cfg,
            DType::F32,
            device,
        )?);

        log::info!(
            "DeepSeek2 GGUF: loading {} decoder layers...",
            num_hidden_layers
        );
        let mut layers = Vec::with_capacity(num_hidden_layers);
        for i in 0..num_hidden_layers {
            let prefix = format!("blk.{i}");
            log::info!(
                "DeepSeek2 GGUF: loading layer {}/{}...",
                i + 1,
                num_hidden_layers
            );
            log::info!("  -> loading {}.attn_norm.weight", prefix);
            let input_layernorm =
                gg.rms_norm(&format!("{prefix}.attn_norm.weight"), rms_norm_eps)?;
            log::info!("  -> loading {}.ffn_norm.weight", prefix);
            let post_attention_layernorm =
                gg.rms_norm(&format!("{prefix}.ffn_norm.weight"), rms_norm_eps)?;
            log::info!("  -> loading attention for {}", prefix);
            let attn =
                QuantizedAttention::new(&mut gg, &prefix, dtype, device, &cfg, rotary_emb.clone())?;

            log::info!("  -> loading MLP/MoE for {}", prefix);
            let (mlp, shared_experts) = if i < cfg.first_k_dense_replace {
                // Dense MLP
                log::info!("     (Dense MLP)");
                let mlp = Mlp::new(&mut gg, &prefix)?;
                (MoeOrMlp::Mlp(mlp), None)
            } else {
                // MoE
                log::info!("     (MoE with Shared Experts)");
                // Shared experts
                let shared = SharedExperts::new(&mut gg, &prefix)?;

                // Fused MoE
                let gate_weight = gg
                    .tensor(&format!("{prefix}.ffn_gate_inp.weight"))?
                    .dequantize(device)?;
                let gate = candle_nn::Linear::new(gate_weight, None); // Bias? DeepSeek usually no bias in simplified models

                let gate_experts_t = gg.tensor(&format!("{prefix}.ffn_gate_exps.weight"))?;
                let gate_experts = if matches!(
                    gate_experts_t.dtype(),
                    GgmlDType::Q2K
                        | GgmlDType::Q3K
                        | GgmlDType::Q4K
                        | GgmlDType::Q5K
                        | GgmlDType::Q6K
                        | GgmlDType::Q8_0
                ) {
                    ExpertWeights::Quantized(Arc::new(gate_experts_t))
                } else {
                    log::info!(
                        "Dequantizing {} MOE experts to F16 (dtype {:?})",
                        prefix,
                        gate_experts_t.dtype()
                    );
                    ExpertWeights::Dequantized(gate_experts_t.dequantize_f16(device)?)
                };

                let up_experts_t = gg.tensor(&format!("{prefix}.ffn_up_exps.weight"))?;
                let up_experts = if matches!(
                    up_experts_t.dtype(),
                    GgmlDType::Q2K
                        | GgmlDType::Q3K
                        | GgmlDType::Q4K
                        | GgmlDType::Q5K
                        | GgmlDType::Q6K
                        | GgmlDType::Q8_0
                ) {
                    ExpertWeights::Quantized(Arc::new(up_experts_t))
                } else {
                    ExpertWeights::Dequantized(up_experts_t.dequantize_f16(device)?)
                };

                let down_experts_t = gg.tensor(&format!("{prefix}.ffn_down_exps.weight"))?;
                let down_experts = if matches!(
                    down_experts_t.dtype(),
                    GgmlDType::Q2K
                        | GgmlDType::Q3K
                        | GgmlDType::Q4K
                        | GgmlDType::Q5K
                        | GgmlDType::Q6K
                        | GgmlDType::Q8_0
                ) {
                    ExpertWeights::Quantized(Arc::new(down_experts_t))
                } else {
                    ExpertWeights::Dequantized(down_experts_t.dequantize_f16(device)?)
                };

                let moe = FusedMoeGGUF {
                    gate,
                    gate_experts,
                    up_experts,
                    down_experts,
                    act: cfg.hidden_act,
                    norm_topk_prob: cfg.norm_topk_prob,
                    num_experts_per_tok: cfg.num_experts_per_tok.unwrap_or(6),
                    dtype,
                };
                (MoeOrMlp::FusedMoe(moe), Some(shared))
            };

            layers.push(DecoderLayer {
                input_layernorm,
                post_attention_layernorm,
                attn,
                mlp,
                shared_experts,
            });
        }

        log::info!("DeepSeek2 GGUF: model loaded successfully");
        Ok(Self {
            embed_tokens,
            layers,
            norm,
            output,
            device: device.clone(),
        })
    }

    pub fn forward(&mut self, input: &Tensor, pos: usize) -> Result<Tensor> {
        let (_b_sz, seq_len) = input.dims2()?;
        let is_prefill = seq_len > 1;
        let mut xs = self.embed_tokens.forward(input)?;

        let mask = if seq_len == 1 {
            None
        } else {
            // Simplified mask
            Some(Tensor::zeros((seq_len, seq_len), DType::F32, &self.device)?)
        };

        for layer in &mut self.layers {
            let residual = &xs;
            let x = layer.input_layernorm.forward(&xs)?;
            let x = layer.attn.forward(&x, mask.as_ref(), pos)?;
            let x = (x + residual)?;

            let residual = &x;
            let x = layer.post_attention_layernorm.forward(&x)?;
            let x = match &layer.shared_experts {
                Some(shared) => {
                    let moe_out = layer.mlp.forward(&x, is_prefill)?;
                    let shared_out = shared.forward(&x)?;
                    (moe_out + shared_out)?
                }
                None => layer.mlp.forward(&x, is_prefill)?,
            };
            xs = (x + residual)?;
        }

        let xs = xs.i((.., seq_len - 1, ..))?.contiguous()?;
        let xs = self.norm.forward(&xs)?;
        let logits = self.output.forward(&xs)?;
        logits.to_dtype(DType::F32)
    }

    pub fn clear_kv_cache(&mut self) {
        for layer in &mut self.layers {
            layer.attn.clear_kv_cache();
        }
    }
}
