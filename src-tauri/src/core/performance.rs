// Модуль для мониторинга производительности
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{Pid, System};
use tokio::sync::RwLock;

/// Метрики производительности для одной операции
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetric {
    pub operation_name: String,
    pub duration_ms: u64,
    pub timestamp: String,
    pub memory_usage_mb: f64,
    pub additional_data: Option<serde_json::Value>,
}

/// Метрики загрузки модели
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelLoadMetrics {
    pub total_duration_ms: u64,
    pub stages: Vec<LoadStage>,
    pub model_size_mb: f64,
    pub memory_before_mb: f64,
    pub memory_after_mb: f64,
    pub memory_delta_mb: f64,
}

/// Стадия загрузки модели
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadStage {
    pub name: String,
    pub duration_ms: u64,
}

/// Метрики inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceMetrics {
    pub prompt_tokens: usize,
    pub generated_tokens: usize,
    pub total_duration_ms: u64,
    pub prefill_duration_ms: u64,
    pub generation_duration_ms: u64,
    pub tokens_per_second: f64,
    pub prefill_tokens_per_second: f64,
    pub memory_usage_mb: f64,
    pub timestamp: String,
}

/// Метрики запуска приложения
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupMetrics {
    pub total_duration_ms: u64,
    pub stages: Vec<StartupStage>,
    pub memory_at_start_mb: f64,
    pub memory_at_ready_mb: f64,
    pub timestamp: String,
}

/// Стадия запуска приложения
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartupStage {
    pub name: String,
    pub duration_ms: u64,
}

/// Использование системных ресурсов
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemUsage {
    pub cpu_usage_percent: f32,
    pub memory_usage_mb: f64,
    pub gpu_usage_percent: Option<f32>,
    pub gpu_memory_mb: Option<f64>,
    pub timestamp: String,
}

/// Монитор производительности
pub struct PerformanceMonitor {
    metrics: Arc<RwLock<Vec<PerformanceMetric>>>,
    max_entries: usize,
    system: Arc<RwLock<System>>,
    startup_metrics: Arc<RwLock<Option<StartupMetrics>>>,
}

