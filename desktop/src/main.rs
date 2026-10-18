// A release build is a windowed program; a debug build keeps its console for the logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tracing_subscriber::{EnvFilter, filter::LevelFilter};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::WARN.into())
                .from_env()?,
        )
        .init();
    openscq30_desktop::run()
}
