use crate::core::state::SharedState;
use std::path::Path;

fn read_gguf_metadata_keys(path: &Path) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let parsed = gguf::GGUFFile::read(&bytes)?
        .ok_or_else(|| "GGUF file appears truncated or incomplete".to_string())?;

    let mut keys: Vec<String> = parsed
        .header
        .metadata
        .into_iter()
        .map(|entry| entry.key)
        .collect();
    keys.sort();
    keys.dedup();
    Ok(keys)
}

#[tauri::command]
pub fn gguf_list_metadata_keys_from_path(path: String) -> Result<Vec<String>, String> {
    let p = Path::new(&path);
    if !p.is_file() {
        return Err(format!("Not a file: {path}"));
    }
    if !p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false)
    {
        return Err("Path is not a .gguf file".to_string());
    }
    read_gguf_metadata_keys(p)
}

#[tauri::command]
pub fn gguf_list_metadata_keys(
    state: tauri::State<'_, SharedState>,
) -> Result<Vec<String>, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let path_str = guard
        .model_path
        .as_ref()
        .ok_or_else(|| "No model loaded".to_string())?;
    let p = Path::new(path_str);
    if !p.is_file() {
        return Err(format!("Not a file: {path_str}"));
    }
    if !p
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.eq_ignore_ascii_case("gguf"))
        .unwrap_or(false)
    {
        return Err("Loaded model is not a .gguf file".to_string());
    }
    read_gguf_metadata_keys(p)
}
