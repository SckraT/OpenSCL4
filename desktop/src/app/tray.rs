//! The icon in the notification area: its menu, opening the window from it, and quitting, which only it and the
//! settings page do while closing the window leaves the app there.

use slint::{ComponentHandle, ModelRc, VecModel};

use super::{App, Connection, with_app};
use crate::{
    Strings, Tray,
    controller::Command,
    model::battery::is_only_one_earbud_connected,
    overview,
    rows::{self, Edit},
};

fn native_battery_value(value: &str) -> String {
    value.replace(['\u{2068}', '\u{2069}'], "")
}

fn new_tray() -> Option<Tray> {
    let tray = match Tray::new() {
        Ok(tray) => tray,
        Err(err) => {
            tracing::warn!("could not make the tray icon: {err}");
            return None;
        }
    };
    tray.on_open(|| with_app(App::open_window));
    tray.on_quit(|| with_app(App::quit));
    tray.on_connect(|| {
        with_app(|app| {
            if let Some(mac) = app.current {
                app.connect(mac, false);
            }
        });
    });
    tray.on_choose_mode(|index| with_app(|app| app.edit("ambientSoundMode", Edit::Option(index))));
    tray.on_apply_preset(|name| {
        with_app(|app| app.send(Command::ActivatePreset(name.to_string())));
    });
    tray.on_set_ldac(|on| with_app(|app| app.edit("ldac", Edit::Toggle(on))));
    Some(tray)
}

impl App {
    /// Shows or takes away the tray icon, as the settings say.
    pub(super) fn sync_tray(&mut self) {
        match (
            self.config.settings.tray && self.tray_host,
            self.tray.is_some(),
        ) {
            (true, false) => {
                self.tray = new_tray();
                self.tray_registered = !cfg!(target_os = "linux") && self.tray.is_some();
            }
            (false, true) => {
                self.tray = None;
                self.tray_registered = false;
            }
            _ => {}
        }
    }

    /// Closing the window leaves the app in the notification area only while its icon is there.
    pub(super) fn closes_to_tray(&self) -> bool {
        self.tray.is_some() && self.tray_registered && self.config.settings.close_to_tray
    }

    pub(super) fn open_window(&mut self) {
        let Some(window) = self.window() else { return };
        if let Err(err) = window.show() {
            tracing::warn!("could not show the window: {err}");
        }
        use slint::winit_030::WinitWindowAccessor;
        window.window().with_winit_window(|window| {
            window.set_minimized(false);
            window.focus_window();
        });
        self.render();
    }

    /// Ends the app: the tray icon and the window go, then the event loop, after which `stop` lets go of the device.
    pub(super) fn quit(&mut self) {
        self.tray = None;
        if let Some(window) = self.window() {
            let _ = window.hide();
        }
        if let Err(err) = slint::quit_event_loop() {
            tracing::warn!("could not end the event loop: {err}");
        }
    }

    /// The device's battery levels named in the window's language, such as "Left 97% · Right 76% · Case 60%".
    fn battery_line(&self) -> String {
        let (Some(window), Some(snapshot)) = (self.window(), self.snapshot()) else {
            return String::new();
        };
        let strings = window.global::<Strings>();
        let one_earbud = is_only_one_earbud_connected(&snapshot.settings);
        overview::batteries(snapshot, one_earbud)
            .iter()
            .map(|battery| {
                let name = match battery.label {
                    0 => strings.get_left_battery(),
                    1 => strings.get_right_battery(),
                    2 => strings.get_case_battery(),
                    _ => strings.get_device_battery(),
                };
                format!("{name} {}", native_battery_value(&battery.value))
            })
            .collect::<Vec<_>>()
            .join(" · ")
    }

    pub(super) fn render_tray(&self) {
        let Some(tray) = &self.tray else { return };
        let settings = &self.config.settings;
        let current = self
            .current
            .and_then(|mac| self.paired.iter().find(|p| p.mac_address == mac));
        tray.set_has_device(current.is_some());
        tray.set_device_name(
            current
                .map(|p| crate::view::model_name(p.model))
                .unwrap_or_default()
                .into(),
        );
        tray.set_connecting(matches!(self.connection, Connection::Connecting));
        tray.set_show_battery(settings.tray_items.battery);
        tray.set_show_modes(settings.tray_items.modes);
        tray.set_show_presets(settings.tray_items.presets);
        tray.set_show_ldac(settings.tray_items.ldac);

        let Some(snapshot) = self.snapshot() else {
            tray.set_connected(false);
            return;
        };
        tray.set_connected(true);
        tray.set_battery(self.battery_line().into());
        let scale_preferred = settings.sound_mode_view == crate::config::SoundModeView::Scale;
        match rows::mode_view(snapshot, scale_preferred) {
            Some(mode) => {
                tray.set_mode_index(mode.index);
                tray.set_modes(ModelRc::new(VecModel::from(mode.options)));
            }
            None => tray.set_modes(ModelRc::default()),
        }
        let (on, off) = self.on_off();
        let last_preset = snapshot.last_applied_preset.as_deref();
        tray.set_presets(ModelRc::new(VecModel::from(overview::presets(
            snapshot,
            last_preset,
            crate::presets::OnOff { on: &on, off: &off },
        ))));
        let ldac = snapshot
            .settings
            .get(&openscq30_lib::settings::SettingId::Ldac);
        tray.set_has_ldac(ldac.is_some());
        tray.set_ldac(matches!(
            ldac,
            Some(openscq30_lib::settings::Setting::Toggle { value: true })
        ));
    }
}

/// Opens the window when another start of the app asks for it.
pub fn show_from_elsewhere() {
    with_app(App::open_window);
}

#[cfg(test)]
mod tests {
    use super::native_battery_value;

    #[test]
    fn native_battery_values_omit_fluent_direction_markers() {
        assert_eq!("100%", native_battery_value("\u{2068}100\u{2069}%"));
        assert_eq!(
            "64%",
            native_battery_value("\u{2068}\u{2068}64\u{2069}\u{2069}%")
        );
        assert_eq!("٦٠٪", native_battery_value("\u{2068}٦٠\u{2069}٪"));
        assert_eq!("—", native_battery_value("—"));
    }
}
