//! What the equalizer page does: changing bands, choosing and saving profiles, and importing and exporting them.

use std::path::PathBuf;

use openscq30_lib::settings::{ModifiableSelectCommand, SettingId, Value};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use super::{App, with_app};
use crate::{
    Actions, AppState, AppWindow, ToastKind, TransferNote,
    controller::{Command, DeviceSnapshot},
    equalizer::{self as eq_view, Numbers},
    model::equalizer::{TransferProfile, parse_equalizer_value, profiles_json},
};

const EXPORT_FILE_NAME: &str = "openscq30-equalizer-profiles.json";
const MAX_IMPORT_BYTES: u64 = 1024 * 1024;

pub(super) fn install_actions(window: &AppWindow) {
    let actions = window.global::<Actions>();
    actions.on_eq_preset(|value| {
        with_app(|app| {
            let Some(snapshot) = app.snapshot() else {
                return;
            };
            if let Some(value) = eq_view::preset_value(snapshot, &value) {
                app.send(Command::SetSettings(vec![(
                    SettingId::PresetEqualizerProfile,
                    value,
                )]));
            }
        });
    });
    actions.on_eq_expand(|expanded| {
        with_app(|app| {
            app.equalizer.expanded = expanded;
            app.render();
        });
    });
    actions.on_eq_preview(|band, value| with_app(|app| app.preview_band(band, value)));
    actions.on_eq_commit(|band, value| with_app(|app| app.commit_band(band, value)));
    actions.on_eq_type(|band, text| {
        with_app(|app| {
            let Some((equalizer, _, _)) = app.snapshot().and_then(eq_view::equalizer) else {
                return;
            };
            if let Some(value) = parse_equalizer_value(&text, equalizer) {
                app.commit_band(band, i32::from(value));
            }
        });
    });
    actions.on_eq_reset(|| {
        with_app(|app| {
            let Some((_, values, _)) = app.snapshot().and_then(eq_view::equalizer) else {
                return;
            };
            let zeros = vec![0; values.len()];
            app.send_values(zeros);
        });
    });
    actions.on_eq_apply(|name| {
        with_app(|app| {
            app.send(Command::SetSettings(vec![(
                SettingId::CustomEqualizerProfile,
                Value::String(name.to_string().into()),
            )]));
        });
    });
    actions.on_eq_save(|name, replace| with_app(|app| app.save_profile(&name, replace)));
    actions.on_eq_delete(|name| {
        with_app(|app| {
            app.equalizer.pending_toast = Some((ToastKind::ProfileDeleted, 0));
            app.send(Command::SetSettings(vec![(
                SettingId::CustomEqualizerProfile,
                Value::ModifiableSelectCommand(ModifiableSelectCommand::Remove(
                    name.to_string().into(),
                )),
            )]));
        });
    });
    actions.on_eq_export_choose(|name, chosen| {
        with_app(|app| {
            let names = app.snapshot().map(eq_view::saved_names).unwrap_or_default();
            let unchecked = &mut app.equalizer.unchecked;
            match (name.as_str(), chosen) {
                ("", true) => unchecked.clear(),
                ("", false) => *unchecked = names,
                (name, true) => unchecked.retain(|n| n != name),
                (name, false) => unchecked.push(name.to_string()),
            }
            app.render();
        });
    });
    actions.on_eq_export_one(|name| {
        with_app(|app| {
            let profiles: Vec<_> = app
                .saved_profiles()
                .into_iter()
                .filter(|p| p.name == name.as_str())
                .collect();
            save_to_file(profiles);
        });
    });
    actions.on_eq_export_file(|| with_app(|app| save_to_file(app.chosen_for_export())));
    actions.on_eq_export_copy(|| {
        with_app(|app| app.copy(&profiles_json(&app.chosen_for_export())));
    });
    actions.on_eq_import_file(open_file);
    actions.on_eq_import_paste(|| {
        with_app(|app| {
            match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                Ok(text) if !text.trim().is_empty() => app.preview_import(text),
                Ok(_) | Err(arboard::Error::ContentNotAvailable) => {
                    app.transfer_note(TransferNote::ClipboardEmpty, 0);
                }
                Err(err) => {
                    tracing::warn!("could not read the clipboard: {err}");
                    app.transfer_note(TransferNote::ClipboardEmpty, 0);
                }
            }
        });
    });
    actions.on_eq_import(|| with_app(App::import));
    actions.on_eq_migrate(|name| {
        with_app(|app| {
            let Some(snapshot) = app.snapshot() else {
                return;
            };
            let moving: Vec<(String, Vec<i16>)> = eq_view::movable_legacy(&app.legacy, snapshot)
                .into_iter()
                .filter(|(n, _)| name.is_empty() || *n == name.as_str())
                .map(|(n, profile)| (n.clone(), profile.volume_offsets.clone()))
                .collect();
            if moving.is_empty() {
                return;
            }
            app.equalizer.pending_toast = Some((ToastKind::Migrated, moving.len() as i32));
            app.send(Command::MigrateLegacyProfiles(moving));
        });
    });
}

