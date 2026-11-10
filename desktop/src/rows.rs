//! The device's settings as rows on the section pages, and what a change to a row sends to the device. Which page a
//! setting is on comes from model::groups; how a page splits them into panels follows the approved mockup.

use std::{borrow::Cow, str::FromStr};

use openscq30_i18n::Translate;
use openscq30_lib::client::availability::{SettingAvailability, setting_availability};
use openscq30_lib::client::{
    controls::{ControlSide, SettingControl, setting_control},
    sound_mode,
};
use openscq30_lib::settings::{self, MultiSelectWithRemoveCommand, Setting, SettingId, Value};
use slint::{Model, ModelRc, SharedString, VecModel};

use crate::{
    ModeOption, Page, PanelKind, RowKind, SettingRow, SettingsPanel,
    controller::DeviceSnapshot,
    model::sound_mode::{sound_mode_scale_value, supports_sound_mode_scale},
    view,
};

pub const MODE_CYCLE: [SettingId; 3] = [
    SettingId::NoiseCancelingModeInCycle,
    SettingId::TransparencyModeInCycle,
    SettingId::NormalModeInCycle,
];
const LIMIT_HIGH_VOLUME: [&str; 3] = [
    "limitHighVolume",
    "limitHighVolumeDbLimit",
    "limitHighVolumeRefreshRate",
];
/// Shown with the sound mode itself rather than as rows below it.
const WITH_THE_MODE: [&str; 6] = [
    "ambientSoundMode",
    "noiseCancelingMode",
    "transparencyMode",
    "manualNoiseCanceling",
    "manualTransparency",
    "airplaneMode",
];

fn id_of(id: SettingId) -> &'static str {
    id.into()
}

fn strings(items: impl IntoIterator<Item = String>) -> ModelRc<SharedString> {
    ModelRc::new(VecModel::from(
        items
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))
}

/// A row for one setting, or None for settings that only have their own pages, such as the equalizer.
pub fn row(
    id: SettingId,
    whole: &Setting,
    enabled: bool,
    availability: SettingAvailability,
) -> Option<SettingRow> {
    let reason = match availability {
        SettingAvailability::Available => String::new(),
        SettingAvailability::ReadOnly { reason } | SettingAvailability::Unavailable { reason } => {
            reason.translate()
        }
    };
    let mut row = SettingRow {
        id: id_of(id).into(),
        label: id.translate().into(),
        enabled: enabled && matches!(availability, SettingAvailability::Available),
        reason: reason.into(),
        options: strings([]),
        index: -1,
        ..SettingRow::default()
    };
    match whole {
        Setting::Toggle { value } => {
            row.kind = RowKind::Toggle;
            row.checked = *value;
        }
        Setting::I32Range { setting, value } => {
            row.kind = RowKind::Range;
            row.minimum = *setting.range.start();
            row.maximum = *setting.range.end();
            row.step = setting.step.max(1);
            row.value = *value;
        }
        Setting::Select { setting, value } => {
            row.kind = RowKind::Select;
            row.options = strings(setting.localized_options.iter().cloned());
            row.index = setting
                .options
                .iter()
                .position(|o| o == value)
                .map_or(-1, |i| i as i32);
        }
        Setting::OptionalSelect { setting, value } => {
            row.kind = RowKind::Select;
            let none = settings::localize_value(Some(whole), &Value::OptionalString(None));
            row.options =
                strings(std::iter::once(none).chain(setting.localized_options.iter().cloned()));
            row.index = match value {
                None => 0,
                Some(value) => setting
                    .options
                    .iter()
                    .position(|o| o == value)
                    .map_or(-1, |i| i as i32 + 1),
            };
        }
        Setting::Information {
            translated_value, ..
        } => {
            row.kind = RowKind::Info;
            row.text = translated_value.clone().into();
        }
        Setting::Action => row.kind = RowKind::Action,
        Setting::MultiSelectWithRemove { setting, values } => {
            row.kind = RowKind::List;
            row.options = strings(values.iter().map(|value| {
                setting
                    .options
                    .iter()
                    .position(|o| o == value)
                    .and_then(|i| setting.localized_options.get(i).cloned())
                    .unwrap_or_else(|| value.to_string())
            }));
        }
        Setting::MultiSelect { setting, values } => {
            row.kind = RowKind::Info;
            let shown: Vec<String> = values
                .iter()
                .filter_map(|value| setting.options.iter().position(|o| o == value))
                .filter_map(|i| setting.localized_options.get(i).cloned())
                .collect();
            row.text = shown.join(", ").into();
        }
        Setting::HueColorPicker { hue } => {
            row.kind = RowKind::Hue;
            row.minimum = 0;
            row.maximum = 360;
            row.step = 1;
            row.value = hue.round() as i32;
        }
        Setting::TimeOfDay {
            minutes_after_midnight,
        } => {
            row.kind = RowKind::Time;
            row.value = *minutes_after_midnight;
        }
        Setting::Equalizer { .. }
        | Setting::PresetEqualizerProfileSelect { .. }
        | Setting::ModifiableSelect { .. }
        | Setting::ImportString { .. } => return None,
    }
    Some(row)
}

