//! Lightweight model scheduler for external process-host engines.
//!
//! Keeps only active model id and keep-alive expiration metadata.

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub keep_alive: Duration,
    pub max_loaded_models: usize,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            keep_alive: Duration::from_secs(5 * 60),
            max_loaded_models: 1,
        }
    }
}

impl SchedulerConfig {
    pub fn from_env() -> Self {
        Self::default()
    }

    pub fn with_keep_alive_secs(mut self, secs: u64) -> Self {
        self.keep_alive = Duration::from_secs(secs);
        self
    }
}

#[derive(Debug, Clone)]
pub struct LoadedModelEntry {
    pub last_used: Instant,
    pub model_id: String,
}

impl LoadedModelEntry {
    pub fn new(model_id: String) -> Self {
        Self {
            last_used: Instant::now(),
            model_id,
        }
    }

    pub fn touch(&mut self) {
        self.last_used = Instant::now();
    }

    pub fn is_expired(&self, keep_alive: Duration) -> bool {
        self.last_used.elapsed() > keep_alive
    }
}

pub struct ModelScheduler {
    pub active_model: Option<LoadedModelEntry>,
    pub config: SchedulerConfig,
}

impl ModelScheduler {
    pub fn new(config: SchedulerConfig) -> Self {
        Self {
            active_model: None,
            config,
        }
    }

    pub fn load_model(&mut self, id: String) {
        self.active_model = Some(LoadedModelEntry::new(id));
    }

    pub fn unload_model(&mut self) {
        self.active_model = None;
    }

    pub fn touch_model(&mut self) {
        if let Some(entry) = &mut self.active_model {
            entry.touch();
        }
    }

    pub fn check_expiration(&mut self) -> Option<String> {
        if let Some(entry) = &self.active_model
            && entry.is_expired(self.config.keep_alive)
        {
            let id = entry.model_id.clone();
            self.active_model = None;
            return Some(id);
        }
        None
    }

    pub fn has_model(&self) -> bool {
        self.active_model.is_some()
    }

    pub fn get_model_id(&self) -> Option<String> {
        self.active_model.as_ref().map(|e| e.model_id.clone())
    }
}