/// Asks where to save the profiles and saves them there, off the window's thread so the window keeps drawing.
fn save_to_file(profiles: Vec<TransferProfile>) {
    if profiles.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_file_name(EXPORT_FILE_NAME)
            .add_filter("JSON", &["json"])
            .save_file()
        else {
            return;
        };
        let saved = std::fs::write(&path, profiles_json(&profiles));
        if let Err(err) = &saved {
            tracing::warn!("could not save {}: {err}", path.display());
        }
        let _ = slint::invoke_from_event_loop(move || {
            with_app(|app| {
                if saved.is_ok() {
                    app.transfer_note(TransferNote::FileSaved, profiles.len() as i32);
                    app.toast(ToastKind::FileSaved);
                } else {
                    app.transfer_note(TransferNote::FileSaveFailed, 0);
                }
            });
        });
    });
}

fn open_file() {
    std::thread::spawn(|| {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .add_filter("*", &["*"])
            .pick_file()
        else {
            return;
        };
        let read = read_profiles_file(&path);
        let _ = slint::invoke_from_event_loop(move || {
            with_app(|app| match read {
                Ok(text) => app.preview_import(text),
                Err(note) => app.transfer_note(note, 0),
            });
        });
    });
}

fn read_profiles_file(path: &PathBuf) -> Result<String, TransferNote> {
    let size = std::fs::metadata(path)
        .map_err(|_| TransferNote::ReadFailed)?
        .len();
    if size > MAX_IMPORT_BYTES {
        return Err(TransferNote::TooLarge);
    }
    std::fs::read_to_string(path).map_err(|err| {
        tracing::warn!("could not read {}: {err}", path.display());
        TransferNote::ReadFailed
    })
}

/// The equalizer page's own state: what is being dragged, and what the page shows apart from the device.
#[derive(Default)]
pub(super) struct EqualizerState {
    /// The values while a band is dragged or was just sent, until the device says what it has.
    draft: Option<Vec<i16>>,
    expanded: bool,
    /// Saved profiles not chosen for export; every new one is chosen.
    unchecked: Vec<String>,
    /// The profiles last selected in the library so it gives their values; sent again only when they change.
    selection_sent: Option<Value>,
    import_text: Option<String>,
    /// A message for once the device has done what was asked.
    pub(super) pending_toast: Option<(ToastKind, i32)>,
    note: Option<(TransferNote, i32)>,
}

impl App {
    fn numbers(&self) -> Numbers {
        Numbers {
            comma: crate::model::languages::find(self.language).is_some_and(|l| l.decimal_comma),
        }
    }

