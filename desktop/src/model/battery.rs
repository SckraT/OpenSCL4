//! Battery levels and the one earbud state, as android/app/src/main/java/com/oppzippy/openscq30/ui/deviceselection/
//! ActiveDevice.kt and ui/sound/SoundOverview.kt read them.

use openscq30_lib::settings::{Setting, SettingId};

use super::SettingMap;

/// Settings the device only exposes while both earbuds are connected.
pub const BOTH_EARBUDS_SETTING_IDS: [&str; 3] = [
    "presetEqualizerProfile",
    "customEqualizerProfile",
    "volumeAdjustments",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatteryLabel {
    Left,
    Right,
    Case,
    Battery,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DeviceBattery {
    pub label: BatteryLabel,
    /// As the library shows it, such as "77%", or a dash for the missing earbud.
    pub value: String,
    /// From 0 to 1, when the reading is a level.
    pub progress: Option<f32>,
}

/// Battery levels of the connected device, in the same order as on the device overview.
pub fn device_batteries(settings: &SettingMap) -> Vec<DeviceBattery> {
    [
        (SettingId::BatteryLevelLeft, BatteryLabel::Left),
        (SettingId::BatteryLevelRight, BatteryLabel::Right),
        (SettingId::CaseBatteryLevel, BatteryLabel::Case),
        (SettingId::BatteryLevel, BatteryLabel::Battery),
    ]
    .into_iter()
    .filter_map(|(id, label)| match settings.get(&id) {
        Some(Setting::Information {
            value,
            translated_value,
        }) => Some(DeviceBattery {
            label,
            value: translated_value.clone(),
            progress: battery_progress(value),
        }),
        _ => None,
    })
    .collect()
}

/// With only one earbud connected, the device reports no level for the other one; show it as a dash in its place so
/// it is clear which earbud is missing.
pub fn with_missing_earbud(
    batteries: Vec<DeviceBattery>,
    one_earbud_only: bool,
) -> Vec<DeviceBattery> {
    if !one_earbud_only {
        return batteries;
    }
    let has = |label| batteries.iter().any(|b| b.label == label);
    let (has_left, has_right) = (has(BatteryLabel::Left), has(BatteryLabel::Right));
    if has_left == has_right {
        return batteries;
    }
    let is_earbud = |b: &DeviceBattery| matches!(b.label, BatteryLabel::Left | BatteryLabel::Right);
    let missing = DeviceBattery {
        label: if has_left {
            BatteryLabel::Right
        } else {
            BatteryLabel::Left
        },
        value: "—".to_string(),
        progress: None,
    };
    let mut earbuds: Vec<_> = batteries
        .iter()
        .filter(|b| is_earbud(b))
        .cloned()
        .chain([missing])
        .collect();
    earbuds.sort_by_key(|b| b.label != BatteryLabel::Left);
    earbuds.extend(batteries.into_iter().filter(|b| !is_earbud(b)));
    earbuds
}

/// Native battery values are fractions, e.g. 83/100 or 7/10 for coarse case readings.
pub fn battery_progress(raw: &str) -> Option<f32> {
    let parts: Vec<&str> = raw.trim().split('/').collect();
    let (level, maximum) = match parts.as_slice() {
        [level, maximum] => (
            level.trim().parse::<f32>().ok()?,
            maximum.trim().parse::<f32>().ok()?,
        ),
        [level] => (
            level
                .trim()
                .trim_end_matches('%')
                .trim()
                .parse::<f32>()
                .ok()?,
            100.0,
        ),
        _ => return None,
    };
    (level.is_finite() && maximum.is_finite() && maximum > 0.0)
        .then(|| (level / maximum).clamp(0.0, 1.0))
}

/// The earbuds report whether they are connected to each other. With one of them in the case, settings that need
/// both, such as the equalizer, are not available.
pub fn is_only_one_earbud_connected(settings: &SettingMap) -> bool {
    matches!(settings.get(&SettingId::TwsStatus), Some(Setting::Information { value, .. }) if value == "Disconnected")
}

/// Whether a settings category is empty while only one earbud is connected because all of it needs both earbuds.
pub fn category_needs_both_earbuds(category_id: &str) -> bool {
    category_id == "equalizer"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn information(raw: &str, shown: &str) -> Setting {
        Setting::Information {
            value: raw.to_string(),
            translated_value: shown.to_string(),
        }
    }

    fn battery(label: BatteryLabel, value: &str, progress: Option<f32>) -> DeviceBattery {
        DeviceBattery {
            label,
            value: value.to_string(),
            progress,
        }
    }

    #[test]
    fn native_battery_fractions_respect_the_device_resolution() {
        let close =
            |raw: &str, expected: f32| (battery_progress(raw).unwrap() - expected).abs() < 0.0001;
        assert!(close("83/100", 0.83));
        assert!(close("7/10", 0.7));
        assert!(close("5/5", 1.0));
        assert!(close("0/100", 0.0));
    }

    #[test]
    fn malformed_readings_do_not_render_an_invented_level() {
        for raw in ["", "unknown", "1/0", "NaN/100", "50/Infinity", "1/2/3"] {
            assert_eq!(None, battery_progress(raw), "{raw}");
        }
    }

    #[test]
    fn batteries_are_read_in_overview_order() {
        let settings = SettingMap::from([
            (SettingId::CaseBatteryLevel, information("8/10", "80%")),
            (SettingId::BatteryLevelLeft, information("77/100", "77%")),
            (SettingId::BatteryLevelRight, information("66/100", "66%")),
        ]);
        let labels: Vec<_> = device_batteries(&settings)
            .into_iter()
            .map(|b| (b.label, b.value))
            .collect();
        assert_eq!(
            vec![
                (BatteryLabel::Left, "77%".to_string()),
                (BatteryLabel::Right, "66%".to_string()),
                (BatteryLabel::Case, "80%".to_string()),
            ],
            labels
        );
    }

    #[test]
    fn the_missing_earbud_is_shown_as_a_dash() {
        let right = battery(BatteryLabel::Right, "66%", Some(0.66));
        let case = battery(BatteryLabel::Case, "80%", Some(0.8));
        assert_eq!(
            vec![
                battery(BatteryLabel::Left, "—", None),
                right.clone(),
                case.clone()
            ],
            with_missing_earbud(vec![right, case], true)
        );
    }

    #[test]
    fn batteries_stay_as_they_are_with_both_earbuds_or_none() {
        let left = battery(BatteryLabel::Left, "77%", Some(0.77));
        let right = battery(BatteryLabel::Right, "66%", Some(0.66));
        let headphones = battery(BatteryLabel::Battery, "50%", Some(0.5));
        assert_eq!(
            vec![right.clone()],
            with_missing_earbud(vec![right.clone()], false)
        );
        assert_eq!(
            vec![left.clone(), right.clone()],
            with_missing_earbud(vec![left, right], true)
        );
        assert_eq!(
            vec![headphones.clone()],
            with_missing_earbud(vec![headphones], true)
        );
    }

    #[test]
    fn one_earbud_is_detected_from_the_tws_status() {
        let status = |value| SettingMap::from([(SettingId::TwsStatus, information(value, value))]);
        assert!(is_only_one_earbud_connected(&status("Disconnected")));
        assert!(!is_only_one_earbud_connected(&status("Connected")));
        // devices without the status are not treated as one earbud
        assert!(!is_only_one_earbud_connected(&SettingMap::new()));
    }

    #[test]
    fn only_the_equalizer_category_needs_both_earbuds() {
        assert!(category_needs_both_earbuds("equalizer"));
        assert!(!category_needs_both_earbuds("equalizerImportExport"));
        assert!(!category_needs_both_earbuds("soundModes"));
    }
}
