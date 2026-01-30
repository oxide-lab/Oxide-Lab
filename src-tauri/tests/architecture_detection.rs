use candle::quantized::gguf_file::Value;
use oxide_lib::models::registry::{ArchKind, detect_arch};
use std::collections::HashMap;

#[test]
fn test_architecture_detection() {
    // Test Qwen3 detection
    let mut metadata = HashMap::new();
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("qwen3".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::qwen3));

    // Test Qwen3 MoE detection (safetensors style)
    let mut metadata = HashMap::new();
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("qwen3_moe".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::qwen3moe));

    // Test Qwen3 MoE detection (gguf style)
    let mut metadata = HashMap::new();
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("qwen3moe".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::qwen3moe));

    // Test Llama detection
    let mut metadata = HashMap::new();
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("llama".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::llama));

    // Test unknown architecture
    let mut metadata = HashMap::new();
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("unknown".to_string()),
    );
    assert_eq!(detect_arch(&metadata), None);

    // Test empty metadata
    let metadata = HashMap::new();
    assert_eq!(detect_arch(&metadata), None);
}
