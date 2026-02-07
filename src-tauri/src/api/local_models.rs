//! Local model management with GGUF metadata parsing via `gguf` crate.

use crate::api::model_manager::manifest::{
    DownloadManifest, infer_quantization_from_label, load_manifest, save_manifest,
};
use bytes::{BufMut, BytesMut};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::borrow::Borrow;
use std::fs;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ValidationLevel {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationStatus {
    pub level: ValidationLevel,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub messages: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelFormat {
    Gguf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GGUFKeyValue {
    pub key: String,
    pub value: JsonValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GGUFMetadata {
    pub format_version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub alignment: u64,
    pub tensor_count: usize,
    pub metadata_kv_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attention_head_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kv_head_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rope_dimension: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bos_token_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eos_token_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_tokens: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokenizer_scores: Option<Vec<f32>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub custom_metadata: Vec<GGUFKeyValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    pub name: String,
    pub path: PathBuf,
    pub file_size: u64,
    pub format: ModelFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detected_architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vocab_size: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_repo_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_repo_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_quantization: Option<String>,
    pub validation_status: ValidationStatus,
    pub created_at: DateTime<Utc>,
    pub metadata: GGUFMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteGGUFFile {
    pub filename: String,
    pub size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    pub download_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HFModelInfo {
    pub repo_id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub languages: Vec<String>,
    pub downloads: u64,
    pub likes: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub architectures: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quantizations: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gguf_files: Vec<RemoteGGUFFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_modified: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HFModelMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_count: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_length: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_file_size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_downloads: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_order: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadedFileInfo {
    pub repo_id: String,
    pub filename: String,
    pub local_path: PathBuf,
    pub size: u64,
}

pub fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .user_agent("oxide-lab/0.13")
        .build()
        .map_err(|e| e.to_string())
}

fn metadata_value<'a>(file: &'a gguf::GGUFFile, key: &str) -> Option<&'a gguf::GGUFMetadataValue> {
    file.header
        .metadata
        .iter()
        .find(|entry| entry.key == key)
        .map(|entry| &entry.value)
}

fn as_string(v: Option<&gguf::GGUFMetadataValue>) -> Option<String> {
    match v {
        Some(gguf::GGUFMetadataValue::String(s)) => Some(s.clone()),
        _ => None,
    }
}

fn as_u64(v: Option<&gguf::GGUFMetadataValue>) -> Option<u64> {
    match v {
        Some(gguf::GGUFMetadataValue::Uint64(x)) => Some(*x),
        Some(gguf::GGUFMetadataValue::Uint32(x)) => Some(*x as u64),
        Some(gguf::GGUFMetadataValue::Int64(x)) if *x >= 0 => Some(*x as u64),
        Some(gguf::GGUFMetadataValue::Int32(x)) if *x >= 0 => Some(*x as u64),
        _ => None,
    }
}

fn read_gguf_file(path: &Path) -> Result<gguf::GGUFFile, String> {
    const READ_BUFFER_SIZE: usize = 1_000_000;

    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::with_capacity(READ_BUFFER_SIZE, file);
    let mut buffer = BytesMut::with_capacity(READ_BUFFER_SIZE);

    loop {
        let chunk = reader.fill_buf().map_err(|e| e.to_string())?;
        if chunk.is_empty() {
            return Err(format!("Failed to parse GGUF metadata: {}", path.display()));
        }

        let len = chunk.len();
        buffer.put(chunk);
        reader.consume(len);

        match gguf::GGUFFile::read(buffer.borrow()) {
            Ok(Some(parsed)) => return Ok(parsed),
            Ok(None) => buffer.reserve(READ_BUFFER_SIZE),
            Err(e) => return Err(e),
        }
    }
}

fn parse_gguf_metadata_impl(path: &Path) -> Result<GGUFMetadata, String> {
    let parsed = read_gguf_file(path)?;
    let architecture = as_string(metadata_value(&parsed, "general.architecture"));
    let tokenizer_model = as_string(metadata_value(&parsed, "tokenizer.ggml.model"));

    // Use the architecture name as prefix for architecture-specific keys
    let arch_prefix = architecture.clone().unwrap_or_else(|| "llama".to_string());

    let custom_metadata = parsed
        .header
        .metadata
        .iter()
        .take(128)
        .map(|kv| GGUFKeyValue {
            key: kv.key.clone(),
            value: serde_json::to_value(&kv.value).unwrap_or(JsonValue::Null),
        })
        .collect::<Vec<_>>();

    Ok(GGUFMetadata {
        format_version: parsed.header.version,
        architecture,
        name: as_string(metadata_value(&parsed, "general.name")),
        version: as_string(metadata_value(&parsed, "general.version")),
        author: as_string(metadata_value(&parsed, "general.author")),
        alignment: as_u64(metadata_value(&parsed, "general.alignment")).unwrap_or(0),
        tensor_count: parsed.tensors.len(),
        metadata_kv_count: parsed.header.metadata.len(),
        parameter_count: as_u64(metadata_value(&parsed, "general.parameter_count")),
        size_label: as_string(metadata_value(&parsed, "general.size_label")),
        context_length: as_u64(metadata_value(&parsed, &format!("{}.context_length", arch_prefix))),
        embedding_length: as_u64(metadata_value(&parsed, &format!("{}.embedding_length", arch_prefix))),
        block_count: as_u64(metadata_value(&parsed, &format!("{}.block_count", arch_prefix))),
        attention_head_count: as_u64(metadata_value(&parsed, &format!("{}.attention.head_count", arch_prefix))),
        kv_head_count: as_u64(metadata_value(&parsed, &format!("{}.attention.head_count_kv", arch_prefix))),
        rope_dimension: as_u64(metadata_value(&parsed, &format!("{}.rope.dimension_count", arch_prefix))),
        tokenizer_model,
        bos_token_id: as_u64(metadata_value(&parsed, "tokenizer.ggml.bos_token_id"))
            .map(|v| v as u32),
        eos_token_id: as_u64(metadata_value(&parsed, "tokenizer.ggml.eos_token_id"))
            .map(|v| v as u32),
        tokenizer_tokens: None,
        tokenizer_scores: None,
        custom_metadata,
    })
}

fn validation_for(metadata: &GGUFMetadata) -> ValidationStatus {
    let mut warnings = Vec::new();
    if metadata.architecture.is_none() {
        warnings.push("Missing general.architecture metadata".to_string());
    }
    if metadata.tokenizer_model.is_none() {
        warnings.push("Missing tokenizer.ggml.model metadata".to_string());
    }

    if warnings.is_empty() {
        ValidationStatus {
            level: ValidationLevel::Ok,
            messages: vec![],
        }
    } else {
        ValidationStatus {
            level: ValidationLevel::Warning,
            messages: warnings,
        }
    }
}

fn build_model_info(path: &Path) -> Result<ModelInfo, String> {
    let metadata = parse_gguf_metadata_impl(path)?;
    let stat = fs::metadata(path).map_err(|e| e.to_string())?;
    let created = stat
        .created()
        .or_else(|_| stat.modified())
        .unwrap_or(std::time::SystemTime::now());

    let name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "model.gguf".to_string());

    let quantization = infer_quantization_from_label(&name);
    let detected_architecture = metadata.architecture.clone();

    let manifest = load_manifest(path);

    Ok(ModelInfo {
        name,
        path: path.to_path_buf(),
        file_size: stat.len(),
        format: ModelFormat::Gguf,
        architecture: metadata.architecture.clone(),
        detected_architecture,
        model_name: metadata.name.clone(),
        version: metadata.version.clone(),
        context_length: metadata.context_length,
        parameter_count: metadata.parameter_count.map(|v| v.to_string()),
        quantization: quantization.clone(),
        tokenizer_type: metadata.tokenizer_model.clone(),
        vocab_size: None,
        source_repo_id: manifest.as_ref().map(|m| m.repo_id.clone()),
        source_repo_name: manifest.as_ref().map(|m| m.repo_name.clone()),
        source_quantization: manifest.as_ref().and_then(|m| m.quantization.clone()),
        validation_status: validation_for(&metadata),
        created_at: DateTime::<Utc>::from(created),
        metadata,
    })
}

fn collect_gguf_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(root).map_err(|e| e.to_string())?;
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let _ = collect_gguf_files(&p, out);
            continue;
        }
        let is_gguf = p
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.eq_ignore_ascii_case("gguf"))
            .unwrap_or(false);
        if is_gguf {
            out.push(p);
        }
    }
    Ok(())
}

