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

/// Makes the system title bar dark or light to match the app, or follow the system with None. winit draws its
/// title bar to the system's theme by itself, so it is the one to tell. Does nothing before the window exists.
pub fn set_title_bar(window: &slint::Window, dark: Option<bool>) {
    use slint::winit_030::{WinitWindowAccessor, winit::window::Theme};
    window.with_winit_window(|window| {
        window.set_theme(dark.map(|dark| if dark { Theme::Dark } else { Theme::Light }));
    });
}
