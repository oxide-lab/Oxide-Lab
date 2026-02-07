//! Download manager implementation providing background downloads with pause/resume support.
//! The manager exposes a set of Tauri commands consumed by the Svelte frontend.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use once_cell::sync::Lazy;
use reqwest::header::{HeaderMap, HeaderValue, RANGE};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    fs::OpenOptions,
    io::{AsyncSeekExt, AsyncWriteExt},
    sync::{RwLock, mpsc},
    task::JoinHandle,
};

use super::local_models::build_http_client;

/// Event sent to the frontend whenever the downloads state changes.
pub const DOWNLOAD_EVENT: &str = "download-manager-updated";

/// Describes the status of a download job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Paused,
    Completed,
    Error,
    Cancelled,
}

/// Immutable information for a download job exposed to the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: String,
    pub repo_id: String,
    pub filename: String,
    pub download_url: String,
    pub destination_dir: PathBuf,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub status: DownloadStatus,
    pub speed_bytes_per_sec: Option<f64>,
    pub eta_seconds: Option<f64>,
    pub started_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub error: Option<String>,
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

/// Download job persisted to history once finished.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadHistoryEntry {
    pub id: String,
    pub repo_id: String,
    pub filename: String,
    pub destination_path: PathBuf,
    pub status: DownloadStatus,
    pub total_bytes: Option<u64>,
    pub downloaded_bytes: u64,
    pub finished_at: DateTime<Utc>,
    pub error: Option<String>,
    pub sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DownloadManagerState {
    pub active: HashMap<String, DownloadJob>,
    pub history: Vec<DownloadHistoryEntry>,
    #[serde(skip)]
    history_loaded: bool,
}

/// Control message sent to a running download task.
enum DownloadControl {
    Pause,
    Cancel,
}

struct DownloadTaskHandle {
    control: mpsc::Sender<DownloadControl>,
    join: JoinHandle<()>,
}

struct DownloadTaskChannels {
    tx: mpsc::Sender<DownloadControl>,
    rx: mpsc::Receiver<DownloadControl>,
}

impl DownloadTaskChannels {
    fn new() -> Self {
        let (tx, rx) = mpsc::channel(8);
        Self { tx, rx }
    }
}

struct DownloadManager {
    state: RwLock<DownloadManagerState>,
    tasks: RwLock<HashMap<String, DownloadTaskHandle>>,
}

static MANAGER: Lazy<DownloadManager> = Lazy::new(|| DownloadManager {
    state: RwLock::new(DownloadManagerState::default()),
    tasks: RwLock::new(HashMap::new()),
});

#[derive(Debug)]
struct DownloadContext {
    job_id: String,
    repo_id: String,
    filename: String,
    download_url: String,
    destination_dir: PathBuf,
    total_bytes: Option<u64>,
    sha256: Option<String>,
    group_id: Option<String>,
    display_name: Option<String>,
}

#[derive(Debug)]
enum DownloadLoopOutcome {
    Completed,
    Paused,
    Cancelled,
    Error(String),
}

/// Request payload used to start a download.
#[derive(Debug, Deserialize)]
pub struct StartDownloadRequest {
    pub repo_id: String,
    pub filename: String,
    pub download_url: String,
    pub destination_dir: String,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Snapshot emitted to the frontend.
#[derive(Debug, Clone, Serialize)]
pub struct DownloadManagerSnapshot {
    active: Vec<DownloadJob>,
    history: Vec<DownloadHistoryEntry>,
}

impl DownloadManager {
    fn history_path(app: &AppHandle) -> Result<PathBuf, String> {
        let dir = app
            .path()
            .app_local_data_dir()
            .map_err(|e| format!("Failed to resolve app data directory: {e}"))?;
        let profile_dir = dir.join("oxide-lab").join("downloads");
        fs::create_dir_all(&profile_dir)
            .map_err(|e| format!("Failed to ensure downloads directory: {e}"))?;
        Ok(profile_dir.join("history.json"))
    }

