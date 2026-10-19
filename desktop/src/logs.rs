//! Logs go to the console and are kept in memory too, so that the settings page can copy the latest ones for a bug
//! report.

use std::{
    collections::VecDeque,
    io::Write,
    sync::{LazyLock, Mutex},
};

use tracing_subscriber::{EnvFilter, filter::LevelFilter};

/// How many log lines are kept.
const KEPT: usize = 1000;

static RECENT: LazyLock<Mutex<VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(KEPT)));

/// One log event, written to the console and kept once it is complete.
struct EventWriter(Vec<u8>);

impl Write for EventWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for EventWriter {
    fn drop(&mut self) {
        let _ = std::io::stderr().write_all(&self.0);
        keep(String::from_utf8_lossy(&self.0).trim_end());
    }
}

fn keep(line: &str) {
    if line.is_empty() {
        return;
    }
    let mut recent = RECENT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    if recent.len() == KEPT {
        recent.pop_front();
    }
    recent.push_back(line.to_string());
}

/// Warnings and errors from everything, and what the app and the library do besides; RUST_LOG overrides it.
pub fn init() -> anyhow::Result<()> {
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::WARN.into())
        .parse(
            std::env::var("RUST_LOG")
                .unwrap_or_else(|_| "openscq30_desktop=info,openscq30_lib=info".to_string()),
        )?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(|| EventWriter(Vec::new()))
        .init();
    Ok(())
}

/// The latest log lines, oldest first, after a line that says which app wrote them.
pub fn recent() -> String {
    let recent = RECENT
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut text = format!(
        "OpenSCL4 {} ({}, {})\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    for line in recent.iter() {
        text.push_str(line);
        text.push('\n');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_latest_lines_after_the_header() {
        for i in 0..KEPT + 5 {
            keep(&format!("line {i}"));
        }
        keep("");
        let text = recent();
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines[0].starts_with("OpenSCL4 "));
        assert_eq!(KEPT + 1, lines.len());
        assert_eq!("line 5", lines[1]);
        assert_eq!(format!("line {}", KEPT + 4), lines[KEPT]);
    }
}
