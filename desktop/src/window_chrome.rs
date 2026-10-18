//! The system title bar and theme. Windows draws the title bar, so it only needs to know whether the app is dark.

/// Whether Windows is set to dark mode for apps, which is what "As in Windows" follows.
pub fn system_prefers_dark() -> bool {
    #[cfg(windows)]
    {
        use windows::{
            Win32::{
                Foundation::ERROR_SUCCESS,
                System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW},
            },
            core::w,
        };
        let mut light: u32 = 1;
        let mut size = size_of::<u32>() as u32;
        // SAFETY: the value is a DWORD and the buffer is one.
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("AppsUseLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some((&raw mut light).cast()),
                Some(&raw mut size),
            )
        };
        result == ERROR_SUCCESS && light == 0
    }
    #[cfg(not(windows))]
    false
}

/// Makes the Windows title bar dark or light to match the app. Does nothing elsewhere, or before the window exists.
pub fn set_dark_title_bar(window: &slint::Window, dark: bool) {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        use windows::Win32::{
            Foundation::HWND,
            Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute},
        };

        let handle = window.window_handle();
        let Ok(handle) = handle.window_handle() else {
            return;
        };
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            return;
        };
        let hwnd = HWND(handle.hwnd.get() as *mut core::ffi::c_void);
        let value = windows::core::BOOL::from(dark);
        // SAFETY: the window handle is alive while the Slint window is, and the attribute takes a BOOL.
        let result = unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_USE_IMMERSIVE_DARK_MODE,
                (&raw const value).cast(),
                size_of_val(&value) as u32,
            )
        };
        if let Err(err) = result {
            tracing::warn!("could not change the title bar: {err:?}");
        }
    }
    #[cfg(not(windows))]
    let _ = (window, dark);
}
