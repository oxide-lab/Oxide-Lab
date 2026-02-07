use crate::api::download_manager::{StartDownloadRequest, start_model_download};
use crate::api::model_manager::manifest::{
    DownloadManifest, infer_quantization_from_label, save_manifest,
};
use crate::log_load;
use chrono::Utc;
use hf_hub::api::tokio::ApiBuilder;
use hf_hub::{Repo, RepoType};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tauri::{AppHandle, async_runtime};

/// Проверяет, поддерживается ли данное квантование в текущем runtime.
fn is_quantization_supported(quant: &str) -> bool {
    let normalized = quant.to_ascii_uppercase();
    matches!(
        normalized.as_str(),
        // Поддерживаемые типы для GGUF-пайплайна.
        "F32" | "F16" | "BF16" |
        "Q4_0" | "Q4_1" | "Q5_0" | "Q5_1" | "Q8_0" | "Q8_1" |
        "Q2_K" | "Q3_K" | "Q4_K" | "Q5_K" | "Q6_K" | "Q8_K" |
        // Вариации K-квантований с суффиксами
        "Q2_K_S" | "Q2_K_M" | "Q2_K_L" |
        "Q3_K_S" | "Q3_K_M" | "Q3_K_L" |
        "Q4_K_S" | "Q4_K_M" | "Q4_K_L" |
        "Q5_K_S" | "Q5_K_M" | "Q5_K_L" |
        "Q8_K_S" | "Q8_K_M" | "Q8_K_L"
    )
}

static MODEL_CARDS: Lazy<RwLock<ModelCardsConfig>> =
    Lazy::new(|| RwLock::new(ModelCardsConfig::default()));

#[derive(Debug, Clone, Default)]
struct ModelCardsConfig {
    version: u32,
    cards: Vec<ModelCard>,
}

#[derive(Debug, Serialize)]
pub struct ModelCardsResponse {
    pub version: u32,
    pub cards: Vec<ModelCardSummary>,
}

#[derive(Debug, Deserialize)]
struct ModelCardsFile {
    version: u32,
    cards: Vec<ModelCard>,
}

/// Command: return summaries of available model cards.
#[tauri::command]
pub fn get_model_cards() -> Result<ModelCardsResponse, String> {
    ensure_model_cards_loaded()?;
    let guard = MODEL_CARDS.read().map_err(|e| e.to_string())?;
    Ok(ModelCardsResponse {
        version: guard.version,
        cards: guard
            .cards
            .iter()
            .map(ModelCardSummary::from_card)
            .collect(),
    })
}

/// Command: download files for the selected card and format.
#[derive(Debug, Clone, Deserialize)]
pub struct DownloadModelCardFormatArgs {
    #[serde(alias = "cardId")]
    card_id: String,
    format: String,
    #[serde(alias = "modelsRoot")]
    models_root: String,
    #[serde(default)]
    quantization: Option<String>,
}

