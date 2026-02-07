use serde::Serialize;
use tauri::Emitter;

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static LOAD_SEQ: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize)]
pub struct LoadProgressEvent {
    pub stage: String,
    pub progress: u8,
    pub message: Option<String>,
    pub done: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct LoadDebugCtx {
    start: Instant,
    load_id: u64,
    enabled: bool,
}

impl LoadDebugCtx {
    pub fn new() -> Self {
        let enabled = std::env::var("OXIDE_DEBUG_MODEL_LOAD").ok().as_deref() == Some("1");
        Self {
            start: Instant::now(),
            load_id: LOAD_SEQ.fetch_add(1, Ordering::Relaxed) + 1,
            enabled,
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }

    pub fn stage_begin(&self, stage: &str) {
        if self.enabled {
            crate::log_debug!(
                crate::core::log::Component::Load,
                "[load:{} +{}ms] stage_begin={}",
                self.load_id,
                self.elapsed_ms(),
                stage
            );
        }
    }

    pub fn stage_end(&self, stage: &str, duration: Duration) {
        if self.enabled {
            crate::log_debug!(
                crate::core::log::Component::Load,
                "[load:{} +{}ms] stage_end={} dur_ms={}",
                self.load_id,
                self.elapsed_ms(),
                stage,
                duration.as_millis()
            );
        }
    }
}

impl Default for LoadDebugCtx {
    fn default() -> Self {
        Self::new()
    }
}

pub fn emit_load_progress(
    app: &tauri::AppHandle,
    stage: &str,
    progress: u8,
    message: Option<&str>,
    done: bool,
    error: Option<&str>,
) {
    let payload = LoadProgressEvent {
        stage: stage.to_string(),
        progress,
        message: message.map(|s| s.to_string()),
        done,
        error: error.map(|s| s.to_string()),
    };
    let _ = app.emit("load_progress", payload);
}

pub fn emit_load_progress_debug(
    ctx: &LoadDebugCtx,
    app: &tauri::AppHandle,
    stage: &str,
    progress: u8,
    message: Option<&str>,
    done: bool,
    error: Option<&str>,
) {
    if ctx.enabled {
        crate::log_debug!(
            crate::core::log::Component::Load,
            "[load:{} +{}ms] emit stage={} progress={} done={} message={:?} error={:?}",
            ctx.load_id,
            ctx.elapsed_ms(),
            stage,
            progress,
            done,
            message,
            error
        );
    }
    emit_load_progress(app, stage, progress, message, done, error);
}
