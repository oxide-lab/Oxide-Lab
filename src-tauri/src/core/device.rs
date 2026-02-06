use crate::core::types::DevicePreference;

fn has_cuda_runtime() -> bool {
    std::env::var("CUDA_PATH").is_ok() || std::env::var("CUDA_VISIBLE_DEVICES").is_ok()
}

#[cfg(target_os = "macos")]
fn has_metal_runtime() -> bool {
    true
}

#[cfg(not(target_os = "macos"))]
fn has_metal_runtime() -> bool {
    false
}

pub fn select_device(pref: Option<DevicePreference>) -> DevicePreference {
    match pref.unwrap_or(DevicePreference::Auto) {
        DevicePreference::Auto => {
            if has_cuda_runtime() {
                DevicePreference::Cuda { index: 0 }
            } else if has_metal_runtime() {
                DevicePreference::Metal
            } else {
                DevicePreference::Cpu
            }
        }
        explicit => explicit,
    }
}

pub fn device_label(d: &DevicePreference) -> &'static str {
    match d {
        DevicePreference::Auto => "AUTO",
        DevicePreference::Cpu => "CPU",
        DevicePreference::Cuda { .. } => "CUDA",
        DevicePreference::Metal => "Metal",
    }
}

pub fn cuda_available() -> bool {
    has_cuda_runtime()
}

pub fn simd_caps() -> (bool, bool, bool, bool) {
    let arch = std::env::consts::ARCH;
    let avx = arch.eq_ignore_ascii_case("x86_64") || arch.eq_ignore_ascii_case("x86");
    let neon = arch.eq_ignore_ascii_case("aarch64") || arch.eq_ignore_ascii_case("arm");
    // WebAssembly SIMD is irrelevant in native desktop app; expose false.
    let simd128 = false;
    let f16c = avx;
    (avx, neon, simd128, f16c)
}