    /// The device answered: the draft gave way to its values, and what waited for it can be said.
    pub(super) fn equalizer_changed(&mut self, snapshot: &DeviceSnapshot) {
        self.equalizer.draft = None;
        if let Some((kind, count)) = self.equalizer.pending_toast.take() {
            if kind == ToastKind::Imported {
                self.equalizer.note = Some((TransferNote::Imported, count));
            }
            self.toast_count(kind, count);
        }
        // keep every saved profile selected for export, so the library gives all their values
        if let Some((id, value)) = eq_view::export_selection(snapshot)
            && self.equalizer.selection_sent.as_ref() != Some(&value)
        {
            self.equalizer.selection_sent = Some(value.clone());
            self.controller
                .send(Command::SetSettings(vec![(id, value)]));
        }
    }

    pub(super) fn equalizer_failed(&mut self) {
        self.equalizer.draft = None;
        self.equalizer.pending_toast = None;
    }

    pub(super) fn equalizer_disconnected(&mut self) {
        self.equalizer = EqualizerState {
            expanded: self.equalizer.expanded,
            ..EqualizerState::default()
        };
    }

    fn saved_profiles(&self) -> Vec<TransferProfile> {
        self.snapshot()
            .map(eq_view::saved_profiles)
            .unwrap_or_default()
    }

    fn chosen_for_export(&self) -> Vec<TransferProfile> {
        self.saved_profiles()
            .into_iter()
            .filter(|p| !self.equalizer.unchecked.contains(&p.name))
            .collect()
    }

    fn values_with(&self, band: i32, value: i32) -> Option<Vec<i16>> {
        let snapshot = self.snapshot()?;
        let (equalizer, _, read_only) = eq_view::equalizer(snapshot)?;
        if read_only {
            return None;
        }
        let mut values = eq_view::shown_values(snapshot, self.equalizer.draft.as_deref()).to_vec();
        let value = i16::try_from(value)
            .ok()?
            .clamp(equalizer.min, equalizer.max);
        *values.get_mut(usize::try_from(band).ok()?)? = value;
        Some(values)
    }

    /// Shows a band where it is dragged without telling the device yet. Only that band's row and the curve change,
    /// so the slider being dragged stays as it is.
    fn preview_band(&mut self, band: i32, value: i32) {
        let Some(values) = self.values_with(band, value) else {
            return;
        };
        let (Some(window), Some(snapshot)) = (self.window(), self.snapshot()) else {
            return;
        };
        let Some((equalizer, _, _)) = eq_view::equalizer(snapshot) else {
            return;
        };
        let state = window.global::<AppState>();
        let index = band as usize;
        let bands = state.get_eq_bands();
        if let (Some(hz), Some(value)) = (equalizer.band_hz.get(index), values.get(index)) {
            bands.set_row_data(
                index,
                eq_view::band(*hz, *value, equalizer.fraction_digits, self.numbers()),
            );
        }
        let (line, area) = eq_view::chart(&values, equalizer.min, equalizer.max);
        state.set_eq_chart_line(line.into());
        state.set_eq_chart_area(area.into());
        self.equalizer.draft = Some(values);
    }

    fn commit_band(&mut self, band: i32, value: i32) {
        if let Some(values) = self.values_with(band, value) {
            self.send_values(values);
        }
    }

    fn send_values(&mut self, values: Vec<i16>) {
        self.equalizer.draft = Some(values.clone());
        self.send(Command::SetSettings(vec![(
            SettingId::VolumeAdjustments,
            Value::I16Vec(values),
        )]));
    }

    fn save_profile(&mut self, name: &str, replace: bool) {
        let Some(window) = self.window() else { return };
        let state = window.global::<AppState>();
        let name = name.trim();
        if name.is_empty() {
            state.set_eq_save_problem(1);
            return;
        }
        let names = self
            .snapshot()
            .map(eq_view::saved_names)
            .unwrap_or_default();
        let existing = eq_view::existing_name(&names, name);
        if let (Some(existing), false) = (existing, replace) {
            state.set_eq_save_existing(existing.into());
            state.set_eq_save_problem(2);
            return;
        }
        let name = existing.unwrap_or(name).to_string();
        state.set_eq_saving(false);
        self.equalizer.unchecked.retain(|n| *n != name);
        self.equalizer.pending_toast = Some((ToastKind::ProfileSaved, 0));
        self.send(Command::SetSettings(vec![(
            SettingId::CustomEqualizerProfile,
            Value::ModifiableSelectCommand(ModifiableSelectCommand::Add(name.into())),
        )]));
    }

