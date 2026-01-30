use std::fs::File;
use std::path::{Path, PathBuf};

use candle::{Device, Tensor};
use oxide_lib::core::tokenizer::tokenizer_from_gguf_metadata;
use oxide_lib::models::registry::{detect_arch, get_model_factory};

fn models_root() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("OXIDE_TEST_MODELS_DIR") {
        let root = PathBuf::from(path);
        if root.exists() {
            return Some(root);
        }
    }
    let fallback = PathBuf::from(r"D:\models");
    if fallback.exists() {
        return Some(fallback);
    }
    None
}

fn run_gguf_inference(model_path: &Path) -> Result<(), String> {
    let mut file = File::open(model_path).map_err(|e| {
        format!(
            "Failed to open GGUF model at {}: {}",
            model_path.display(),
            e
        )
    })?;
    let content = candle::quantized::gguf_file::Content::read(&mut file)
        .map_err(|e| format!("Failed to read GGUF header: {}", e))?;
    let metadata = content.metadata.clone();

    let arch = detect_arch(&metadata).ok_or_else(|| "Failed to detect architecture".to_string())?;
    let device = Device::Cpu;
    let factory = get_model_factory();
    let mut model = factory
        .build_from_gguf(arch, content, &mut file, &device, 256, false)
        .map_err(|e| format!("Failed to build model: {}", e))?;

    let tokenizer = tokenizer_from_gguf_metadata(&metadata)
        .map_err(|e| format!("Failed to build tokenizer: {}", e))?;
    let encoding = tokenizer
        .encode("Hello", false)
        .map_err(|e| format!("Tokenizer encode failed: {}", e))?;
    let input_ids = encoding.get_ids();
    if input_ids.is_empty() {
        return Err("Tokenizer returned empty input ids".to_string());
    }

    let input_tensor = Tensor::from_vec(vec![input_ids[0] as i64], (1, 1), &device)
        .map_err(|e| format!("Failed to build input tensor: {}", e))?;
    model
        .forward_layered(&input_tensor, 0)
        .map_err(|e| format!("Prefill forward failed: {}", e))?;

    let decode_input = Tensor::from_vec(vec![1i64], (1, 1), &device)
        .map_err(|e| format!("Failed to build decode input: {}", e))?;
    model
        .forward_layered(&decode_input, 1)
        .map_err(|e| format!("Decode forward failed: {}", e))?;

    Ok(())
}

#[test]
fn test_glm4_gguf_inference() {
    let Some(root) = models_root() else {
        println!("Skipping GLM4 GGUF test: models root not found");
        return;
    };
    let model_path = root
        .join("lmstudio-community")
        .join("GLM-4-9B-0414-GGUF")
        .join("GLM-4-9B-0414-Q4_K_M.gguf");
    if !model_path.exists() {
        println!(
            "Skipping GLM4 GGUF test: file not found at {:?}",
            model_path
        );
        return;
    }
    if let Err(err) = run_gguf_inference(&model_path) {
        panic!("GLM4 GGUF inference failed: {}", err);
    }
}

#[test]
fn test_glm4v_gguf_text_only_inference() {
    let Some(root) = models_root() else {
        println!("Skipping GLM4V GGUF test: models root not found");
        return;
    };
    let model_path = root
        .join("lmstudio-community")
        .join("GLM-4.6V-Flash-GGUF")
        .join("GLM-4.6V-Flash-Q4_K_M.gguf");
    if !model_path.exists() {
        println!(
            "Skipping GLM4V GGUF test: file not found at {:?}",
            model_path
        );
        return;
    }
    if let Err(err) = run_gguf_inference(&model_path) {
        panic!("GLM4V GGUF text-only inference failed: {}", err);
    }
}

#[test]
fn test_deepseek2_glm4_moe_lite_gguf_inference() {
    let Some(root) = models_root() else {
        println!("Skipping DeepSeek2 (GLM4 MoE Lite) GGUF test: models root not found");
        return;
    };
    if !cfg!(feature = "cuda") {
        println!("Skipping DeepSeek2 (GLM4 MoE Lite) GGUF test: moe_gemm_gguf requires CUDA");
        return;
    };
    let model_dir = root.join("unsloth").join("GLM-4.7-Flash-GGUF");
    if !model_dir.exists() {
        println!(
            "Skipping DeepSeek2 (GLM4 MoE Lite) GGUF test: dir not found at {:?}",
            model_dir
        );
        return;
    }
    let mut gguf_files = std::fs::read_dir(&model_dir)
        .map_err(|e| format!("read_dir failed: {}", e))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "gguf"))
        .collect::<Vec<_>>();

    if gguf_files.is_empty() {
        println!(
            "Skipping DeepSeek2 (GLM4 MoE Lite) GGUF test: no .gguf files in {:?}",
            model_dir
        );
        return;
    }
    gguf_files.sort();
    let model_path = gguf_files.remove(0);
    if let Err(err) = run_gguf_inference(&model_path) {
        panic!("DeepSeek2 (GLM4 MoE Lite) GGUF inference failed: {}", err);
    }
}
