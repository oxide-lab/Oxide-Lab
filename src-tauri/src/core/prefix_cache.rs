//! Prefix Cache для переиспользования KV-кэшей в multi-turn диалогах
//!
//! Кэширует hash токенов промпта и соответствующую позицию KV-кэша.
//! При совпадении промпта позволяет пропустить prefill и начать генерацию
//! с сохранённой позиции.
//!
//! ## Архитектура (MVP)
//!
//! - **Без блоков**: кэшируем весь промпт целиком (не по блокам как в PagedAttention)
//! - **LRU eviction**: по числу записей (не по памяти)
//! - **In-memory only**: без персистентности между сессиями
//!
//! ## Пример использования
//!
//! ```ignore
//! let config = PrefixCacheConfig::enabled(32); // max 32 записей
//! let mut cache = PrefixCache::new(config);
//!
//! // Первый запрос — miss
//! let tokens = vec![1, 2, 3, 4, 5];
//! assert!(cache.match_prefix(&tokens).is_none());
//!
//! // Сохраняем после генерации
//! cache.insert(&tokens, 5); // kv_position = 5
//!
//! // Второй запрос — hit
//! let match_result = cache.match_prefix(&tokens);
//! assert!(match_result.is_some());
//! assert_eq!(match_result.unwrap().kv_position, 5);
//! ```

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

/// Конфигурация Prefix Cache
#[derive(Clone, Debug)]
pub struct PrefixCacheConfig {
    /// Включён ли кэш
    pub enabled: bool,
    /// Максимальное число записей в кэше
    pub max_entries: usize,
}

impl Default for PrefixCacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_entries: 32,
        }
    }
}

impl PrefixCacheConfig {
    /// Создаёт конфигурацию с включённым кэшем
    pub fn enabled(max_entries: usize) -> Self {
        Self {
            enabled: true,
            max_entries,
        }
    }

    /// Создаёт отключённый кэш
    pub fn disabled() -> Self {
        Self::default()
    }
}

/// Результат успешного match в кэше
#[derive(Clone, Debug, PartialEq)]
pub struct PrefixMatch {
    /// Позиция KV-кэша, до которой можно пропустить prefill
    pub kv_position: usize,
    /// Количество токенов, которые совпали
    pub matched_tokens: usize,
    /// Hash промпта (для отладки)
    pub tokens_hash: u64,
}

/// Статистика кэша
#[derive(Clone, Debug, Default)]
pub struct PrefixCacheStats {
    /// Количество попаданий
    pub hits: u64,
    /// Количество промахов
    pub misses: u64,
    /// Количество вытеснений
    pub evictions: u64,
    /// Текущее число записей
    pub entries: usize,
}

/// Внутренняя запись кэша
#[derive(Clone, Debug)]
struct CacheEntry {
    /// Позиция KV-кэша
    kv_position: usize,
    /// Количество токенов
    token_count: usize,
    /// Время последнего доступа (для LRU)
    last_access: Instant,
}

/// Prefix Cache для переиспользования KV-кэшей
pub struct PrefixCache {
    config: PrefixCacheConfig,
    /// Записи: hash токенов -> entry
    entries: HashMap<u64, CacheEntry>,
    /// Статистика
    stats: PrefixCacheStats,
}

impl PrefixCache {
    /// Создаёт новый кэш с заданной конфигурацией
    pub fn new(config: PrefixCacheConfig) -> Self {
        Self {
            config,
            entries: HashMap::new(),
            stats: PrefixCacheStats::default(),
        }
    }

    /// Проверяет, включён ли кэш
    pub fn enabled(&self) -> bool {
        self.config.enabled && self.config.max_entries > 0
    }

    /// Возвращает текущую статистику кэша
    pub fn stats(&self) -> PrefixCacheStats {
        let mut stats = self.stats.clone();
        stats.entries = self.entries.len();
        stats
    }

    /// Ищет совпадение в кэше для заданных токенов
    ///
    /// Возвращает `Some(PrefixMatch)` если найдено точное совпадение,
    /// иначе `None`.
    pub fn match_prefix(&mut self, tokens: &[u32]) -> Option<PrefixMatch> {
        if !self.enabled() || tokens.is_empty() {
            self.stats.misses += 1;
            return None;
        }

        let hash = Self::hash_tokens(tokens);

        if let Some(entry) = self.entries.get_mut(&hash) {
            // Проверяем что количество токенов совпадает
            // (защита от hash-коллизий)
            if entry.token_count == tokens.len() {
                // Обновляем время доступа для LRU
                entry.last_access = Instant::now();
                self.stats.hits += 1;

                return Some(PrefixMatch {
                    kv_position: entry.kv_position,
                    matched_tokens: entry.token_count,
                    tokens_hash: hash,
                });
            }
        }

        self.stats.misses += 1;
        None
    }

