use crate::core::template_registry::TemplateEntry;

mod alpaca;
mod chatml;
mod chatqa;
mod deepseekr1_llama;
mod deepseekv3;
mod gemma2;
mod gemma3;
mod llama;
mod llama3;
mod llama32;
mod phi3;
mod qwen2;
mod qwen3;
mod qwen3coder;
mod vicuna;
mod zephyr;

pub fn get_all() -> Vec<TemplateEntry> {
    vec![
        llama::TEMPLATE,
        llama3::TEMPLATE,
        llama32::TEMPLATE,
        qwen3::TEMPLATE,
        gemma2::TEMPLATE,
        gemma3::TEMPLATE,
        qwen3coder::TEMPLATE,
        zephyr::TEMPLATE,
        vicuna::TEMPLATE,
        phi3::TEMPLATE,
        alpaca::TEMPLATE,
        chatml::TEMPLATE,
        chatqa::TEMPLATE,
        deepseekr1_llama::TEMPLATE,
        deepseekv3::TEMPLATE,
        qwen2::TEMPLATE,
    ]
}