    async fn ensure_history_loaded(&self, app: &AppHandle) -> Result<(), String> {
        let mut guard = self.state.write().await;
        if guard.history_loaded {
            return Ok(());
        }

        let path = Self::history_path(app)?;
        if path.exists() {
            let data =
                fs::read(&path).map_err(|e| format!("Failed to read download history: {e}"))?;
            guard.history = serde_json::from_slice(&data)
                .map_err(|e| format!("Failed to parse download history: {e}"))?;
        }
        guard.history_loaded = true;
        Ok(())
    }

    async fn persist_history(&self, app: &AppHandle) -> Result<(), String> {
        let guard = self.state.read().await;
        if !guard.history_loaded {
            return Ok(());
        }
        let path = Self::history_path(app)?;
        let payload = serde_json::to_vec_pretty(&guard.history)
            .map_err(|e| format!("Failed to serialize download history: {e}"))?;
        fs::write(path, payload).map_err(|e| format!("Failed to write download history: {e}"))
    }

    async fn snapshot(&self) -> DownloadManagerSnapshot {
        let guard = self.state.read().await;
        DownloadManagerSnapshot {
            active: guard.active.values().cloned().collect(),
            history: guard.history.clone(),
        }
    }

    async fn emit_update(&self, app: &AppHandle) {
        let snapshot = self.snapshot().await;
        let _ = app.emit(DOWNLOAD_EVENT, snapshot);
    }

    async fn register_task(&self, job_id: String, handle: DownloadTaskHandle) {
        let mut guard = self.tasks.write().await;
        guard.insert(job_id, handle);
    }

    async fn unregister_task(&self, job_id: &str) {
        let mut guard = self.tasks.write().await;
        guard.remove(job_id);
    }

    async fn get_task_control(&self, job_id: &str) -> Option<mpsc::Sender<DownloadControl>> {
        let guard = self.tasks.read().await;
        guard.get(job_id).map(|handle| handle.control.clone())
    }

    async fn take_task_handle(&self, job_id: &str) -> Option<DownloadTaskHandle> {
        let mut guard = self.tasks.write().await;
        guard.remove(job_id)
    }

    async fn update_job<F, T>(&self, job_id: &str, mutate: F) -> Option<T>
    where
        F: FnOnce(&mut DownloadJob) -> T,
    {
        let mut guard = self.state.write().await;
        guard.active.get_mut(job_id).map(mutate)
    }

    async fn remove_job(&self, job_id: &str) -> Option<DownloadJob> {
        let mut guard = self.state.write().await;
        guard.active.remove(job_id)
    }

    async fn record_history(&self, entry: DownloadHistoryEntry) {
        let mut guard = self.state.write().await;
        guard.history.push(entry);
    }
}

fn sanitize_download_url(repo_id: &str, url: &str) -> Result<(), String> {
    if !url.starts_with("https://huggingface.co/") {
        return Err("Download URL must originate from huggingface.co".to_string());
    }
    let encoded_repo = repo_id.replace('/', "%2F");
    if !(url.contains(repo_id) || url.contains(&encoded_repo)) {
        return Err("Download URL does not match repository id".to_string());
    }
    Ok(())
}

fn build_job_id(repo_id: &str, filename: &str) -> String {
    format!("{repo_id}::{filename}")
}

fn resolve_destination_path(destination_dir: &Path, filename: &str) -> PathBuf {
    destination_dir.join(filename)
}

fn resolve_partial_path(destination_dir: &Path, filename: &str) -> PathBuf {
    destination_dir.join(format!("{filename}.part"))
}

async fn ensure_destination_dir(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|e| format!("Failed to create destination directory: {e}"))?;
    }
    Ok(())
}

