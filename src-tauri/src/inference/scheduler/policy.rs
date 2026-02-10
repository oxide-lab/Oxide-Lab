use crate::inference::scheduler::runtime::RunnerRef;
use crate::inference::scheduler::types::{SchedulerConfig, SessionKey};
use oxide_hardware::commands::get_system_usage;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;

pub const DEFAULT_UNKNOWN_MODEL_ESTIMATE_MB: u64 = 2_000;
pub const DEFAULT_GPU_OFFLOAD_FACTOR: f64 = 0.55;

#[derive(Debug, Clone, Default)]
pub struct TelemetrySnapshot {
    pub total_vram_mb: u64,
    pub used_vram_mb: u64,
    pub free_vram_mb: u64,
    pub gpu_count: usize,
}

pub fn read_telemetry() -> Option<TelemetrySnapshot> {
    let usage = get_system_usage();
    if usage.gpus.is_empty() {
        return None;
    }

    let mut total = 0_u64;
    let mut used = 0_u64;
    for gpu in &usage.gpus {
        if gpu.total_memory > 0 {
            total = total.saturating_add(gpu.total_memory);
            used = used.saturating_add(gpu.used_memory.min(gpu.total_memory));
        }
    }
    if total == 0 {
        return None;
    }

    Some(TelemetrySnapshot {
        total_vram_mb: total,
        used_vram_mb: used,
        free_vram_mb: total.saturating_sub(used),
        gpu_count: usage.gpus.len(),
    })
}

pub fn estimate_candidate_vram_mb(model_path: &str) -> u64 {
    if let Ok(meta) = fs::metadata(model_path) {
        let file_size_mb = meta.len() / (1024 * 1024);
        return (file_size_mb as f64 * DEFAULT_GPU_OFFLOAD_FACTOR).floor() as u64;
    }
    DEFAULT_UNKNOWN_MODEL_ESTIMATE_MB
}

pub fn avg_estimated_runner_vram_mb(runners: &HashMap<SessionKey, RunnerRef>) -> Option<u64> {
    let mut count = 0_u64;
    let mut sum = 0_u64;
    for runner in runners.values() {
        if matches!(
            runner.key.kind,
            crate::inference::engine::EngineSessionKind::Chat
        ) {
            sum = sum.saturating_add(runner.estimated_vram_mb.max(1));
            count += 1;
        }
    }
    if count == 0 {
        return None;
    }
    Some((sum / count).max(1))
}

pub fn resolve_capacity_limit(
    cfg: &SchedulerConfig,
    loaded: &HashMap<SessionKey, RunnerRef>,
    candidate_estimate_mb: u64,
) -> usize {
    if cfg.max_loaded_models > 0 {
        return usize::try_from(cfg.max_loaded_models).unwrap_or(1).max(1);
    }

    let telemetry = read_telemetry().unwrap_or_default();
    let gpu_count = telemetry.gpu_count.max(1);
    let base_limit = 3_usize.saturating_mul(gpu_count);
    if telemetry.gpu_count == 0 {
        return base_limit.max(1);
    }

    let base_estimate =
        avg_estimated_runner_vram_mb(loaded).unwrap_or(candidate_estimate_mb.max(1));
    let guard_cap = (telemetry.free_vram_mb / base_estimate.max(1)).max(1) as usize;
    base_limit.min(guard_cap).max(1)
}

pub fn needs_vram_eviction(candidate_estimate_mb: u64) -> bool {
    if let Some(telemetry) = read_telemetry() {
        return telemetry.free_vram_mb < candidate_estimate_mb;
    }
    false
}

pub fn pick_eviction_candidate(loaded: &HashMap<SessionKey, RunnerRef>) -> Option<SessionKey> {
    let mut candidates: Vec<&RunnerRef> = loaded.values().filter(|r| r.ref_count == 0).collect();
    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| {
        let d = a.session_duration_secs.cmp(&b.session_duration_secs);
        if d != Ordering::Equal {
            return d;
        }
        let lru = a.last_used.cmp(&b.last_used);
        if lru != Ordering::Equal {
            return lru;
        }
        a.key.model_id.cmp(&b.key.model_id)
    });

    candidates.first().map(|c| c.key.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inference::engine::{EngineId, EngineSessionInfo, EngineSessionKind};

    fn runner(model: &str, kind: EngineSessionKind, ref_count: usize, duration: u64) -> RunnerRef {
        RunnerRef {
            key: SessionKey {
                model_id: model.to_string(),
                kind,
            },
            session: EngineSessionInfo {
                engine_id: EngineId::Llamacpp,
                model_id: model.to_string(),
                model_path: format!("{}.gguf", model),
                mmproj_path: None,
                pid: 1,
                port: 1234,
                api_key: "k".to_string(),
                kind,
                created_at: 1,
                last_health_ok_at: None,
            },
            ref_count,
            estimated_vram_mb: 1024,
            session_duration_secs: duration,
            last_used: std::time::Instant::now(),
            created_at: std::time::Instant::now(),
        }
    }

    #[test]
    fn eviction_prefers_idle_runner() {
        let mut loaded = HashMap::new();
        loaded.insert(
            SessionKey {
                model_id: "a".to_string(),
                kind: EngineSessionKind::Chat,
            },
            runner("a", EngineSessionKind::Chat, 1, 100),
        );
        loaded.insert(
            SessionKey {
                model_id: "b".to_string(),
                kind: EngineSessionKind::Chat,
            },
            runner("b", EngineSessionKind::Chat, 0, 100),
        );

        let selected = pick_eviction_candidate(&loaded).expect("candidate");
        assert_eq!(selected.model_id, "b");
    }
}
