use super::policy;
use super::types::{
    AcquireError, AcquireResult, LeaseRelease, LoadedSessionSnapshot, RequestPriority,
    SchedulerConfig, SchedulerSnapshot, SchedulerStats, SessionKey, SessionLease,
};
use crate::core::types::LlamaRuntimeConfig;
use crate::inference::engine::{self, EngineSessionInfo, EngineSessionKind, ResolvedModelSource};
use crate::inference::llamacpp::state::LlamaCppState;
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::Emitter;
use tokio::sync::{Mutex, Notify, mpsc, watch};

const CAPACITY_BUSY_ERR: &str = "scheduler_capacity_busy";

#[derive(Debug, Clone)]
pub struct RunnerRef {
    pub key: SessionKey,
    pub session: EngineSessionInfo,
    pub ref_count: usize,
    pub estimated_vram_mb: u64,
    pub session_duration_secs: u64,
    pub last_used: Instant,
    pub created_at: Instant,
}

#[derive(Debug)]
struct QueueEntry {
    id: u64,
    priority: RequestPriority,
    notify: Arc<Notify>,
}

#[derive(Debug)]
struct Inner {
    loaded: HashMap<SessionKey, RunnerRef>,
    queue: VecDeque<QueueEntry>,
    next_lease_id: u64,
    next_queue_id: u64,
    active_loading: bool,
    shutting_down: bool,
    config: SchedulerConfig,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            loaded: HashMap::new(),
            queue: VecDeque::new(),
            next_lease_id: 1,
            next_queue_id: 1,
            active_loading: false,
            shutting_down: false,
            config: SchedulerConfig::default(),
        }
    }
}

#[derive(Clone)]
pub struct VramScheduler {
    app_handle: tauri::AppHandle,
    llama_state: LlamaCppState,
    inner: Arc<Mutex<Inner>>,
    release_tx: mpsc::UnboundedSender<LeaseRelease>,
    snapshot_tx: watch::Sender<SchedulerSnapshot>,
}

impl VramScheduler {
    pub fn new(app_handle: tauri::AppHandle, llama_state: LlamaCppState) -> Self {
        let inner = Arc::new(Mutex::new(Inner::default()));
        let (release_tx, release_rx) = mpsc::unbounded_channel::<LeaseRelease>();
        let (snapshot_tx, _) = watch::channel(SchedulerSnapshot::default());
        let scheduler = Self {
            app_handle,
            llama_state,
            inner,
            release_tx,
            snapshot_tx,
        };

        scheduler.spawn_release_loop(release_rx);
        scheduler.spawn_expiration_loop();
        scheduler
    }

    pub fn snapshot(&self) -> SchedulerSnapshot {
        self.snapshot_tx.borrow().clone()
    }

    pub fn stats(&self) -> SchedulerStats {
        self.snapshot().into()
    }

    pub fn subscribe(&self) -> watch::Receiver<SchedulerSnapshot> {
        self.snapshot_tx.subscribe()
    }

