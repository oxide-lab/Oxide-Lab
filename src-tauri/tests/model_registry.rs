use candle::quantized::gguf_file::Value;
use oxide_lib::models::registry::{ArchKind, detect_arch};
use std::collections::HashMap;

#[test]
fn test_supported_models_detection() {
    let mut metadata = HashMap::new();

    // Test Qwen3 (base assertion)
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("qwen3".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::qwen3));

    // Test Qwen3 MoE (exact match)
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("qwen3_moe".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::qwen3moe));

    // Test Qwen3 MoE (gguf match)
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("qwen3moe".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::qwen3moe));

    // Test Llama detection
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("llama".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::llama));
}

#[test]
fn test_unknown_architecture() {
    let mut metadata = HashMap::new();
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("unknown_arch_xyz".to_string()),
    );
    assert_eq!(detect_arch(&metadata), None);
}

#[test]
fn test_missing_architecture_key() {
    let mut metadata = HashMap::new();
    metadata.insert(
        "model.name".to_string(),
        Value::String("some-model".to_string()),
    );
    // Should return None because general.architecture is missing
    assert_eq!(detect_arch(&metadata), None);
}