fn scan_models_folder_impl(folder_path: &str) -> Result<Vec<ModelInfo>, String> {
    let root = Path::new(folder_path);
    if !root.exists() || !root.is_dir() {
        return Err(format!("Invalid models folder: {}", folder_path));
    }

    let mut files = Vec::new();
    collect_gguf_files(root, &mut files)?;

    let mut models = Vec::new();
    for p in files {
        match build_model_info(&p) {
            Ok(info) => models.push(info),
            Err(e) => log::warn!("Skipping GGUF model {}: {}", p.display(), e),
        }
    }

    models.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok(models)
}

#[tauri::command]
pub async fn parse_gguf_metadata(file_path: String) -> Result<GGUFMetadata, String> {
    tauri::async_runtime::spawn_blocking(move || parse_gguf_metadata_impl(Path::new(&file_path)))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn scan_models_folder(folder_path: String) -> Result<Vec<ModelInfo>, String> {
    tauri::async_runtime::spawn_blocking(move || scan_models_folder_impl(&folder_path))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn scan_local_models_folder(folder_path: String) -> Result<Vec<ModelInfo>, String> {
    scan_models_folder(folder_path).await
}

#[derive(Debug, Deserialize)]
struct HfModelApiEntry {
    id: String,
    #[serde(default)]
    downloads: u64,
    #[serde(default)]
    likes: u64,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    pipeline_tag: Option<String>,
    #[serde(default)]
    library_name: Option<String>,
    #[serde(default)]
    siblings: Vec<HfSibling>,
    #[serde(default)]
    #[serde(rename = "cardData")]
    card_data: Option<serde_json::Value>,
    #[serde(default)]
    #[serde(rename = "lastModified")]
    last_modified: Option<String>,
    #[serde(default)]
    #[serde(rename = "createdAt")]
    created_at: Option<String>,
    #[serde(default)]
    gguf: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct HfSibling {
    rfilename: String,
    #[serde(default)]
    size: Option<u64>,
}

fn get_card_data_string(entry: &HfModelApiEntry, key: &str) -> Option<String> {
    entry
        .card_data
        .as_ref()
        .and_then(|value| value.get(key))
        .and_then(|value| value.as_str())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn get_card_data_string_or_first(entry: &HfModelApiEntry, key: &str) -> Option<String> {
    let value = entry
        .card_data
        .as_ref()
        .and_then(|card_data| card_data.get(key))?;

    if let Some(single) = value.as_str() {
        let trimmed = single.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    if let Some(array) = value.as_array() {
        for item in array {
            if let Some(text) = item.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    None
}

fn get_card_data_string_list(entry: &HfModelApiEntry, key: &str) -> Vec<String> {
    let Some(value) = entry
        .card_data
        .as_ref()
        .and_then(|card_data| card_data.get(key))
    else {
        return Vec::new();
    };

    if let Some(single) = value.as_str() {
        let trimmed = single.trim();
        if trimmed.is_empty() {
            return Vec::new();
        }
        return vec![trimmed.to_string()];
    }

    let mut items: Vec<String> = Vec::new();
    if let Some(array) = value.as_array() {
        for item in array {
            if let Some(text) = item.as_str() {
                let trimmed = text.trim();
                if !trimmed.is_empty() && !items.iter().any(|existing| existing == trimmed) {
                    items.push(trimmed.to_string());
                }
            }
        }
    }
    items
}

fn normalize_language_value(raw: &str) -> Option<String> {
    let mut value = raw.trim().to_ascii_lowercase();
    if value.is_empty() {
        return None;
    }

    if let Some(stripped) = value.strip_prefix("language:") {
        value = stripped.trim().to_string();
    }

    if !value
        .chars()
        .all(|c| c.is_ascii_alphabetic() || c == '-')
    {
        return None;
    }

    let parts: Vec<&str> = value.split('-').collect();
    match parts.as_slice() {
        [primary] if primary.len() == 2 => Some((*primary).to_string()),
        [primary, region] if primary.len() == 2 && region.len() == 2 => {
            Some(format!("{}-{}", primary, region))
        }
        _ => None,
    }
}

fn get_entry_languages(entry: &HfModelApiEntry) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();

    for language in get_card_data_string_list(entry, "language")
        .into_iter()
        .chain(get_card_data_string_list(entry, "languages"))
    {
        if let Some(normalized) = normalize_language_value(&language) {
            if !items.iter().any(|existing| existing == &normalized) {
                items.push(normalized);
            }
        }
    }

    for tag in &entry.tags {
        if let Some(normalized) = normalize_language_value(tag) {
            if !items.iter().any(|existing| existing == &normalized) {
                items.push(normalized);
            }
        }
    }

    items
}

fn get_entry_license(entry: &HfModelApiEntry) -> Option<String> {
    if let Some(license) = get_card_data_string_or_first(entry, "license") {
        return Some(license);
    }

    entry.tags.iter().find_map(|tag| {
        let trimmed = tag.trim();
        let stripped = trimmed.strip_prefix("license:")?;
        let value = stripped.trim();
        if value.is_empty() {
            return None;
        }
        Some(value.to_string())
    })
}

fn card_data_value<'a>(entry: &'a HfModelApiEntry, key: &str) -> Option<&'a serde_json::Value> {
    entry
        .card_data
        .as_ref()
        .and_then(|card_data| card_data.get(key))
}

fn json_as_u64(value: &serde_json::Value) -> Option<u64> {
    if let Some(raw) = value.as_u64() {
        return Some(raw);
    }
    let float = value.as_f64()?;
    if !float.is_finite() || float <= 0.0 {
        return None;
    }
    Some(float.round() as u64)
}

fn json_get_u64_at_path(value: &serde_json::Value, path: &[&str]) -> Option<u64> {
    let mut current = value;
    for segment in path {
        current = current.get(*segment)?;
    }
    json_as_u64(current)
}

fn normalize_parameter_number(value: f64) -> String {
    if !value.is_finite() {
        return "0".to_string();
    }
    if (value.fract()).abs() < f64::EPSILON {
        return format!("{value:.0}");
    }
    let rounded_1dp = (value * 10.0).round() / 10.0;
    if (value - rounded_1dp).abs() < 0.05 {
        return format!("{rounded_1dp:.1}");
    }
    format!("{:.2}", (value * 100.0).round() / 100.0)
}

fn format_parameter_count_from_total(total: u64) -> Option<String> {
    if total == 0 {
        return None;
    }
    const BILLION: f64 = 1_000_000_000.0;
    const MILLION: f64 = 1_000_000.0;

    if total >= BILLION as u64 {
        let value = total as f64 / BILLION;
        return Some(format!("{}B", normalize_parameter_number(value)));
    }
    if total >= MILLION as u64 {
        let value = total as f64 / MILLION;
        return Some(format!("{}M", normalize_parameter_number(value)));
    }
    Some(total.to_string())
}

fn parse_parameter_token(token: &str) -> Option<String> {
    let lower = token.trim().to_ascii_lowercase();
    if lower.len() < 2 {
        return None;
    }

    let unit = lower.chars().last()?;
    if unit != 'b' && unit != 'm' {
        return None;
    }

    let number = &lower[..lower.len() - 1];
    if number.is_empty() || number.chars().filter(|ch| *ch == '.').count() > 1 {
        return None;
    }
    if !number.chars().all(|ch| ch.is_ascii_digit() || ch == '.') {
        return None;
    }

    let value = number.parse::<f64>().ok()?;
    if !value.is_finite() || value <= 0.0 {
        return None;
    }

    let normalized = normalize_parameter_number(value);
    let suffix = if unit == 'b' { "B" } else { "M" };
    Some(format!("{normalized}{suffix}"))
}

fn extract_parameter_label_from_text(text: &str) -> Option<String> {
    text.split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '.')
        .find_map(parse_parameter_token)
}

fn infer_parameter_count_from_entry(entry: &HfModelApiEntry) -> Option<String> {
    for key in [
        "parameter_count",
        "parameterCount",
        "params",
        "parameters",
        "model_size",
    ] {
        if let Some(value) = card_data_value(entry, key) {
            if let Some(raw) = value.as_str() {
                if let Some(label) = extract_parameter_label_from_text(raw) {
                    return Some(label);
                }
            }
            if let Some(raw) = json_as_u64(value) {
                if let Some(label) = format_parameter_count_from_total(raw) {
                    return Some(label);
                }
            }
        }
    }

    if let Some(gguf) = entry.gguf.as_ref() {
        if let Some(total) = json_get_u64_at_path(gguf, &["total"]) {
            if let Some(label) = format_parameter_count_from_total(total) {
                return Some(label);
            }
        }
    }

    None
}

fn infer_context_length_from_entry(entry: &HfModelApiEntry) -> Option<u64> {
    if let Some(gguf) = entry.gguf.as_ref() {
        if let Some(length) = json_get_u64_at_path(gguf, &["context_length"]) {
            return Some(length);
        }
    }

    for key in [
        "context_length",
        "max_position_embeddings",
        "model_max_length",
        "context_window",
    ] {
        if let Some(value) = card_data_value(entry, key) {
            if let Some(raw) = json_as_u64(value) {
                return Some(raw);
            }
            if let Some(text) = value.as_str() {
                let trimmed = text.trim();
                if let Ok(parsed) = trimmed.parse::<u64>() {
                    if parsed > 0 {
                        return Some(parsed);
                    }
                }
            }
        }
    }

    None
}

fn format_hf_size(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    const GB: f64 = 1024.0 * MB;

    if bytes >= GB as u64 {
        format!("{:.1} GB", bytes as f64 / GB)
    } else if bytes >= MB as u64 {
        format!("{:.1} MB", bytes as f64 / MB)
    } else if bytes >= KB as u64 {
        format!("{:.1} KB", bytes as f64 / KB)
    } else {
        format!("{bytes} B")
    }
}

fn build_model_card_markdown(entry: &HfModelApiEntry) -> String {
    let mut out = String::new();
    out.push_str(&format!("# {}\n\n", entry.id));

    if let Some(description) = get_card_data_string(entry, "description") {
        out.push_str(&description);
        out.push_str("\n\n");
    }

    out.push_str("## Repository\n\n");
    out.push_str(&format!("- **Repo:** `{}`\n", entry.id));
    out.push_str(&format!("- **Downloads:** {}\n", entry.downloads));
    out.push_str(&format!("- **Likes:** {}\n", entry.likes));
    if let Some(updated) = entry.last_modified.as_ref() {
        out.push_str(&format!("- **Last updated:** {updated}\n"));
    }
    if let Some(created) = entry.created_at.as_ref() {
        out.push_str(&format!("- **Created:** {created}\n"));
    }
    if let Some(license) = get_entry_license(entry) {
        out.push_str(&format!("- **License:** {license}\n"));
    }
    out.push('\n');

    if !entry.tags.is_empty() {
        out.push_str("## Tags\n\n");
        for tag in entry.tags.iter().take(16) {
            out.push_str(&format!("- `{tag}`\n"));
        }
        out.push('\n');
    }

    let gguf_files: Vec<&HfSibling> = entry
        .siblings
        .iter()
        .filter(|file| file.rfilename.to_ascii_lowercase().ends_with(".gguf"))
        .collect();

    out.push_str("## GGUF Files\n\n");
    if gguf_files.is_empty() {
        out.push_str("- No GGUF files found in this repository.\n");
    } else {
        for file in gguf_files.iter().take(50) {
            let size_label = file
                .size
                .map(format_hf_size)
                .unwrap_or_else(|| "Unknown size".to_string());
            let quant = infer_quantization_from_label(&file.rfilename)
                .unwrap_or_else(|| "unknown".to_string());
            out.push_str(&format!(
                "- `{}` ({size_label}, quant: {quant})\n",
                file.rfilename
            ));
        }
    }

    out
}

#[tauri::command]
pub async fn search_huggingface_gguf(
    query: String,
    filters: Option<ModelFilters>,
) -> Result<Vec<HFModelInfo>, String> {
    let limit = filters
        .as_ref()
        .and_then(|f| f.limit)
        .unwrap_or(20)
        .clamp(1, 1000);
    let offset = filters
        .as_ref()
        .and_then(|f| f.offset)
        .unwrap_or(0);
    let url = format!(
        "https://huggingface.co/api/models?search={}&limit={}&offset={}&full=true",
        urlencoding::encode(&query),
        limit,
        offset
    );

    let client = build_http_client()?;
    let entries: Vec<HfModelApiEntry> = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    let mut out = Vec::new();
    for entry in entries {
        let gguf_files: Vec<RemoteGGUFFile> = entry
            .siblings
            .iter()
            .filter(|f| f.rfilename.to_ascii_lowercase().ends_with(".gguf"))
            .map(|f| RemoteGGUFFile {
                filename: f.rfilename.clone(),
                size: f.size.unwrap_or(0),
                sha256: None,
                quantization: infer_quantization_from_label(&f.rfilename),
                download_url: format!(
                    "https://huggingface.co/{}/resolve/main/{}",
                    entry.id, f.rfilename
                ),
            })
            .collect();

        if gguf_files.is_empty() {
            continue;
        }

        let name = entry
            .id
            .split('/')
            .next_back()
            .unwrap_or(&entry.id)
            .to_string();

        let parameter_count = infer_parameter_count_from_entry(&entry);
        let context_length = infer_context_length_from_entry(&entry);
        let last_modified = entry.last_modified.clone();
        let created_at = entry.created_at.clone();

        out.push(HFModelInfo {
            repo_id: entry.id.clone(),
            name,
            author: entry.id.split('/').next().map(|s| s.to_string()),
            description: entry
                .card_data
                .as_ref()
                .and_then(|d| d.get("description"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            license: get_entry_license(&entry),
            pipeline_tag: entry
                .pipeline_tag
                .clone()
                .or_else(|| get_card_data_string(&entry, "pipeline_tag")),
            library: entry
                .library_name
                .clone()
                .or_else(|| get_card_data_string(&entry, "library_name")),
            languages: get_entry_languages(&entry),
            downloads: entry.downloads,
            likes: entry.likes,
            tags: entry.tags.clone(),
            architectures: vec![],
            quantizations: gguf_files
                .iter()
                .filter_map(|f| f.quantization.clone())
                .collect(),
            gguf_files,
            last_modified,
            created_at,
            parameter_count,
            context_length,
        });
    }

    Ok(out)
}

#[tauri::command]
pub async fn get_hf_model_metadata(repo_id: String) -> Result<HFModelMetadata, String> {
    let url = format!(
        "https://huggingface.co/api/models/{}?blobs=true&files_metadata=true",
        repo_id
    );
    let client = build_http_client()?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "Failed to fetch model metadata via HF REST API: HTTP {}",
            resp.status()
        ));
    }
    let entry: HfModelApiEntry = resp.json().await.map_err(|e| e.to_string())?;
    Ok(HFModelMetadata {
        parameter_count: infer_parameter_count_from_entry(&entry),
        context_length: infer_context_length_from_entry(&entry),
    })
}

#[tauri::command]
pub async fn download_hf_model_file(
    repo_id: String,
    filename: String,
    destination_dir: String,
) -> Result<DownloadedFileInfo, String> {
    let api = hf_hub::api::sync::Api::new().map_err(|e| e.to_string())?;
    let repo = hf_hub::Repo::new(repo_id.clone(), hf_hub::RepoType::Model);
    let src = api
        .repo(repo)
        .get(&filename)
        .map_err(|e| format!("hf_hub get {} failed: {}", filename, e))?;

    let dest_dir = PathBuf::from(destination_dir);
    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let dest = dest_dir.join(&filename);
    fs::copy(&src, &dest).map_err(|e| e.to_string())?;

    let size = fs::metadata(&dest).map_err(|e| e.to_string())?.len();

    Ok(DownloadedFileInfo {
        repo_id,
        filename,
        local_path: dest,
        size,
    })
}

#[tauri::command]
pub async fn get_model_readme(repo_id: String) -> Result<String, String> {
    let url = format!("https://huggingface.co/api/models/{}", repo_id);
    let client = build_http_client()?;
    let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!(
            "Failed to fetch model card via HF REST API: HTTP {}",
            resp.status()
        ));
    }
    let model_entry: HfModelApiEntry = resp.json().await.map_err(|e| e.to_string())?;
    Ok(build_model_card_markdown(&model_entry))
}

