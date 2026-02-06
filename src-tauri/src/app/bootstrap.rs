use std::sync::{Arc, Mutex};
use tauri::Emitter;
#[cfg(debug_assertions)]
use tauri::Manager;

use crate::api::commands::threads::{apply_rayon_thread_limit, default_rayon_thread_limit};
use crate::core::device::select_device;
use crate::core::performance::StartupTracker;
use crate::core::rayon_pool::init_global_low_priority_pool;
use crate::core::state::{ModelState, SharedState};
use crate::core::thread_priority::set_current_thread_above_normal;
use crate::core::types::DevicePreference;
use crate::i18n;
use crate::log_load_warn;

use tauri_plugin_sql::{Builder, Migration, MigrationKind};

#[tauri::command]
fn get_app_info() -> serde_json::Value {
    serde_json::json!({
        "name": env!("CARGO_PKG_NAME"),
        "version": env!("CARGO_PKG_VERSION"),
        "authors": env!("CARGO_PKG_AUTHORS"),
        "description": env!("CARGO_PKG_DESCRIPTION"),
    })
}

fn build_shared_state() -> SharedState {
    let initial_device = select_device(Some(DevicePreference::Auto));
    Arc::new(Mutex::new(ModelState::new(initial_device)))
}

fn spawn_startup_tracker(
    app_handle: tauri::AppHandle,
    performance_monitor: Arc<crate::core::performance::PerformanceMonitor>,
) {
    tauri::async_runtime::spawn(async move {
        let mut tracker = StartupTracker::new(performance_monitor).await;

        tracker.stage_completed("tauri_init");
        tracker.stage_completed("plugins_init");
        tracker.stage_completed("state_init");

        let startup_metrics = tracker.finish().await;

        if let Err(e) = app_handle.emit("startup_metrics", &startup_metrics) {
            eprintln!("Failed to emit startup metrics: {e}");
        }

        println!(
            "Application startup completed in {} ms",
            startup_metrics.total_duration_ms
        );
    });
}