    /// Добавляет запись в кэш
    ///
    /// # Arguments
    /// * `tokens` - токены промпта
    /// * `kv_position` - позиция KV-кэша после prefill этих токенов
    pub fn insert(&mut self, tokens: &[u32], kv_position: usize) {
        if !self.enabled() || tokens.is_empty() {
            return;
        }

        // Сначала проверяем, нужно ли вытеснять
        self.evict_if_needed();

        let hash = Self::hash_tokens(tokens);
        let entry = CacheEntry {
            kv_position,
            token_count: tokens.len(),
            last_access: Instant::now(),
        };

        self.entries.insert(hash, entry);
    }

    /// Очищает весь кэш
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Вытесняет записи по LRU если кэш переполнен
    fn evict_if_needed(&mut self) {
        while self.entries.len() >= self.config.max_entries {
            // Находим самую старую запись
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(hash, _)| *hash);

            if let Some(hash) = oldest {
                self.entries.remove(&hash);
                self.stats.evictions += 1;
            } else {
                break;
            }
        }
    }

    /// Вычисляет hash для последовательности токенов
    fn hash_tokens(tokens: &[u32]) -> u64 {
        let mut hasher = DefaultHasher::new();
        tokens.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_cache_disabled_by_default() {
        let cache = PrefixCache::new(PrefixCacheConfig::default());
        assert!(!cache.enabled());
    }

    #[test]
    fn test_prefix_cache_enabled() {
        let cache = PrefixCache::new(PrefixCacheConfig::enabled(32));
        assert!(cache.enabled());
    }

    #[test]
    fn test_prefix_cache_exact_match() {
        let mut cache = PrefixCache::new(PrefixCacheConfig::enabled(32));

        let tokens = vec![1u32, 2, 3, 4, 5];

        // Первый запрос — miss
        assert!(cache.match_prefix(&tokens).is_none());

        // Вставляем
        cache.insert(&tokens, 5);

        // Второй запрос — hit
        let result = cache.match_prefix(&tokens);
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.kv_position, 5);
        assert_eq!(m.matched_tokens, 5);
    }

    #[test]
    fn test_prefix_cache_no_match_different_tokens() {
        let mut cache = PrefixCache::new(PrefixCacheConfig::enabled(32));

        let tokens1 = vec![1u32, 2, 3, 4, 5];
        let tokens2 = vec![6u32, 7, 8, 9, 10];

        cache.insert(&tokens1, 5);

        // Другие токены — miss
        assert!(cache.match_prefix(&tokens2).is_none());
    }

    #[test]
    fn test_prefix_cache_lru_eviction() {
        let mut cache = PrefixCache::new(PrefixCacheConfig::enabled(2));

        let tokens1 = vec![1u32, 2, 3];
        let tokens2 = vec![4u32, 5, 6];
        let tokens3 = vec![7u32, 8, 9];

        // Вставляем две записи
        cache.insert(&tokens1, 3);
        cache.insert(&tokens2, 6);

        // Обе должны быть доступны
        assert!(cache.match_prefix(&tokens1).is_some());
        assert!(cache.match_prefix(&tokens2).is_some());

        // Вставляем третью — должна вытеснить одну из первых двух
        cache.insert(&tokens3, 9);

        // tokens3 точно должен быть доступен
        assert!(cache.match_prefix(&tokens3).is_some());

        // Одна из первых двух должна быть вытеснена
        let count = [&tokens1, &tokens2]
            .iter()
            .filter(|t| cache.match_prefix(t).is_some())
            .count();
        assert_eq!(count, 1, "One of the first two entries should be evicted");
    }

    #[test]
    fn test_prefix_cache_stats() {
        let mut cache = PrefixCache::new(PrefixCacheConfig::enabled(32));

        let tokens = vec![1u32, 2, 3];

        // Miss
        cache.match_prefix(&tokens);
        assert_eq!(cache.stats().misses, 1);
        assert_eq!(cache.stats().hits, 0);

        // Insert
        cache.insert(&tokens, 3);

        // Hit
        cache.match_prefix(&tokens);
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().entries, 1);
    }

    #[test]
    fn test_prefix_cache_clear() {
        let mut cache = PrefixCache::new(PrefixCacheConfig::enabled(32));

        let tokens = vec![1u32, 2, 3];
        cache.insert(&tokens, 3);
        assert!(cache.match_prefix(&tokens).is_some());

        cache.clear();
        assert!(cache.match_prefix(&tokens).is_none());
    }

    #[test]
    fn test_prefix_cache_empty_tokens() {
        let mut cache = PrefixCache::new(PrefixCacheConfig::enabled(32));

        let empty: Vec<u32> = vec![];

        // Empty tokens should not match
        assert!(cache.match_prefix(&empty).is_none());

        // Insert with empty should be a no-op
        cache.insert(&empty, 0);
        assert_eq!(cache.stats().entries, 0);
    }
}
