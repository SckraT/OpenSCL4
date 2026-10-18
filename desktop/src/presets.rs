//! The quick presets page: a card per preset, and the editor of which of its settings it applies.

use openscq30_i18n::Translate;
use openscq30_lib::{
    settings::{Equalizer, Setting, SettingId, Value, localize_value},
    storage::{QuickPreset, QuickPresetField},
};
use slint::{ModelRc, VecModel};

use crate::{
    PresetCard, PresetFieldRow, PresetGroup,
    controller::DeviceSnapshot,
    equalizer::{self as eq_view},
    model::{
        groups::{SettingGroup, setting_group},
        presets::{PresetStatus, preset_fields, preset_status},
    },
    view,
};

/// How many of a preset's settings its card names; the rest it counts.
const NAMED_FIELDS: usize = 3;

/// The words for a setting that is on or off, in the window's language.
#[derive(Clone, Copy)]
pub struct OnOff<'a> {
    pub on: &'a str,
    pub off: &'a str,
}

/// A field's value in words, as the device would show it. A custom curve has none: it is drawn instead.
pub fn value_text(snapshot: &DeviceSnapshot, field: &QuickPresetField, words: OnOff) -> String {
    let setting = snapshot.settings.get(&field.setting_id);
    match (setting, &field.value) {
        _ if field.setting_id == SettingId::VolumeAdjustments => String::new(),
        (_, Value::Bool(on)) => (if *on { words.on } else { words.off }).to_string(),
        (Some(Setting::PresetEqualizerProfileSelect { select, .. }), value) => localize_value(
            Some(&Setting::OptionalSelect {
                setting: select.clone(),
                value: None,
            }),
            value,
        ),
        (setting, value) => localize_value(setting, value),
    }
}

/// The curve a preset puts on the equalizer, if it changes it: its own values, or those of one of the device's
/// profiles.
fn field_curve(field: &QuickPresetField, snapshot: &DeviceSnapshot, eq: &Equalizer) -> String {
    match (field.setting_id, &field.value) {
        (SettingId::VolumeAdjustments, Value::I16Vec(values)) => eq_view::mini_curve(values, eq),
        (SettingId::PresetEqualizerProfile, value) => {
            let Some(Setting::PresetEqualizerProfileSelect {
                select, presets, ..
            }) = snapshot.settings.get(&SettingId::PresetEqualizerProfile)
            else {
                return String::new();
            };
            let chosen = match value {
                Value::OptionalString(Some(name)) | Value::String(name) => name,
                _ => return String::new(),
            };
            select
                .options
                .iter()
                .position(|option| option == chosen)
                .and_then(|index| presets.get(index))
                .map(|values| eq_view::mini_curve(values, eq))
                .unwrap_or_default()
        }
        _ => String::new(),
    }
}

pub fn cards(
    snapshot: &DeviceSnapshot,
    last_applied: Option<&str>,
    words: OnOff,
) -> Vec<PresetCard> {
    let eq = eq_view::equalizer(snapshot).map(|(eq, _, _)| eq);
    snapshot
        .presets
        .iter()
        .map(|preset| {
            let enabled: Vec<_> = preset_fields(preset)
                .into_iter()
                .filter(|f| f.is_enabled)
                .collect();
            let named: Vec<slint::SharedString> = enabled
                .iter()
                .take(NAMED_FIELDS)
                .map(|field| {
                    let value = value_text(snapshot, field, words);
                    let label = field.setting_id.translate();
                    if value.is_empty() {
                        label.into()
                    } else {
                        format!("{label} · {value}").into()
                    }
                })
                .collect();
            let curve = eq
                .and_then(|eq| {
                    enabled
                        .iter()
                        .map(|field| field_curve(field, snapshot, eq))
                        .find(|curve| !curve.is_empty())
                })
                .unwrap_or_default();
            PresetCard {
                name: preset.name.clone().into(),
                status: match preset_status(preset, &snapshot.settings, last_applied) {
                    PresetStatus::NotApplied => 0,
                    PresetStatus::MatchesCurrent => 1,
                    PresetStatus::Applied => 2,
                },
                fields: ModelRc::new(VecModel::from(named)),
                more: enabled.len().saturating_sub(NAMED_FIELDS) as i32,
                enabled_count: enabled.len() as i32,
                curve: curve.into(),
            }
        })
        .collect()
}

pub fn find<'a>(snapshot: &'a DeviceSnapshot, name: &str) -> Option<&'a QuickPreset> {
    snapshot.presets.iter().find(|p| p.name == name)
}

/// The preset's settings by the page they are on, in the sidebar's order and each page's own order. Settings the
/// device doesn't show right now keep their place by their name.
pub fn editor_groups(
    snapshot: &DeviceSnapshot,
    preset: &QuickPreset,
    words: OnOff,
) -> Vec<PresetGroup> {
    let eq = eq_view::equalizer(snapshot).map(|(eq, _, _)| eq);
    let shown = view::groups(snapshot);
    let fields = preset_fields(preset);
    let group_of = |field: &QuickPresetField| {
        let id: &'static str = field.setting_id.into();
        shown
            .iter()
            .find(|(_, ids)| ids.contains(&id))
            .map_or_else(|| setting_group(id, None), |(group, _)| *group)
    };
    let position = |field: &QuickPresetField| {
        let id: &'static str = field.setting_id.into();
        shown
            .iter()
            .flat_map(|(_, ids)| ids.iter())
            .position(|other| *other == id)
            .unwrap_or(usize::MAX)
    };
    SettingGroup::ALL
        .iter()
        .filter_map(|group| {
            let mut in_group: Vec<&QuickPresetField> = fields
                .iter()
                .copied()
                .filter(|field| group_of(field) == *group)
                .collect();
            if in_group.is_empty() {
                return None;
            }
            in_group.sort_by_key(|field| position(field));
            let rows: Vec<PresetFieldRow> = in_group
                .into_iter()
                .map(|field| {
                    let id: &'static str = field.setting_id.into();
                    PresetFieldRow {
                        id: id.into(),
                        label: field.setting_id.translate().into(),
                        value: value_text(snapshot, field, words).into(),
                        enabled: field.is_enabled,
                        curve: match (field.setting_id, eq) {
                            (SettingId::VolumeAdjustments, Some(eq)) => {
                                field_curve(field, snapshot, eq).into()
                            }
                            _ => Default::default(),
                        },
                    }
                })
                .collect();
            Some(PresetGroup {
                page: view::page_of(*group),
                fields: ModelRc::new(VecModel::from(rows)),
            })
        })
        .collect()
}

/// The preset a new one of this name replaces, ignoring case.
pub fn existing_name<'a>(snapshot: &'a DeviceSnapshot, name: &str) -> Option<&'a str> {
    let name = name.to_lowercase();
    snapshot
        .presets
        .iter()
        .find(|p| p.name.to_lowercase() == name)
        .map(|p| p.name.as_str())
}