async fn compute_existing_size(path: &Path) -> Result<u64, String> {
    match tokio::fs::metadata(path).await {
        Ok(meta) => Ok(meta.len()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(err) => Err(format!("Failed to inspect partial file: {err}")),
    }
}

async fn rename_partial_to_final(partial: &Path, final_path: &Path) -> Result<(), String> {
    tokio::fs::rename(partial, final_path)
        .await
        .map_err(|e| format!("Failed to finalize downloaded file: {e}"))
}

fn compute_speed_and_eta(
    prev_bytes: u64,
    current_bytes: u64,
    elapsed: Duration,
    total: Option<u64>,
) -> (Option<f64>, Option<f64>) {
    if elapsed.as_secs_f64() <= 0.0 {
        return (None, None);
    }
    let delta = current_bytes.saturating_sub(prev_bytes) as f64;
    let speed = delta / elapsed.as_secs_f64();
    if speed <= 0.0 {
        return (None, None);
    }
    let eta = total
        .filter(|total_size| *total_size > current_bytes)
        .map(|total_size| (total_size - current_bytes) as f64 / speed);
    (Some(speed), eta)
}

async fn persist_download_error(job_id: &str, err: &str) {
    log::error!("Download {job_id} failed: {err}");
}

async fn persist_download_cancelled(job_id: &str) {
    log::info!("Download {job_id} cancelled");
}

async fn persist_download_paused(job_id: &str) {
    log::info!("Download {job_id} paused");
}

async fn persist_download_completed(job_id: &str, destination: &Path) {
    log::info!(
        "Download {job_id} completed and saved to {}",
        destination.display()
    );
}

async fn run_download_loop(
    app: AppHandle,
    ctx: DownloadContext,
    mut control_rx: mpsc::Receiver<DownloadControl>,
) -> DownloadLoopOutcome {
    let DownloadContext {
        job_id,
        repo_id,
        filename,
        download_url,
        destination_dir,
        total_bytes,
        sha256,
        group_id,
        display_name,
    } = ctx;
    let mut total_bytes = total_bytes;

    let destination_dir = Arc::new(destination_dir);
    if let Err(err) = ensure_destination_dir(&destination_dir).await {
        return DownloadLoopOutcome::Error(err);
    }

    let final_path = resolve_destination_path(&destination_dir, &filename);
    let partial_path = resolve_partial_path(&destination_dir, &filename);

    let mut downloaded_bytes = match compute_existing_size(&partial_path).await {
        Ok(size) => size,
        Err(err) => return DownloadLoopOutcome::Error(err),
    };

    {
        let manager = &*MANAGER;
        manager
            .update_job(&job_id, |job| {
                job.status = DownloadStatus::Downloading;
                job.started_at.get_or_insert_with(Utc::now);
                job.updated_at = Some(Utc::now());
                job.downloaded_bytes = downloaded_bytes;
                job.total_bytes = total_bytes;
                job.speed_bytes_per_sec = None;
                job.eta_seconds = None;
            })
            .await;
        manager.emit_update(&app).await;
    }

    let client = match build_http_client() {
        Ok(client) => client,
        Err(err) => return DownloadLoopOutcome::Error(err),
    };

    let mut headers = HeaderMap::new();
    if downloaded_bytes > 0 {
        let header_value = format!("bytes={downloaded_bytes}-");
        if let Ok(value) = HeaderValue::from_str(&header_value) {
            headers.insert(RANGE, value);
        }
    }

    let request = match client.get(&download_url).headers(headers).build() {
        Ok(req) => req,
        Err(err) => return DownloadLoopOutcome::Error(format!("Failed to build request: {err}")),
    };

    let response = match client.execute(request).await {
        Ok(resp) => resp,
        Err(err) => return DownloadLoopOutcome::Error(format!("Failed to start download: {err}")),
    };

    if !response.status().is_success() && response.status().as_u16() != 206 {
        return DownloadLoopOutcome::Error(format!(
            "Unexpected response status: {}",
            response.status()
        ));
    }

    if total_bytes.is_none() {
        let content_len = response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|raw| raw.parse::<u64>().ok());
        if let Some(len) = content_len {
            total_bytes = Some(len + downloaded_bytes);
            let manager = &*MANAGER;
            manager
                .update_job(&job_id, |job| {
                    job.total_bytes = total_bytes;
                })
                .await;
            manager.emit_update(&app).await;
        }
    }

    let mut stream = response.bytes_stream();
    let mut file = match OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(&partial_path)
        .await
    {
        Ok(file) => file,
        Err(err) => return DownloadLoopOutcome::Error(format!("Failed to open file: {err}")),
    };

    if downloaded_bytes > 0
        && let Err(err) = file.seek(std::io::SeekFrom::Start(downloaded_bytes)).await
    {
        return DownloadLoopOutcome::Error(format!("Failed to seek partial file: {err}"));
    }

    let mut last_instant = Instant::now();
    let mut last_bytes = downloaded_bytes;

    loop {
        tokio::select! {
            control = control_rx.recv() => {
                match control {
                    Some(DownloadControl::Pause) => {
                        if let Err(err) = file.flush().await {
                            return DownloadLoopOutcome::Error(format!("Failed to flush file on pause: {err}"));
                        }
                        persist_download_paused(&job_id).await;
                        return DownloadLoopOutcome::Paused;
                    }
                    Some(DownloadControl::Cancel) => {
                        if let Err(err) = file.flush().await {
                            log::warn!("Failed to flush file on cancel: {err}");
                        }
                        let _ = tokio::fs::remove_file(&partial_path).await;
                        let _ = tokio::fs::remove_file(&final_path).await;
                        persist_download_cancelled(&job_id).await;
                        return DownloadLoopOutcome::Cancelled;
                    }
                    None => {}
                }
            }
            chunk = stream.next() => {
                match chunk {
                    Some(Ok(bytes)) => {
                        if let Err(err) = file.write_all(&bytes).await {
                            return DownloadLoopOutcome::Error(format!("Failed to write chunk: {err}"));
                        }
                        downloaded_bytes += bytes.len() as u64;

                        let now = Instant::now();
                        if now.duration_since(last_instant) >= Duration::from_millis(500) {
                            let (speed, eta) = compute_speed_and_eta(
                                last_bytes,
                                downloaded_bytes,
                                now.duration_since(last_instant),
                                total_bytes,
                            );
                            last_instant = now;
                            last_bytes = downloaded_bytes;

                            let manager = &*MANAGER;
                            manager
                                .update_job(&job_id, |job| {
                                    job.downloaded_bytes = downloaded_bytes;
                                    job.speed_bytes_per_sec = speed;
                                    job.eta_seconds = eta;
                                    job.updated_at = Some(Utc::now());
                                })
                                .await;
                            manager.emit_update(&app).await;
                        }
                    }
                    Some(Err(err)) => {
                        return DownloadLoopOutcome::Error(format!("Network error: {err}"));
                    }
                    None => {
                        break;
                    }
                }
            }
        }
    }

    if let Err(err) = file.flush().await {
        return DownloadLoopOutcome::Error(format!("Failed to flush file: {err}"));
    }

    if let Err(err) = rename_partial_to_final(&partial_path, &final_path).await {
        return DownloadLoopOutcome::Error(err);
    }

    persist_download_completed(&job_id, &final_path).await;

    {
        let manager = &*MANAGER;
        manager
            .update_job(&job_id, |job| {
                job.status = DownloadStatus::Completed;
                job.downloaded_bytes = downloaded_bytes;
                job.total_bytes = job.total_bytes.or(total_bytes).or(Some(downloaded_bytes));
                job.speed_bytes_per_sec = None;
                job.eta_seconds = None;
                job.updated_at = Some(Utc::now());
                job.finished_at = Some(Utc::now());
            })
            .await;
        manager.emit_update(&app).await;

        manager
            .record_history(DownloadHistoryEntry {
                id: job_id.clone(),
                repo_id,
                filename,
                destination_path: final_path,
                status: DownloadStatus::Completed,
                total_bytes: total_bytes.or(Some(downloaded_bytes)),
                downloaded_bytes,
                finished_at: Utc::now(),
                error: None,
                sha256,
                group_id: group_id.clone(),
                display_name: display_name.clone(),
            })
            .await;
        if let Err(err) = manager.persist_history(&app).await {
            log::warn!("Failed to persist download history: {err}");
        }
        manager.remove_job(&job_id).await;
        manager.emit_update(&app).await;
    }

    DownloadLoopOutcome::Completed
}