#[tauri::command]
pub async fn download_model_card_format(
    app: AppHandle,
    args: DownloadModelCardFormatArgs,
) -> Result<ModelCardDownloadResult, String> {
    let DownloadModelCardFormatArgs {
        card_id,
        format,
        models_root,
        quantization,
    } = args;
    if models_root.trim().is_empty() {
        return Err("Корневая папка моделей не указана".to_string());
    }

    let card_format = ModelCardFormat::from_str(&format).ok_or_else(|| {
        format!(
            "Поддерживаются только форматы {}",
            ModelCardFormat::variants()
        )
    })?;
    if card_format != ModelCardFormat::Gguf {
        return Err("Unsupported format: only gguf is available in this build".to_string());
    }

    ensure_model_cards_loaded()?;
    let card = {
        let guard = MODEL_CARDS.read().map_err(|e| e.to_string())?;
        guard
            .cards
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(&card_id))
            .ok_or_else(|| format!("Карточка '{}' не найдена", card_id))?
            .clone()
    };

    if !card.supports_format(card_format) {
        return Err(format!(
            "Карточка '{}' не содержит формат {}",
            card_id,
            card_format.as_str()
        ));
    }

    let api = ApiBuilder::new()
        .with_progress(false)
        .build()
        .map_err(|e| format!("Не удалось инициализировать hf-hub: {e}"))?;

    let files = card
        .files_for_format(ModelCardFormat::Gguf, quantization.as_deref())
        .map_err(|e| format!("Карточка некорректна: {e}"))?;

    if files.is_empty() {
        return Err("Нет файлов для загрузки".to_string());
    }

    let (format_repo_id, _) = card.repo_for_format(card_format);
    let (publisher, repo_name) = split_repo_parts(&format_repo_id);
    let manifest_quantization = quantization
        .clone()
        .or_else(|| files.iter().find_map(|file| file.quantization.clone()))
        .or_else(|| infer_quantization_from_label(&repo_name));
    let manifest = DownloadManifest {
        version: 1,
        repo_id: format_repo_id.clone(),
        repo_name: repo_name.clone(),
        publisher: publisher.clone(),
        format: card_format.as_str().to_string(),
        quantization: manifest_quantization,
        card_id: Some(card.id.clone()),
        card_name: Some(card.name.clone()),
        downloaded_at: Utc::now().to_rfc3339(),
    };

    let sanitized = sanitize_folder_name(&card);
    let dest_root = Path::new(&models_root)
        .join(&sanitized)
        .join(card_format.folder_name());
    async_runtime::spawn_blocking({
        let dest_root = dest_root.clone();
        move || {
            fs::create_dir_all(&dest_root)
                .map_err(|e| format!("Не удалось создать папку {}: {e}", dest_root.display()))
        }
    })
    .await
    .map_err(|e| e.to_string())??;

    if let Err(err) = save_manifest(&dest_root, &manifest) {
        eprintln!(
            "Warning: failed to write manifest for {} {}: {}",
            card.id,
            card_format.as_str(),
            err
        );
    }

    let mut downloaded = Vec::new();
    let group_id = format!("model-card::{}::{}", card.id, card_format.as_str());
    let display_name = format!("{}/{} ({})", publisher, repo_name, card_format.as_str());

    for file in files {
        let (repo_id, repo_revision) = card.repo_for_file(card_format, &file);
        let revision = repo_revision.unwrap_or_else(|| "main".to_string());
        let repo = api.repo(Repo::with_revision(
            repo_id.clone(),
            RepoType::Model,
            revision,
        ));
        let download_url = repo.url(&file.filename);

        let request = StartDownloadRequest {
            repo_id: repo_id.clone(),
            filename: file.filename.clone(),
            download_url,
            destination_dir: dest_root.to_string_lossy().to_string(),
            total_bytes: None,
            sha256: None,
            group_id: Some(group_id.clone()),
            display_name: Some(display_name.clone()),
        };

        start_model_download(app.clone(), request)
            .await
            .map_err(|e| {
                format!(
                    "Не удалось поставить {} в менеджер загрузок: {e}",
                    file.filename
                )
            })?;

        downloaded.push(dest_root.join(&file.filename).to_string_lossy().to_string());
    }

    log_load!(
        "model card downloaded: card={} format={} files={}",
        card.id,
        card_format.as_str(),
        downloaded.len()
    );

    Ok(ModelCardDownloadResult {
        card_id: card.id.clone(),
        format: card_format.as_str().to_string(),
        destination_dir: dest_root.to_string_lossy().to_string(),
        downloaded_files: downloaded,
        total_bytes: 0,
    })
}

#[tauri::command]
pub fn import_model_cards(config_path: String) -> Result<ModelCardsResponse, String> {
    let config = read_model_cards_config(Path::new(&config_path))?;
    let mut guard = MODEL_CARDS.write().map_err(|e| e.to_string())?;
    *guard = config;
    Ok(ModelCardsResponse {
        version: guard.version,
        cards: guard
            .cards
            .iter()
            .map(ModelCardSummary::from_card)
            .collect(),
    })
}

