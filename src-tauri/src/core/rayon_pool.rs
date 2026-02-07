use crate::core::thread_priority::set_current_thread_below_normal;
use std::sync::LazyLock;

/// Sets platform-specific thread affinity/priority for inference threads.
///
/// - macOS: Uses QOS_CLASS_USER_INTERACTIVE for P-core scheduling
/// - Windows: Uses default priority (inference pool uses normal priority for max performance)
/// - Other platforms: No-op
#[cfg(target_os = "macos")]
unsafe fn set_inference_thread_affinity() {
    // USER_INTERACTIVE has the highest scheduling priority that user code
    // can request and is most likely to be scheduled on P-cores.
    use libc::{pthread_set_qos_class_self_np, qos_class_t::QOS_CLASS_USER_INTERACTIVE};
    pthread_set_qos_class_self_np(QOS_CLASS_USER_INTERACTIVE, 0);
}

#[cfg(not(target_os = "macos"))]
#[inline(always)]
unsafe fn set_inference_thread_affinity() {
    // On non-macOS platforms we leave affinity untouched for inference pool
}

/// High-priority rayon pool for inference tasks.
/// Uses platform-specific optimizations:
/// - macOS: P-core affinity
/// - Other platforms: Default thread scheduling
pub static INFERENCE_POOL: LazyLock<rayon::ThreadPool> = LazyLock::new(|| {
    rayon::ThreadPoolBuilder::new()
        .thread_name(|idx| format!("oxide-inference-{}", idx))
        .start_handler(|_| unsafe {
            set_inference_thread_affinity();
        })
        .build()
        .expect("Failed to build inference Rayon thread pool")
});

/// Initializes the global Rayon thread pool with a low-priority start handler.
/// This pool is used for background tasks that shouldn't compete with inference.
///
/// Returns `Ok(true)` if the global pool was initialized by this call, or `Ok(false)` if it
/// was already initialized elsewhere.
pub fn init_global_low_priority_pool(num_threads: usize) -> Result<bool, String> {
    let threads = num_threads.max(1);

    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|idx| format!("oxide-rayon-{}", idx))
        .start_handler(|_| {
            let _ = set_current_thread_below_normal();
        })
        .build_global()
        .map(|()| true)
        .or_else(|e| {
            // Rayon can only build the global pool once.
            if e.to_string()
                .to_lowercase()
                .contains("global thread pool has already been initialized")
            {
                Ok(false)
            } else {
                Err(e.to_string())
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_or_reports_already_initialized() {
        let _res = init_global_low_priority_pool(2).unwrap();
        // Either we init it here, or something else already did.
    }

    #[test]
    fn inference_pool_can_be_accessed() {
        // Force lazy initialization
        let pool = &*INFERENCE_POOL;
        assert!(pool.current_num_threads() > 0);
    }
}
