/*!
 * Locale Commands for Tauri
 *
 * Команды для управления локалью из frontend.
 */
use crate::core::settings_v2::SettingsV2State;
use crate::i18n::{self, Locale};
use tauri::Manager;

/// Получить текущую локаль
#[tauri::command]
pub fn get_locale() -> String {
    i18n::get_locale().as_str().to_string()
}

/// Установить локаль
#[tauri::command]
pub fn set_locale(app: tauri::AppHandle, locale: String) -> Result<(), String> {
    let locale_enum: Locale = locale.parse().map_err(|_| "Invalid locale".to_string())?;
    i18n::set_locale(locale_enum);
    if let Some(settings_state) = app.try_state::<SettingsV2State>() {
        let mut guard = settings_state.inner.lock().map_err(|e| e.to_string())?;
        let mut settings = guard.get();
        settings.general.locale = locale;
        guard.set(settings)?;
    }
    Ok(())
}
