//! The desktop's own UI scale on Linux. winit reports the scale its windowing system gives it: `Xft.dpi` on X11
//! and the compositor's scale on Wayland. It does not report the scales toolkits and desktops apply themselves:
//! GTK's `GDK_SCALE` and `GDK_DPI_SCALE`, Qt's `QT_SCALE_FACTOR`, and GNOME's accessibility text scale, which the
//! settings portal forwards. A desktop that scales its applications with those draws this window at 1:1, and it
//! looks smaller than the same window on Windows, where the system DPI always reaches the application.
//!
//! Slint takes a scale factor from `SLINT_SCALE_FACTOR` before winit's, so the desktop's scale is given to it
//! there. The window, its text and its controls all follow it, as they do on Windows.

/// Where the scale came from, for the log line that explains a window drawn at something other than 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Gdk,
    Qt,
    Portal,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Gdk => "GDK_SCALE",
            Self::Qt => "QT_SCALE_FACTOR",
            Self::Portal => "the desktop portal",
        })
    }
}

/// Gives Slint the desktop's scale, unless the user or the packager already chose one. Runs before the window
/// exists, because Slint reads the variable when it makes it.
#[cfg(target_os = "linux")]
pub fn init() {
    // an explicit choice wins, here as in winit
    if std::env::var_os("SLINT_SCALE_FACTOR").is_some()
        || std::env::var_os("WINIT_X11_SCALE_FACTOR").is_some()
    {
        return;
    }
    let Some((scale, source)) = choose(|name| std::env::var(name).ok(), portal_scale) else {
        return;
    };
    tracing::info!("drawing at the desktop's scale factor {scale}, from {source}");
    // SAFETY: called from `run` before the window and the controller exist; the only thread running is the one the
    // portal query may have left behind, and it does not read the environment.
    unsafe { std::env::set_var("SLINT_SCALE_FACTOR", scale.to_string()) };
}

/// The desktop's scale, from the first source that names one worth applying: GTK's window and text scale, Qt's, or
/// GNOME's window scale times its text scale. The portal is asked only when neither toolkit variable answers.
fn choose(
    env: impl Fn(&str) -> Option<String>,
    portal: impl FnOnce() -> Option<f32>,
) -> Option<(f32, Source)> {
    let gdk = env("GDK_SCALE")
        .and_then(|window| parse(&window))
        .map(|window| {
            let text = env("GDK_DPI_SCALE")
                .and_then(|text| parse(&text))
                .unwrap_or(1.0);
            (window * text, Source::Gdk)
        })
        .filter(|(scale, _)| is_sane(*scale));
    let qt = env("QT_SCALE_FACTOR")
        .and_then(|scale| parse(&scale))
        .map(|scale| (scale, Source::Qt))
        .filter(|(scale, _)| is_sane(*scale));
    gdk.or(qt).or_else(|| {
        portal()
            .map(|scale| (scale, Source::Portal))
            .filter(|(scale, _)| is_sane(*scale))
    })
}

/// A number from an environment variable, such as "1.25".
fn parse(text: &str) -> Option<f32> {
    text.trim().parse::<f32>().ok()
}

/// Whether the number is a scale a window can be drawn at. Below 1 it would make the window smaller than Slint's
/// layout, and anything this large is a misconfigured desktop rather than a scale.
fn is_sane(scale: f32) -> bool {
    scale.is_finite() && (1.005..=4.0).contains(&scale)
}

/// GNOME's window scale and its accessibility text scale, read through the XDG settings portal: the same values
/// GTK applies, and the ones winit never sees. A window scale of 0 means "pick per monitor", and then only the
/// compositor knows it, through winit; text scaling alone is not a window scale, and taking it for one would shrink
/// a window the compositor already drew at 2x.
#[cfg(target_os = "linux")]
fn portal_scale() -> Option<f32> {
    use std::time::Duration;

    // a session bus that does not answer must not hold up the window; the thread is left to finish or fail by itself
    let (sender, receiver) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = sender.send(read_portal());
    });
    receiver
        .recv_timeout(Duration::from_millis(300))
        .ok()
        .flatten()
}

