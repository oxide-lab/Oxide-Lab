use crate::inference::engine::{EngineSessionInfo, EngineSessionKind};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    pub keep_alive_secs: u64,
    pub max_loaded_models: u32,
    pub max_queue: u32,
    pub queue_wait_timeout_ms: u64,
    pub vram_recovery_timeout_ms: u64,
    pub vram_recovery_poll_ms: u64,
    pub vram_recovery_threshold: f32,
    pub expiration_tick_ms: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            keep_alive_secs: 300,
            max_loaded_models: 0,
            max_queue: 128,
            queue_wait_timeout_ms: 15_000,
            vram_recovery_timeout_ms: 5_000,
            vram_recovery_poll_ms: 250,
            vram_recovery_threshold: 0.75,
            expiration_tick_ms: 1_000,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RequestPriority {
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct SessionKey {
    pub model_id: String,
    pub kind: EngineSessionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadedSessionSnapshot {
    pub model_id: String,
    pub kind: EngineSessionKind,
    pub port: u16,
    pub pid: i32,
    pub ref_count: usize,
    pub estimated_vram_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SchedulerSnapshot {
    pub loaded_models: Vec<String>,
    pub loaded_sessions: Vec<LoadedSessionSnapshot>,
    pub queue_len: usize,
    pub inflight: usize,
    pub timestamp: u64,
    pub shutting_down: bool,
}

pub type LeaseId = u64;

#[derive(Debug)]
pub struct SessionLease {
    lease_id: LeaseId,
    key: SessionKey,
    session: EngineSessionInfo,
    release_tx: mpsc::UnboundedSender<LeaseRelease>,
    released: std::sync::atomic::AtomicBool,
}

impl SessionLease {
    pub(crate) fn new(
        lease_id: LeaseId,
        key: SessionKey,
        session: EngineSessionInfo,
        release_tx: mpsc::UnboundedSender<LeaseRelease>,
    ) -> Self {
        Self {
            lease_id,
            key,
            session,
            release_tx,
            released: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn session(&self) -> &EngineSessionInfo {
        &self.session
    }
}

impl Drop for SessionLease {
    fn drop(&mut self) {
        if self
            .released
            .swap(true, std::sync::atomic::Ordering::SeqCst)
        {
            return;
        }
        let _ = self.release_tx.send(LeaseRelease {
            lease_id: self.lease_id,
            key: self.key.clone(),
        });
    }
}

#[derive(Debug)]
pub(crate) struct LeaseRelease {
    pub lease_id: LeaseId,
    pub key: SessionKey,
}

#[derive(Debug)]
pub enum AcquireError {
    Busy,
    Timeout { queue_position: usize },
    Shutdown,
    Internal(String),
}

impl Display for AcquireError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AcquireError::Busy => write!(f, "server busy, maximum scheduler queue size reached"),
            AcquireError::Timeout { queue_position } => {
                write!(f, "queue wait timeout at position {}", queue_position)
            }
            AcquireError::Shutdown => write!(f, "scheduler is shutting down"),
            AcquireError::Internal(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AcquireError {}

#[derive(Debug)]
pub struct AcquireResult {
    pub lease: SessionLease,
    pub waited_ms: u64,
    pub queue_position: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerStats {
    pub queue_len: usize,
    pub inflight: usize,
    pub loaded_models: usize,
    pub loaded_sessions: usize,
    pub shutting_down: bool,
    pub timestamp: u64,
}

impl From<SchedulerSnapshot> for SchedulerStats {
    fn from(value: SchedulerSnapshot) -> Self {
        Self {
            queue_len: value.queue_len,
            inflight: value.inflight,
            loaded_models: value.loaded_models.len(),
            loaded_sessions: value.loaded_sessions.len(),
            shutting_down: value.shutting_down,
            timestamp: value.timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn session_lease_drop_is_idempotent() {
        let (tx, mut rx) = mpsc::unbounded_channel::<LeaseRelease>();
        let lease = SessionLease::new(
            7,
            SessionKey {
                model_id: "model".to_string(),
                kind: EngineSessionKind::Chat,
            },
            EngineSessionInfo {
                engine_id: crate::inference::engine::EngineId::Llamacpp,
                model_id: "model".to_string(),
                model_path: "m.gguf".to_string(),
                mmproj_path: None,
                pid: 1,
                port: 1111,
                api_key: "k".to_string(),
                kind: EngineSessionKind::Chat,
                created_at: 1,
                last_health_ok_at: None,
            },
            tx,
        );

        drop(lease);
        let first = rx.recv().await;
        assert!(first.is_some());
        assert!(rx.try_recv().is_err());
    }
}
