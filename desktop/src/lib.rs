//! OpenSCQ30 for Windows. The window is described in ui/*.slint; `model` works out what it shows from the device,
//! `controller` talks to the device, and `app` keeps the window up to date.

mod app;
mod art;
mod config;
mod controller;
mod equalizer;
pub mod logs;
pub mod model;
mod models;
mod overview;
mod presets;
mod rows;
mod single_instance;
mod view;
mod window_chrome;

use anyhow::Context;
use slint::ComponentHandle;

slint::include_modules!();

/// Opens the main window and runs until it is closed.
pub fn run() -> anyhow::Result<()> {
    // the same folder as gui/, so both use the same devices and presets, unless another one is asked for
    let config_dir = match std::env::var_os("OPENSCQ30_CONFIG_DIR") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => dirs::config_dir()
            .context("no config folder")?
            .join("openscq30"),
    };
    let window = AppWindow::new()?;
    if !single_instance::claim(&config_dir, app::show_from_elsewhere) {
        return Ok(());
    }
    // OPENSCQ30_SIZE=1280x820 opens the window at that size, for screenshots; debug builds only
    if cfg!(debug_assertions)
        && let Some((width, height)) = std::env::var("OPENSCQ30_SIZE").ok().and_then(|size| {
            let (w, h) = size.split_once('x')?;
            Some((w.parse::<f32>().ok()?, h.parse::<f32>().ok()?))
        })
    {
        window
            .window()
            .set_size(slint::LogicalSize::new(width, height));
    }
    app::start(&window, &config_dir)?;
    let result = (|| {
        window.show()?;
        tracing::debug!(
            "window {:?} at scale {}",
            window.window().size(),
            window.window().scale_factor()
        );
        app::shown();
        slint::run_event_loop()
    })();
    app::stop();
    result?;
    Ok(())
}
