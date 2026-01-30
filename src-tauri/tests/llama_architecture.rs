//! Integration tests for Llama architecture

use candle::quantized::gguf_file::Value;
use oxide_lib::models::registry::{ArchKind, detect_arch};
use std::collections::HashMap;

#[test]
fn test_llama_architecture_detection() {
    let mut metadata = HashMap::new();

    // Test Llama
    metadata.insert(
        "general.architecture".to_string(),
        Value::String("llama".to_string()),
    );
    assert_eq!(detect_arch(&metadata), Some(ArchKind::llama));
}
