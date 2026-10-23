//! What the quick presets page does: creating presets, choosing which settings they apply, and deleting them.

use openscq30_lib::settings::SettingId;
use slint::{ComponentHandle, ModelRc, VecModel};

use super::{App, with_app};
use crate::{
    Actions, AppState, AppWindow, Strings, ToastKind,
    controller::{Command, DeviceSnapshot},
    model::presets::presets_use_blocked_settings,
    presets,
};

pub(super) fn install_actions(window: &AppWindow) {
    let actions = window.global::<Actions>();
    actions.on_preset_create(|name, replace| with_app(|app| app.create_preset(&name, replace)));
    actions.on_preset_edit(|name| {
        with_app(|app| {
            app.editing_preset = (!name.is_empty()).then(|| name.to_string());
            app.render();
        });
    });
    actions.on_preset_toggle(|name, id, enabled| {
        with_app(|app| {
            let Ok(setting_id) = id.parse::<SettingId>() else {
                return;
            };
            app.send(Command::TogglePresetField {
                name: name.to_string(),
                setting_id,
                enabled,
            });
        });
    });
    actions.on_preset_load(|name| {
        with_app(|app| {
            app.equalizer.pending_toast = Some((ToastKind::PresetLoaded, 0));
            app.send(Command::SavePreset(name.to_string()));
        });
    });
    actions.on_preset_delete(|name| {
        with_app(|app| {
            app.equalizer.pending_toast = Some((ToastKind::PresetDeleted, 0));
            app.send(Command::DeletePreset(name.to_string()));
        });
    });
}

impl App {
    /// The window's words for on and off, which follow its language.
    pub(super) fn on_off(&self) -> (String, String) {
        self.window().map_or_else(Default::default, |window| {
            let strings = window.global::<Strings>();
            (strings.get_on().into(), strings.get_off().into())
        })
    }

    fn create_preset(&mut self, name: &str, replace: bool) {
        let Some(window) = self.window() else { return };
        let state = window.global::<AppState>();
        let name = name.trim();
        if name.is_empty() {
            state.set_preset_create_problem(1);
            return;
        }
        let existing = self
            .snapshot()
            .and_then(|s| presets::existing_name(s, name))
            .map(str::to_string);
        match (existing, replace) {
            (Some(existing), false) => {
                state.set_preset_existing(existing.into());
                state.set_preset_create_problem(2);
            }
            (Some(existing), true) => {
                state.set_creating_preset(false);
                self.equalizer.pending_toast = Some((ToastKind::PresetReplaced, 0));
                self.send(Command::SavePreset(existing));
            }
            (None, _) => {
                // a new preset applies nothing yet, so the editor opens to choose what it does
                state.set_creating_preset(false);
                self.editing_preset = Some(name.to_string());
                self.send(Command::SavePreset(name.to_string()));
            }
        }
    }

    /// The device applied every enabled field of a preset, so it is now the applied one.
    pub(super) fn preset_applied(&self, name: &str) {
        if let Some(window) = self.window() {
            window.global::<AppState>().set_toast_name(name.into());
        }
        self.toast(ToastKind::PresetApplied);
    }

    pub(super) fn render_presets(&self, state: &AppState<'_>, snapshot: &DeviceSnapshot) {
        let last_applied = snapshot.last_applied_preset.as_deref();
        let (on, off) = self.on_off();
        let words = presets::OnOff { on: &on, off: &off };
        state.set_preset_cards(ModelRc::new(VecModel::from(presets::cards(
            snapshot,
            last_applied,
            words,
        ))));
        state.set_presets_need_both_earbuds(presets_use_blocked_settings(
            &snapshot.presets,
            &snapshot.settings,
        ));
        let editing = self.editing_preset.as_deref().unwrap_or_default();
        state.set_editing_preset(editing.into());
        let groups = presets::find(snapshot, editing)
            .map(|preset| presets::editor_groups(snapshot, preset, words))
            .unwrap_or_default();
        state.set_preset_groups(ModelRc::new(VecModel::from(groups)));
    }
}