    pub async fn acquire(
        &self,
        kind: EngineSessionKind,
        source: ResolvedModelSource,
        runtime_cfg: LlamaRuntimeConfig,
        priority: RequestPriority,
    ) -> Result<AcquireResult, AcquireError> {
        let begin = Instant::now();
        let queue_wait_timeout = Duration::from_millis(runtime_cfg.scheduler.queue_wait_timeout_ms);
        let mut queued_position = None;
        let mut wait_handle: Option<(u64, Arc<Notify>)> = None;

        loop {
            let mut should_load = false;
            let mut queued_notify = None;
            let key = SessionKey {
                model_id: source.model_id.clone(),
                kind,
            };

            {
                let mut inner = self.inner.lock().await;
                inner.config = SchedulerConfig::from_runtime(&runtime_cfg);

                if inner.shutting_down {
                    return Err(AcquireError::Shutdown);
                }

                if let Some(runner) = inner.loaded.get_mut(&key) {
                    runner.ref_count = runner.ref_count.saturating_add(1);
                    runner.last_used = Instant::now();
                    let lease_key = runner.key.clone();
                    let lease_session = runner.session.clone();
                    let lease_id = Self::take_lease_id(&mut inner);
                    let lease = SessionLease::new(
                        lease_id,
                        lease_key,
                        lease_session,
                        self.release_tx.clone(),
                    );
                    drop(inner);
                    self.publish_snapshot().await;
                    return Ok(AcquireResult {
                        lease,
                        waited_ms: begin.elapsed().as_millis() as u64,
                        queue_position: queued_position,
                    });
                }

                if !inner.active_loading {
                    inner.active_loading = true;
                    should_load = true;
                } else if let Some((_id, notify)) = &wait_handle {
                    queued_notify = Some(notify.clone());
                } else {
                    let max_queue = usize::try_from(inner.config.max_queue).unwrap_or(usize::MAX);
                    if inner.queue.len() >= max_queue {
                        return Err(AcquireError::Busy);
                    }

                    let id = inner.next_queue_id;
                    inner.next_queue_id = inner.next_queue_id.saturating_add(1);
                    let notify = Arc::new(Notify::new());
                    let pos = Self::enqueue_with_priority(&mut inner.queue, id, priority, &notify);
                    wait_handle = Some((id, notify.clone()));
                    queued_position = Some(pos);
                    queued_notify = Some(notify);
                }
            }

            self.publish_snapshot().await;

            if should_load {
                let load_outcome = self
                    .load_session_with_eviction(kind, &source, &runtime_cfg)
                    .await;
                let mut inner = self.inner.lock().await;
                inner.active_loading = false;
                Self::notify_next_waiter(&mut inner.queue);

                match load_outcome {
                    Ok((session, estimate_mb)) => {
                        let runner = RunnerRef {
                            key: key.clone(),
                            session: session.clone(),
                            ref_count: 1,
                            estimated_vram_mb: estimate_mb,
                            session_duration_secs: runtime_cfg.scheduler.keep_alive_secs,
                            last_used: Instant::now(),
                            created_at: Instant::now(),
                        };
                        inner.loaded.insert(key.clone(), runner);
                        let (lease_key, lease_session) = {
                            let runner_ref = inner.loaded.get(&key).expect("runner just inserted");
                            (runner_ref.key.clone(), runner_ref.session.clone())
                        };
                        let lease_id = Self::take_lease_id(&mut inner);
                        let lease = SessionLease::new(
                            lease_id,
                            lease_key,
                            lease_session,
                            self.release_tx.clone(),
                        );
                        drop(inner);
                        self.publish_snapshot().await;
                        return Ok(AcquireResult {
                            lease,
                            waited_ms: begin.elapsed().as_millis() as u64,
                            queue_position: queued_position,
                        });
                    }
                    Err(e) => {
                        drop(inner);
                        self.publish_snapshot().await;
                        if e == CAPACITY_BUSY_ERR {
                            return Err(AcquireError::Busy);
                        }
                        return Err(AcquireError::Internal(e));
                    }
                }
            }

            if let Some(notify) = queued_notify {
                let timeout_res = tokio::time::timeout(queue_wait_timeout, notify.notified()).await;
                match timeout_res {
                    Ok(_) => {
                        if let Some((id, _)) = wait_handle {
                            let mut inner = self.inner.lock().await;
                            Self::remove_queue_entry(&mut inner.queue, id);
                        }
                        wait_handle = None;
                        continue;
                    }
                    Err(_) => {
                        if let Some((id, _)) = wait_handle {
                            let mut inner = self.inner.lock().await;
                            Self::remove_queue_entry(&mut inner.queue, id);
                        }
                        self.publish_snapshot().await;
                        return Err(AcquireError::Timeout {
                            queue_position: queued_position.unwrap_or(1),
                        });
                    }
                }
            }
        }
    }

