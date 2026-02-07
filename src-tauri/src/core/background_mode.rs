#[cfg(target_os = "windows")]
mod windows {
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, PROCESS_MODE_BACKGROUND_BEGIN, PROCESS_MODE_BACKGROUND_END,
        SetPriorityClass,
    };

    pub fn begin() -> bool {
        unsafe { SetPriorityClass(GetCurrentProcess(), PROCESS_MODE_BACKGROUND_BEGIN) != 0 }
    }

    pub fn end() -> bool {
        unsafe { SetPriorityClass(GetCurrentProcess(), PROCESS_MODE_BACKGROUND_END) != 0 }
    }
}

/// RAII guard that switches the process into Windows "background mode" while alive.
///
/// This reduces scheduling and IO priority to help keep the UI thread responsive under heavy load.
pub struct BackgroundModeGuard {
    #[cfg(target_os = "windows")]
    active: bool,
}

impl BackgroundModeGuard {
    pub fn new() -> Self {
        #[cfg(target_os = "windows")]
        {
            let active = windows::begin();
            Self { active }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self {}
        }
    }
}

impl Default for BackgroundModeGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BackgroundModeGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if self.active {
            let _ = windows::end();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_create_and_drop_guard() {
        let _guard = BackgroundModeGuard::new();
    }
}
