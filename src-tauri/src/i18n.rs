/*!
 * Backend i18n Module for Oxide Lab
 *
 * Простой модуль локализации для Rust backend.
 * Использует простой формат хранения переводов в памяти.
 */
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// Поддерживаемые локали
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Locale {
    #[default]
    En,
    Ru,
    PtBr,
}

impl Locale {
    /// Получить строковое представление локали
    pub fn as_str(&self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ru => "ru",
            Locale::PtBr => "pt-BR",
        }
    }

    fn from_code(code: u8) -> Self {
        match code {
            x if x == Locale::Ru as u8 => Locale::Ru,
            x if x == Locale::PtBr as u8 => Locale::PtBr,
            _ => Locale::En,
        }
    }
}

impl FromStr for Locale {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "ru" | "ru-RU" => Ok(Locale::Ru),
            "pt" | "pt-BR" | "pt-PT" => Ok(Locale::PtBr),
            _ => Ok(Locale::En),
        }
    }
}

/// Глобальное хранилище переводов (immutable after init)
static TRANSLATIONS: LazyLock<HashMap<Locale, HashMap<String, String>>> = LazyLock::new(|| {
    let mut translations = HashMap::new();

    // English translations
    let mut en = HashMap::new();
    en.insert(
        "error.model.load_failed".to_string(),
        "Failed to load model".to_string(),
    );
    en.insert(
        "error.model.unload_failed".to_string(),
        "Failed to unload model".to_string(),
    );
    en.insert(
        "error.model.not_loaded".to_string(),
        "Model is not loaded".to_string(),
    );
    en.insert(
        "error.settings.load_failed".to_string(),
        "Failed to load settings".to_string(),
    );
    en.insert(
        "error.settings.save_failed".to_string(),
        "Failed to save settings".to_string(),
    );
    translations.insert(Locale::En, en);

    // Russian translations
    let mut ru = HashMap::new();
    ru.insert(
        "error.model.load_failed".to_string(),
        "Не удалось загрузить модель".to_string(),
    );
    ru.insert(
        "error.model.unload_failed".to_string(),
        "Не удалось выгрузить модель".to_string(),
    );
    ru.insert(
        "error.model.not_loaded".to_string(),
        "Модель не загружена".to_string(),
    );
    ru.insert(
        "error.settings.load_failed".to_string(),
        "Не удалось загрузить настройки".to_string(),
    );
    ru.insert(
        "error.settings.save_failed".to_string(),
        "Не удалось сохранить настройки".to_string(),
    );
    translations.insert(Locale::Ru, ru);

    // Portuguese translations
    let mut pt_br = HashMap::new();
    pt_br.insert(
        "error.model.load_failed".to_string(),
        "Falha ao carregar modelo".to_string(),
    );
    pt_br.insert(
        "error.model.unload_failed".to_string(),
        "Falha ao descarregar modelo".to_string(),
    );
    pt_br.insert(
        "error.model.not_loaded".to_string(),
        "Modelo não está carregado".to_string(),
    );
    pt_br.insert(
        "error.settings.load_failed".to_string(),
        "Falha ao carregar configurações".to_string(),
    );
    pt_br.insert(
        "error.settings.save_failed".to_string(),
        "Falha ao salvar configurações".to_string(),
    );
    translations.insert(Locale::PtBr, pt_br);

    translations
});
static CURRENT_LOCALE: AtomicU8 = AtomicU8::new(Locale::En as u8);

/// Инициализация переводов
pub fn init() {
    // Force lazy initialization and reset locale to default for deterministic startup.
    let _ = &*TRANSLATIONS;
    CURRENT_LOCALE.store(Locale::default() as u8, Ordering::SeqCst);
}

/// Установить текущую локаль
pub fn set_locale(locale: Locale) {
    CURRENT_LOCALE.store(locale as u8, Ordering::SeqCst);
}

/// Получить текущую локаль
pub fn get_locale() -> Locale {
    Locale::from_code(CURRENT_LOCALE.load(Ordering::SeqCst))
}

/// Получить перевод по ключу
pub fn t(key: &str) -> String {
    let current_locale = get_locale();
    if let Some(locale_translations) = TRANSLATIONS.get(&current_locale)
        && let Some(translation) = locale_translations.get(key)
    {
        return translation.clone();
    }
    // Fallback to English
    if let Some(en_translations) = TRANSLATIONS.get(&Locale::En)
        && let Some(translation) = en_translations.get(key)
    {
        return translation.clone();
    }
    // Return key if translation not found
    key.to_string()
}

/// Макрос для удобного использования переводов
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::t($key)
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_roundtrip_is_thread_safe_storage() {
        init();
        set_locale(Locale::Ru);
        assert_eq!(get_locale(), Locale::Ru);
        set_locale(Locale::En);
        assert_eq!(get_locale(), Locale::En);
    }

    #[test]
    fn translation_fallback_works() {
        init();
        set_locale(Locale::Ru);
        assert_eq!(t("error.model.load_failed"), "Не удалось загрузить модель");
        assert_eq!(t("missing.key"), "missing.key");
        set_locale(Locale::PtBr);
        assert_eq!(t("error.settings.save_failed"), "Falha ao salvar configurações");
    }
}