async fn init_job(request: &StartDownloadRequest, job_id: &str) -> Result<DownloadJob, String> {
    sanitize_download_url(&request.repo_id, &request.download_url)?;
    let destination_dir = PathBuf::from(&request.destination_dir);
    Ok(DownloadJob {
        id: job_id.to_string(),
        repo_id: request.repo_id.clone(),
        filename: request.filename.clone(),
        download_url: request.download_url.clone(),
        destination_dir,
        total_bytes: request.total_bytes,
        downloaded_bytes: 0,
        status: DownloadStatus::Queued,
        speed_bytes_per_sec: None,
        eta_seconds: None,
        started_at: None,
        updated_at: None,
        finished_at: None,
        error: None,
        sha256: request.sha256.clone(),
        group_id: request.group_id.clone(),
        display_name: request.display_name.clone(),
    })
}

async fn start_task(app: AppHandle, job: DownloadJob) -> Result<(), String> {
    let job_id = job.id.clone();
    let job_id_for_task = job_id.clone();
    let ctx = DownloadContext {
        job_id: job.id.clone(),
        repo_id: job.repo_id.clone(),
        filename: job.filename.clone(),
        download_url: job.download_url.clone(),
        destination_dir: job.destination_dir.clone(),
        total_bytes: job.total_bytes,
        sha256: job.sha256.clone(),
        group_id: job.group_id.clone(),
        display_name: job.display_name.clone(),
    };

    {
        let manager = &*MANAGER;
        manager
            .update_job(&job_id, |job| {
                job.status = DownloadStatus::Queued;
                job.speed_bytes_per_sec = None;
                job.eta_seconds = None;
                job.error = None;
                job.updated_at = Some(Utc::now());
            })
            .await;
        manager.emit_update(&app).await;
    }

    let DownloadTaskChannels { tx, rx } = DownloadTaskChannels::new();
    let app_clone = app.clone();

    let handle = tokio::spawn(async move {
        let outcome = run_download_loop(app_clone.clone(), ctx, rx).await;
        match outcome {
            DownloadLoopOutcome::Completed => {}
            DownloadLoopOutcome::Paused => {
                let manager = &*MANAGER;
                manager
                    .update_job(&job_id_for_task, |job| {
                        job.status = DownloadStatus::Paused;
                        job.speed_bytes_per_sec = None;
                        job.eta_seconds = None;
                        job.updated_at = Some(Utc::now());
                    })
                    .await;
                manager.emit_update(&app_clone).await;
            }
            DownloadLoopOutcome::Cancelled => {
                let manager = &*MANAGER;
                if let Some(mut job) = manager.remove_job(&job_id_for_task).await {
                    job.status = DownloadStatus::Cancelled;
                    job.updated_at = Some(Utc::now());
                    manager.emit_update(&app_clone).await;
                    manager
                        .record_history(DownloadHistoryEntry {
                            id: job.id.clone(),
                            repo_id: job.repo_id.clone(),
                            filename: job.filename.clone(),
                            destination_path: resolve_destination_path(
                                &job.destination_dir,
                                &job.filename,
                            ),
                            status: DownloadStatus::Cancelled,
                            total_bytes: job.total_bytes,
                            downloaded_bytes: job.downloaded_bytes,
                            finished_at: Utc::now(),
                            error: None,
                            sha256: job.sha256.clone(),
                            group_id: job.group_id.clone(),
                            display_name: job.display_name.clone(),
                        })
                        .await;
                    if let Err(err) = MANAGER.persist_history(&app_clone).await {
                        log::warn!("Failed to persist history after cancel: {err}");
                    }
                }
            }
            DownloadLoopOutcome::Error(err) => {
                persist_download_error(&job_id_for_task, &err).await;
                let manager = &*MANAGER;
                manager
                    .update_job(&job_id_for_task, |job| {
                        job.status = DownloadStatus::Error;
                        job.error = Some(err.clone());
                        job.speed_bytes_per_sec = None;
                        job.eta_seconds = None;
                        job.updated_at = Some(Utc::now());
                        job.finished_at = Some(Utc::now());
                    })
                    .await;
                manager.emit_update(&app_clone).await;

                let guard = manager.state.read().await;
                if let Some(job) = guard.active.get(&job_id_for_task) {
                    let entry = DownloadHistoryEntry {
                        id: job.id.clone(),
                        repo_id: job.repo_id.clone(),
                        filename: job.filename.clone(),
                        destination_path: resolve_destination_path(
                            &job.destination_dir,
                            &job.filename,
                        ),
                        status: DownloadStatus::Error,
                        total_bytes: job.total_bytes,
                        downloaded_bytes: job.downloaded_bytes,
                        finished_at: Utc::now(),
                        error: job.error.clone(),
                        sha256: job.sha256.clone(),
                        group_id: job.group_id.clone(),
                        display_name: job.display_name.clone(),
                    };
                    drop(guard);
                    manager.record_history(entry).await;
                    if let Err(err) = manager.persist_history(&app_clone).await {
                        log::warn!("Failed to persist history after error: {err}");
                    }
                }
            }
        }

        MANAGER.unregister_task(&job_id_for_task).await;
    });

    MANAGER
        .register_task(
            job_id.clone(),
            DownloadTaskHandle {
                control: tx,
                join: handle,
            },
        )
        .await;

    Ok(())
}

