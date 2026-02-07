use std::sync::atomic::{AtomicBool, Ordering};

// Глобальный флаг отмены генерации (разделяем с модулем stream)
pub(crate) static CANCEL_GENERATION: AtomicBool = AtomicBool::new(false);

pub fn cancel_generation_cmd() -> Result<(), String> {
    log::info!("cancel_generation_cmd called - setting CANCEL_GENERATION flag");
    CANCEL_GENERATION.store(true, Ordering::SeqCst);
    Ok(())
}

// Глобальный флаг отмены загрузки модели
pub(crate) static CANCEL_LOADING: AtomicBool = AtomicBool::new(false);

pub fn cancel_model_loading_cmd() -> Result<(), String> {
    CANCEL_LOADING.store(true, Ordering::SeqCst);
    Ok(())
}
