//! Adapted from Jan's tauri-plugin-llamacpp process/session state (Apache-2.0).
//! Source reference: example/clients/jan/src-tauri/plugins/tauri-plugin-llamacpp/src/state.rs

use crate::core::types::LlamaSessionKind;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::process::Child;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub pid: i32,
    pub port: u16,
    pub model_id: String,
    pub model_path: String,
    pub api_key: String,
    pub kind: LlamaSessionKind,
    pub created_at: u64,
    pub last_health_ok_at: Option<u64>,
}

#[derive(Debug, Clone, Eq)]
pub struct SessionKey {
    pub model_id: String,
    pub kind: LlamaSessionKind,
}

impl PartialEq for SessionKey {
    fn eq(&self, other: &Self) -> bool {
        self.model_id == other.model_id && self.kind == other.kind
    }
}

impl Hash for SessionKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.model_id.hash(state);
        self.kind.hash(state);
    }
}

pub struct LlamaSession {
    pub child: Child,
    pub info: SessionInfo,
}

#[derive(Clone, Default)]
pub struct LlamaCppState {
    pub sessions: Arc<Mutex<HashMap<SessionKey, LlamaSession>>>,
}

impl LlamaCppState {
    pub fn new() -> Self {
        Self::default()
    }
}
