use crate::core::attachments_text::read_attachment_bytes;
use crate::core::limits::MAX_ATTACHMENTS_PER_MESSAGE;
use crate::core::types::Attachment;
use base64::Engine;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Manager;

fn infer_kind_from_mime(mime: Option<&str>) -> Option<String> {
    let lower = mime?.to_ascii_lowercase();
    if lower.starts_with("image/") {
        return Some("image".to_string());
    }
    if lower.starts_with("audio/") {
        return Some("audio".to_string());
    }
    if lower.starts_with("video/") {
        return Some("video".to_string());
    }
    if lower.starts_with("text/") {
        return Some("text".to_string());
    }
    Some("file".to_string())
}

fn parse_data_url(raw: &str) -> Result<(Option<String>, String), String> {
    if !raw.starts_with("data:") {
        return Ok((None, raw.to_string()));
    }
    let Some(comma_pos) = raw.find(',') else {
        return Err("invalid data URL attachment".to_string());
    };
    let header = &raw[..comma_pos];
    let payload = &raw[comma_pos + 1..];
    let mime = header
        .strip_prefix("data:")
        .and_then(|v| v.split(';').next())
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    Ok((mime, payload.to_string()))
}

fn safe_filename(raw: Option<&str>) -> String {
    let default_name = "attachment.bin".to_string();
    let Some(name) = raw else {
        return default_name;
    };
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return default_name;
    }
    Path::new(trimmed)
        .file_name()
        .and_then(|v| v.to_str())
        .map(|v| v.to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or(default_name)
}

#[tauri::command]
pub async fn persist_chat_attachments(
    app: tauri::AppHandle,
    session_id: String,
    attachments: Vec<Attachment>,
) -> Result<Vec<Attachment>, String> {
    if attachments.len() > MAX_ATTACHMENTS_PER_MESSAGE {
        return Err(format!(
            "too many attachments: {} (max {})",
            attachments.len(),
            MAX_ATTACHMENTS_PER_MESSAGE
        ));
    }
    if attachments.is_empty() {
        return Ok(Vec::new());
    }

    let profile_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app data dir: {e}"))?
        .join("oxide-lab")
        .join("attachments")
        .join(session_id);
    fs::create_dir_all(&profile_dir).map_err(|e| e.to_string())?;

    let mut normalized = Vec::with_capacity(attachments.len());
    for mut attachment in attachments {
        if let Some(bytes_b64) = attachment.bytes_b64.clone() {
            let (data_url_mime, payload) = parse_data_url(&bytes_b64)?;
            if data_url_mime.is_some() && attachment.mime.is_none() {
                attachment.mime = data_url_mime;
            }
            attachment.bytes_b64 = Some(payload);
        }

        let bytes = read_attachment_bytes(&attachment)?
            .ok_or_else(|| "attachment content is missing".to_string())?;

        let filename = safe_filename(attachment.name.as_deref());
        let unique_name = format!("{}-{}", uuid::Uuid::new_v4(), filename);
        let target_path: PathBuf = profile_dir.join(unique_name);
        fs::write(&target_path, &bytes).map_err(|e| e.to_string())?;

        let mime = attachment.mime.clone().or_else(|| {
            target_path.extension().and_then(|v| v.to_str()).map(|ext| {
                match ext.to_ascii_lowercase().as_str() {
                    "png" => "image/png".to_string(),
                    "jpg" | "jpeg" => "image/jpeg".to_string(),
                    "webp" => "image/webp".to_string(),
                    "gif" => "image/gif".to_string(),
                    "txt" => "text/plain".to_string(),
                    "md" => "text/markdown".to_string(),
                    _ => "application/octet-stream".to_string(),
                }
            })
        });

        normalized.push(Attachment {
            kind: attachment
                .kind
                .clone()
                .or_else(|| infer_kind_from_mime(mime.as_deref())),
            mime,
            name: Some(filename),
            path: Some(target_path.to_string_lossy().to_string()),
            bytes_b64: Some(base64::engine::general_purpose::STANDARD.encode(bytes)),
            size: Some(
                attachment.size.unwrap_or(0).max(
                    fs::metadata(&target_path)
                        .map(|m| m.len())
                        .unwrap_or_default(),
                ),
            ),
        });
    }

    Ok(normalized)
}

#[tauri::command]
pub fn delete_chat_attachment_files(paths: Vec<String>) -> Result<(), String> {
    for raw in paths {
        let path = PathBuf::from(raw);
        if !path.exists() {
            continue;
        }
        if let Err(err) = fs::remove_file(&path) {
            log::warn!("failed to remove attachment '{}': {}", path.display(), err);
        }
    }
    Ok(())
}
