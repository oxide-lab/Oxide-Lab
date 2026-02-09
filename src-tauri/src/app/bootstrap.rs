use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};

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
            crate::api::local_models::download_hf_model_file,
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
            crate::api::get_app_settings_v2,
            crate::api::patch_app_settings_v2,
            crate::api::reset_app_settings_v2,
            crate::api::get_data_locations,
            crate::api::export_user_data,
            crate::api::clear_user_data,
            crate::api::get_openai_server_status,
            crate::api::set_openai_server_config,
            crate::api::restart_openai_server,
            crate::api::search_settings_v2,
            crate::api::prefix_cache_api::get_prefix_cache_info,
            crate::api::prefix_cache_api::set_prefix_cache_enabled,
            crate::api::prefix_cache_api::clear_prefix_cache,
            crate::api::get_active_backend,
            crate::api::get_backend_preference,
            crate::api::set_backend_preference,
            crate::api::get_llama_runtime_config,
            crate::api::set_llama_runtime_config,
            crate::api::get_loaded_models,
            crate::api::get_scheduler_stats,
            crate::api::fetch_url,
        ])
        .setup(move |app| {
            // Hybrid responsiveness: keep the window/event-loop thread slightly prioritized on Windows,
            // while compute threads run at lower priority via the Rayon start handler.
            let _ = set_current_thread_above_normal();

            let handle = app.handle();
            let settings_store = match crate::core::settings_v2::SettingsV2Store::load(handle) {
                Ok(store) => store,
                Err(err) => {
                    log::error!("Failed to initialize settings_v2 store: {}", err);
                    return Err(std::io::Error::other(err).into());
                }
            };
            let settings_snapshot = settings_store.get();

            if let Ok(locale) = settings_snapshot.general.locale.parse::<crate::i18n::Locale>() {
                crate::i18n::set_locale(locale);
            }

            // Leave 1 core free by default to keep UI responsive during heavy loads.
            // If user explicitly configured a limit, always respect it.
            let configured_limit = settings_snapshot.performance.manual_thread_limit;
            let effective_limit = configured_limit.or_else(|| Some(default_rayon_thread_limit()));
            apply_rayon_thread_limit(effective_limit);
            if let Some(threads) = effective_limit {
                match init_global_low_priority_pool(threads) {
                    Ok(true) => {}
                    Ok(false) => log_load_warn!(
                        "global rayon pool already initialized; low-priority start handler not applied"
                    ),
                    Err(e) => log_load_warn!("failed to initialize global rayon pool: {}", e),
                }
            }

            if let Ok(mut guard) = shared.lock() {
                guard.rayon_thread_limit = configured_limit;
                guard.llama_runtime = settings_snapshot.performance.llama_runtime.clone();
            }

            app.manage(crate::core::settings_v2::SettingsV2State::new(
                settings_store,
            ));
            spawn_startup_tracker(app.handle().clone(), performance_monitor.clone());

            // Startup cleanup to keep process ownership consistent after crashes/restarts.
            let startup_handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let _ = oxide_llamacpp::cleanup::cleanup_llama_processes(startup_handle).await;
            });

            // Initialize and store the scheduler (single source of runtime sessions ownership).
            let scheduler = crate::inference::scheduler::VramScheduler::new(
                app.handle().clone(),
                llama_cpp_state.clone(),
            );
            app.manage(scheduler);

            let openai_controller =
                crate::api::openai::OpenAiServerController::new(app.handle().clone(), shared.clone());
            app.manage(openai_controller.clone());
            let openai_cfg = settings_snapshot.developer.openai_server.clone();
            tauri::async_runtime::block_on(async move {
                if let Err(err) = openai_controller.apply_config(openai_cfg).await {
                    log::error!("Failed to apply OpenAI server config: {}", err);
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
            if let Some(openai) = _handle.try_state::<crate::api::openai::OpenAiServerController>()
            {
                let controller = openai.inner().clone();
                tauri::async_runtime::block_on(async move {
                    controller.stop().await;
                });
            }
            if let Some(state) = _handle.try_state::<crate::inference::scheduler::VramScheduler>() {
                let scheduler = state.inner().clone();
                let _ = tauri::async_runtime::block_on(async { scheduler.shutdown().await });
            } else {
                tauri::async_runtime::block_on(async {
                    let _ = oxide_llamacpp::cleanup::cleanup_llama_processes(_handle.clone()).await;
                });
            }
        }
    });
}
