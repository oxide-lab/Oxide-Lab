use crate::core::types::LlamaSessionKind;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct SessionKey {
    pub model_id: String,
    pub kind: LlamaSessionKind,
}

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub created_at: u64,
    pub last_health_ok_at: Option<u64>,
}

#[derive(Clone, Default)]
pub struct LlamaCppState {
    pub meta: Arc<Mutex<HashMap<SessionKey, SessionMeta>>>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl LlamaCppState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ensure_meta(
        &self,
        model_id: &str,
        kind: LlamaSessionKind,
    ) -> Result<SessionMeta, String> {
        let mut guard = self.meta.lock().map_err(|e| e.to_string())?;
        let key = SessionKey {
            model_id: model_id.to_string(),
            kind,
        };
        let created = now_unix();
        let entry = guard.entry(key).or_insert_with(|| SessionMeta {
            created_at: created,
            last_health_ok_at: None,
        });
        Ok(entry.clone())
    }

    pub fn mark_health_ok(&self, model_id: &str, kind: LlamaSessionKind) -> Result<(), String> {
        let mut guard = self.meta.lock().map_err(|e| e.to_string())?;
        let key = SessionKey {
            model_id: model_id.to_string(),
            kind,
        };
        let created = now_unix();
        let entry = guard.entry(key).or_insert_with(|| SessionMeta {
            created_at: created,
            last_health_ok_at: None,
        });
        entry.last_health_ok_at = Some(now_unix());
        Ok(())
    }

    pub fn remove_model(&self, model_id: &str) -> Result<(), String> {
        let mut guard = self.meta.lock().map_err(|e| e.to_string())?;
        guard.retain(|k, _| k.model_id != model_id);
        Ok(())
    }

    pub fn remove_session(&self, model_id: &str, kind: LlamaSessionKind) -> Result<(), String> {
        let mut guard = self.meta.lock().map_err(|e| e.to_string())?;
        let key = SessionKey {
            model_id: model_id.to_string(),
            kind,
        };
        guard.remove(&key);
        Ok(())
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut guard = self.meta.lock().map_err(|e| e.to_string())?;
        guard.clear();
        Ok(())
    }
}

