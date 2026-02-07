//! Precision commands - deprecated, kept for API compatibility
//!
//! Dtype is now automatically detected from model config.json
//! These commands are no-ops to maintain UI compatibility.

use crate::core::precision::{Precision, PrecisionPolicy};
use crate::core::state::SharedState;

#[tauri::command]
pub fn get_precision_policy(
    _state: tauri::State<'_, SharedState>,
) -> Result<PrecisionPolicy, String> {
    // Return default - dtype is now from model config
    Ok(PrecisionPolicy::Default)
}

#[tauri::command]
pub fn set_precision_policy(
    _state: tauri::State<'_, SharedState>,
    _policy: PrecisionPolicy,
) -> Result<(), String> {
    // No-op - dtype is now from model config
    Ok(())
}

#[tauri::command]
pub fn get_precision(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, SharedState>,
) -> Result<Precision, String> {
    // Return default
    Ok(Precision::default())
}

#[tauri::command]
pub fn set_precision(
    _app: tauri::AppHandle,
    _state: tauri::State<'_, SharedState>,
    _precision_str: String,
) -> Result<(), String> {
    // No-op - dtype is now from model config
    Ok(())
}