#[tauri::command]
pub fn delete_local_model(model_path: String) -> Result<(), String> {
    let path = PathBuf::from(model_path);
    if !path.exists() {
        return Ok(());
    }
    fs::remove_file(path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_model_manifest(
    model_path: String,
    repo_name: Option<String>,
    publisher: Option<String>,
) -> Result<(), String> {
    let target = PathBuf::from(&model_path);
    let existing = load_manifest(&target);

    let repo_name = repo_name
        .or_else(|| existing.as_ref().map(|m| m.repo_name.clone()))
        .or_else(|| target.file_name().map(|s| s.to_string_lossy().to_string()))
        .unwrap_or_else(|| "local-model".to_string());

    let publisher = publisher
        .or_else(|| existing.as_ref().map(|m| m.publisher.clone()))
        .unwrap_or_else(|| "local".to_string());

    let manifest = DownloadManifest {
        version: 1,
        repo_id: existing
            .as_ref()
            .map(|m| m.repo_id.clone())
            .unwrap_or_else(|| "local/local-model".to_string()),
        repo_name: repo_name.clone(),
        publisher,
        format: "gguf".to_string(),
        quantization: infer_quantization_from_label(&repo_name),
        card_id: existing.as_ref().and_then(|m| m.card_id.clone()),
        card_name: existing.as_ref().and_then(|m| m.card_name.clone()),
        downloaded_at: Utc::now().to_rfc3339(),
    };

    save_manifest(&target, &manifest)
}
