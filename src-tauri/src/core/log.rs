//! Единообразная система логирования для Oxide Lab
//!
//! Этот модуль предоставляет унифицированные макросы и функции для логирования
//! с префиксами для различных компонентов системы.

use log::{Level, LevelFilter};
use std::sync::Once;

static INIT: Once = Once::new();

/// Инициализация системы логирования
///
/// Настраивает env_logger с кастомным форматированием для Oxide Lab.
/// Вызывается автоматически при первом использовании макросов логирования.
pub fn init() {
    INIT.call_once(|| {
        let mut builder = env_logger::Builder::from_default_env();

        // Respect RUST_LOG when provided; otherwise default to INFO.
        if std::env::var("RUST_LOG").is_err() {
            builder.filter_level(LevelFilter::Info);
        }

        builder
            .format(|buf, record| {
                use std::io::Write;

                let level = match record.level() {
                    Level::Error => "ERROR",
                    Level::Warn => "WARN ",
                    Level::Info => "INFO ",
                    Level::Debug => "DEBUG",
                    Level::Trace => "TRACE",
                };

                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                writeln!(
                    buf,
                    "{} {} [{}] {}",
                    timestamp,
                    level,
                    record.target(),
                    record.args()
                )
            })
            .init();
    });
}

/// Компоненты системы для логирования
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    /// Загрузка моделей
    Load,
    /// Вывод/инференс
    Infer,
    /// Hub (HuggingFace)
    Hub,
    /// Локальные файлы
    Local,
    /// Шаблоны чата
    Template,
    /// Устройства (CPU/CUDA/Metal)
    Device,
    /// Валидация
    Validate,
    /// Веса моделей
    Weights,
    /// Генерация
    Generate,
    /// Токенизация
    Tokenizer,
    /// Архитектура моделей
    Architecture,
}

impl Component {
    /// Получить строковое представление компонента
    pub fn as_str(self) -> &'static str {
        match self {
            Component::Load => "load",
            Component::Infer => "infer",
            Component::Hub => "hub",
            Component::Local => "local",
            Component::Template => "template",
            Component::Device => "device",
            Component::Validate => "validate",
            Component::Weights => "weights",
            Component::Generate => "generate",
            Component::Tokenizer => "tokenizer",
            Component::Architecture => "arch",
        }
    }
}

/// Макрос для логирования с префиксом компонента
#[macro_export]
macro_rules! log_info {
    ($component:expr_2021, $($arg:tt)*) => {
        {
            $crate::core::log::init();
            log::info!(target: $component.as_str(), $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! log_warn {
    ($component:expr_2021, $($arg:tt)*) => {
        {
            $crate::core::log::init();
            log::warn!(target: $component.as_str(), $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($component:expr_2021, $($arg:tt)*) => {
        {
            $crate::core::log::init();
            log::error!(target: $component.as_str(), $($arg)*)
        }
    };
}

#[macro_export]
macro_rules! log_debug {
    ($component:expr_2021, $($arg:tt)*) => {
        {
            $crate::core::log::init();
            log::debug!(target: $component.as_str(), $($arg)*)
        }
    };
}

/// Удобные макросы для каждого компонента
#[macro_export]
macro_rules! log_load {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Load, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_infer {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Infer, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_hub {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Hub, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_local {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Local, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_template {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Template, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_device {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Device, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_validate {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Validate, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_weights {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Weights, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_generate {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Generate, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_tokenizer {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Tokenizer, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_arch {
    ($($arg:tt)*) => {
        $crate::log_info!($crate::core::log::Component::Architecture, $($arg)*)
    };
}

/// Макросы для ошибок с компонентами
#[macro_export]
macro_rules! log_load_error {
    ($($arg:tt)*) => {
        $crate::log_error!($crate::core::log::Component::Load, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_hub_error {
    ($($arg:tt)*) => {
        $crate::log_error!($crate::core::log::Component::Hub, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_local_error {
    ($($arg:tt)*) => {
        $crate::log_error!($crate::core::log::Component::Local, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_template_error {
    ($($arg:tt)*) => {
        $crate::log_error!($crate::core::log::Component::Template, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_device_error {
    ($($arg:tt)*) => {
        $crate::log_error!($crate::core::log::Component::Device, $($arg)*)
    };
}

/// Макросы для предупреждений с компонентами
#[macro_export]
macro_rules! log_load_warn {
    ($($arg:tt)*) => {
        $crate::log_warn!($crate::core::log::Component::Load, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_hub_warn {
    ($($arg:tt)*) => {
        $crate::log_warn!($crate::core::log::Component::Hub, $($arg)*)
    };
}

#[macro_export]
macro_rules! log_local_warn {
    ($($arg:tt)*) => {
        $crate::log_warn!($crate::core::log::Component::Local, $($arg)*)
    };
}

/// Функция для логирования производительности
pub fn log_performance(component: Component, operation: &str, duration: std::time::Duration) {
    log_info!(
        component,
        "{} completed in {:.3}s",
        operation,
        duration.as_secs_f64()
    );
}

/// Функция для логирования размера данных
pub fn log_data_size(component: Component, data_type: &str, size_bytes: usize) {
    let size_mb = size_bytes as f64 / 1_048_576.0;
    if size_mb >= 1.0 {
        log_info!(component, "{} size: {:.2} MB", data_type, size_mb);
    } else {
        let size_kb = size_bytes as f64 / 1024.0;
        log_info!(component, "{} size: {:.2} KB", data_type, size_kb);
    }
}

/// Функция для логирования прогресса
pub fn log_progress(component: Component, current: usize, total: usize, operation: &str) {
    let percentage = (current as f64 / total as f64) * 100.0;
    log_info!(
        component,
        "{}: {}/{} ({:.1}%)",
        operation,
        current,
        total,
        percentage
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_as_str() {
        assert_eq!(Component::Load.as_str(), "load");
        assert_eq!(Component::Infer.as_str(), "infer");
        assert_eq!(Component::Hub.as_str(), "hub");
        assert_eq!(Component::Local.as_str(), "local");
        assert_eq!(Component::Template.as_str(), "template");
        assert_eq!(Component::Device.as_str(), "device");
        assert_eq!(Component::Validate.as_str(), "validate");
        assert_eq!(Component::Weights.as_str(), "weights");
        assert_eq!(Component::Generate.as_str(), "generate");
        assert_eq!(Component::Tokenizer.as_str(), "tokenizer");
        assert_eq!(Component::Architecture.as_str(), "arch");
    }

    #[test]
    fn test_log_macros() {
        // Тестируем, что макросы компилируются без ошибок
        // В реальном тесте мы бы проверили вывод, но здесь достаточно компиляции
        log_load!("Test load message");
        log_infer!("Test infer message");
        log_hub!("Test hub message");
        log_local!("Test local message");
        log_template!("Test template message");
        log_device!("Test device message");
        log_validate!("Test validate message");
        log_weights!("Test weights message");
        log_generate!("Test generate message");
        log_tokenizer!("Test tokenizer message");
        log_arch!("Test architecture message");
    }
}
