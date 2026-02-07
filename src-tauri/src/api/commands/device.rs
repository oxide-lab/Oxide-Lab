use crate::core::device::{cuda_available, device_label, select_device, simd_caps};
use crate::core::state::SharedState;
use crate::core::types::DevicePreference;

use serde::{Deserialize, Serialize};

fn has_cuda_binary_bundle() -> bool {
    let repo_bin = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("example")
        .join("bin");
    let Ok(entries) = std::fs::read_dir(repo_bin) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let p = entry.path();
        p.is_dir()
            && p.file_name()
                .and_then(|s| s.to_str())
                .map(|s| s.to_ascii_lowercase().contains("cuda"))
                .unwrap_or(false)
    })
}

#[tauri::command]
pub fn set_device(
    state: tauri::State<'_, SharedState>,
    pref: DevicePreference,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.device_pref = select_device(Some(pref));
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceInfoDto {
    pub cuda_build: bool,
    pub cuda_available: bool,
    pub current: String,
    pub avx: bool,
    pub neon: bool,
    pub simd128: bool,
    pub f16c: bool,
}

#[tauri::command]
pub fn get_device_info(state: tauri::State<'_, SharedState>) -> Result<DeviceInfoDto, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let current = device_label(&guard.device_pref).to_string();
    let cuda_build = has_cuda_binary_bundle();
    let system_info = oxide_hardware::commands::get_system_info();
    let cuda_available = system_info
        .gpus
        .iter()
        .any(|gpu| matches!(gpu.vendor, oxide_hardware::Vendor::NVIDIA))
        || cuda_available();
    let (avx, neon, simd128, f16c) = simd_caps();

    Ok(DeviceInfoDto {
        cuda_build,
        cuda_available,
        current,
        avx,
        neon,
        simd128,
        f16c,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProbeCudaDto {
    pub cuda_build: bool,
    pub ok: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn probe_cuda() -> Result<ProbeCudaDto, String> {
    let cuda_build = has_cuda_binary_bundle();
    let system_info = oxide_hardware::commands::get_system_info();
    let ok = system_info
        .gpus
        .iter()
        .any(|gpu| matches!(gpu.vendor, oxide_hardware::Vendor::NVIDIA))
        || cuda_available();
    Ok(ProbeCudaDto {
        cuda_build,
        ok,
        error: if ok {
            None
        } else {
            Some("CUDA runtime not detected in environment".to_string())
        },
    })
}
