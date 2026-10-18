//! The app's own settings page: how it looks, its language, and copying the logs.

use slint::ComponentHandle;

use super::{App, with_app};
use crate::{
    Actions, AppState, AppWindow, ToastKind,
    config::{Accent, AppTheme, Settings, SoundModeView},
    controller::Command,
    logs,
    model::device_art::EarbudsColor,
};

/// Changes a setting, saves it and shows the window with it.
fn change(edit: impl FnOnce(&mut Settings)) {
    with_app(|app| {
        edit(&mut app.config.settings);
        app.config.save();
        app.apply_theme();
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
    actions.on_set_language(|index| {
        with_app(|app| {
            app.config.settings.language = match index {
                1 => Some("ru".to_string()),
                2 => Some("en".to_string()),
                _ => None,
            };
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
        state.set_app_language(match settings.language.as_deref() {
            Some("ru") => 1,
            Some("en") => 2,
            _ => 0,
        });
        state.set_show_one_earbud_badge(settings.one_earbud_badge);
        state.set_app_version(env!("CARGO_PKG_VERSION").into());
    }
}
