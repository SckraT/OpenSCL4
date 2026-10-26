//! One app at a time for the same devices and settings. Only one app can hold the connection to a device, so a second
//! start asks the first to show its window, which may be hidden in the notification area, and ends. Apps with another
//! config folder, such as for tests, run beside it. Windows uses a named mutex and event; Linux uses a name on the
//! session bus, which the bus releases when the app exits or crashes, so no stale lock is left behind. Without a
//! session bus Linux allows the start instead.

use std::path::Path;

/// A name for the config folder that stays the same between builds, unlike std's hasher. Windows folder names are
/// case-insensitive; Linux ones are not, and aliases of a folder that exists are resolved first.
#[cfg(any(windows, target_os = "linux", test))]
fn folder_key(config_dir: &Path) -> String {
    #[cfg(windows)]
    let path = config_dir.to_string_lossy().to_lowercase();
    #[cfg(not(windows))]
    let path = normalized(config_dir).to_string_lossy().into_owned();
    fnv1a(&path)
}

#[cfg(any(windows, target_os = "linux", test))]
fn fnv1a(text: &str) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

/// The folder as one absolute path, following symlinks in the longest prefix that exists. Parts that do not exist yet,
/// including `..`, stay as written.
#[cfg(all(not(windows), any(target_os = "linux", test)))]
fn normalized(config_dir: &Path) -> std::path::PathBuf {
    let Ok(absolute) = std::path::absolute(config_dir) else {
        return config_dir.to_path_buf();
    };
    let mut missing = Vec::new();
    let mut ancestor = absolute.as_path();
    loop {
        if let Ok(mut path) = std::fs::canonicalize(ancestor) {
            for part in missing.iter().rev() {
                path.push(part);
            }
            return path;
        }
        let (Some(parent), Some(name)) = (ancestor.parent(), ancestor.file_name()) else {
            return absolute;
        };
        missing.push(name.to_os_string());
        ancestor = parent;
    }
}

/// The session-bus name for a folder. Each dot-separated part of a D-Bus name must start with a letter, hence the `f`
/// before the key.
#[cfg(any(target_os = "linux", test))]
fn instance_name(key: &str) -> String {
    format!("io.github.sckrat.openscl4.Instance.f{key}")
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

#[cfg(target_os = "linux")]
type ShowCallback = std::sync::Arc<dyn Fn() + Send + Sync>;

#[cfg(target_os = "linux")]
struct ShowOnBus {
    show: ShowCallback,
}

#[cfg(target_os = "linux")]
#[zbus::interface(name = "io.github.sckrat.openscl4.SingleInstance")]
impl ShowOnBus {
    fn show(&self) {
        (self.show)();
    }
}

/// Whether this is the only app running for the config folder. If it is, a later start makes it call `show`; if not,
/// the app that runs has been asked to show its window. A session bus releases the name when its owner ends, so a
/// crash leaves no lock behind.
#[cfg(target_os = "linux")]
pub fn claim(config_dir: &Path, show: fn()) -> bool {
    use std::sync::OnceLock;

    /// The connection and the runtime that drives it, kept for as long as the app runs.
    static INSTANCE: OnceLock<(tokio::runtime::Runtime, zbus::Connection)> = OnceLock::new();

    let name = instance_name(&folder_key(config_dir));
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(err) => {
            tracing::warn!(
                "no runtime for the session bus, so a later start can not be asked to show: {err}"
            );
            return true;
        }
    };
    let connection = match runtime.block_on(connect(&name, show)) {
        Ok(Some(connection)) => connection,
        Ok(None) => return false,
        Err(err) => {
            tracing::warn!("no session bus, so a later start can not be asked to show: {err}");
            return true;
        }
    };
    // the name lives while its connection does, so keep both for as long as the app runs
    let _ = INSTANCE.set((runtime, connection));
    true
}

#[cfg(target_os = "linux")]
async fn connect(name: &str, show: fn()) -> zbus::Result<Option<zbus::Connection>> {
    use std::sync::Arc;

    let connection = zbus::Connection::session().await?;
    // the handler runs on the bus's thread, so hand the window to the event loop as the Windows thread does
    let callback: ShowCallback = Arc::new(move || {
        if slint::invoke_from_event_loop(show).is_err() {
            tracing::debug!("the window is gone");
        }
    });
    if claim_on(&connection, name, callback).await? {
        Ok(Some(connection))
    } else {
        Ok(None)
    }
}

