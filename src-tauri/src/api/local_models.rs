//! Local model management with GGUF metadata parsing via `gguf` crate.

use crate::api::model_manager::manifest::{
    DownloadManifest, infer_quantization_from_label, load_manifest, save_manifest,
};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::fs;
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
    Safetensors,
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
    pub candle_compatible: bool,
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

fn parse_gguf_metadata_impl(path: &Path) -> Result<GGUFMetadata, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let parsed = gguf::GGUFFile::read(&bytes)?
        .ok_or_else(|| "GGUF file appears truncated or incomplete".to_string())?;
    let architecture = as_string(metadata_value(&parsed, "general.architecture"));
    let tokenizer_model = as_string(metadata_value(&parsed, "tokenizer.ggml.model"));

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
        context_length: as_u64(metadata_value(&parsed, "llama.context_length"))
            .or_else(|| as_u64(metadata_value(&parsed, "qwen.context_length"))),
        embedding_length: as_u64(metadata_value(&parsed, "llama.embedding_length"))
            .or_else(|| as_u64(metadata_value(&parsed, "qwen.embedding_length"))),
        block_count: as_u64(metadata_value(&parsed, "llama.block_count"))
            .or_else(|| as_u64(metadata_value(&parsed, "qwen.block_count"))),
        attention_head_count: as_u64(metadata_value(&parsed, "llama.attention.head_count"))
            .or_else(|| as_u64(metadata_value(&parsed, "qwen.attention.head_count"))),
        kv_head_count: as_u64(metadata_value(&parsed, "llama.attention.head_count_kv"))
            .or_else(|| as_u64(metadata_value(&parsed, "qwen.attention.head_count_kv"))),
        rope_dimension: as_u64(metadata_value(&parsed, "llama.rope.dimension_count"))
            .or_else(|| as_u64(metadata_value(&parsed, "qwen.rope.dimension_count"))),
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
    let created = stat.created().or_else(|_| stat.modified()).unwrap_or(std::time::SystemTime::now());

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
        candle_compatible: true,
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

#[tauri::command]
pub fn parse_gguf_metadata(file_path: String) -> Result<GGUFMetadata, String> {
    parse_gguf_metadata_impl(Path::new(&file_path))
}

#[tauri::command]
pub fn scan_models_folder(folder_path: String) -> Result<Vec<ModelInfo>, String> {
    let root = Path::new(&folder_path);
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
pub fn scan_local_models_folder(folder_path: String) -> Result<Vec<ModelInfo>, String> {
    scan_models_folder(folder_path)
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
}

#[derive(Debug, Deserialize)]
struct HfSibling {
    rfilename: String,
    #[serde(default)]
    size: Option<u64>,
}

#[tauri::command]
pub async fn search_huggingface_gguf(
    query: String,
    filters: Option<ModelFilters>,
) -> Result<Vec<HFModelInfo>, String> {
    let limit = filters.as_ref().and_then(|f| f.limit).unwrap_or(20).clamp(1, 100);
    let url = format!(
        "https://huggingface.co/api/models?search={}&limit={}&full=true",
        urlencoding::encode(&query),
        limit
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

        let name = entry.id.split('/').next_back().unwrap_or(&entry.id).to_string();

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
            license: entry
                .card_data
                .as_ref()
                .and_then(|d| d.get("license"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            downloads: entry.downloads,
            likes: entry.likes,
            tags: entry.tags.clone(),
            architectures: vec![],
            quantizations: gguf_files
                .iter()
                .filter_map(|f| f.quantization.clone())
                .collect(),
            gguf_files,
            last_modified: entry.last_modified,
            created_at: entry.created_at,
            parameter_count: None,
            context_length: None,
        });
    }

    Ok(out)
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
    let url = format!("https://huggingface.co/{}/raw/main/README.md", repo_id);
    let client = build_http_client()?;
    let resp = client.get(url).send().await.map_err(|e| e.to_string())?;
    if !resp.status().is_success() {
        return Err(format!("Failed to fetch README: HTTP {}", resp.status()));
    }
    resp.text().await.map_err(|e| e.to_string())
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
        .or_else(|| {
            target
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
        })
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
