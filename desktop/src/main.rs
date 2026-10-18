// A release build is a windowed program; a debug build keeps its console for the logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod window_chrome;

use slint::ComponentHandle;
use tracing_subscriber::{EnvFilter, filter::LevelFilter};

slint::include_modules!();

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env()?,
        )
        .init();

    let window = AppWindow::new()?;
    window.show()?;
    window_chrome::set_dark_title_bar(window.window(), window.global::<Theme>().get_dark());
    slint::run_event_loop()?;
    Ok(())
}
