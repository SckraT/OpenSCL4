//! The app's own settings page: how it looks, its language, and copying the logs.

use slint::ComponentHandle;

use super::{App, with_app};
use crate::{
    Actions, AppState, AppWindow, Strings, ToastKind,
    config::{Accent, AppTheme, Settings, SoundModeView},
    controller::Command,
    logs,
    model::{device_art::EarbudsColor, languages::LANGUAGES},
    system_settings,
};

/// Changes a setting, saves it and shows the window with it.
fn change(edit: impl FnOnce(&mut Settings)) {
    with_app(|app| {
        edit(&mut app.config.settings);
        app.config.save();
        app.apply_theme();
        app.sync_tray();
        app.render();
    });
}

pub(super) fn install_actions(window: &AppWindow) {
    let actions = window.global::<Actions>();
    actions.on_set_app_theme(|index| {
        change(|settings| {
            settings.theme = match index {
                1 => AppTheme::Light,
                2 => AppTheme::Dark,
                _ => AppTheme::System,
            };
        });
    });
    actions.on_set_palette(|index| {
        change(|settings| {
            settings.accent = match index {
                1 => Accent::Sage,
                2 => Accent::Rose,
                _ => Accent::Purple,
            };
        });
    });
    actions.on_set_device_color(|index| {
        change(|settings| {
            settings.earbuds_color = match index {
                1 => Some(EarbudsColor::Silver),
                2 => Some(EarbudsColor::Black),
                _ => None,
            };
        });
    });
    actions.on_set_mode_view(|index| {
        change(|settings| {
            settings.sound_mode_view = if index == 0 {
                SoundModeView::Tiles
            } else {
                SoundModeView::Scale
            };
        });
    });
    actions.on_set_badge(|on| change(|settings| settings.one_earbud_badge = on));
    actions.on_set_tray(|on| change(|settings| settings.tray = on));
    actions.on_set_close_to_tray(|on| change(|settings| settings.close_to_tray = on));
    actions.on_set_tray_item(|index, on| {
        change(|settings| match index {
            0 => settings.tray_items.battery = on,
            1 => settings.tray_items.modes = on,
            2 => settings.tray_items.presets = on,
            3 => settings.tray_items.ldac = on,
            _ => {}
        });
    });
    actions.on_quit_app(|| with_app(App::quit));
    actions.on_set_language(|index| {
        with_app(|app| {
            // the first choice is the system's language
            app.config.settings.language = usize::try_from(index - 1)
                .ok()
                .and_then(|i| LANGUAGES.get(i))
                .map(|l| l.code.to_string());
            app.config.save();
            app.apply_language();
            // the library words the device's settings as it reads them
            app.controller.send(Command::Reread);
            app.render();
        });
    });
    actions.on_copy_logs(|| {
        with_app(|app| {
            match arboard::Clipboard::new().and_then(|mut c| c.set_text(logs::recent())) {
                Ok(()) => app.toast(ToastKind::Copied),
                Err(err) => tracing::warn!("could not copy the logs: {err}"),
            }
        });
    });
}

impl App {
    pub(super) fn render_settings(&self, state: &AppState<'_>) {
        let settings = &self.config.settings;
        state.set_app_theme(match settings.theme {
            AppTheme::System => 0,
            AppTheme::Light => 1,
            AppTheme::Dark => 2,
        });
        state.set_app_palette(match settings.accent {
            Accent::Purple => 0,
            Accent::Sage => 1,
            Accent::Rose => 2,
        });
        state.set_app_device_color(match settings.earbuds_color {
            None => 0,
            Some(EarbudsColor::Silver) => 1,
            Some(EarbudsColor::Black) => 2,
        });
        state.set_app_mode_view(match settings.sound_mode_view {
            SoundModeView::Tiles => 0,
            SoundModeView::Scale => 1,
        });
        state.set_app_language(
            settings
                .language
                .as_deref()
                .and_then(|code| LANGUAGES.iter().position(|l| l.code == code))
                .map_or(0, |i| i as i32 + 1),
        );
        // the first choice, in the window's language
        let system = self
            .window()
            .map(|w| w.global::<Strings>().get_system_language())
            .unwrap_or_default();
        let names: Vec<slint::SharedString> = std::iter::once(system)
            .chain(LANGUAGES.iter().map(|l| l.name.into()))
            .collect();
        state.set_language_names(slint::ModelRc::new(slint::VecModel::from(names)));
        state.set_show_one_earbud_badge(settings.one_earbud_badge);
        state.set_app_tray(settings.tray);
        state.set_app_tray_available(self.tray.is_some() && self.tray_registered);
        state.set_app_close_to_tray(settings.close_to_tray);
        state.set_tray_battery(settings.tray_items.battery);
        state.set_tray_modes(settings.tray_items.modes);
        state.set_tray_presets(settings.tray_items.presets);
        state.set_tray_ldac(settings.tray_items.ldac);
        state.set_app_version(env!("CARGO_PKG_VERSION").into());
        state.set_linux(cfg!(target_os = "linux"));
        state.set_system_settings_available(system_settings::available());
    }
}