    pub async fn preload_chat(
        &self,
        source: ResolvedModelSource,
        runtime_cfg: LlamaRuntimeConfig,
    ) -> Result<EngineSessionInfo, String> {
        let acquired = self
            .acquire(
                EngineSessionKind::Chat,
                source,
                runtime_cfg,
                RequestPriority::Low,
            )
            .await
            .map_err(|e| e.to_string())?;
        let session = acquired.lease.session().clone();
        drop(acquired.lease);
        Ok(session)
    }

    pub async fn force_unload_model(&self, model_id: &str) -> Result<(), String> {
        let keys = {
            let mut inner = self.inner.lock().await;
            let keys: Vec<SessionKey> = inner
                .loaded
                .keys()
                .filter(|k| k.model_id == model_id)
                .cloned()
                .collect();
            for key in &keys {
                inner.loaded.remove(key);
            }
            keys
        };
        self.publish_snapshot().await;

        let manager =
            engine::default_session_manager(self.app_handle.clone(), self.llama_state.clone());
        for key in keys {
            manager
                .stop_session_kind(&key.model_id, key.kind)
                .await
                .map_err(|e| format!("failed to stop session {}: {}", key.model_id, e))?;
        }
        Ok(())
    }

    pub async fn force_unload_all(&self) -> Result<(), String> {
        let keys = {
            let mut inner = self.inner.lock().await;
            let keys: Vec<SessionKey> = inner.loaded.keys().cloned().collect();
            inner.loaded.clear();
            keys
        };
        self.publish_snapshot().await;
        let manager =
            engine::default_session_manager(self.app_handle.clone(), self.llama_state.clone());
        for key in keys {
            manager
                .stop_session_kind(&key.model_id, key.kind)
                .await
                .map_err(|e| format!("failed to stop session {}: {}", key.model_id, e))?;
        }
        Ok(())
    }

    pub async fn shutdown(&self) -> Result<(), String> {
        {
            let mut inner = self.inner.lock().await;
            inner.shutting_down = true;
            while let Some(waiter) = inner.queue.pop_front() {
                waiter.notify.notify_one();
            }
        }
        self.publish_snapshot().await;

        let wait_deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let inflight = {
                let inner = self.inner.lock().await;
                inner.loaded.values().map(|r| r.ref_count).sum::<usize>()
            };
            if inflight == 0 || Instant::now() >= wait_deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        self.force_unload_all().await?;
        let _ = oxide_llamacpp::cleanup::cleanup_llama_processes(self.app_handle.clone()).await;
        self.publish_snapshot().await;
        Ok(())
    }

