use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum Precision {
    #[default]
    Auto,
    Float16,
    BFloat16,
    Float32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrecisionPolicy {
    #[default]
    Default,
    Force(Precision),
}

#[derive(Debug, Clone, Copy, Default)]
pub struct GpuKernelConfig;

impl GpuKernelConfig {
    pub fn apply_for_device<T>(&self, _device: &T) {}
}

pub fn select_dtype_default<T>(_device: &T) -> Precision {
    Precision::Auto
}
