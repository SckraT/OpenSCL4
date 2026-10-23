//! The overview: the device card, the blocks in the user's order, and the equalizer's curve.

use std::str::FromStr;

use openscq30_i18n::Translate;
use openscq30_lib::client::availability::{SettingAvailability, setting_availability};
use openscq30_lib::settings::{Setting, SettingId, Value};
use slint::{ModelRc, VecModel};

use crate::{
    Battery, BlockKind, LayoutBlock, OverviewBlockData, OverviewRow, Page, PresetItem, QuickToggle,
    controller::DeviceSnapshot,
    model::{
        battery::{BatteryLabel, device_batteries, with_missing_earbud},
        overview::{OverviewBlock, OverviewLayout},
        presets::{PresetStatus, preset_fields, preset_status},
        quick_toggles::quick_toggle_ids,
    },
    view,
};

pub fn batteries(snapshot: &DeviceSnapshot, one_earbud_only: bool) -> Vec<Battery> {
    with_missing_earbud(device_batteries(&snapshot.settings), one_earbud_only)
        .into_iter()
        .map(|battery| Battery {
            label: match battery.label {
                BatteryLabel::Left => 0,
                BatteryLabel::Right => 1,
                BatteryLabel::Case => 2,
                BatteryLabel::Battery => 3,
            },
            value: battery.value.into(),
            progress: battery.progress.unwrap_or_default(),
            known: battery.progress.is_some(),
        })
        .collect()
}

