//! Local copy of FusedMoeGGUF with fix for CUDA arg_sort issue
//!
//! This is a modified version of candle_transformers::fused_moe::FusedMoeGGUF
//! that adds contiguous() before arg_sort to fix CUDA_ERROR_INVALID_VALUE.
//! Adapted for DeepSeek2 (same logic as Qwen3 MoE).

use candle::{D, DType, Device, Result, Tensor, quantized::QTensor};
use candle_nn::{Activation, Linear, Module, moe};
use std::sync::Arc;

pub enum ExpertWeights {
    Quantized(Arc<QTensor>),
    Dequantized(Tensor),
}

impl ExpertWeights {
    pub fn get_weights(&mut self, device: &Device) -> Result<Tensor> {
        match self {
            Self::Dequantized(t) => Ok(t.clone()),
            Self::Quantized(q) => {
                // Dequantize once and cache for CPU efficiency
                let t = q.dequantize(device)?;
                *self = Self::Dequantized(t.clone());
                Ok(t)
            }
        }
    }
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
    #[allow(clippy::too_many_arguments)]
    fn forward_moe(
        xs: &Tensor,
        weights: &mut ExpertWeights,
        num_experts_per_tok: usize,
        dtype: DType,
        topk_weights: &Option<Tensor>,
        sorted_token_ids: &Tensor,
        expert_ids: &Tensor,
        is_prefill: bool,
    ) -> Result<Tensor> {
        if xs.device().is_cpu() {
            return forward_moe_cpu(
                xs,
                weights,
                num_experts_per_tok,
                topk_weights,
                sorted_token_ids,
                expert_ids,
            );
        }

        match weights {
            ExpertWeights::Quantized(q) => moe::moe_gemm_gguf(
                xs,
                q,
                topk_weights,
                sorted_token_ids,
                expert_ids,
                num_experts_per_tok,
                is_prefill,
                if is_prefill { dtype } else { DType::F32 },
            ),
            ExpertWeights::Dequantized(t) => {
                let xs = xs.to_dtype(t.dtype())?;
                let out = moe::moe_gemm(
                    &xs,
                    t,
                    topk_weights,
                    sorted_token_ids,
                    expert_ids,
                    num_experts_per_tok,
                    is_prefill,
                )?;
                out.to_dtype(DType::F32)
            }
        }
    }

    pub fn forward(&mut self, xs: &Tensor, is_prefill: bool) -> Result<Tensor> {
        let (batch, seq_len, hidden_dim) = xs.dims3()?;
        let xs_reshaped = xs.reshape(((), hidden_dim))?.contiguous()?;
        let (num_tokens, _) = xs_reshaped.dims2()?;
        let original_dtype = xs_reshaped.dtype();
        let xs_f32 = if xs_reshaped.dtype() != DType::F32 {
            xs_reshaped.to_dtype(DType::F32)?.contiguous()?
        } else {
            xs_reshaped.to_owned().contiguous()?
        };

        let router_logits = self.gate.forward(&xs_f32)?;

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

        let (expert_ids, sorted_token_ids) = topk_ids.flatten_all()?.sort_last_dim(true)?;
        let expert_ids = expert_ids.contiguous()?;
        let sorted_token_ids = sorted_token_ids.contiguous()?;

        let ys = {
            let gate_out = Self::forward_moe(
                &xs_f32,
                &mut self.gate_experts,
                self.num_experts_per_tok,
                self.dtype,
                &None,
                &sorted_token_ids,
                &expert_ids,
                is_prefill,
            )?;
            let up_out = Self::forward_moe(
                &xs_f32,
                &mut self.up_experts,
                self.num_experts_per_tok,
                self.dtype,
                &None,
                &sorted_token_ids,
                &expert_ids,
                is_prefill,
            )?;

            let down_inputs = (up_out * gate_out.apply(&self.act)?)?.contiguous()?;
            Self::forward_moe(
                &down_inputs,
                &mut self.down_experts,
                self.num_experts_per_tok,
                self.dtype,
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

fn forward_moe_cpu(
    xs: &Tensor,
    weights: &mut ExpertWeights,
    num_experts_per_tok: usize,
    topk_weights: &Option<Tensor>,
    sorted_token_ids: &Tensor,
    expert_ids: &Tensor,
) -> Result<Tensor> {
    let device = xs.device();
    let weights = weights.get_weights(device)?; // [num_experts, d_out, d_in]
    let (_, d_out, _) = weights.dims3()?;

    let xs_gathered = if topk_weights.is_none() {
        // Map flattened slot indices back to original token indices
        // Token_idx = slot_idx / top_k
        let token_indices = (sorted_token_ids.to_dtype(DType::F32)? / num_experts_per_tok as f64)?
            .floor()?
            .to_dtype(DType::U32)?;
        xs.index_select(&token_indices, 0)?
    } else {
        xs.clone()
    };

    let num_slots = xs_gathered.dims1()?;
    let mut ys_gathered = Tensor::zeros((num_slots, d_out), DType::F32, device)?;

    let expert_ids_vec: Vec<u32> = expert_ids.to_vec1()?;
    let mut start = 0;
    while start < expert_ids_vec.len() {
        let expert_id = expert_ids_vec[start] as usize;
        let mut end = start + 1;
        while end < expert_ids_vec.len() && expert_ids_vec[end] == expert_id as u32 {
            end += 1;
        }

        let count = end - start;
        let expert_xs = xs_gathered.narrow(0, start, count)?;
        let expert_w = weights.get(expert_id)?; // [d_out, d_in]
        let expert_ys = expert_xs.matmul(&expert_w.t()?)?;

        ys_gathered = ys_gathered.slice_assign(&[start..end, 0..d_out], &expert_ys)?;
        start = end;
    }

    if let Some(w) = topk_weights {
        let w_flat = w.flatten_all()?.unsqueeze(1)?;
        ys_gathered = ys_gathered.broadcast_mul(&w_flat)?;
    }

    Ok(ys_gathered)
}