fn rows_for(snapshot: &DeviceSnapshot, ids: &[&'static str]) -> Vec<SettingRow> {
    ids.iter()
        .filter_map(|id| {
            let control = setting_control(&snapshot.settings, id);
            if control == SettingControl::Hidden {
                return None;
            }
            let setting_id = SettingId::from_str(id).ok()?;
            let setting = sound_mode::displayed_setting(&snapshot.settings, setting_id)?;
            row(
                setting_id,
                &setting,
                control == SettingControl::Enabled,
                setting_availability(&snapshot.settings, id),
            )
        })
        .collect()
}

fn panel(kind: PanelKind, rows: Vec<SettingRow>) -> Option<SettingsPanel> {
    (!rows.is_empty()).then(|| SettingsPanel {
        kind,
        rows: ModelRc::new(VecModel::from(rows)),
    })
}

/// Disables every control while the session is recovering; the last values stay on screen.
pub fn read_only(panels: Vec<SettingsPanel>) -> Vec<SettingsPanel> {
    panels
        .into_iter()
        .map(|panel| SettingsPanel {
            kind: panel.kind,
            rows: ModelRc::new(VecModel::from(
                panel
                    .rows
                    .iter()
                    .map(|mut row| {
                        row.enabled = false;
                        row
                    })
                    .collect::<Vec<_>>(),
            )),
        })
        .collect()
}

/// The settings of a page, split into the panels the mockup shows.
pub fn panels(page: Page, snapshot: &DeviceSnapshot, side: ControlSide) -> Vec<SettingsPanel> {
    let groups = view::groups(snapshot);
    let Some(group) = groups
        .iter()
        .find(|(group, _)| view::page_of(*group) == page)
    else {
        return Vec::new();
    };
    let ids: &[&'static str] = &group.1;
    let rest = |taken: &[&str]| -> Vec<&'static str> {
        ids.iter()
            .copied()
            .filter(|id| !taken.contains(id))
            .collect()
    };
    let panels = match page {
        Page::SoundModes => {
            let mut ids = if matches!(
                snapshot.settings.get(&SettingId::AmbientSoundMode),
                Some(Setting::Select { .. })
            ) {
                rest(&WITH_THE_MODE)
            } else {
                ids.to_vec()
            };
            ids.retain(|id| sound_mode::mode_setting_is_visible(&snapshot.settings, id, false));
            vec![panel(PanelKind::Plain, rows_for(snapshot, &ids))]
        }
        Page::Controls => {
            let side_gestures: Vec<_> = side.gestures().iter().copied().map(id_of).collect();
            let side_calls = side
                .call_gestures()
                .iter()
                .copied()
                .map(id_of)
                .collect::<Vec<_>>();
            let mut taken: Vec<_> = ControlSide::ALL
                .into_iter()
                .flat_map(ControlSide::settings)
                .map(id_of)
                .collect();
            let presses: Vec<_> = ids
                .iter()
                .copied()
                .filter(|id| {
                    side_calls.contains(id)
                        || (!taken.contains(id) && (id.contains("Press") || id.contains("Slide")))
                })
                .collect();
            let cycle = MODE_CYCLE.map(id_of);
            taken.extend(&presses);
            taken.extend(cycle);
            taken.push("resetButtonsToDefault");
            vec![
                panel(PanelKind::Gestures, rows_for(snapshot, &side_gestures)),
                panel(PanelKind::Presses, rows_for(snapshot, &presses)),
                panel(PanelKind::Cycle, rows_for(snapshot, &cycle)),
                panel(PanelKind::Plain, rows_for(snapshot, &rest(&taken))),
                panel(
                    PanelKind::Reset,
                    rows_for(snapshot, &["resetButtonsToDefault"]),
                ),
            ]
        }
        Page::Comfort => vec![
            panel(
                PanelKind::Plain,
                rows_for(snapshot, &rest(&LIMIT_HIGH_VOLUME)),
            ),
            panel(PanelKind::Volume, rows_for(snapshot, &LIMIT_HIGH_VOLUME)),
        ],
        Page::Power => vec![
            panel(PanelKind::Plain, rows_for(snapshot, &rest(&["powerOff"]))),
            panel(PanelKind::PowerOff, rows_for(snapshot, &["powerOff"])),
        ],
        Page::Connections => vec![
            panel(
                PanelKind::Plain,
                rows_for(snapshot, &rest(&["dualConnectionsDevices"])),
            ),
            panel(
                PanelKind::DualDevices,
                rows_for(snapshot, &["dualConnectionsDevices"]),
            ),
        ],
        Page::Case => {
            let status: Vec<&'static str> = ids
                .iter()
                .copied()
                .filter(|id| {
                    matches!(
                        snapshot.settings.get(&SettingId::from_str(id).unwrap()),
                        Some(Setting::Information { .. })
                    )
                })
                .collect();
            vec![
                panel(PanelKind::CaseScreen, rows_for(snapshot, &rest(&status))),
                panel(PanelKind::CaseStatus, rows_for(snapshot, &status)),
            ]
        }
        Page::About => vec![panel(PanelKind::Info, rows_for(snapshot, ids))],
        _ => vec![panel(PanelKind::Plain, rows_for(snapshot, ids))],
    };
    panels.into_iter().flatten().collect()
}

/// The sound mode as tiles or as the 1 to 11 scale, and the settings that go with the current mode.
pub struct ModeView {
    pub options: Vec<ModeOption>,
    pub index: i32,
    pub scale: bool,
    /// 0 while the device is in a mode the scale does not show, such as airplane mode.
    pub position: i32,
    pub rows: Vec<SettingRow>,
}

pub fn mode_view(snapshot: &DeviceSnapshot, scale_preferred: bool) -> Option<ModeView> {
    let Some(Setting::Select { setting, value }) =
        snapshot.settings.get(&SettingId::AmbientSoundMode)
    else {
        return None;
    };
    let model: &'static str = snapshot.model.into();
    let scale = scale_preferred && supports_sound_mode_scale(model, &snapshot.settings);
    let options = setting
        .options
        .iter()
        .zip(&setting.localized_options)
        .map(|(value, label)| ModeOption {
            value: value.to_string().into(),
            label: label.clone().into(),
        })
        .collect();
    let ids: Vec<_> = WITH_THE_MODE
        .into_iter()
        .filter(|id| sound_mode::mode_setting_is_visible(&snapshot.settings, id, scale))
        .collect();
    Some(ModeView {
        options,
        index: setting
            .options
            .iter()
            .position(|o| o == value)
            .map_or(-1, |i| i as i32),
        scale,
        position: sound_mode_scale_value(&snapshot.settings)
            .filter(|_| scale)
            .unwrap_or(0),
        rows: rows_for(snapshot, &ids),
    })
}

/// What a change to a row sends to the device.
pub enum Edit {
    Toggle(bool),
    Option(i32),
    Number(i32),
    Press,
    Remove(i32),
    Hue(i32),
    Time(i32),
}

pub fn values(snapshot: &DeviceSnapshot, id: &str, edit: Edit) -> Option<Vec<(SettingId, Value)>> {
    let setting_id = SettingId::from_str(id).ok()?;
    let setting = snapshot.settings.get(&setting_id)?;
    let value = match (setting, edit) {
        (Setting::Toggle { .. }, Edit::Toggle(on)) => Value::Bool(on),
        (Setting::I32Range { setting, .. }, Edit::Number(number)) => {
            Value::I32(number.clamp(*setting.range.start(), *setting.range.end()))
        }
        (Setting::Select { setting, .. }, Edit::Option(index)) => Value::String(Cow::Owned(
            setting
                .options
                .get(usize::try_from(index).ok()?)?
                .to_string(),
        )),
        (Setting::OptionalSelect { setting, .. }, Edit::Option(index)) => match index {
            0 => Value::OptionalString(None),
            index => Value::OptionalString(Some(Cow::Owned(
                setting
                    .options
                    .get(usize::try_from(index - 1).ok()?)?
                    .to_string(),
            ))),
        },
        (Setting::Action, Edit::Press) => Value::Bool(true),
        (Setting::MultiSelectWithRemove { values, .. }, Edit::Remove(index)) => {
            Value::MultiSelectWithRemoveCommand(MultiSelectWithRemoveCommand::Remove(Cow::Owned(
                values.get(usize::try_from(index).ok()?)?.to_string(),
            )))
        }
        (Setting::HueColorPicker { .. }, Edit::Hue(hue)) => Value::F32(hue.clamp(0, 360) as f32),
        (Setting::TimeOfDay { .. }, Edit::Time(minutes)) => Value::I32(minutes.rem_euclid(24 * 60)),
        _ => return None,
    };
    Some(vec![(setting_id, value)])
}

#[cfg(test)]
mod tests {
    use macaddr::MacAddr6;
    use openscq30_lib::DeviceModel;

    use super::*;
    use crate::model::SettingMap;

    fn snapshot(settings: SettingMap) -> DeviceSnapshot {
        DeviceSnapshot {
            mac_address: MacAddr6::nil(),
            model: DeviceModel::SoundcoreA3954,
            is_demo: false,
            categories: Vec::new(),
            settings,
            presets: Vec::new(),
            last_applied_preset: None,
        }
    }

    fn information(value: &str) -> Setting {
        Setting::Information {
            value: value.to_string(),
            translated_value: value.to_string(),
        }
    }

    fn one_earbud_settings() -> SettingMap {
        SettingMap::from([
            (SettingId::TwsStatus, information("Disconnected")),
            (SettingId::Ldac, Setting::Toggle { value: true }),
        ])
    }

    #[test]
    fn adaptive_noise_canceling_shows_automatically_instead_of_a_level() {
        let panels_for_mode = |mode| {
            let mut settings = custom_mode_settings(mode);
            settings.insert(SettingId::AdaptiveNoiseCanceling, information("1/5"));
            let mut snapshot = snapshot(settings);
            snapshot.categories = vec![(
                settings::CategoryId::SoundModes,
                vec![
                    SettingId::NoiseCancelingMode,
                    SettingId::AdaptiveNoiseCanceling,
                ],
            )];
            panels(Page::SoundModes, &snapshot, ControlSide::Left)
        };
        assert!(panels_for_mode("Custom").is_empty());
        let adaptive = panels_for_mode("Adaptive");
        let row = adaptive[0].rows.row_data(0).unwrap();
        assert_eq!(row.id.as_str(), "adaptiveNoiseCanceling");
        assert_eq!(row.text.as_str(), "Automatically");
    }

    /// The sound modes of a device that calls its user-set mode "Custom", as the Space One Pro does, with the
    /// strength the user sets in that mode.
    fn custom_mode_settings(noise_canceling_mode: &'static str) -> SettingMap {
        let modes = |values: Vec<&'static str>| settings::Select {
            options: values
                .iter()
                .map(|value| value.to_string().into())
                .collect(),
            localized_options: values.iter().map(|value| value.to_string()).collect(),
        };
        SettingMap::from([
            (
                SettingId::AmbientSoundMode,
                Setting::Select {
                    setting: modes(vec!["NoiseCanceling", "Transparency", "Normal"]),
                    value: "NoiseCanceling".into(),
                },
            ),
            (
                SettingId::NoiseCancelingMode,
                Setting::Select {
                    setting: modes(vec!["Custom", "Adaptive"]),
                    value: noise_canceling_mode.into(),
                },
            ),
            (SettingId::ManualNoiseCanceling, information("3/5")),
        ])
    }

    fn row_ids(settings: SettingMap) -> Vec<String> {
        mode_view(&snapshot(settings), false)
            .unwrap()
            .rows
            .iter()
            .map(|row| row.id.to_string())
            .collect()
    }

    #[test]
    fn the_custom_noise_canceling_mode_keeps_its_strength() {
        // The Space One Pro calls the mode the user sets "Custom", not "Manual", so matching the name "Manual" alone
        // left it without the strength slider its transparency page still had.
        assert!(
            row_ids(custom_mode_settings("Custom")).contains(&"manualNoiseCanceling".to_string()),
            "{:?}",
            row_ids(custom_mode_settings("Custom"))
        );
    }

    #[test]
    fn the_adaptive_noise_canceling_mode_has_no_strength_to_set() {
        assert!(
            !row_ids(custom_mode_settings("Adaptive"))
                .contains(&"manualNoiseCanceling".to_string()),
            "{:?}",
            row_ids(custom_mode_settings("Adaptive"))
        );
    }

    #[test]
    fn read_only_ldac_keeps_its_value_and_says_why() {
        let snapshot = snapshot(one_earbud_settings());
        let row = row(
            SettingId::Ldac,
            &snapshot.settings[&SettingId::Ldac],
            true,
            setting_availability(&snapshot.settings, "ldac"),
        )
        .unwrap();

        assert!(!row.enabled);
        assert!(row.checked, "the current value stays visible");
        assert_eq!(
            "Changing is available when both earbuds are connected",
            row.reason.as_str(),
        );
    }

    #[test]
    fn available_settings_stay_enabled_without_a_reason() {
        let mut settings = one_earbud_settings();
        settings.insert(SettingId::WearingDetection, Setting::Toggle { value: true });
        let snapshot = snapshot(settings);
        let row = row(
            SettingId::WearingDetection,
            &snapshot.settings[&SettingId::WearingDetection],
            true,
            setting_availability(&snapshot.settings, "wearingDetection"),
        )
        .unwrap();

        assert!(row.enabled);
        assert_eq!("", row.reason.as_str());
    }
}