/// Start a download job and return the queued job information.
#[tauri::command]
pub async fn start_model_download(
    app: AppHandle,
    request: StartDownloadRequest,
) -> Result<DownloadJob, String> {
    MANAGER.ensure_history_loaded(&app).await?;

    if request.destination_dir.trim().is_empty() {
        return Err("Destination directory cannot be empty".to_string());
    }

    let job_id = build_job_id(&request.repo_id, &request.filename);

    {
        let manager = &*MANAGER;
        let guard = manager.state.read().await;
        if guard.active.contains_key(&job_id) {
            return Err("Download is already in progress".to_string());
        }
    }

    let mut job = init_job(&request, &job_id).await?;
    job.status = DownloadStatus::Queued;
    job.updated_at = Some(Utc::now());

    {
        let manager = &*MANAGER;
        manager
            .state
            .write()
            .await
            .active
            .insert(job_id.clone(), job.clone());
        manager.emit_update(&app).await;
    }

    start_task(app.clone(), job.clone()).await?;

    Ok(job)
}

/// Retrieve a snapshot of active downloads and history.
#[tauri::command]
pub async fn get_downloads_snapshot(app: AppHandle) -> Result<DownloadManagerSnapshot, String> {
    MANAGER.ensure_history_loaded(&app).await?;
    Ok(MANAGER.snapshot().await)
}

