/*!
 * Locale Commands for Tauri
 *
 * Команды для управления локалью из frontend.
 */
use crate::i18n::{self, Locale};

/// Получить текущую локаль
#[tauri::command]
pub fn get_locale() -> String {
    i18n::get_locale().as_str().to_string()
}

/// Установить локаль
#[tauri::command]
pub fn set_locale(locale: String) -> Result<(), String> {
    let locale_enum: Locale = locale.parse().map_err(|_| "Invalid locale".to_string())?;
    i18n::set_locale(locale_enum);
    Ok(())
}

