pub mod adapter;
pub mod llamacpp;
pub mod registry;
pub mod session_manager;
pub mod types;

use crate::inference::llamacpp::state::LlamaCppState;
use std::sync::Arc;

pub use registry::EngineRegistry;
pub use session_manager::EngineSessionManager;
pub use types::{EngineId, EngineSessionInfo, EngineSessionKind, ResolvedModelSource};

pub fn default_session_manager(
    app_handle: tauri::AppHandle,
    llama_state: LlamaCppState,
) -> EngineSessionManager {
    let mut registry = EngineRegistry::default();
    registry.register(Arc::new(llamacpp::LlamaCppAdapter::new(
        app_handle,
        llama_state,
    )));
    EngineSessionManager::new(registry, EngineId::Llamacpp)
}