#[tauri::command]
pub fn reset_model_cards() -> Result<ModelCardsResponse, String> {
    let default_config = load_default_config()?;
    let mut guard = MODEL_CARDS.write().map_err(|e| e.to_string())?;
    *guard = default_config;
    Ok(ModelCardsResponse {
        version: guard.version,
        cards: guard
            .cards
            .iter()
            .map(ModelCardSummary::from_card)
            .collect(),
    })
}

fn ensure_model_cards_loaded() -> Result<(), String> {
    let guard = MODEL_CARDS.read().map_err(|e| e.to_string())?;
    if guard.cards.is_empty() {
        drop(guard);
        let default = load_default_config()?;
        let mut guard = MODEL_CARDS.write().map_err(|e| e.to_string())?;
        *guard = default;
    }
    Ok(())
}

fn load_default_config() -> Result<ModelCardsConfig, String> {
    let path = find_config_path()?;
    read_model_cards_config(&path)
}

fn read_model_cards_config(path: &Path) -> Result<ModelCardsConfig, String> {
    let json = fs::read_to_string(path)
        .map_err(|e| format!("Не удалось прочитать {}: {e}", path.display()))?;
    let parsed: ModelCardsFile =
        serde_json::from_str(&json).map_err(|e| format!("Конфиг некорректен: {e}"))?;
    if parsed.version == 0 {
        return Err("В конфиге должна быть указана версия".to_string());
    }
    Ok(ModelCardsConfig {
        version: parsed.version,
        cards: parsed.cards,
    })
}

fn split_repo_parts(repo_id: &str) -> (String, String) {
    if let Some((publisher, name)) = repo_id.split_once('/') {
        (publisher.to_string(), name.to_string())
    } else {
        ("unknown".to_string(), repo_id.to_string())
    }
}

fn sanitize_folder_name(card: &ModelCard) -> String {
    let raw = if card.id.is_empty() {
        &card.hf_repo_id
    } else {
        &card.id
    };
    raw.chars()
        .map(|ch| match ch {
            '/' | '\\' | ' ' => '_',
            other => other,
        })
        .collect::<String>()
        .to_lowercase()
}

