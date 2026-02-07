use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

pub const MANIFEST_FILE_NAME: &str = ".oxide-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadManifest {
    pub version: u32,
    pub repo_id: String,
    pub repo_name: String,
    pub publisher: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_name: Option<String>,
    pub downloaded_at: String,
}

pub fn resolve_manifest_path(target: &Path) -> std::path::PathBuf {
    if target.is_dir() {
        target.join(MANIFEST_FILE_NAME)
    } else {
        // For files (like .gguf), we append the manifest extension to the filename
        // e.g. model.gguf -> model.gguf.oxide-manifest.json
        let file_name = target
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        target.with_file_name(format!("{}{}", file_name, MANIFEST_FILE_NAME))
    }
}

pub fn save_manifest(target: &Path, manifest: &DownloadManifest) -> Result<(), String> {
    let path = resolve_manifest_path(target);
    let serialized = serde_json::to_string_pretty(manifest)
        .map_err(|e| format!("Не удалось сериализовать манифест: {e}"))?;
    fs::write(&path, serialized)
        .map_err(|e| format!("Не удалось сохранить манифест {}: {e}", path.display()))
}

pub fn load_manifest(target: &Path) -> Option<DownloadManifest> {
    let path = resolve_manifest_path(target);
    // If specific file manifest missing for a file, try directory manifest fallback?
    // No, strictly separate for now to avoid pollution.
    let data = fs::read_to_string(&path)
        .or_else(|_| {
            // Fallback: checks hidden .oxide-manifest.json if target is a file
            if target.is_file()
                && let Some(parent) = target.parent()
            {
                return fs::read_to_string(parent.join(MANIFEST_FILE_NAME));
            }
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Manifest not found",
            ))
        })
        .ok()?;
    serde_json::from_str(&data).ok()
}

pub fn infer_quantization_from_label(label: &str) -> Option<String> {
    static REGEX: OnceCell<Regex> = OnceCell::new();
    let regex = REGEX.get_or_init(|| {
        Regex::new(r"(?i)(Q\d+[_A-Z0-9]*|INT\d+|FP\d+|BNB[-_\s]?\d+(?:BIT)?)")
            .expect("quantization regex")
    });
    regex
        .find(label)
        .map(|m| canonicalize_quantization_label(m.as_str()))
}

fn canonicalize_quantization_label(raw: &str) -> String {
    let mut value = raw.trim().to_ascii_lowercase();
    value = value.replace(' ', "");
    value = value.replace('-', "_");
    if let Some(idx) = value.find("_k_m") {
        value.replace_range(idx..idx + 4, "km");
    }
    value
}
