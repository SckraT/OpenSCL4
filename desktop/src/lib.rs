//! OpenSCQ30 for Windows. The window is described in ui/*.slint; `model` works out what it shows from the device.

pub mod model;
mod window_chrome;

use slint::ComponentHandle;

slint::include_modules!();

/// Opens the main window and runs until it is closed.
pub fn run() -> anyhow::Result<()> {
    let window = AppWindow::new()?;
    window.show()?;
    window_chrome::set_dark_title_bar(window.window(), window.global::<Theme>().get_dark());
    slint::run_event_loop()?;
    Ok(())
}
