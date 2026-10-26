//! The system's own Bluetooth and sound settings. Windows opens each by its settings URI. On Linux the desktop's
//! settings app takes the page as an argument; GNOME and KDE, the desktops of the Linux release, are known. Other
//! desktops and systems have no page to open, and the window leaves out the buttons for them.

use std::io;

use crate::SystemPage;

/// Whether the window can offer the settings pages.
#[cfg(target_os = "linux")]
pub fn available() -> bool {
    static FOUND: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *FOUND.get_or_init(|| linux_command(SystemPage::Bluetooth).is_some())
}

/// Whether the window can offer the settings pages.
#[cfg(not(target_os = "linux"))]
pub fn available() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
pub fn open(page: SystemPage) -> io::Result<()> {
    open::that_detached(match page {
        SystemPage::Bluetooth => "ms-settings:bluetooth",
        SystemPage::Sound => "ms-settings:sound",
    })
}

#[cfg(target_os = "linux")]
pub fn open(page: SystemPage) -> io::Result<()> {
    let (program, argument) = linux_command(page).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "no settings app for this desktop")
    })?;
    open::with_detached(argument, program)
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn open(_page: SystemPage) -> io::Result<()> {
    Err(io::ErrorKind::Unsupported.into())
}

#[cfg(target_os = "linux")]
fn linux_command(page: SystemPage) -> Option<(&'static str, &'static str)> {
    let desktops = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    command(&desktops, page, |program| {
        std::env::var_os("PATH").is_some_and(|paths| {
            std::env::split_paths(&paths).any(|dir| dir.join(program).is_file())
        })
    })
}

/// The settings app and its argument for the page, from the first of the desktops (as in XDG_CURRENT_DESKTOP, such as
/// "ubuntu:GNOME") that is known and whose app exists.
#[cfg(any(target_os = "linux", test))]
fn command(
    desktops: &str,
    page: SystemPage,
    exists: impl Fn(&str) -> bool,
) -> Option<(&'static str, &'static str)> {
    desktops.split(':').find_map(|desktop| {
        let command = match (desktop, page) {
            ("GNOME", SystemPage::Bluetooth) => ("gnome-control-center", "bluetooth"),
            ("GNOME", SystemPage::Sound) => ("gnome-control-center", "sound"),
            ("KDE", SystemPage::Bluetooth) => ("systemsettings", "kcm_bluetooth"),
            ("KDE", SystemPage::Sound) => ("systemsettings", "kcm_pulseaudio"),
            _ => return None,
        };
        exists(command.0).then_some(command)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_and_kde_open_their_own_pages() {
        let installed = |_: &str| true;
        assert_eq!(
            Some(("gnome-control-center", "bluetooth")),
            command("ubuntu:GNOME", SystemPage::Bluetooth, installed)
        );
        assert_eq!(
            Some(("gnome-control-center", "sound")),
            command("ubuntu:GNOME", SystemPage::Sound, installed)
        );
        assert_eq!(
            Some(("systemsettings", "kcm_bluetooth")),
            command("KDE", SystemPage::Bluetooth, installed)
        );
        assert_eq!(
            Some(("systemsettings", "kcm_pulseaudio")),
            command("KDE", SystemPage::Sound, installed)
        );
    }

    #[test]
    fn other_desktops_and_missing_apps_have_no_page() {
        assert_eq!(None, command("XFCE", SystemPage::Bluetooth, |_| true));
        assert_eq!(None, command("", SystemPage::Sound, |_| true));
        let without_gnome_settings = |program: &str| program != "gnome-control-center";
        assert_eq!(
            None,
            command("GNOME", SystemPage::Sound, without_gnome_settings)
        );
    }
}
