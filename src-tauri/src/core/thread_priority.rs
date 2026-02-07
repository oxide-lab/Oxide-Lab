#[cfg(target_os = "windows")]
mod windows {
    use windows_sys::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_ABOVE_NORMAL,
        THREAD_PRIORITY_BELOW_NORMAL, THREAD_PRIORITY_NORMAL,
    };

    pub fn set_below_normal() -> bool {
        unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) != 0 }
    }

    pub fn set_above_normal() -> bool {
        unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_ABOVE_NORMAL) != 0 }
    }

    pub fn set_normal() -> bool {
        unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_NORMAL) != 0 }
    }
}

/// Sets the current thread priority to below normal (Windows only).
pub fn set_current_thread_below_normal() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::set_below_normal()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Sets the current thread priority to above normal (Windows only).
pub fn set_current_thread_above_normal() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::set_above_normal()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// Sets the current thread priority back to normal (Windows only).
pub fn set_current_thread_normal() -> bool {
    #[cfg(target_os = "windows")]
    {
        windows::set_normal()
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

/// RAII guard that lowers the current thread priority while alive (Windows only).
pub struct ThreadPriorityGuard {
    #[cfg(target_os = "windows")]
    active: bool,
}

impl ThreadPriorityGuard {
    pub fn below_normal() -> Self {
        #[cfg(target_os = "windows")]
        {
            let active = set_current_thread_below_normal();
            Self { active }
        }
        #[cfg(not(target_os = "windows"))]
        {
            Self {}
        }
    }
}

impl Drop for ThreadPriorityGuard {
    fn drop(&mut self) {
        #[cfg(target_os = "windows")]
        if self.active {
            let _ = set_current_thread_normal();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_create_and_drop_guard() {
        let _guard = ThreadPriorityGuard::below_normal();
    }
}