/// The device's on/off settings, which can be quick toggles.
pub fn toggle_ids(snapshot: &DeviceSnapshot) -> Vec<&'static str> {
    let mut ids: Vec<&'static str> = Vec::new();
    for (_, settings) in &snapshot.categories {
        for id in settings {
            if matches!(snapshot.settings.get(id), Some(Setting::Toggle { .. })) {
                let id: &'static str = id.into();
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    ids
}

fn toggle(snapshot: &DeviceSnapshot, id: &str) -> Option<QuickToggle> {
    let setting_id = SettingId::from_str(id).ok()?;
    let Some(Setting::Toggle { value }) = snapshot.settings.get(&setting_id) else {
        return None;
    };
    let (enabled, reason) = match setting_availability(&snapshot.settings, id) {
        SettingAvailability::Available => (true, String::new()),
        SettingAvailability::ReadOnly { reason } | SettingAvailability::Unavailable { reason } => {
            (false, reason.translate())
        }
    };
    Some(QuickToggle {
        id: id.into(),
        label: setting_id.translate().into(),
        on: *value,
        enabled,
        reason: reason.into(),
    })
}

/// The chosen quick toggles, and every toggle with whether it is chosen, for choosing them.
pub fn quick_toggles(
    snapshot: &DeviceSnapshot,
    chosen: Option<&[String]>,
) -> (Vec<QuickToggle>, Vec<QuickToggle>) {
    let available = toggle_ids(snapshot);
    let ids = quick_toggle_ids(chosen, &available);
    let shown = ids.iter().filter_map(|id| toggle(snapshot, id)).collect();
    let all = available
        .iter()
        .filter_map(|id| {
            toggle(snapshot, id).map(|t| QuickToggle {
                on: ids.iter().any(|i| i == id),
                ..t
            })
        })
        .collect();
    (shown, all)
}

fn block_kind(block: OverviewBlock) -> BlockKind {
    match block {
        OverviewBlock::SoundMode => BlockKind::SoundMode,
        OverviewBlock::Equalizer => BlockKind::Equalizer,
        OverviewBlock::QuickPresets => BlockKind::QuickPresets,
        OverviewBlock::Controls => BlockKind::Controls,
        OverviewBlock::Comfort => BlockKind::Comfort,
        OverviewBlock::Spatial => BlockKind::Spatial,
        OverviewBlock::Power => BlockKind::Power,
        OverviewBlock::Connections => BlockKind::Connections,
        OverviewBlock::Case => BlockKind::Case,
    }
}

fn block_page(block: OverviewBlock) -> Page {
    match block {
        OverviewBlock::SoundMode => Page::SoundModes,
        OverviewBlock::Equalizer => Page::Equalizer,
        OverviewBlock::QuickPresets => Page::Presets,
        OverviewBlock::Controls => Page::Controls,
        OverviewBlock::Comfort => Page::Comfort,
        OverviewBlock::Spatial => Page::Effects,
        OverviewBlock::Power => Page::Power,
        OverviewBlock::Connections => Page::Connections,
        OverviewBlock::Case => Page::Case,
    }
}

/// Whether the device has what a block shows.
pub fn supported(
    block: OverviewBlock,
    snapshot: &DeviceSnapshot,
    sound: &[Page],
    device: &[Page],
) -> bool {
    match block {
        OverviewBlock::SoundMode => snapshot.settings.contains_key(&SettingId::AmbientSoundMode),
        OverviewBlock::QuickPresets => true,
        block => sound.contains(&block_page(block)) || device.contains(&block_page(block)),
    }
}

fn is_on(snapshot: &DeviceSnapshot, id: SettingId) -> bool {
    matches!(
        snapshot.settings.get(&id),
        Some(Setting::Toggle { value: true })
    )
}

/// What a shortcut says about its section, in numbers and names the window puts into words.
fn summary(block: OverviewBlock, snapshot: &DeviceSnapshot) -> OverviewBlockData {
    let mut data = OverviewBlockData {
        kind: block_kind(block),
        page: block_page(block),
        ..OverviewBlockData::default()
    };
    let settings = &snapshot.settings;
    match block {
        OverviewBlock::Controls => data.on = settings.contains_key(&SettingId::LeftSinglePress),
        OverviewBlock::Comfort => {
            let groups = view::groups(snapshot);
            let comfort: Vec<SettingId> = groups
                .iter()
                .find(|(group, _)| view::page_of(*group) == Page::Comfort)
                .map(|(_, ids)| {
                    ids.iter()
                        .filter_map(|id| SettingId::from_str(id).ok())
                        .collect()
                })
                .unwrap_or_default();
            let toggles: Vec<SettingId> = comfort
                .iter()
                .copied()
                .filter(|id| matches!(settings.get(id), Some(Setting::Toggle { .. })))
                .collect();
            data.total = toggles.len() as i32;
            data.number = toggles.iter().filter(|id| is_on(snapshot, **id)).count() as i32;
            if let [only] = toggles.as_slice() {
                data.text = only.translate().into();
                data.on = is_on(snapshot, *only);
            } else if toggles.is_empty()
                && let Some(Setting::I32Range { value, .. }) = settings.get(&SettingId::Volume)
            {
                data.number = *value;
            }
        }
        OverviewBlock::Spatial => {
            if settings.contains_key(&SettingId::SpatialAudio) {
                data.text = SettingId::SpatialAudio.translate().into();
                data.on = is_on(snapshot, SettingId::SpatialAudio);
            } else {
                data.text = SettingId::Ldac.translate().into();
                data.on = is_on(snapshot, SettingId::Ldac);
            }
        }
        OverviewBlock::Power => {
            if let Some(setting) = settings.get(&SettingId::AutoPowerOff) {
                let value = Value::from(setting.clone());
                data.text = openscq30_lib::settings::localize_value(Some(setting), &value).into();
            }
        }
        OverviewBlock::Connections => data.on = is_on(snapshot, SettingId::DualConnections),
        OverviewBlock::Case => {
            if let Some(Setting::Information {
                translated_value, ..
            }) = settings.get(&SettingId::CaseBatteryLevel)
            {
                data.text = translated_value.clone().into();
            }
        }
        _ => {}
    }
    data
}

/// The visible blocks in order: the larger blocks two to a row, the shortcuts together.
pub fn rows(
    layout: &OverviewLayout,
    snapshot: &DeviceSnapshot,
    sound: &[Page],
    device: &[Page],
) -> Vec<OverviewRow> {
    let panel = |block: OverviewBlock| {
        matches!(
            block,
            OverviewBlock::SoundMode | OverviewBlock::Equalizer | OverviewBlock::QuickPresets
        )
    };
    let blocks: Vec<OverviewBlock> = layout
        .visible_blocks()
        .into_iter()
        .filter(|block| supported(*block, snapshot, sound, device))
        .collect();
    let mut rows: Vec<(bool, Vec<OverviewBlockData>)> = Vec::new();
    for block in blocks {
        let shortcut = !panel(block);
        match rows.last_mut() {
            Some((is_shortcut, items))
                if *is_shortcut == shortcut && (shortcut || items.len() < 2) =>
            {
                items.push(summary(block, snapshot));
            }
            _ => rows.push((shortcut, vec![summary(block, snapshot)])),
        }
    }
    rows.into_iter()
        .map(|(shortcuts, blocks)| OverviewRow {
            shortcuts,
            blocks: ModelRc::new(VecModel::from(blocks)),
        })
        .collect()
}

pub fn layout_blocks(
    layout: &OverviewLayout,
    snapshot: &DeviceSnapshot,
    sound: &[Page],
    device: &[Page],
) -> Vec<LayoutBlock> {
    layout
        .blocks
        .iter()
        .filter(|(block, _)| supported(*block, snapshot, sound, device))
        .map(|(block, visible)| LayoutBlock {
            kind: block_kind(*block),
            visible: *visible,
        })
        .collect()
}

pub fn presets(
    snapshot: &DeviceSnapshot,
    last_applied: Option<&str>,
    words: crate::presets::OnOff,
) -> Vec<PresetItem> {
    snapshot
        .presets
        .iter()
        .map(|preset| {
            let enabled: Vec<_> = preset_fields(preset)
                .into_iter()
                .filter(|f| f.is_enabled)
                .collect();
            let detail: Vec<String> = enabled
                .iter()
                .take(2)
                .map(|field| crate::presets::value_text(snapshot, field, words))
                .filter(|value| !value.is_empty())
                .collect();
            PresetItem {
                name: preset.name.clone().into(),
                detail: detail.join(" · ").into(),
                status: match preset_status(preset, &snapshot.settings, last_applied) {
                    PresetStatus::NotApplied => 0,
                    PresetStatus::MatchesCurrent => 1,
                    PresetStatus::Applied => 2,
                },
                enabled_count: enabled.len() as i32,
            }
        })
        .collect()
}

/// The equalizer's curve as path commands in a box of the given size, for Slint's Path.
pub fn curve(values: &[i16], min: i16, max: i16, width: f32, height: f32) -> String {
    if values.is_empty() || max <= min {
        return String::new();
    }
    let left = width * 0.04;
    let right = width * 0.96;
    let step = if values.len() > 1 {
        (right - left) / (values.len() - 1) as f32
    } else {
        0.0
    };
    let y = |v: i16| {
        let v = f32::from(v.clamp(min, max));
        height - 4.0 - (v - f32::from(min)) / f32::from(max - min) * (height - 8.0)
    };
    let points: Vec<(f32, f32)> = values
        .iter()
        .enumerate()
        .map(|(i, v)| (left + i as f32 * step, y(*v)))
        .collect();
    let mut path = format!("M {:.1} {:.1}", points[0].0, points[0].1);
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        let mid = (x0 + x1) / 2.0;
        path.push_str(&format!(
            " C {mid:.1} {y0:.1} {mid:.1} {y1:.1} {x1:.1} {y1:.1}"
        ));
    }
    path
}

#[cfg(test)]
mod tests {
    use macaddr::MacAddr6;
    use openscq30_lib::DeviceModel;
    use openscq30_lib::settings::CategoryId;

    use super::*;

    fn snapshot(settings: openscq30_lib::client::SettingMap) -> DeviceSnapshot {
        DeviceSnapshot {
            mac_address: MacAddr6::nil(),
            model: DeviceModel::SoundcoreA3954,
            is_demo: false,
            categories: vec![(CategoryId::Miscellaneous, vec![SettingId::Ldac])],
            settings,
            presets: Vec::new(),
            last_applied_preset: None,
        }
    }

    #[test]
    fn the_curve_runs_through_every_band_inside_the_box() {
        let path = curve(&[-120, 0, 134], -120, 134, 600.0, 100.0);
        assert!(path.starts_with("M 24.0 96.0"), "{path}");
        assert!(path.ends_with("576.0 4.0"), "{path}");
        assert_eq!(2, path.matches(" C ").count());
        assert_eq!("", curve(&[], -6, 6, 600.0, 100.0));
    }

    #[test]
    fn a_read_only_quick_toggle_keeps_its_value_and_says_why() {
        let settings = openscq30_lib::client::SettingMap::from([
            (
                SettingId::TwsStatus,
                Setting::Information {
                    value: "Disconnected".to_string(),
                    translated_value: "Disconnected".to_string(),
                },
            ),
            (SettingId::Ldac, Setting::Toggle { value: true }),
        ]);
        let snapshot = snapshot(settings);
        let chosen = ["ldac".to_string()];
        let (shown, all) = quick_toggles(&snapshot, Some(&chosen));

        let ldac = shown.iter().find(|toggle| toggle.id == "ldac").unwrap();
        assert!(ldac.on, "the current value stays visible");
        assert!(!ldac.enabled);
        assert_eq!(
            "Changing is available when both earbuds are connected",
            ldac.reason.as_str(),
        );
        assert!(
            all.iter()
                .all(|toggle| toggle.id != "ldac" || !toggle.enabled)
        );
    }

    #[test]
    fn an_available_quick_toggle_stays_enabled_without_a_reason() {
        let settings = openscq30_lib::client::SettingMap::from([
            (
                SettingId::TwsStatus,
                Setting::Information {
                    value: "Connected".to_string(),
                    translated_value: "Connected".to_string(),
                },
            ),
            (SettingId::Ldac, Setting::Toggle { value: true }),
        ]);
        let snapshot = snapshot(settings);
        let chosen = ["ldac".to_string()];
        let (shown, _) = quick_toggles(&snapshot, Some(&chosen));

        let ldac = shown.iter().find(|toggle| toggle.id == "ldac").unwrap();
        assert!(ldac.enabled);
        assert_eq!("", ldac.reason.as_str());
    }
}