    fn spawn_release_loop(&self, mut release_rx: mpsc::UnboundedReceiver<LeaseRelease>) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            while let Some(release) = release_rx.recv().await {
                let mut inner = this.inner.lock().await;
                if let Some(runner) = inner.loaded.get_mut(&release.key)
                    && runner.ref_count > 0
                {
                    runner.ref_count -= 1;
                    runner.last_used = Instant::now();
                    let _ = release.lease_id;
                }
                drop(inner);
                this.publish_snapshot().await;
            }
        });
    }

    fn spawn_expiration_loop(&self) {
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let tick_ms = {
                    let inner = this.inner.lock().await;
                    inner.config.expiration_tick_ms.max(100)
                };
                tokio::time::sleep(Duration::from_millis(tick_ms)).await;

                let expired_keys = {
                    let inner = this.inner.lock().await;
                    if inner.shutting_down {
                        return;
                    }
                    let keep_alive = Duration::from_secs(inner.config.keep_alive_secs);
                    inner
                        .loaded
                        .iter()
                        .filter_map(|(k, r)| {
                            if r.ref_count == 0 && r.last_used.elapsed() > keep_alive {
                                Some(k.clone())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                };

                if expired_keys.is_empty() {
                    continue;
                }

                for key in expired_keys {
                    if let Err(e) = this.unload_key_with_recovery(&key, None).await {
                        log::warn!(
                            "scheduler expiration unload failed for {}: {}",
                            key.model_id,
                            e
                        );
                    } else if matches!(key.kind, EngineSessionKind::Chat) {
                        let _ = this.app_handle.emit("model_unloaded", key.model_id.clone());
                    }
                }
            }
        });
    }

    async fn load_session_with_eviction(
        &self,
        kind: EngineSessionKind,
        source: &ResolvedModelSource,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<(EngineSessionInfo, u64), String> {
        let candidate_estimate_mb = policy::estimate_candidate_vram_mb(&source.model_path).max(512);
        self.ensure_capacity(candidate_estimate_mb, runtime_cfg)
            .await?;

        let manager =
            engine::default_session_manager(self.app_handle.clone(), self.llama_state.clone());
        let session = manager.start_session(kind, source, runtime_cfg).await?;
        let session = manager.ensure_health(session, runtime_cfg).await?;
        Ok((session, candidate_estimate_mb))
    }

    async fn ensure_capacity(
        &self,
        candidate_estimate_mb: u64,
        runtime_cfg: &LlamaRuntimeConfig,
    ) -> Result<(), String> {
        enum CapacityDecision {
            Ready,
            Evict(SessionKey),
            Busy,
        }

        loop {
            let decision = {
                let inner = self.inner.lock().await;
                let limit = policy::resolve_capacity_limit(
                    &inner.config,
                    &inner.loaded,
                    candidate_estimate_mb,
                );
                let over_capacity = inner.loaded.len() >= limit;
                let need_vram = policy::needs_vram_eviction(candidate_estimate_mb);
                if !(over_capacity || need_vram) {
                    CapacityDecision::Ready
                } else if let Some(candidate) = policy::pick_eviction_candidate(&inner.loaded) {
                    CapacityDecision::Evict(candidate)
                } else {
                    CapacityDecision::Busy
                }
            };

            match decision {
                CapacityDecision::Ready => return Ok(()),
                CapacityDecision::Evict(candidate) => {
                    self.unload_key_with_recovery(&candidate, Some(runtime_cfg))
                        .await?;
                }
                CapacityDecision::Busy => return Err(CAPACITY_BUSY_ERR.to_string()),
            }
        }
    }

    async fn unload_key_with_recovery(
        &self,
        key: &SessionKey,
        runtime_cfg: Option<&LlamaRuntimeConfig>,
    ) -> Result<(), String> {
        let maybe_runner = {
            let mut inner = self.inner.lock().await;
            inner.loaded.remove(key)
        };
        self.publish_snapshot().await;

        let Some(runner) = maybe_runner else {
            return Ok(());
        };

        let before = policy::read_telemetry();

        let manager =
            engine::default_session_manager(self.app_handle.clone(), self.llama_state.clone());
        manager
            .stop_session_kind(&key.model_id, key.kind)
            .await
            .map_err(|e| e.to_string())?;

        let cfg = runtime_cfg
            .map(SchedulerConfig::from_runtime)
            .unwrap_or_default();
        self.wait_for_vram_recovery(before, runner.estimated_vram_mb, &cfg)
            .await;

        Ok(())
    }

    async fn wait_for_vram_recovery(
        &self,
        baseline: Option<policy::TelemetrySnapshot>,
        estimated_vram_mb: u64,
        cfg: &SchedulerConfig,
    ) {
        let Some(before) = baseline else {
            return;
        };
        if before.gpu_count == 0 || estimated_vram_mb == 0 {
            return;
        }

        let timeout_at = Instant::now() + Duration::from_millis(cfg.vram_recovery_timeout_ms);
        let poll = Duration::from_millis(cfg.vram_recovery_poll_ms.max(50));
        let expected_recovery =
            (estimated_vram_mb as f32 * cfg.vram_recovery_threshold.max(0.1)).round() as u64;
        let mut prev_used = before.used_vram_mb;
        let mut read_errors = 0_u8;
        let mut unreliable = false;

        loop {
            if Instant::now() >= timeout_at {
                return;
            }

            tokio::time::sleep(poll).await;

            match policy::read_telemetry() {
                Some(now) => {
                    if now.used_vram_mb > now.total_vram_mb {
                        unreliable = true;
                    }
                    let jump = now.used_vram_mb.abs_diff(prev_used);
                    if now.total_vram_mb > 0 && jump > ((now.total_vram_mb as f32) * 0.95) as u64 {
                        unreliable = true;
                    }
                    prev_used = now.used_vram_mb;

                    if !unreliable {
                        let recovered = now.free_vram_mb.saturating_sub(before.free_vram_mb);
                        if recovered >= expected_recovery {
                            return;
                        }
                    }
                }
                None => {
                    read_errors = read_errors.saturating_add(1);
                    if read_errors >= 2 {
                        unreliable = true;
                    }
                }
            }
        }
    }

    fn take_lease_id(inner: &mut Inner) -> u64 {
        let lease_id = inner.next_lease_id;
        inner.next_lease_id = inner.next_lease_id.saturating_add(1);
        lease_id
    }

    fn notify_next_waiter(queue: &mut VecDeque<QueueEntry>) {
        if let Some(next) = queue.pop_front() {
            next.notify.notify_one();
        }
    }

    fn remove_queue_entry(queue: &mut VecDeque<QueueEntry>, id: u64) {
        if let Some(pos) = queue.iter().position(|e| e.id == id) {
            queue.remove(pos);
        }
    }

    fn enqueue_with_priority(
        queue: &mut VecDeque<QueueEntry>,
        id: u64,
        priority: RequestPriority,
        notify: &Arc<Notify>,
    ) -> usize {
        let rank = |p: RequestPriority| -> i32 {
            match p {
                RequestPriority::High => 0,
                RequestPriority::Normal => 1,
                RequestPriority::Low => 2,
            }
        };

        let mut index = queue.len();
        for (i, item) in queue.iter().enumerate() {
            if rank(priority) < rank(item.priority) {
                index = i;
                break;
            }
        }

        queue.insert(
            index,
            QueueEntry {
                id,
                priority,
                notify: notify.clone(),
            },
        );
        index + 1
    }

    async fn publish_snapshot(&self) {
        let snapshot = {
            let inner = self.inner.lock().await;
            Self::build_snapshot(&inner)
        };
        let _ = self.snapshot_tx.send(snapshot.clone());
        let _ = self.app_handle.emit("scheduler_snapshot", &snapshot);
    }

    fn build_snapshot(inner: &Inner) -> SchedulerSnapshot {
        let mut models = BTreeSet::new();
        let mut sessions = Vec::with_capacity(inner.loaded.len());
        let mut inflight = 0_usize;

        for runner in inner.loaded.values() {
            models.insert(runner.key.model_id.clone());
            inflight = inflight.saturating_add(runner.ref_count);
            sessions.push(LoadedSessionSnapshot {
                model_id: runner.key.model_id.clone(),
                kind: runner.key.kind,
                port: runner.session.port,
                pid: runner.session.pid,
                ref_count: runner.ref_count,
                estimated_vram_mb: runner.estimated_vram_mb,
            });
            let _ = runner.created_at;
        }

        SchedulerSnapshot {
            loaded_models: models.into_iter().collect(),
            loaded_sessions: sessions,
            queue_len: inner.queue.len(),
            inflight,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            shutting_down: inner.shutting_down,
        }
    }
}

impl SchedulerConfig {
    pub fn from_runtime(runtime_cfg: &LlamaRuntimeConfig) -> Self {
        let cfg = &runtime_cfg.scheduler;
        Self {
            keep_alive_secs: cfg.keep_alive_secs,
            max_loaded_models: cfg.max_loaded_models,
            max_queue: cfg.max_queue,
            queue_wait_timeout_ms: cfg.queue_wait_timeout_ms,
            vram_recovery_timeout_ms: cfg.vram_recovery_timeout_ms,
            vram_recovery_poll_ms: cfg.vram_recovery_poll_ms,
            vram_recovery_threshold: cfg.vram_recovery_threshold,
            expiration_tick_ms: cfg.expiration_tick_ms,
        }
    }
}