#[cfg(target_os = "linux")]
fn read_portal() -> Option<f32> {
    const INTERFACE: &str = "org.gnome.desktop.interface";
    let scaling = portal_value(INTERFACE, "scaling-factor")
        .and_then(|value| value.downcast_ref::<u32>().ok())
        .filter(|scaling| *scaling > 0)?;
    let text = portal_value(INTERFACE, "text-scaling-factor")
        .and_then(|value| value.downcast_ref::<f64>().ok())
        .unwrap_or(1.0) as f32;
    Some(scaling as f32 * text)
}

#[cfg(target_os = "linux")]
fn portal_value(namespace: &str, key: &str) -> Option<zbus::zvariant::OwnedValue> {
    let connection = zbus::blocking::Connection::session().ok()?;
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )
    .ok()?;
    proxy.call("ReadOne", &(namespace, key)).ok()
}

#[cfg(test)]
mod tests {
    use super::{Source, choose, is_sane};

    /// The environment as the desktop wrote it.
    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let pairs: Vec<(String, String)> = pairs
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect();
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.clone())
        }
    }

    #[test]
    fn gdk_scales_the_window_and_its_text() {
        // a window scale with a text scale that brings it back to 1 is not a reason to scale anything
        let chosen = choose(env(&[("GDK_SCALE", "2"), ("GDK_DPI_SCALE", "0.5")]), || {
            None
        });
        assert_eq!(None, chosen);
        let chosen = choose(env(&[("GDK_SCALE", "2")]), || None);
        assert_eq!(Some((2.0, Source::Gdk)), chosen);
        let chosen = choose(
            env(&[("GDK_SCALE", "1"), ("GDK_DPI_SCALE", "1.25")]),
            || None,
        );
        assert_eq!(Some((1.25, Source::Gdk)), chosen);
    }

    #[test]
    fn qt_comes_after_gdk_and_the_portal_after_both() {
        let both = choose(
            env(&[("GDK_SCALE", "2"), ("QT_SCALE_FACTOR", "1.5")]),
            || Some(1.25),
        );
        assert_eq!(Some((2.0, Source::Gdk)), both);
        let qt = choose(env(&[("QT_SCALE_FACTOR", "1.5")]), || Some(1.25));
        assert_eq!(Some((1.5, Source::Qt)), qt);
        let portal = choose(env(&[]), || Some(1.25));
        assert_eq!(Some((1.25, Source::Portal)), portal);
    }

    #[test]
    fn a_desktop_that_names_no_scale_leaves_the_window_alone() {
        assert_eq!(None, choose(env(&[]), || None));
        // a source that names 1 asks for nothing
        assert_eq!(None, choose(env(&[]), || Some(1.0)));
    }

    #[test]
    fn the_portal_is_only_asked_when_no_toolkit_variable_answers() {
        let asked = std::cell::Cell::new(false);
        let chosen = choose(env(&[("GDK_SCALE", "2")]), || {
            asked.set(true);
            Some(1.25)
        });
        assert_eq!(Some((2.0, Source::Gdk)), chosen);
        assert!(
            !asked.get(),
            "the portal was asked although GDK named a scale"
        );
    }

    #[test]
    fn only_a_usable_scale_is_taken() {
        assert!(is_sane(1.25));
        assert!(is_sane(2.0));
        assert!(is_sane(4.0));
        assert!(!is_sane(0.5), "never smaller than the layout");
        assert!(!is_sane(1.0), "1 is what the window already does");
        assert!(!is_sane(f32::NAN));
        assert!(!is_sane(f32::INFINITY));
        assert!(!is_sane(8.0), "a desktop that says 8 is misconfigured");
    }

    #[test]
    fn a_bad_variable_falls_through_to_the_next_source() {
        let chosen = choose(
            env(&[("GDK_SCALE", "two"), ("QT_SCALE_FACTOR", "1.5")]),
            || None,
        );
        assert_eq!(Some((1.5, Source::Qt)), chosen);
        let chosen = choose(
            env(&[("GDK_SCALE", "1.5"), ("GDK_DPI_SCALE", "oops")]),
            || None,
        );
        assert_eq!(Some((1.5, Source::Gdk)), chosen);
    }
}
