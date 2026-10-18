// A release build is a windowed program; a debug build keeps its console for the logs.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    openscq30_desktop::logs::init()?;
    openscq30_desktop::run()
}