impl PerformanceMonitor {
    pub fn new(max_entries: usize) -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        Self {
            metrics: Arc::new(RwLock::new(Vec::new())),
            max_entries,
            system: Arc::new(RwLock::new(system)),
            startup_metrics: Arc::new(RwLock::new(None)),
        }
    }

    /// Записать метрику
    pub async fn record_metric(&self, metric: PerformanceMetric) {
        let mut metrics = self.metrics.write().await;

        // Ограничиваем количество записей
        if metrics.len() >= self.max_entries {
            metrics.remove(0);
        }

        metrics.push(metric);
    }

    /// Получить все метрики
    pub async fn get_metrics(&self) -> Vec<PerformanceMetric> {
        let metrics = self.metrics.read().await;
        metrics.clone()
    }

    /// Получить среднюю длительность операции
    pub async fn get_average_duration(&self, operation_name: &str) -> Option<f64> {
        let metrics = self.metrics.read().await;
        let operation_metrics: Vec<_> = metrics
            .iter()
            .filter(|m| m.operation_name == operation_name)
            .collect();

        if operation_metrics.is_empty() {
            return None;
        }

        let total: u64 = operation_metrics.iter().map(|m| m.duration_ms).sum();
        Some(total as f64 / operation_metrics.len() as f64)
    }

    /// Получить текущее использование памяти процессом
    pub async fn get_memory_usage_mb(&self) -> f64 {
        let mut system = self.system.write().await;
        system.refresh_all();

        let pid = Pid::from_u32(std::process::id());
        if let Some(process) = system.process(pid) {
            // Память в байтах
            let memory_bytes = process.memory();
            // Конвертируем в мегабайты
            (memory_bytes as f64) / (1024.0 * 1024.0)
        } else {
            0.0
        }
    }

    /// Очистить все метрики
    pub async fn clear_metrics(&self) {
        let mut metrics = self.metrics.write().await;
        metrics.clear();
    }

    /// Сохранить метрики запуска
    pub async fn set_startup_metrics(&self, metrics: StartupMetrics) {
        let mut startup_metrics = self.startup_metrics.write().await;
        *startup_metrics = Some(metrics);
    }

    /// Получить метрики запуска
    pub async fn get_startup_metrics(&self) -> Option<StartupMetrics> {
        let startup_metrics = self.startup_metrics.read().await;
        startup_metrics.clone()
    }

    /// Получить текущее использование системных ресурсов
    pub async fn get_system_usage(&self) -> SystemUsage {
        // Use the hardware plugin as a single source of system telemetry (Jan-like flow).
        let hw_usage = oxide_hardware::commands::get_system_usage();

        let memory_usage_mb = hw_usage.used_memory as f64;
        let total_gpu_memory: u64 = hw_usage.gpus.iter().map(|g| g.total_memory).sum();
        let used_gpu_memory: u64 = hw_usage.gpus.iter().map(|g| g.used_memory).sum();

        let gpu_memory_mb = if hw_usage.gpus.is_empty() {
            None
        } else {
            Some(used_gpu_memory as f64)
        };

        let gpu_usage_percent = if total_gpu_memory > 0 {
            Some(((used_gpu_memory as f64 / total_gpu_memory as f64) * 100.0) as f32)
        } else {
            None
        };

        SystemUsage {
            cpu_usage_percent: hw_usage.cpu,
            memory_usage_mb,
            gpu_usage_percent,
            gpu_memory_mb,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Таймер для измерения производительности
pub struct PerformanceTimer {
    start: Instant,
    operation_name: String,
    monitor: Option<Arc<PerformanceMonitor>>,
}

impl PerformanceTimer {
    /// Создать новый таймер
    pub fn new(operation_name: impl Into<String>) -> Self {
        Self {
            start: Instant::now(),
            operation_name: operation_name.into(),
            monitor: None,
        }
    }

    /// Создать таймер с мониторингом
    pub fn with_monitor(
        operation_name: impl Into<String>,
        monitor: Arc<PerformanceMonitor>,
    ) -> Self {
        Self {
            start: Instant::now(),
            operation_name: operation_name.into(),
            monitor: Some(monitor),
        }
    }

    /// Получить длительность с момента создания таймера
    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    /// Получить длительность в миллисекундах
    pub fn elapsed_ms(&self) -> u64 {
        self.elapsed().as_millis() as u64
    }

    /// Завершить измерение и записать метрику
    pub async fn finish(self) -> u64 {
        let duration_ms = self.elapsed_ms();

        if let Some(monitor) = self.monitor {
            let memory_usage = monitor.get_memory_usage_mb().await;
            let metric = PerformanceMetric {
                operation_name: self.operation_name,
                duration_ms,
                timestamp: chrono::Utc::now().to_rfc3339(),
                memory_usage_mb: memory_usage,
                additional_data: None,
            };

            monitor.record_metric(metric).await;
        }

        duration_ms
    }

    /// Завершить измерение с дополнительными данными
    pub async fn finish_with_data(self, data: serde_json::Value) -> u64 {
        let duration_ms = self.elapsed_ms();

        if let Some(monitor) = self.monitor {
            let memory_usage = monitor.get_memory_usage_mb().await;
            let metric = PerformanceMetric {
                operation_name: self.operation_name,
                duration_ms,
                timestamp: chrono::Utc::now().to_rfc3339(),
                memory_usage_mb: memory_usage,
                additional_data: Some(data),
            };

            monitor.record_metric(metric).await;
        }

        duration_ms
    }
}

/// Трекер загрузки модели
pub struct ModelLoadTracker {
    start: Instant,
    stages: Vec<(String, Instant)>,
    memory_before_mb: f64,
    monitor: Arc<PerformanceMonitor>,
}

impl ModelLoadTracker {
    /// Создать новый трекер загрузки
    pub async fn new(monitor: Arc<PerformanceMonitor>) -> Self {
        let memory_before = monitor.get_memory_usage_mb().await;

        Self {
            start: Instant::now(),
            stages: Vec::new(),
            memory_before_mb: memory_before,
            monitor,
        }
    }

    /// Начать новую стадию загрузки
    pub fn start_stage(&mut self, stage_name: impl Into<String>) {
        self.stages.push((stage_name.into(), Instant::now()));
    }

    /// Завершить трекинг и вернуть метрики
    pub async fn finish(self, model_size_mb: f64) -> ModelLoadMetrics {
        let total_duration_ms = self.start.elapsed().as_millis() as u64;
        let memory_after_mb = self.monitor.get_memory_usage_mb().await;
        let memory_delta_mb = memory_after_mb - self.memory_before_mb;
        let finished_at = Instant::now();

        let mut load_stages = Vec::new();
        for (idx, (stage_name, stage_start)) in self.stages.iter().enumerate() {
            let stage_end = self
                .stages
                .get(idx + 1)
                .map(|(_, next_start)| *next_start)
                .unwrap_or(finished_at);
            let stage_duration = stage_end
                .saturating_duration_since(*stage_start)
                .as_millis() as u64;
            load_stages.push(LoadStage {
                name: stage_name.clone(),
                duration_ms: stage_duration,
            });
        }

        ModelLoadMetrics {
            total_duration_ms,
            stages: load_stages,
            model_size_mb,
            memory_before_mb: self.memory_before_mb,
            memory_after_mb,
            memory_delta_mb,
        }
    }
}

/// Трекер inference
pub struct InferenceTracker {
    start: Instant,
    prefill_start: Option<Instant>,
    generation_start: Option<Instant>,
    prompt_tokens: usize,
    generated_tokens: usize,
    monitor: Arc<PerformanceMonitor>,
}

impl InferenceTracker {
    /// Создать новый трекер inference
    pub fn new(prompt_tokens: usize, monitor: Arc<PerformanceMonitor>) -> Self {
        Self {
            start: Instant::now(),
            prefill_start: None,
            generation_start: None,
            prompt_tokens,
            generated_tokens: 0,
            monitor,
        }
    }

    /// Отметить начало prefill
    pub fn start_prefill(&mut self) {
        self.prefill_start = Some(Instant::now());
    }

    /// Отметить начало generation
    pub fn start_generation(&mut self) {
        self.generation_start = Some(Instant::now());
    }

    /// Увеличить счётчик сгенерированных токенов
    pub fn increment_generated_tokens(&mut self) {
        self.generated_tokens += 1;
    }

    /// Завершить трекинг и вернуть метрики
    pub async fn finish(self) -> InferenceMetrics {
        let total_duration_ms = self.start.elapsed().as_millis() as u64;

        let prefill_duration_ms = if let Some(prefill_start) = self.prefill_start {
            if let Some(generation_start) = self.generation_start {
                (generation_start - prefill_start).as_millis() as u64
            } else {
                0
            }
        } else {
            0
        };

        let generation_duration_ms = if let Some(generation_start) = self.generation_start {
            generation_start.elapsed().as_millis() as u64
        } else {
            total_duration_ms
        };

        let tokens_per_second = if generation_duration_ms > 0 {
            (self.generated_tokens as f64) / (generation_duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        let prefill_tokens_per_second = if prefill_duration_ms > 0 {
            (self.prompt_tokens as f64) / (prefill_duration_ms as f64 / 1000.0)
        } else {
            0.0
        };

        let memory_usage_mb = self.monitor.get_memory_usage_mb().await;

        InferenceMetrics {
            prompt_tokens: self.prompt_tokens,
            generated_tokens: self.generated_tokens,
            total_duration_ms,
            prefill_duration_ms,
            generation_duration_ms,
            tokens_per_second,
            prefill_tokens_per_second,
            memory_usage_mb,
            timestamp: chrono::Utc::now().to_rfc3339(),
        }
    }
}

/// Трекер запуска приложения
pub struct StartupTracker {
    start: Instant,
    stages: Vec<(String, Instant)>,
    memory_at_start_mb: f64,
    monitor: Arc<PerformanceMonitor>,
}

impl StartupTracker {
    /// Создать новый трекер запуска
    pub async fn new(monitor: Arc<PerformanceMonitor>) -> Self {
        let memory_at_start = monitor.get_memory_usage_mb().await;

        Self {
            start: Instant::now(),
            stages: Vec::new(),
            memory_at_start_mb: memory_at_start,
            monitor,
        }
    }

    /// Отметить завершение стадии
    pub fn stage_completed(&mut self, stage_name: impl Into<String>) {
        self.stages.push((stage_name.into(), Instant::now()));
    }

    /// Завершить трекинг и сохранить метрики
    pub async fn finish(self) -> StartupMetrics {
        let total_duration_ms = self.start.elapsed().as_millis() as u64;
        let memory_at_ready_mb = self.monitor.get_memory_usage_mb().await;

        let mut startup_stages = Vec::new();
        let mut prev_time = self.start;

        for (stage_name, stage_time) in self.stages {
            let stage_duration = (stage_time - prev_time).as_millis() as u64;
            startup_stages.push(StartupStage {
                name: stage_name,
                duration_ms: stage_duration,
            });
            prev_time = stage_time;
        }

        let metrics = StartupMetrics {
            total_duration_ms,
            stages: startup_stages,
            memory_at_start_mb: self.memory_at_start_mb,
            memory_at_ready_mb,
            timestamp: chrono::Utc::now().to_rfc3339(),
        };

        // Сохраняем метрики в мониторе
        self.monitor.set_startup_metrics(metrics.clone()).await;

        metrics
    }
}

/// Макрос для измерения производительности блока кода
#[macro_export]
macro_rules! measure_performance {
    ($monitor:expr_2021, $operation_name:expr_2021, $block:expr_2021) => {{
        let timer = $crate::core::performance::PerformanceTimer::with_monitor(
            $operation_name,
            $monitor.clone(),
        );
        let result = $block;
        timer.finish().await;
        result
    }};
}
