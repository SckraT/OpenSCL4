//! The system title bar. Windows draws it, so it only needs to know whether the app is dark.

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