fn find_config_path() -> Result<PathBuf, String> {
    let mut candidates = Vec::new();

    if let Ok(custom) = std::env::var("OXIDE_MODEL_CARDS") {
        candidates.push(PathBuf::from(custom));
    }

    if let Ok(current) = std::env::current_dir() {
        candidates.push(current.join("models/model_cards.json"));
    }

    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        candidates.push(parent.join("models/model_cards.json"));
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    candidates.push(manifest_dir.join("models/model_cards.json"));
    if let Some(parent) = manifest_dir.parent() {
        candidates.push(parent.join("models/model_cards.json"));
    }

    for candidate in candidates {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err("Не удалось найти models/model_cards.json".to_string())
}

#[derive(Debug, Clone, Deserialize)]
struct ModelCard {
    id: String,
    name: String,
    description: String,
    family: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
    hf_repo_id: String,
    revision: Option<String>,
    #[serde(default)]
    supported_formats: Vec<String>,
    sources: Option<ModelCardSources>,
    gguf: Option<ModelCardGguf>,
    safetensors: Option<ModelCardSafetensors>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelCardSources {
    gguf: Option<ModelCardRepo>,
    safetensors: Option<ModelCardRepo>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelCardRepo {
    repo_id: String,
    #[serde(default)]
    revision: Option<String>,
}

impl ModelCard {
    fn supports_format(&self, format: ModelCardFormat) -> bool {
        if !self.supported_formats.is_empty() {
            self.supported_formats
                .iter()
                .any(|item| item.eq_ignore_ascii_case(format.as_str()))
        } else {
            match format {
                ModelCardFormat::Gguf => self.gguf.is_some(),
                ModelCardFormat::Safetensors => self.safetensors.is_some(),
            }
        }
    }

    fn gguf_quantization_options(&self) -> Vec<String> {
        let Some(gguf) = self.gguf.as_ref() else {
            return Vec::new();
        };
        let mut seen = HashSet::new();
        let mut options = Vec::new();
        for file in &gguf.files {
            if let Some(quant) = file.quantization.as_ref() {
                // Пропускаем квантования, которые не поддержаны текущим runtime.
                if !is_quantization_supported(quant) {
                    log::debug!("Skipping unsupported quantization: {}", quant);
                    continue;
                }
                let key = quant.to_ascii_lowercase();
                if seen.insert(key) {
                    options.push(quant.clone());
                }
            }
        }
        options
    }

    fn files_for_format(
        &self,
        format: ModelCardFormat,
        quantization: Option<&str>,
    ) -> Result<Vec<ModelCardFile>, String> {
        match format {
            ModelCardFormat::Gguf => {
                let gg = self.gguf.as_ref().ok_or("GGUF-блок не заполнен")?;
                let base_files = gg
                    .files
                    .iter()
                    .filter(|file| file.quantization.is_none())
                    .cloned()
                    .collect::<Vec<_>>();
                let quant_files = gg
                    .files
                    .iter()
                    .filter(|file| {
                        if let Some(quant) = file.quantization.as_ref() {
                            // Фильтруем только поддерживаемые квантования
                            is_quantization_supported(quant)
                        } else {
                            false
                        }
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                if quant_files.is_empty() {
                    if base_files.is_empty() {
                        Err("GGUF-блок пуст".to_string())
                    } else {
                        Ok(base_files)
                    }
                } else {
                    let mut result = base_files;
                    let selected = select_quantization_files(&quant_files, quantization)?;
                    result.extend(selected);
                    if result.is_empty() {
                        Err("GGUF-блок пуст".to_string())
                    } else {
                        Ok(result)
                    }
                }
            }
            ModelCardFormat::Safetensors => {
                let saf = self
                    .safetensors
                    .as_ref()
                    .ok_or("Safetensors-блок не заполнен")?;
                if saf.weight_files.is_empty() {
                    return Err("Нет safetensors-файлов".to_string());
                }
                let files = saf.weight_files.clone();
                let mut result = Vec::with_capacity(files.len() + 2);
                for filename in files {
                    result.push(ModelCardFile {
                        filename,
                        purpose: Some("weight".to_string()),
                        quantization: None,
                    });
                }
                result.push(ModelCardFile {
                    filename: saf.tokenizer_file.clone(),
                    purpose: Some("tokenizer".to_string()),
                    quantization: None,
                });
                result.push(ModelCardFile {
                    filename: saf.config_file.clone(),
                    purpose: Some("config".to_string()),
                    quantization: None,
                });
                Ok(result)
            }
        }
    }

    fn repo_for_format(&self, format: ModelCardFormat) -> (String, Option<String>) {
        if let Some(sources) = &self.sources {
            match format {
                ModelCardFormat::Gguf => {
                    if let Some(repo) = sources.gguf.as_ref() {
                        return (repo.repo_id.clone(), repo.revision.clone());
                    }
                }
                ModelCardFormat::Safetensors => {
                    if let Some(repo) = sources.safetensors.as_ref() {
                        return (repo.repo_id.clone(), repo.revision.clone());
                    }
                }
            }
        }
        (self.hf_repo_id.clone(), self.revision.clone())
    }

    fn tokenizer_repo(&self) -> (String, Option<String>) {
        if let Some(sources) = &self.sources
            && let Some(repo) = &sources.gguf
        {
            return (repo.repo_id.clone(), repo.revision.clone());
        }
        (self.hf_repo_id.clone(), self.revision.clone())
    }

    fn repo_for_file(
        &self,
        format: ModelCardFormat,
        file: &ModelCardFile,
    ) -> (String, Option<String>) {
        if format == ModelCardFormat::Gguf && file.is_tokenizer() {
            self.tokenizer_repo()
        } else {
            self.repo_for_format(format)
        }
    }
}

impl ModelCardFile {
    fn is_tokenizer(&self) -> bool {
        matches!(self.purpose.as_deref(), Some("tokenizer"))
            || self.filename.eq_ignore_ascii_case("tokenizer.json")
    }
}

fn select_quantization_files(
    quant_files: &[ModelCardFile],
    quantization: Option<&str>,
) -> Result<Vec<ModelCardFile>, String> {
    if quant_files.is_empty() {
        return Ok(vec![]);
    }
    let normalized_requested = quantization.map(|q| q.to_ascii_lowercase());
    let default_quantization = quant_files
        .iter()
        .filter_map(|file| file.quantization.as_deref().map(|q| q.to_ascii_lowercase()))
        .next();
    let target = normalized_requested
        .clone()
        .or(default_quantization.clone())
        .ok_or_else(|| "В GGUF-файлах не найдены квантованные веса".to_string())?;
    let selected = quant_files
        .iter()
        .filter(|file| {
            file.quantization
                .as_ref()
                .map(|q| q.eq_ignore_ascii_case(&target))
                .unwrap_or(false)
        })
        .cloned()
        .collect::<Vec<_>>();
    if selected.is_empty() {
        let display_quant = quantization
            .map(|q| q.to_string())
            .unwrap_or_else(|| target.clone());
        Err(format!("Квантизация '{}' не найдена", display_quant))
    } else {
        Ok(selected)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ModelCardGguf {
    files: Vec<ModelCardFile>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelCardFile {
    filename: String,
    #[serde(default)]
    purpose: Option<String>,
    #[serde(default)]
    quantization: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ModelCardSafetensors {
    #[serde(default)]
    weight_files: Vec<String>,
    tokenizer_file: String,
    config_file: String,
}

#[derive(Debug, Serialize)]
pub struct ModelCardSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub family: Option<String>,
    pub tags: Vec<String>,
    pub hf_repo_id: String,
    pub supported_formats: Vec<String>,
    pub has_gguf: bool,
    pub has_safetensors: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sources: Option<ModelCardSources>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gguf_quantizations: Vec<String>,
}

impl ModelCardSummary {
    fn from_card(card: &ModelCard) -> Self {
        Self {
            id: card.id.clone(),
            name: card.name.clone(),
            description: card.description.clone(),
            family: card.family.clone(),
            tags: card.tags.clone(),
            hf_repo_id: card.hf_repo_id.clone(),
            supported_formats: vec!["gguf".to_string()],
            has_gguf: card.gguf.is_some(),
            has_safetensors: false,
            sources: card.sources.clone(),
            gguf_quantizations: card.gguf_quantization_options(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ModelCardDownloadResult {
    pub card_id: String,
    pub format: String,
    pub destination_dir: String,
    pub downloaded_files: Vec<String>,
    pub total_bytes: u64,
}

#[derive(Copy, Clone, PartialEq)]
enum ModelCardFormat {
    Gguf,
    Safetensors,
}

impl ModelCardFormat {
    fn as_str(&self) -> &'static str {
        match self {
            ModelCardFormat::Gguf => "gguf",
            ModelCardFormat::Safetensors => "safetensors",
        }
    }

    fn from_str(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "gguf" => Some(ModelCardFormat::Gguf),
            "safetensors" => Some(ModelCardFormat::Safetensors),
            _ => None,
        }
    }

    fn variants() -> &'static str {
        "gguf, safetensors"
    }

    fn folder_name(&self) -> &'static str {
        match self {
            ModelCardFormat::Gguf => "gguf",
            ModelCardFormat::Safetensors => "safetensors",
        }
    }
}
