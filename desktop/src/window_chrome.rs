//! The system title bar and theme. Windows draws the title bar, so it only needs to know whether the app is dark.

use slint::winit_030::winit::window::Theme;

use crate::config::AppTheme;

/// Whether Windows is set to dark mode for apps, which is what "As in Windows" follows.
pub fn system_prefers_dark(window: &slint::Window) -> bool {
    #[cfg(windows)]
    {
        let _ = window;
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
    // Slint gives the window the desktop's theme from the portal, so this holds only until the app sets one; the app
    // follows `linux_appearance` once it reports
    #[cfg(not(windows))]
    {
        use slint::winit_030::WinitWindowAccessor;
        let mut dark = false;
        window.with_winit_window(|window| dark = window.theme() == Some(Theme::Dark));
        dark
    }
}

pub fn init() -> anyhow::Result<()> {
    #[cfg(windows)]
    unsafe {
        windows::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(windows::core::w!(
            "io.github.sckrat.openscl4"
        ))?;
    }
    let backend = slint::BackendSelector::new().backend_name("winit".into());
    #[cfg(target_os = "linux")]
    let backend = backend.with_winit_window_attributes_hook(|attributes| {
        use slint::winit_030::winit::platform::{
            wayland::WindowAttributesExtWayland, x11::WindowAttributesExtX11,
        };
        let attributes = WindowAttributesExtWayland::with_name(
            attributes,
            "io.github.sckrat.openscl4",
            "OpenSCL4",
        );
        WindowAttributesExtX11::with_name(attributes, "io.github.sckrat.openscl4", "OpenSCL4")
    });
    #[cfg(target_os = "linux")]
    let backend = backend.with_winit_custom_application_handler(KeepTitleBar);
    backend.select()?;
    Ok(())
}

/// The title bar's theme for the app's: dark or light, or None to follow the system. Only Windows is left to follow
/// it: on Linux winit's "follow" is the theme Slint gave the window last, and on X11 it is always dark.
pub fn title_bar(theme: AppTheme, dark: bool) -> Option<bool> {
    match theme {
        AppTheme::System if cfg!(windows) => None,
        _ => Some(dark),
    }
}

/// Makes the system title bar dark or light to match the app, or follow the system with None. winit draws its
/// title bar to the system's theme by itself, so it is the one to tell. Does nothing before the window exists.
pub fn set_title_bar(window: &slint::Window, dark: Option<bool>) {
    use slint::winit_030::WinitWindowAccessor;
    window.with_winit_window(|window| {
        window.set_theme(dark.map(|dark| if dark { Theme::Dark } else { Theme::Light }));
    });
}

/// Whether something other than the app has given the window another theme. Only Wayland says which theme the
/// window has; X11 always says None.
#[cfg(any(target_os = "linux", test))]
fn overridden(current: Option<Theme>, wanted: Theme) -> bool {
    current.is_some_and(|current| current != wanted)
}

/// Puts the app's dark or light back on the title bar if it was changed: Slint gives every window the desktop's
/// theme whenever the desktop's changes, also while the app has its own.
#[cfg(target_os = "linux")]
pub fn keep_title_bar(window: &slint::Window, dark: bool) {
    use slint::winit_030::WinitWindowAccessor;
    let wanted = if dark { Theme::Dark } else { Theme::Light };
    window.with_winit_window(|window| {
        if overridden(window.theme(), wanted) {
            window.set_theme(Some(wanted));
        }
    });
}

/// Keeps the app's title bar after each round of events, so after Slint has given it the desktop's theme.
#[cfg(target_os = "linux")]
struct KeepTitleBar;

#[cfg(target_os = "linux")]
impl slint::winit_030::CustomApplicationHandler for KeepTitleBar {
    fn about_to_wait(
        &mut self,
        _: &slint::winit_030::winit::event_loop::ActiveEventLoop,
    ) -> slint::winit_030::EventResult {
        crate::app::keep_title_bar();
        slint::winit_030::EventResult::Propagate
    }
}

#[cfg(test)]
mod tests {
    use slint::winit_030::winit::window::Theme;

    use super::{overridden, title_bar};
    use crate::config::AppTheme;

    #[test]
    fn a_chosen_theme_is_the_title_bar_theme() {
        assert_eq!(title_bar(AppTheme::Light, false), Some(false));
        assert_eq!(title_bar(AppTheme::Dark, true), Some(true));
    }

    #[test]
    fn only_windows_leaves_the_title_bar_to_the_system() {
        // winit on X11 draws "follow the system" dark, and on Wayland it is whatever Slint last set
        let expected = if cfg!(windows) { None } else { Some(true) };
        assert_eq!(title_bar(AppTheme::System, true), expected);
    }

    #[test]
    fn another_theme_on_the_window_is_put_back() {
        assert!(overridden(Some(Theme::Dark), Theme::Light));
        assert!(!overridden(Some(Theme::Light), Theme::Light));
        // X11 never says which theme the window has
        assert!(!overridden(None, Theme::Light));
    }
}
