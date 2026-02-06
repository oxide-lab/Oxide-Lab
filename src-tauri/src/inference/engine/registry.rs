use super::adapter::EngineAdapter;
use super::types::EngineId;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Default, Clone)]
pub struct EngineRegistry {
    adapters: HashMap<EngineId, Arc<dyn EngineAdapter>>,
}

impl EngineRegistry {
    pub fn register(&mut self, adapter: Arc<dyn EngineAdapter>) {
        self.adapters.insert(adapter.id(), adapter);
    }

    pub fn get(&self, id: EngineId) -> Option<Arc<dyn EngineAdapter>> {
        self.adapters.get(&id).cloned()
    }
}