    fn preview_import(&mut self, text: String) {
        let Some(snapshot) = self.snapshot() else {
            return;
        };
        let Some(items) = eq_view::import_items(&text, snapshot).filter(|i| !i.is_empty()) else {
            self.transfer_note(TransferNote::NotProfiles, 0);
            return;
        };
        let Some(window) = self.window() else { return };
        let state = window.global::<AppState>();
        state.set_import_blocked(items.iter().any(|i| i.status >= 2));
        state.set_import_items(ModelRc::new(VecModel::from(items)));
        state.set_importing(true);
        self.equalizer.import_text = Some(text);
    }

    fn import(&mut self) {
        let Some(text) = self.equalizer.import_text.take() else {
            return;
        };
        let count = self
            .window()
            .map_or(0, |w| w.global::<AppState>().get_import_items().row_count());
        if let Some(window) = self.window() {
            window.global::<AppState>().set_importing(false);
        }
        self.equalizer.pending_toast = Some((ToastKind::Imported, count as i32));
        self.send(Command::SetSettings(vec![(
            SettingId::ImportCustomEqualizerProfiles,
            Value::String(text.into()),
        )]));
    }

    fn transfer_note(&mut self, note: TransferNote, count: i32) {
        self.equalizer.note = Some((note, count));
        self.render();
    }

    /// The equalizer page.
    pub(super) fn render_equalizer(&self, state: &AppState<'_>, snapshot: &DeviceSnapshot) {
        let numbers = self.numbers();
        let (presets, preset, preset_count) = eq_view::presets(snapshot, self.equalizer.expanded);
        state.set_eq_presets(ModelRc::new(VecModel::from(presets)));
        state.set_eq_preset(preset.into());
        state.set_eq_preset_count(preset_count as i32);
        state.set_eq_expanded(self.equalizer.expanded);

        let equalizer = eq_view::equalizer(snapshot);
        state.set_eq_available(equalizer.is_some());
        if let Some((equalizer, _, read_only)) = equalizer {
            let values = eq_view::shown_values(snapshot, self.equalizer.draft.as_deref());
            state.set_eq_read_only(read_only);
            state.set_eq_bands(ModelRc::new(VecModel::from(eq_view::bands(
                equalizer, values, numbers,
            ))));
            state.set_eq_lines(ModelRc::new(VecModel::from(eq_view::lines(
                equalizer, numbers,
            ))));
            let (line, area) = eq_view::chart(values, equalizer.min, equalizer.max);
            state.set_eq_chart_line(line.into());
            state.set_eq_chart_area(area.into());
            state.set_eq_min_text(
                numbers
                    .signed(equalizer.min, equalizer.fraction_digits)
                    .into(),
            );
            state.set_eq_max_text(
                numbers
                    .signed(equalizer.max, equalizer.fraction_digits)
                    .into(),
            );
        } else {
            state.set_eq_bands(ModelRc::default());
        }

        state.set_has_saved_profiles(
            snapshot
                .settings
                .contains_key(&SettingId::CustomEqualizerProfile),
        );
        let saved = eq_view::saved_profiles(snapshot);
        let profiles = eq_view::profiles(snapshot, &saved, &self.equalizer.unchecked);
        state.set_eq_export_count(profiles.iter().filter(|p| p.export).count() as i32);
        state.set_eq_profiles(ModelRc::new(VecModel::from(profiles)));
        state.set_eq_legacy(ModelRc::new(VecModel::from(eq_view::legacy_items(
            &self.legacy,
            snapshot,
        ))));
        let (note, count) = self.equalizer.note.unwrap_or((TransferNote::None, 0));
        state.set_transfer_note(note);
        state.set_transfer_count(count);
    }
}
