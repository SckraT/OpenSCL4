//! Whether the Linux desktop prefers dark, from the XDG desktop portal. winit can't say: on Wayland it returns the
//! theme its window was last given, which after "Light" is the app's own, and it never reports a change.

#[cfg(target_os = "linux")]
const PATH: &str = "/org/freedesktop/portal/desktop";
#[cfg(target_os = "linux")]
const SETTINGS: &str = "org.freedesktop.portal.Settings";
const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";

fn is_color_scheme(namespace: &str, key: &str) -> bool {
    namespace == NAMESPACE && key == KEY
}

/// The portal's color scheme is 1 for dark, 2 for light and 0 for no preference.
fn prefers_dark(color_scheme: u32) -> bool {
    color_scheme == 1
}

/// Calls `emit` with whether the desktop prefers dark, then again on every change. Without a portal it waits for
/// one, and without a session bus it returns.
#[cfg(target_os = "linux")]
pub async fn watch(emit: impl Fn(bool)) {
    let watched = match zbus::Connection::session().await {
        Ok(connection) => watch_on(&connection, "org.freedesktop.portal.Desktop", emit).await,
        Err(err) => Err(err),
    };
    if let Err(err) = watched {
        tracing::debug!("not following the desktop's color scheme: {err}");
    }
}

#[cfg(target_os = "linux")]
async fn watch_on(
    connection: &zbus::Connection,
    portal: &str,
    emit: impl Fn(bool),
) -> zbus::Result<()> {
    use futures::StreamExt;
    use zbus::zvariant::OwnedValue;

    let dark = |value: &OwnedValue| value.downcast_ref::<u32>().is_ok_and(prefers_dark);
    let proxy = zbus::Proxy::new(connection, portal, PATH, SETTINGS).await?;
    // listening before reading, so a change in between is not lost
    let mut changes = proxy.receive_signal("SettingChanged").await?;
    if let Ok(value) = proxy
        .call::<_, _, OwnedValue>("ReadOne", &(NAMESPACE, KEY))
        .await
    {
        emit(dark(&value));
    }
    while let Some(change) = changes.next().await {
        if let Ok((namespace, key, value)) =
            change.body().deserialize::<(String, String, OwnedValue)>()
            && is_color_scheme(&namespace, &key)
        {
            emit(dark(&value));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_color_scheme, prefers_dark};

    #[test]
    fn only_a_dark_preference_is_dark() {
        assert!(prefers_dark(1));
        assert!(!prefers_dark(0), "no preference shows the app light");
        assert!(!prefers_dark(2));
    }

    #[test]
    fn only_the_portal_color_scheme_counts() {
        assert!(is_color_scheme(
            "org.freedesktop.appearance",
            "color-scheme"
        ));
        assert!(!is_color_scheme("org.freedesktop.appearance", "contrast"));
        assert!(!is_color_scheme(
            "org.gnome.desktop.interface",
            "color-scheme"
        ));
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod bus_tests {
    use std::{sync::mpsc, time::Duration};

    use zbus::zvariant::{OwnedValue, Value};

    use super::{KEY, NAMESPACE, PATH, SETTINGS, watch_on};

    /// A settings portal whose system prefers dark.
    struct Portal;

    #[zbus::interface(name = "org.freedesktop.portal.Settings")]
    impl Portal {
        fn read_one(&self, namespace: &str, key: &str) -> zbus::fdo::Result<OwnedValue> {
            if (namespace, key) == (NAMESPACE, KEY) {
                Ok(OwnedValue::from(1u32))
            } else {
                Err(zbus::fdo::Error::Failed(format!("no {namespace} {key}")))
            }
        }
    }

    #[test]
    fn follows_the_portal_color_scheme() {
        if std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_none() {
            // A skipped test reports ok, so CI must provide the bus rather than skip silently.
            assert!(
                std::env::var_os("CI").is_none(),
                "no session bus in CI: run the tests under dbus-run-session"
            );
            eprintln!("skipped: no session bus; run it under dbus-run-session");
            return;
        }
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        // its own name rather than the real portal's, which a desktop session already has
        let name = format!("io.github.sckrat.openscl4.TestPortal{}", std::process::id());
        let portal = zbus::connection::Builder::session()
            .and_then(|builder| builder.name(name.as_str()))
            .and_then(|builder| builder.serve_at(PATH, Portal))
            .unwrap();
        let portal = runtime.block_on(portal.build()).unwrap();
        let watcher = runtime.block_on(zbus::Connection::session()).unwrap();
        let (sender, reported) = mpsc::channel();
        runtime.spawn(async move {
            watch_on(&watcher, &name, move |dark| {
                let _ = sender.send(dark);
            })
            .await
        });
        let next = || {
            reported
                .recv_timeout(Duration::from_secs(5))
                .expect("the watcher reported nothing")
        };
        assert!(next(), "the portal prefers dark");

        let change = |namespace: &str, key: &str, value: u32| {
            runtime
                .block_on(portal.emit_signal(
                    None::<zbus::names::BusName<'_>>,
                    PATH,
                    SETTINGS,
                    "SettingChanged",
                    &(namespace, key, Value::from(value)),
                ))
                .unwrap();
        };
        // high contrast is 1 too, and is not the color scheme
        change(NAMESPACE, "contrast", 1);
        change(NAMESPACE, KEY, 2);
        assert!(!next(), "the system switched to light");
        change(NAMESPACE, KEY, 1);
        assert!(next(), "the system switched back to dark");
    }
}
