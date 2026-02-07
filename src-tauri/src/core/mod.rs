pub mod attachments_text;
pub mod background_mode;
pub mod config;
pub mod device;
pub mod log;
pub mod performance;
pub mod precision;
pub mod prefix_cache;
pub mod prompt;
pub mod rayon_pool;
pub mod settings_v2;
pub mod state;
pub mod template_registry;
pub mod templates;
pub mod thread_priority;
pub mod types;

pub use rayon_pool::INFERENCE_POOL;
