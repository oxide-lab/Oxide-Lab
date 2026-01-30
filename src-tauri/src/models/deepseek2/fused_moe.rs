//! Local copy of FusedMoeGGUF with fix for CUDA arg_sort issue
//!
//! This is a modified version of candle_transformers::fused_moe::FusedMoeGGUF
//! that adds contiguous() before arg_sort to fix CUDA_ERROR_INVALID_VALUE.
//! Adapted for DeepSeek2 (same logic as Qwen3 MoE).

use candle::{D, DType, Result, Tensor, quantized::QTensor};
use candle_nn::{Activation, Linear, Module, moe};
use std::sync::Arc;

pub enum ExpertWeights {
    Quantized(Arc<QTensor>),
    Dequantized(Tensor),
}

pub struct FusedMoeGGUF {
    pub gate: Linear,
    pub gate_experts: ExpertWeights,
    pub up_experts: ExpertWeights,
    pub down_experts: ExpertWeights,
    pub act: Activation,
    pub norm_topk_prob: bool,
    pub num_experts_per_tok: usize,
    pub dtype: DType,
}

impl FusedMoeGGUF {
    fn forward_moe(
        &self,
        xs: &Tensor,
        weights: &ExpertWeights,
        topk_weights: &Option<Tensor>,
        sorted_token_ids: &Tensor,
        expert_ids: &Tensor,
        is_prefill: bool,
    ) -> Result<Tensor> {
        match weights {
            ExpertWeights::Quantized(q) => moe::moe_gemm_gguf(
                xs,
                q,
                topk_weights,
                sorted_token_ids,
                expert_ids,
                self.num_experts_per_tok,
                is_prefill,
                self.dtype,
            ),
            ExpertWeights::Dequantized(t) => {
                let xs_f16 = xs.to_dtype(t.dtype())?;
                let out = moe::moe_gemm(
                    &xs_f16,
                    t,
                    topk_weights,
                    sorted_token_ids,
                    expert_ids,
                    self.num_experts_per_tok,
                    is_prefill,
                )?;
                out.to_dtype(DType::F32)
            }
        }
    }

    pub fn forward(&self, xs: &Tensor, is_prefill: bool) -> Result<Tensor> {
        let (batch, seq_len, hidden_dim) = xs.dims3()?;
        let xs = xs.reshape(((), hidden_dim))?.contiguous()?;
        let (num_tokens, hidden_dim) = xs.dims2()?;
        let original_dtype = xs.dtype();
        let xs = if xs.dtype() != DType::F32 {
            xs.to_dtype(DType::F32)?.contiguous()?
        } else {
            xs.to_owned().contiguous()?
        };

        let router_logits = self.gate.forward(&xs)?;

        // FIX: Add contiguous() before arg_sort to ensure tensor is in valid memory layout
        let routing_weights =
            candle_nn::ops::softmax_last_dim(&router_logits.to_dtype(DType::F32)?)?.contiguous()?;

        let topk_ids = routing_weights
            .arg_sort_last_dim(false)?
            .narrow(D::Minus1, 0, self.num_experts_per_tok)?
            .contiguous()?;

        let mut topk_weights = routing_weights.gather(&topk_ids, D::Minus1)?.contiguous()?;

        if self.norm_topk_prob {
            topk_weights = topk_weights
                .broadcast_div(&topk_weights.sum_keepdim(D::Minus1)?)?
                .contiguous()?;
        }

        // Sort for expert routing (same for prefill and decode)
        let _ = is_prefill; // Mark as used
        let (expert_ids, sorted_token_ids) = topk_ids.flatten_all()?.sort_last_dim(true)?;
        // FIX: Ensure sorted tensors are contiguous to prevent CUDA_ERROR_INVALID_VALUE
        let expert_ids = expert_ids.contiguous()?;
        let sorted_token_ids = sorted_token_ids.contiguous()?;

        let ys = {
            let gate = self.forward_moe(
                &xs,
                &self.gate_experts,
                &None,
                &sorted_token_ids,
                &expert_ids,
                is_prefill,
            )?;
            let up = self.forward_moe(
                &xs,
                &self.up_experts,
                &None,
                &sorted_token_ids,
                &expert_ids,
                is_prefill,
            )?;

            let down_inputs = (up * gate.apply(&self.act)?)?.contiguous()?;
            self.forward_moe(
                &down_inputs,
                &self.down_experts,
                &Some(topk_weights),
                &sorted_token_ids,
                &expert_ids,
                is_prefill,
            )?
        };
        let mut ys = ys
            .reshape((num_tokens, (), hidden_dim))?
            .sum(D::Minus2)?
            .contiguous()?;
        if ys.dtype() != original_dtype {
            ys = ys.to_dtype(original_dtype)?.contiguous()?;
        }
        ys.reshape((batch, seq_len, hidden_dim))?.contiguous()
    }
}