#[cfg(target_os = "linux")]
async fn claim_on(
    connection: &zbus::Connection,
    name: &str,
    show: ShowCallback,
) -> zbus::Result<bool> {
    use std::time::Duration;

    use zbus::fdo::{RequestNameFlags, RequestNameReply};

    // serve the window before taking the name, so a start that finds the name can call Show at once
    connection
        .object_server()
        .at(
            "/io/github/sckrat/openscl4/SingleInstance",
            ShowOnBus { show },
        )
        .await?;
    for _ in 0..3 {
        match connection
            .request_name_with_flags(name, RequestNameFlags::DoNotQueue.into())
            .await
        {
            Ok(RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner) => return Ok(true),
            Ok(RequestNameReply::Exists | RequestNameReply::InQueue) => {}
            // The flags API reports an existing owner as an error rather than a reply.
            Err(zbus::Error::NameTaken) => {}
            Err(err) => return Err(err),
        }
        if ask_to_show(connection, name).await.is_ok() {
            return Ok(false);
        }
        // the app that had the name may have ended as this one started; try to take it
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    Ok(false)
}

#[cfg(target_os = "linux")]
async fn ask_to_show(connection: &zbus::Connection, name: &str) -> zbus::Result<()> {
    let proxy = zbus::Proxy::new(
        connection,
        name,
        "/io/github/sckrat/openscl4/SingleInstance",
        "io.github.sckrat.openscl4.SingleInstance",
    )
    .await?;
    proxy.call::<_, _, ()>("Show", &()).await
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn claim(_config_dir: &Path, _show: fn()) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(windows)]
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

    #[test]
    fn the_bus_name_carries_the_folder_key_and_has_valid_parts() {
        let name = instance_name("0123456789abcdef");
        assert_eq!("io.github.sckrat.openscl4.Instance.f0123456789abcdef", name);
        assert!(name.len() < 255);
        assert!(name.split('.').all(|part| {
            part.bytes()
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        }));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn folders_that_differ_only_in_case_are_different_instances() {
        let base = tempfile::tempdir().unwrap();
        let lower = base.path().join("a");
        let upper = base.path().join("A");
        std::fs::create_dir_all(&lower).unwrap();
        std::fs::create_dir_all(&upper).unwrap();
        assert_ne!(folder_key(&lower), folder_key(&upper));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_folder_and_its_dot_alias_get_the_same_name() {
        let base = tempfile::tempdir().unwrap();
        let folder = base.path().join("data");
        std::fs::create_dir_all(&folder).unwrap();
        assert_eq!(folder_key(&folder), folder_key(&folder.join(".")));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_folder_that_does_not_exist_yet_gets_the_name_it_has_once_created() {
        let base = tempfile::tempdir().unwrap();
        let real = base.path().join("real");
        let link = base.path().join("link");
        std::fs::create_dir_all(&real).unwrap();
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let folder = link.join("child");
        assert!(!folder.exists());

        let before = folder_key(&folder);
        std::fs::create_dir_all(&folder).unwrap();

        assert_eq!(before, folder_key(&folder));
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod bus_tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use super::{claim_on, instance_name};

    #[test]
    fn a_second_claim_asks_the_first_to_show_and_ends_without_the_name() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            eprintln!("skipped: no session bus; run it under dbus-run-session");
            return;
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let name = instance_name(&format!("test{}", std::process::id()));
        let shown = Arc::new(AtomicUsize::new(0));
        let counted = shown.clone();
        let first = runtime.block_on(zbus::Connection::session()).unwrap();
        assert!(
            runtime
                .block_on(claim_on(
                    &first,
                    &name,
                    Arc::new(move || {
                        counted.fetch_add(1, Ordering::SeqCst);
                    })
                ))
                .unwrap(),
            "the first start should own the name"
        );
        let second = runtime.block_on(zbus::Connection::session()).unwrap();
        assert!(
            !runtime
                .block_on(claim_on(&second, &name, Arc::new(|| {})))
                .unwrap(),
            "the second start should not own the name"
        );
        for _ in 0..100 {
            if shown.load(Ordering::SeqCst) == 1 {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("the first start was not asked to show its window");
    }
}
