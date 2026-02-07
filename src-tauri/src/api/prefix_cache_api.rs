//! Tauri API для управления Prefix Cache

use crate::core::prefix_cache::{PrefixCacheConfig, PrefixCacheStats};
use crate::core::state::SharedState;
use serde::{Deserialize, Serialize};

/// Ответ со статистикой и конфигурацией Prefix Cache
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixCacheInfo {
    /// Включён ли кэш
    pub enabled: bool,
    /// Максимальное число записей
    pub max_entries: usize,
    /// Текущая статистика
    pub stats: PrefixCacheStatsDto,
}

/// DTO для статистики (сериализуемая версия)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixCacheStatsDto {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
}

impl From<PrefixCacheStats> for PrefixCacheStatsDto {
    fn from(s: PrefixCacheStats) -> Self {
        Self {
            hits: s.hits,
            misses: s.misses,
            evictions: s.evictions,
            entries: s.entries,
        }
    }
}

/// Получить информацию о Prefix Cache
#[tauri::command]
pub fn get_prefix_cache_info(
    state: tauri::State<'_, SharedState>,
) -> Result<PrefixCacheInfo, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    let enabled = guard.prefix_cache.enabled();
    let stats = guard.prefix_cache.stats();

    Ok(PrefixCacheInfo {
        enabled,
        max_entries: if enabled { 32 } else { 0 }, // TODO: expose from config
        stats: stats.into(),
    })
}

/// Включить/выключить Prefix Cache
#[tauri::command]
pub fn set_prefix_cache_enabled(
    state: tauri::State<'_, SharedState>,
    enabled: bool,
    max_entries: Option<usize>,
) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;

    let config = if enabled {
        PrefixCacheConfig::enabled(max_entries.unwrap_or(32))
    } else {
        PrefixCacheConfig::disabled()
    };

    guard.prefix_cache = crate::core::prefix_cache::PrefixCache::new(config);
    Ok(())
}

/// Очистить Prefix Cache
#[tauri::command]
pub fn clear_prefix_cache(state: tauri::State<'_, SharedState>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.prefix_cache.clear();
    Ok(())
}
