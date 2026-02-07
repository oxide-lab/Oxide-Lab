/*!
 * Backend i18n Module for Oxide Lab
 *
 * Простой модуль локализации для Rust backend.
 * Использует простой формат хранения переводов в памяти.
 */
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;

/// Поддерживаемые локали
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

/// Глобальное хранилище переводов
static mut TRANSLATIONS: Option<HashMap<Locale, HashMap<String, String>>> = None;
static mut CURRENT_LOCALE: Locale = Locale::En;

/// Инициализация переводов
pub fn init() {
    unsafe {
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

        TRANSLATIONS = Some(translations);
        CURRENT_LOCALE = Locale::default();
    }
}

/// Установить текущую локаль
pub fn set_locale(locale: Locale) {
    unsafe {
        CURRENT_LOCALE = locale;
    }
}

/// Получить текущую локаль
pub fn get_locale() -> Locale {
    unsafe { CURRENT_LOCALE }
}

/// Получить перевод по ключу
pub fn t(key: &str) -> String {
    unsafe {
        let translations_ptr = std::ptr::addr_of!(TRANSLATIONS);
        let current_locale_ptr = std::ptr::addr_of!(CURRENT_LOCALE);
        if let Some(translations) = (*translations_ptr).as_ref() {
            if let Some(locale_translations) = translations.get(&*current_locale_ptr)
                && let Some(translation) = locale_translations.get(key)
            {
                return translation.clone();
            }
            // Fallback to English
            if let Some(en_translations) = translations.get(&Locale::En)
                && let Some(translation) = en_translations.get(key)
            {
                return translation.clone();
            }
        }
        // Return key if translation not found
        key.to_string()
    }
}

/// Макрос для удобного использования переводов
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::t($key)
    };
}
