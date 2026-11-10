//! Logs go to the console, to a file, and are kept in memory too, so that the settings page can copy the latest ones
//! for a bug report. The file matters on Windows: a release build is a windowed program with no console, so without it
//! the lines that explain a failure die with the process.

use std::{
    collections::VecDeque,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
};

use tracing_subscriber::{EnvFilter, filter::LevelFilter};

/// How many log lines are kept.
const KEPT: usize = 1000;

/// A log file larger than this is rotated to `openscl4.log.1` before the next line is written.
const MAX_FILE_BYTES: u64 = 1024 * 1024;

static RECENT: LazyLock<Mutex<VecDeque<String>>> =
    LazyLock::new(|| Mutex::new(VecDeque::with_capacity(KEPT)));

static LOG_FILE: LazyLock<Mutex<Option<(PathBuf, File)>>> = LazyLock::new(|| Mutex::new(None));

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
        let line = String::from_utf8_lossy(&self.0).trim_end().to_string();
        write_to_file(&line);
        keep(&line);
    }
}

/// Starts writing log lines to `<directory>/logs/openscl4.log`, rotating once the file grows past [`MAX_FILE_BYTES`].
/// A directory that cannot be created is reported to the console and otherwise ignored: losing the file is better than
/// refusing to start.
pub fn open_file(directory: &Path) {
    let directory = directory.join("logs");
    if let Err(error) = fs::create_dir_all(&directory) {
        let _ = writeln!(
            std::io::stderr(),
            "could not create {}: {error}; logs stay in the console only",
            directory.display()
        );
        return;
    }
    let path = directory.join("openscl4.log");
    let file = open_append(&path).map(|file| (path, file));
    let mut slot = LOG_FILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *slot = file;
    drop(slot);
    write_to_file(&format!(
        "OpenSCL4 {} ({}, {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
}

fn open_append(path: &Path) -> Option<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| {
            let _ = writeln!(
                std::io::stderr(),
                "could not open {}: {error}; logs stay in the console only",
                path.display()
            );
        })
        .ok()
}

fn write_to_file(line: &str) {
    if line.is_empty() {
        return;
    }
    let mut slot = LOG_FILE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some((path, file)) = slot.as_mut() else {
        return;
    };
    let oversized = file
        .metadata()
        .is_ok_and(|metadata| metadata.len() > MAX_FILE_BYTES);
    if oversized {
        let rotated = path.with_file_name("openscl4.log.1");
        let _ = fs::rename(&*path, rotated);
        match open_append(path) {
            Some(fresh) => *file = fresh,
            None => {
                *slot = None;
                return;
            }
        }
    }
    let _ = writeln!(file, "{line}");
    let _ = file.flush();
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
    fn writes_lines_to_the_file_it_is_given() {
        let directory = tempfile::tempdir().unwrap();
        open_file(directory.path());
        write_to_file("a line that has to survive the process");
        let text = fs::read_to_string(directory.path().join("logs/openscl4.log")).unwrap();
        *LOG_FILE.lock().unwrap() = None;
        assert!(text.starts_with("OpenSCL4 "), "{text}");
        assert!(
            text.contains("a line that has to survive the process"),
            "{text}"
        );
    }

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
