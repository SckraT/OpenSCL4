//! The system title bar and its theme. The system draws the title bar, so it has to be told whether the app is dark;
//! on Linux it is also kept so, because Slint gives the window the desktop's theme.

use slint::winit_030::{
    WinitWindowAccessor,
    winit::window::{Theme, WindowId},
};

use crate::config::AppTheme;

/// Whether Windows is set to dark mode for apps, which is what "As in Windows" follows. Linux has nothing to ask on
/// the spot: the app follows `linux_appearance`, and is light until it reports.
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
    {
        false
    }
}

/// Picks the winit backend. On Linux `keep_title_bar` runs after each round of events, before the event loop waits.
pub fn init(keep_title_bar: fn()) -> anyhow::Result<()> {
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
    let backend = backend.with_winit_custom_application_handler(KeepTitleBar(keep_title_bar));
    // Windows keeps the theme the app gives its title bar
    #[cfg(not(target_os = "linux"))]
    let _ = keep_title_bar;
    backend.select()?;
    Ok(())
}

/// The title bar's theme for the app's, or None to follow the system. Only Windows is left to follow it: winit on X11
/// makes "follow" dark, so Linux gets the app's own, which for "As in the system" is the desktop's.
pub fn title_bar_theme(theme: AppTheme, dark: bool) -> Option<Theme> {
    match theme {
        AppTheme::System if cfg!(windows) => None,
        _ => Some(if dark { Theme::Dark } else { Theme::Light }),
    }
}

/// The app's theme on the system title bar. winit draws the title bar to the system's theme by itself, so it is the
/// one to tell; Slint tells it the desktop's when it makes the window and, on Linux, whenever the desktop's changes.
#[derive(Default)]
pub struct TitleBar {
    theme: Option<Theme>,
    /// "Light" or "Dark": a different theme on the window is Slint's, and goes.
    keep: bool,
    /// The winit window that was given `theme`.
    on: Option<WindowId>,
}

impl TitleBar {
    /// Gives the title bar the app's theme, or leaves it to `keep` until the winit window exists.
    pub fn set(&mut self, window: &slint::Window, theme: AppTheme, dark: bool) {
        self.theme = title_bar_theme(theme, dark);
        self.keep = theme != AppTheme::System;
        let wanted = self.theme;
        self.on = window.with_winit_window(|window| {
            window.set_theme(wanted);
            window.id()
        });
    }

    /// Gives a new winit window the app's theme, and puts a chosen one back where Slint changed it.
    pub fn keep(&mut self, window: &slint::Window) {
        let wanted = self.theme;
        let Some((id, current)) = window.with_winit_window(|window| (window.id(), window.theme()))
        else {
            return;
        };
        if self.needs_theme(id, current) {
            window.with_winit_window(|window| window.set_theme(wanted));
            self.on = Some(id);
        }
    }

    /// Whether the winit window `id`, which says it has `current`, needs the app's theme. Only Wayland says which
    /// theme the window has; X11 always says None, and gets the app's again after the desktop's changes.
    fn needs_theme(&self, id: WindowId, current: Option<Theme>) -> bool {
        self.on != Some(id) || (self.keep && current.is_some() && current != self.theme)
    }
}

/// Calls the app's `keep_title_bar` after each round of events, so after Slint's theme.
#[cfg(target_os = "linux")]
struct KeepTitleBar(fn());

#[cfg(target_os = "linux")]
impl slint::winit_030::CustomApplicationHandler for KeepTitleBar {
    fn about_to_wait(
        &mut self,
        _: &slint::winit_030::winit::event_loop::ActiveEventLoop,
    ) -> slint::winit_030::EventResult {
        (self.0)();
        slint::winit_030::EventResult::Propagate
    }
}

#[cfg(test)]
mod tests {
    use slint::winit_030::winit::window::{Theme, WindowId};

    use super::{TitleBar, title_bar_theme};
    use crate::config::AppTheme;

    /// The title bar after the app gave `theme` to the winit window `on`.
    fn title_bar(theme: AppTheme, dark: bool, on: u64) -> TitleBar {
        TitleBar {
            theme: title_bar_theme(theme, dark),
            keep: theme != AppTheme::System,
            on: Some(WindowId::from(on)),
        }
    }

    #[test]
    fn a_chosen_theme_is_the_title_bar_theme() {
        assert_eq!(title_bar_theme(AppTheme::Light, false), Some(Theme::Light));
        assert_eq!(title_bar_theme(AppTheme::Dark, true), Some(Theme::Dark));
    }

    #[test]
    fn only_windows_leaves_the_title_bar_to_the_system() {
        // winit on X11 makes "follow the system" dark
        let expected = if cfg!(windows) {
            None
        } else {
            Some(Theme::Dark)
        };
        assert_eq!(title_bar_theme(AppTheme::System, true), expected);
    }

    #[test]
    fn a_new_window_gets_the_app_theme() {
        // Slint makes the window after the app has chosen, and gives it the desktop's theme
        assert!(TitleBar::default().needs_theme(WindowId::from(1), Some(Theme::Light)));
        let light = title_bar(AppTheme::Light, false, 1);
        assert!(light.needs_theme(WindowId::from(2), Some(Theme::Light)));
        let system = title_bar(AppTheme::System, true, 1);
        assert!(system.needs_theme(WindowId::from(2), None));
    }

    #[test]
    fn a_chosen_theme_is_put_back() {
        let light = title_bar(AppTheme::Light, false, 1);
        assert!(light.needs_theme(WindowId::from(1), Some(Theme::Dark)));
        assert!(!light.needs_theme(WindowId::from(1), Some(Theme::Light)));
        // X11 never says which theme the window has
        assert!(!light.needs_theme(WindowId::from(1), None));
    }

    #[test]
    fn the_desktop_theme_is_left_to_slint() {
        let system = title_bar(AppTheme::System, false, 1);
        assert!(!system.needs_theme(WindowId::from(1), Some(Theme::Dark)));
    }
}
