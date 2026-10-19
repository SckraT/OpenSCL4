//! One app at a time for the same devices and settings. Only one app can hold the connection to a device, so a second
//! start asks the first to show its window, which may be hidden in the notification area, and ends. Apps with another
//! config folder, such as for tests, run beside it.

use std::path::Path;

/// A name for the config folder that stays the same between builds, unlike std's hasher.
#[cfg(any(windows, test))]
fn folder_key(config_dir: &Path) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in config_dir.to_string_lossy().to_lowercase().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// Whether this is the only app running for the config folder. If it is, a later start makes it call `show`; if not,
/// the app that runs has been asked to show its window.
#[cfg(windows)]
pub fn claim(config_dir: &Path, show: fn()) -> bool {
    use windows::{
        Win32::{
            Foundation::{ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0},
            System::Threading::{
                CreateEventW, CreateMutexW, INFINITE, SetEvent, WaitForSingleObject,
            },
        },
        core::HSTRING,
    };

    let key = folder_key(config_dir);
    let mutex_name = HSTRING::from(format!(r"Local\OpenSCL4-{key}"));
    let event_name = HSTRING::from(format!(r"Local\OpenSCL4Show-{key}"));
    // SAFETY: plain calls with names that live through them; the handles stay open as long as the app runs.
    let (mutex, already) = unsafe {
        let mutex = CreateMutexW(None, false, &mutex_name);
        (mutex, GetLastError() == ERROR_ALREADY_EXISTS)
    };
    // SAFETY: as above
    let event = unsafe { CreateEventW(None, false, false, &event_name) };
    let (Ok(_mutex), Ok(event)) = (mutex, event) else {
        tracing::warn!("could not tell whether the app runs already");
        return true;
    };
    if already {
        // SAFETY: the event handle was just opened
        if let Err(err) = unsafe { SetEvent(event) } {
            tracing::warn!("could not ask the app that runs to show itself: {err}");
        }
        return false;
    }
    // the handle as a number the thread can take along; HANDLE itself is not Send
    let raw = event.0 as isize;
    std::thread::spawn(move || {
        loop {
            // SAFETY: the event handle is never closed while the app runs
            let woken = unsafe { WaitForSingleObject(HANDLE(raw as *mut _), INFINITE) };
            if woken != WAIT_OBJECT_0 || slint::invoke_from_event_loop(show).is_err() {
                break;
            }
        }
    });
    true
}

#[cfg(not(windows))]
pub fn claim(_config_dir: &Path, _show: fn()) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_folder_gets_the_same_name_whatever_its_case() {
        let key = folder_key(Path::new(r"C:\Users\Me\AppData\Roaming\openscq30"));
        assert_eq!(
            key,
            folder_key(Path::new(r"c:\users\me\appdata\roaming\OpenSCQ30"))
        );
        assert_ne!(key, folder_key(Path::new(r"C:\tests\openscq30")));
        assert_eq!(16, key.len());
    }
}
