pub mod error;
pub mod handlers;
pub mod responses;
pub mod router;
pub mod types;
pub mod upstream;

use crate::core::settings_v2::{OpenAiServerConfig, OpenAiServerStatus, openai_status_from_config};
use crate::core::state::SharedState;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, oneshot};

pub struct OpenAIServerState {
    pub app_handle: tauri::AppHandle,
    pub model_state: SharedState,
    pub config: OpenAiServerConfig,
}

pub struct OpenAiServerHandle {
    pub shutdown: broadcast::Sender<()>,
    pub finished: oneshot::Receiver<()>,
}

pub async fn start_server(
    app_handle: tauri::AppHandle,
    model_state: SharedState,
    config: OpenAiServerConfig,
) -> Result<OpenAiServerHandle, String> {
    let (shutdown_tx, _) = broadcast::channel::<()>(1);
    let (finished_tx, finished_rx) = oneshot::channel::<()>();
    let state = Arc::new(OpenAIServerState {
        app_handle,
        model_state,
        config: config.clone(),
    });
    let app = router::create_router(state, &config)?;
    let addr_raw = format!("{}:{}", config.bind_host, config.port);
    let addr: SocketAddr = addr_raw
        .parse()
        .map_err(|e| format!("Invalid OpenAI bind address '{}': {e}", addr_raw))?;
    log::info!("OpenAI API server starting on http://{}", addr);
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind OpenAI server on {addr}: {e}"))?;
    let mut shutdown_rx = shutdown_tx.subscribe();

    tokio::spawn(async move {
        let result = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.recv().await;
                log::info!("OpenAI API server shutting down");
            })
            .await;
        if let Err(err) = result {
            log::error!("OpenAI API server exited with error: {err}");
        }
        let _ = finished_tx.send(());
    });

    Ok(OpenAiServerHandle {
        shutdown: shutdown_tx,
        finished: finished_rx,
    })
}

#[derive(Default)]
struct OpenAiRuntime {
    shutdown: Option<broadcast::Sender<()>>,
    config: Option<OpenAiServerConfig>,
    running: bool,
    instance_id: u64,
}

#[derive(Clone)]
pub struct OpenAiServerController {
    app_handle: tauri::AppHandle,
    model_state: SharedState,
    runtime: Arc<Mutex<OpenAiRuntime>>,
}

impl OpenAiServerController {
    pub fn new(app_handle: tauri::AppHandle, model_state: SharedState) -> Self {
        Self {
            app_handle,
            model_state,
            runtime: Arc::new(Mutex::new(OpenAiRuntime::default())),
        }
    }

    pub async fn apply_config(&self, config: OpenAiServerConfig) -> Result<(), String> {
        let shutdown_to_send = {
            let mut runtime = self.runtime.lock().await;
            runtime.running = false;
            runtime.config = Some(config.clone());
            runtime.instance_id = runtime.instance_id.wrapping_add(1);
            runtime.shutdown.take()
        };
        if let Some(shutdown) = shutdown_to_send {
            let _ = shutdown.send(());
        }

        if !config.enabled {
            log::info!("OpenAI server is disabled in settings");
            return Ok(());
        }

        let OpenAiServerHandle { shutdown, finished } = start_server(
            self.app_handle.clone(),
            self.model_state.clone(),
            config.clone(),
        )
        .await?;
        let runtime_ref = self.runtime.clone();
        let mut runtime = self.runtime.lock().await;
        runtime.shutdown = Some(shutdown);
        runtime.config = Some(config);
        runtime.running = true;
        let instance_id = runtime.instance_id;
        drop(runtime);

        tokio::spawn(async move {
            let _ = finished.await;
            let mut runtime = runtime_ref.lock().await;
            if runtime.instance_id == instance_id {
                runtime.running = false;
                runtime.shutdown = None;
            }
        });

        Ok(())
    }

    pub async fn restart(&self) -> Result<(), String> {
        let config = {
            let runtime = self.runtime.lock().await;
            runtime
                .config
                .clone()
                .unwrap_or_else(OpenAiServerConfig::default)
        };
        self.apply_config(config).await
    }

    pub async fn stop(&self) {
        let shutdown_to_send = {
            let mut runtime = self.runtime.lock().await;
            runtime.running = false;
            runtime.config = None;
            runtime.instance_id = runtime.instance_id.wrapping_add(1);
            runtime.shutdown.take()
        };
        if let Some(shutdown) = shutdown_to_send {
            let _ = shutdown.send(());
        }
    }

    pub async fn is_running(&self) -> bool {
        self.runtime.lock().await.running
    }

    pub async fn status_for(&self, config: &OpenAiServerConfig) -> OpenAiServerStatus {
        let running = self.runtime.lock().await.running;
        openai_status_from_config(config, running)
    }
}