pub fn run() {
    // Инициализируем i18n
    i18n::init();

    let shared = build_shared_state();
    let llama_cpp_state = crate::inference::llamacpp::state::LlamaCppState::new();
    let llama_cpp_state_for_exit = llama_cpp_state.clone();
    let performance_monitor = {
        let guard = shared.lock().expect("Failed to lock shared state");
        guard.performance_monitor.clone()
    };

    let migrations = vec![
        Migration {
            version: 1,
            description: "create sessions and messages tables",
            sql: "
                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    model_path TEXT,
                    repo_id TEXT,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_sessions_updated ON sessions(updated_at DESC);
                
                CREATE TABLE IF NOT EXISTS messages (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    session_id TEXT NOT NULL,
                    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'system')),
                    content TEXT NOT NULL DEFAULT '',
                    created_at INTEGER NOT NULL,
                    FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
                );
                CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id);
            ",
            kind: MigrationKind::Up,
        },
        Migration {
            version: 2,
            description: "add thinking column to messages",
            sql: "ALTER TABLE messages ADD COLUMN thinking TEXT NOT NULL DEFAULT '';",
            kind: MigrationKind::Up,
        },
    ];

    let app = tauri::Builder::default()
        .plugin(oxide_hardware::init())
        .plugin(oxide_llamacpp::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(
            Builder::default()
                .add_migrations("sqlite:chat_history.db", migrations)
                .build(),
        )
        .manage(shared.clone())
        .manage(llama_cpp_state.clone())
        .invoke_handler(tauri::generate_handler![
            crate::api::greet,
            get_app_info,
            crate::api::load_model,
            crate::api::unload_model,
            crate::api::cancel_model_loading,
            crate::api::generate_stream,
            crate::api::cancel_generation,
            crate::api::set_device,
            crate::api::is_model_loaded,
            crate::api::get_chat_template,
            crate::api::render_prompt,
            crate::api::get_device_info,
            crate::api::probe_cuda,
            crate::api::get_precision_policy,
            crate::api::set_precision_policy,
            crate::api::get_precision,
            crate::api::set_precision,
            crate::api::get_rayon_thread_limit,
            crate::api::set_rayon_thread_limit,
            crate::api::gguf_list_metadata_keys_from_path,
            crate::api::gguf_list_metadata_keys,
            crate::api::get_experimental_features_enabled,
            crate::api::set_experimental_features_enabled,
            crate::api::performance_api::get_performance_metrics,
            crate::api::performance_api::get_average_duration,
            crate::api::performance_api::get_memory_usage,
            crate::api::performance_api::clear_performance_metrics,
            crate::api::performance_api::get_startup_metrics,
            crate::api::performance_api::get_system_usage,
            crate::api::local_models::parse_gguf_metadata,
            crate::api::local_models::scan_models_folder,
            crate::api::local_models::scan_local_models_folder,
            crate::api::local_models::search_huggingface_gguf,
            crate::api::local_models::download_hf_model_file,
            crate::api::local_models::get_model_readme,
            crate::api::local_models::delete_local_model,
            crate::api::local_models::update_model_manifest,
            crate::api::model_cards::get_model_cards,
            crate::api::model_cards::import_model_cards,
            crate::api::model_cards::reset_model_cards,
            crate::api::model_cards::download_model_card_format,
            crate::api::download_manager::start_model_download,
            crate::api::download_manager::get_downloads_snapshot,
            crate::api::download_manager::pause_download,
            crate::api::download_manager::resume_download,
            crate::api::download_manager::cancel_download,
            crate::api::download_manager::remove_download_entry,
            crate::api::download_manager::clear_download_history,
            crate::api::get_locale,
            crate::api::set_locale,
            crate::api::openai_server::get_server_config,
            crate::api::prefix_cache_api::get_prefix_cache_info,
            crate::api::prefix_cache_api::set_prefix_cache_enabled,
            crate::api::prefix_cache_api::clear_prefix_cache,
            crate::api::get_active_backend,
            crate::api::get_backend_preference,
            crate::api::set_backend_preference,
            crate::api::get_llama_runtime_config,
            crate::api::set_llama_runtime_config,
        ])
        .setup(move |app| {
            // Hybrid responsiveness: keep the window/event-loop thread slightly prioritized on Windows,
            // while compute threads run at lower priority via the Rayon start handler.
            let _ = set_current_thread_above_normal();

            let handle = app.handle();
            match ModelState::load_thread_limit(handle) {
                Ok(limit) => {
                    // Leave 1 core free by default to keep UI responsive during heavy loads.
                    // If user explicitly configured a limit, always respect it.
                    let effective_limit = limit.or_else(|| Some(default_rayon_thread_limit()));
                    apply_rayon_thread_limit(effective_limit);
                    if let Some(threads) = effective_limit {
                        match init_global_low_priority_pool(threads) {
                            Ok(true) => {}
                            Ok(false) => log_load_warn!("global rayon pool already initialized; low-priority start handler not applied"),
                            Err(e) => log_load_warn!("failed to initialize global rayon pool: {}", e),
                        }
                    }
                    if let Ok(mut guard) = shared.lock() {
                        guard.rayon_thread_limit = limit;
                    }
                }
                Err(err) => {
                    eprintln!("Failed to load saved Rayon thread limit: {}", err);
                }
            }
            match ModelState::load_llama_runtime(handle) {
                Ok(Some(cfg)) => {
                    if let Ok(mut guard) = shared.lock() {
                        guard.llama_runtime = cfg;
                    }
                }
                Ok(None) => {}
                Err(err) => {
                    eprintln!("Failed to load llama runtime config: {}", err);
                }
            }
            spawn_startup_tracker(app.handle().clone(), performance_monitor.clone());

            // Start the model scheduler keep-alive task
            let scheduler_state = shared.clone();
            let scheduler_llama_state = llama_cpp_state.clone();
            let scheduler_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let app = scheduler_app_handle;
                // Проверяем каждую минуту (настраивается)
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                // Первый тик происходит немедленно, пропускаем его
                interval.tick().await;

                loop {
                    interval.tick().await;
                    let expired_model_id = if let Ok(mut guard) = scheduler_state.lock() {
                        guard.scheduler.check_expiration()
                    } else {
                        log::error!("Scheduler keep-alive task: failed to lock state");
                        None
                    };

                    if let Some(unloaded_id) = expired_model_id {
                        let manager = crate::inference::engine::default_session_manager(
                            app.clone(),
                            scheduler_llama_state.clone(),
                        );
                        if let Err(e) = manager.stop_model_sessions(&unloaded_id).await {
                            log::error!(
                                "Scheduler keep-alive task: failed to unload llama sessions for {}: {}",
                                unloaded_id,
                                e
                            );
                        }

                        if let Ok(mut guard) = scheduler_state.lock() {
                            if guard.active_model_id.as_deref() == Some(unloaded_id.as_str()) {
                                guard.context_length = 4096;
                                guard.model_path = None;
                                guard.hub_repo_id = None;
                                guard.hub_revision = None;
                                guard.chat_template = None;
                                guard.active_backend = crate::core::types::ActiveBackend::None;
                                guard.active_model_id = None;
                                guard.active_llama_session = None;
                            }
                        } else {
                            log::error!(
                                "Scheduler keep-alive task: failed to lock state for cleanup"
                            );
                        }

                        if let Err(e) = app.emit("model_unloaded", &unloaded_id) {
                            log::error!("Failed to emit model_unloaded event: {}", e);
                        }
                    }
                }
            });

            // Start OpenAI-compatible API server
            let openai_state = shared.clone();
            let openai_llama_state = llama_cpp_state.clone();
            let openai_app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                use crate::api::openai_server::OPENAI_PORT;
                match crate::api::openai_server::start_server(
                    openai_app_handle,
                    openai_state,
                    openai_llama_state,
                    OPENAI_PORT,
                )
                .await
                {
                    Ok(_shutdown_tx) => {
                        log::info!("OpenAI API server started on port {}", OPENAI_PORT);
                    }
                    Err(e) => {
                        log::error!("Failed to start OpenAI API server: {}", e);
                    }
                }
            });

            #[cfg(debug_assertions)]
            if let Some(main_window) = app.get_webview_window("main") {
                main_window.open_devtools();
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(move |_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            let manager = crate::inference::engine::default_session_manager(
                _handle.clone(),
                llama_cpp_state_for_exit.clone(),
            );
            let _ = tauri::async_runtime::block_on(async { manager.stop_all_sessions(None).await });
        }
    });
}