/// Pause an active download. The partial file remains on disk and can be resumed later.
#[tauri::command]
pub async fn pause_download(app: AppHandle, job_id: String) -> Result<(), String> {
    MANAGER.ensure_history_loaded(&app).await?;
    let Some(control) = MANAGER.get_task_control(&job_id).await else {
        return Err("Download is not active".to_string());
    };
    control
        .send(DownloadControl::Pause)
        .await
        .map_err(|_| "Failed to send pause command".to_string())?;
    MANAGER.emit_update(&app).await;
    Ok(())
}

/// Resume a paused download job.
#[tauri::command]
pub async fn resume_download(app: AppHandle, job_id: String) -> Result<(), String> {
    MANAGER.ensure_history_loaded(&app).await?;

    let job = {
        let manager = &*MANAGER;
        let guard = manager.state.read().await;
        guard
            .active
            .get(&job_id)
            .cloned()
            .ok_or_else(|| "Download not found".to_string())?
    };

    if job.status != DownloadStatus::Paused && job.status != DownloadStatus::Error {
        return Err("Only paused or error downloads can be resumed".to_string());
    }

    start_task(app, job).await?;
    Ok(())
}

/// Cancel an active download. Partial data is removed.
#[tauri::command]
pub async fn cancel_download(app: AppHandle, job_id: String) -> Result<(), String> {
    MANAGER.ensure_history_loaded(&app).await?;
    let mut cancelled_job = None;
    match MANAGER.get_task_control(&job_id).await {
        Some(control) => {
            control
                .send(DownloadControl::Cancel)
                .await
                .map_err(|_| "Failed to send cancel command".to_string())?;
        }
        _ => {
            cancelled_job = MANAGER.remove_job(&job_id).await;
        }
    }

    if let Some(handle) = MANAGER.take_task_handle(&job_id).await {
        let _ = handle.join.await;
    }

    let manager = &*MANAGER;

    if let Some(job) = cancelled_job {
        let partial = resolve_partial_path(&job.destination_dir, &job.filename);
        let final_path = resolve_destination_path(&job.destination_dir, &job.filename);
        let _ = tokio::fs::remove_file(partial).await;
        let _ = tokio::fs::remove_file(final_path).await;

        manager
            .record_history(DownloadHistoryEntry {
                id: job.id.clone(),
                repo_id: job.repo_id.clone(),
                filename: job.filename.clone(),
                destination_path: resolve_destination_path(&job.destination_dir, &job.filename),
                status: DownloadStatus::Cancelled,
                total_bytes: job.total_bytes,
                downloaded_bytes: job.downloaded_bytes,
                finished_at: Utc::now(),
                error: None,
                sha256: job.sha256.clone(),
                group_id: job.group_id.clone(),
                display_name: job.display_name.clone(),
            })
            .await;
        manager.persist_history(&app).await?;
    }

    manager.emit_update(&app).await;
    Ok(())
}

/// Remove a completed download from history and optionally delete the file.
#[tauri::command]
pub async fn remove_download_entry(
    app: AppHandle,
    job_id: String,
    delete_file: bool,
) -> Result<(), String> {
    MANAGER.ensure_history_loaded(&app).await?;
    {
        let mut guard = MANAGER.state.write().await;
        if let Some(pos) = guard.history.iter().position(|entry| entry.id == job_id) {
            let entry = guard.history.remove(pos);
            if delete_file
                && entry.status == DownloadStatus::Completed
                && let Err(err) = fs::remove_file(&entry.destination_path)
            {
                log::warn!(
                    "Failed to delete file {}: {err}",
                    entry.destination_path.display()
                );
            }
        }
    }
    MANAGER.persist_history(&app).await?;
    MANAGER.emit_update(&app).await;
    Ok(())
}

/// Clear the entire download history (completed/cancelled/error jobs).
#[tauri::command]
pub async fn clear_download_history(app: AppHandle) -> Result<(), String> {
    MANAGER.ensure_history_loaded(&app).await?;
    {
        let mut guard = MANAGER.state.write().await;
        guard.history.clear();
    }
    MANAGER.persist_history(&app).await?;
    MANAGER.emit_update(&app).await;
    Ok(())
}
