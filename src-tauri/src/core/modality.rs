use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModalitySupport {
    pub text: bool,
    pub image: bool,
    pub audio: bool,
    pub video: bool,
}

impl Default for ModalitySupport {
    fn default() -> Self {
        Self {
            text: true,
            image: false,
            audio: false,
            video: false,
        }
    }
}

pub fn model_id_looks_vision(model_id: &str) -> bool {
    let lower = model_id.to_ascii_lowercase();
    let markers = [
        "qwen3-vl",
        "qwen2-vl",
        "vl-",
        "-vl",
        "vision",
        "minicpm-v",
        "glm-4v",
        "llava",
        "pixtral",
        "internvl",
        "gemma-3",
    ];
    markers.iter().any(|marker| lower.contains(marker))
}

pub fn detect_modality_support(model_id: &str, mmproj_path: Option<&str>) -> ModalitySupport {
    let _ = model_id;
    let has_mmproj = mmproj_path.map(|v| !v.trim().is_empty()).unwrap_or(false);
    let image = has_mmproj;
    ModalitySupport {
        text: true,
        image,
        audio: false,
        video: false,
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_modality_support, model_id_looks_vision};

    #[test]
    fn vision_marker_detection_recognizes_qwen3_vl() {
        assert!(model_id_looks_vision("Qwen/Qwen3-VL-8B-Instruct"));
    }

    #[test]
    fn detect_modality_support_defaults_to_text_only() {
        let support = detect_modality_support("qwen3-coder-30b", None);
        assert!(support.text);
        assert!(!support.image);
        assert!(!support.audio);
        assert!(!support.video);
    }

    #[test]
    fn detect_modality_support_enables_image_with_mmproj() {
        let support = detect_modality_support("any-model", Some("C:/models/mmproj.gguf"));
        assert!(support.image);
    }
}
