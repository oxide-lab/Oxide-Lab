use base64::Engine as _;

use crate::core::types::Attachment;

const MAX_FILES: usize = 5;
const MAX_SIZE_BYTES: u64 = 20 * 1024 * 1024; // 20 MiB

fn is_txt_md(att: &Attachment) -> bool {
    let mut ok = false;
    if let Some(name) = &att.name {
        let n = name.to_lowercase();
        if n.ends_with(".txt") || n.ends_with(".md") {
            ok = true;
        }
    }
    if !ok && let Some(path) = &att.path {
        let p = path.to_lowercase();
        if p.ends_with(".txt") || p.ends_with(".md") {
            ok = true;
        }
    }
    if !ok && let Some(mime) = &att.mime {
        let mm = mime.to_lowercase();
        if mm == "text/plain" || mm == "text/markdown" || mm == "text/x-markdown" {
            ok = true;
        }
    }
    ok
}

fn read_bytes(att: &Attachment) -> Result<Option<Vec<u8>>, String> {
    if let Some(b64) = &att.bytes_b64 {
        // Быстрая оценка размера без декодирования: ~3/4 длины base64
        let est = (b64.len() as u64) * 3 / 4;
        if est > MAX_SIZE_BYTES {
            return Err(format!(
                "Файл в base64 превышает лимит {} МБ (оценка ~{:.2} МБ)",
                MAX_SIZE_BYTES / (1024 * 1024),
                est as f64 / (1024.0 * 1024.0)
            ));
        }
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .map_err(|e| format!("Failed to decode base64 attachment: {}", e))?;
        if decoded.len() as u64 > MAX_SIZE_BYTES {
            return Err(format!(
                "Файл превышает лимит {} МБ ({} байт)",
                MAX_SIZE_BYTES / (1024 * 1024),
                decoded.len()
            ));
        }
        return Ok(Some(decoded));
    }
    if let Some(p) = &att.path {
        // Проверяем размер по метаданным до чтения
        if let Ok(meta) = std::fs::metadata(p)
            && meta.len() > MAX_SIZE_BYTES
        {
            return Err(format!(
                "Файл '{}' превышает лимит {} МБ ({} байт)",
                p,
                MAX_SIZE_BYTES / (1024 * 1024),
                meta.len()
            ));
        }
        let bytes = std::fs::read(p)
            .map_err(|e| format!("Failed to read attachment from path {}: {}", p, e))?;
        if bytes.len() as u64 > MAX_SIZE_BYTES {
            return Err(format!(
                "Файл '{}' превышает лимит {} МБ после чтения ({} байт)",
                p,
                MAX_SIZE_BYTES / (1024 * 1024),
                bytes.len()
            ));
        }
        return Ok(Some(bytes));
    }
    Ok(None)
}

/// Собрать текст из .txt/.md вложений. Все остальные модальности игнорируются.
/// Возвращает единый блок текста с заголовками для каждого файла, либо пустую строку.
pub fn gather_text_from_attachments(attachments: &[Attachment]) -> Result<String, String> {
    if attachments.is_empty() {
        return Ok(String::new());
    }

    // Фильтруем только .txt/.md
    let txt_md: Vec<&Attachment> = attachments.iter().filter(|a| is_txt_md(a)).collect();
    if txt_md.is_empty() {
        return Ok(String::new());
    }
    if txt_md.len() > MAX_FILES {
        return Err(format!(
            "Слишком много файлов .txt/.md: {} (максимум {})",
            txt_md.len(),
            MAX_FILES
        ));
    }

    let mut out = String::new();
    for att in txt_md.into_iter() {
        let bytes_opt = read_bytes(att)?;
        if let Some(bytes) = bytes_opt {
            let text = String::from_utf8_lossy(&bytes);
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            let title = att
                .name
                .clone()
                .or_else(|| att.path.clone())
                .unwrap_or_else(|| "attachment".to_string());
            out.push_str(&format!("[attached: {}]\n{}", title, text));
        }
    }
    Ok(out)
}
