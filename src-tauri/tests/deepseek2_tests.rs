use oxide_lib::models::deepseek2::model::DeepSeekV2Config;
use oxide_lib::models::registry::{ArchKind, detect_arch_from_string};

#[test]
fn test_deepseek2_config_parsing() {
    let json = r#"{
        "vocab_size": 102400,
        "hidden_size": 2048,
        "intermediate_size": 10944,
        "moe_intermediate_size": 1408,
        "num_hidden_layers": 27,
        "num_attention_heads": 16,
        "n_shared_experts": 2,
        "n_routed_experts": 64,
        "num_experts_per_tok": 6,
        "moe_layer_freq": 1,
        "first_k_dense_replace": 1,
        "rms_norm_eps": 1e-06,
        "max_position_embeddings": 163840,
        "rope_theta": 10000.0,
        "attention_bias": false,
        "q_lora_rank": 1536,
        "qk_rope_head_dim": 64,
        "kv_lora_rank": 512,
        "v_head_dim": 128,
        "qk_nope_head_dim": 128,
        "n_group": 1,
        "topk_group": 1,
        "model_type": "deepseek_v2"
    }"#;

    let config: DeepSeekV2Config = serde_json::from_str(json).expect("Failed to parse config");
    assert_eq!(config.vocab_size, 102400);
    assert_eq!(config.hidden_size, 2048);
    assert_eq!(config.qk_rope_head_dim, 64);
}

#[test]
fn test_deepseek2_arch_detection() {
    assert_eq!(
        detect_arch_from_string("deepseek2"),
        Some(ArchKind::deepseek2)
    );
    assert_eq!(
        detect_arch_from_string("deepseek_v2"),
        Some(ArchKind::deepseek2)
    );
}
