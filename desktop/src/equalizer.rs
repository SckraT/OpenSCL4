//! The equalizer page: the device's profiles, its bands on a chart, the saved profiles and moving them in and out.

use std::borrow::Cow;

use openscq30_lib::settings::{Equalizer, Setting, SettingId, Value};

use crate::{
    EqBand, EqLine, EqProfile, ImportItem, ModeOption,
    controller::DeviceSnapshot,
    model::{
        equalizer::{
            ImportStatus, TransferProfile, device_values, format_equalizer_value, frequency,
            parse_profiles, preview_import,
        },
        legacy::LegacyEqualizerProfile,
    },
    overview,
};

/// How many of the device's profiles show before "All profiles".
pub const SHOWN_PRESETS: usize = 8;

/// The chart's path commands are in a box of this size; the window stretches it to the chart.
const CHART_WIDTH: f32 = 600.0;
const CHART_HEIGHT: f32 = 200.0;
/// The part of the chart's height above the highest and below the lowest value, so the points are not cut off.
const CHART_MARGIN: f32 = 0.06;

/// Numbers as the language writes them: a comma for Russian, and a real minus sign.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Numbers {
    pub comma: bool,
}

impl Numbers {
    fn decimal(self, text: &str) -> String {
        if self.comma {
            text.replace('.', ",")
        } else {
            text.to_string()
        }
    }

    /// A value in decibels with all its fraction digits and a sign: "+13,4", "0,0", "−12,0".
    pub fn signed(self, value: i16, fraction_digits: i16) -> String {
        let digits = usize::try_from(fraction_digits).unwrap_or(0);
        let decibels = f64::from(value) / 10f64.powi(i32::from(fraction_digits));
        let text = format!("{:.digits$}", decibels.abs());
        let sign = match value.signum() {
            1 => "+",
            -1 => "\u{2212}",
            _ => "",
        };
        format!("{sign}{}", self.decimal(&text))
    }

    /// A value as the number field shows it: "13,4", "-1,5", "0".
    pub fn field(self, value: i16, fraction_digits: i16) -> String {
        self.decimal(&format_equalizer_value(value, fraction_digits))
    }
}

/// The device's equalizer with its values and whether it can be changed.
pub fn equalizer(snapshot: &DeviceSnapshot) -> Option<(&Equalizer, &[i16], bool)> {
    match snapshot.settings.get(&SettingId::VolumeAdjustments) {
        Some(Setting::Equalizer {
            setting,
            value,
            read_only,
        }) => Some((setting, value, *read_only)),
        _ => None,
    }
}

/// The values the page shows: the ones being dragged, or else the device's.
pub fn shown_values<'a>(snapshot: &'a DeviceSnapshot, draft: Option<&'a [i16]>) -> &'a [i16] {
    draft
        .or_else(|| equalizer(snapshot).map(|(_, values, _)| values))
        .unwrap_or_default()
}

/// The device's own profiles as chips, with the one in effect. Collapsed, only the first few show, and the one in
/// effect if it is further down.
pub fn presets(snapshot: &DeviceSnapshot, expanded: bool) -> (Vec<ModeOption>, String, usize) {
    let (select, value) = match snapshot.settings.get(&SettingId::PresetEqualizerProfile) {
        Some(
            Setting::PresetEqualizerProfileSelect { select, value, .. }
            | Setting::OptionalSelect {
                setting: select,
                value,
            },
        ) => (select, value.clone()),
        Some(Setting::Select { setting, value }) => (setting, Some(value.clone())),
        _ => return (Vec::new(), String::new(), 0),
    };
    let value = value.map(Cow::into_owned).unwrap_or_default();
    let options: Vec<ModeOption> = select
        .options
        .iter()
        .zip(&select.localized_options)
        .enumerate()
        .filter(|(index, (option, _))| expanded || *index < SHOWN_PRESETS || **option == value)
        .map(|(_, (option, label))| ModeOption {
            value: option.to_string().into(),
            label: label.clone().into(),
        })
        .collect();
    (options, value, select.options.len())
}

/// What choosing one of the device's profiles sends.
pub fn preset_value(snapshot: &DeviceSnapshot, value: &str) -> Option<Value> {
    let value = Cow::Owned(value.to_string());
    match snapshot.settings.get(&SettingId::PresetEqualizerProfile)? {
        Setting::Select { .. } => Some(Value::String(value)),
        _ => Some(Value::OptionalString(Some(value))),
    }
}

/// Each band's frequency and value, as the fields under the chart show them.
pub fn bands(equalizer: &Equalizer, values: &[i16], numbers: Numbers) -> Vec<EqBand> {
    equalizer
        .band_hz
        .iter()
        .zip(values)
        .map(|(hz, value)| band(*hz, *value, equalizer.fraction_digits, numbers))
        .collect()
}

pub fn band(hz: u16, value: i16, fraction_digits: i16, numbers: Numbers) -> EqBand {
    let (frequency, khz) = frequency(hz);
    EqBand {
        frequency: numbers.decimal(&frequency).into(),
        khz,
        value: i32::from(value),
        text: numbers.field(value, fraction_digits).into(),
    }
}

/// The chart's guides: the highest and lowest values, half of each, and zero.
pub fn lines(equalizer: &Equalizer, numbers: Numbers) -> Vec<EqLine> {
    let unit = 10f64.powi(i32::from(equalizer.fraction_digits));
    let half = |v: i16| ((f64::from(v) / 2.0 / unit).round() * unit) as i16;
    let mut values = vec![
        equalizer.max,
        half(equalizer.max),
        0,
        half(equalizer.min),
        equalizer.min,
    ];
    values.dedup();
    values
        .into_iter()
        .filter(|v| (equalizer.min..=equalizer.max).contains(v))
        .map(|value| EqLine {
            value: i32::from(value),
            label: numbers.signed(value, equalizer.fraction_digits).into(),
        })
        .collect()
}

/// Where a band's point is on the chart, as fractions of its width and height. The window places the points, the
/// sliders and the fields the same way.
fn point(index: usize, count: usize, value: i16, min: i16, max: i16) -> (f32, f32) {
    let x = (index as f32 + 0.5) / count.max(1) as f32;
    let fraction = f32::from(value.clamp(min, max) - min) / f32::from((max - min).max(1));
    let y = CHART_MARGIN + (1.0 - fraction) * (1.0 - 2.0 * CHART_MARGIN);
    (x, y)
}

/// The curve through the bands, and the area under it, as path commands for the chart.
pub fn chart(values: &[i16], min: i16, max: i16) -> (String, String) {
    if values.is_empty() || max <= min {
        return (String::new(), String::new());
    }
    let points: Vec<(f32, f32)> = (0..values.len())
        .map(|i| {
            let (x, y) = point(i, values.len(), values[i], min, max);
            (x * CHART_WIDTH, y * CHART_HEIGHT)
        })
        .collect();
    let mut line = format!("M {:.1} {:.1}", points[0].0, points[0].1);
    for pair in points.windows(2) {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        let mid = (x0 + x1) / 2.0;
        line.push_str(&format!(
            " C {mid:.1} {y0:.1} {mid:.1} {y1:.1} {x1:.1} {y1:.1}"
        ));
    }
    let first = points[0].0;
    let last = points[points.len() - 1].0;
    let area = format!("{line} L {last:.1} {CHART_HEIGHT:.1} L {first:.1} {CHART_HEIGHT:.1} Z");
    (line, area)
}

/// A profile's curve drawn small beside its name. It fits the curve around 0 dB with at least ±2 dB, since across the
/// device's whole range most profiles would look flat.
pub fn mini_curve(values: &[i16], equalizer: &Equalizer) -> String {
    if values.len() < 2 || equalizer.min >= equalizer.max {
        return String::new();
    }
    let clamped: Vec<i16> = values
        .iter()
        .map(|v| (*v).clamp(equalizer.min, equalizer.max))
        .collect();
    let minimum_span = (2.0 * 10f64.powi(i32::from(equalizer.fraction_digits))) as i16;
    let span = clamped
        .iter()
        .map(|v| v.saturating_abs())
        .max()
        .unwrap_or(0)
        .max(minimum_span);
    overview::curve(&clamped, -span, span, 600.0, 100.0)
}

/// The names of the saved profiles, in the device's order.
pub fn saved_names(snapshot: &DeviceSnapshot) -> Vec<String> {
    match snapshot.settings.get(&SettingId::CustomEqualizerProfile) {
        Some(Setting::ModifiableSelect { setting, .. }) => {
            setting.options.iter().map(ToString::to_string).collect()
        }
        _ => Vec::new(),
    }
}

/// The saved profiles with their values. The library only gives the values of the profiles selected for export, so
/// the app keeps all of them selected there (see `export_selection`) and picks what to export itself.
pub fn saved_profiles(snapshot: &DeviceSnapshot) -> Vec<TransferProfile> {
    let exported = match snapshot
        .settings
        .get(&SettingId::ExportCustomEqualizerProfilesOutput)
    {
        Some(Setting::Information { value, .. }) => parse_profiles(value).unwrap_or_default(),
        _ => Vec::new(),
    };
    saved_names(snapshot)
        .into_iter()
        .map(|name| {
            exported
                .iter()
                .find(|p| p.name == name)
                .cloned()
                .unwrap_or(TransferProfile {
                    name,
                    volume_adjustments: Vec::new(),
                })
        })
        .collect()
}

/// What selects every saved profile for export, when some are not.
pub fn export_selection(snapshot: &DeviceSnapshot) -> Option<(SettingId, Value)> {
    let Some(Setting::MultiSelect { setting, values }) = snapshot
        .settings
        .get(&SettingId::ExportCustomEqualizerProfiles)
    else {
        return None;
    };
    let all_selected = setting.options.iter().all(|o| values.contains(o));
    (!all_selected).then(|| {
        (
            SettingId::ExportCustomEqualizerProfiles,
            Value::StringVec(setting.options.clone()),
        )
    })
}

/// The saved profile in effect: the device plays none of its own profiles and its values are the saved ones.
fn profile_in_effect(snapshot: &DeviceSnapshot) -> Option<String> {
    let preset_chosen = matches!(
        snapshot.settings.get(&SettingId::PresetEqualizerProfile),
        Some(
            Setting::PresetEqualizerProfileSelect { value: Some(_), .. }
                | Setting::OptionalSelect { value: Some(_), .. }
        )
    );
    match snapshot.settings.get(&SettingId::CustomEqualizerProfile) {
        Some(Setting::ModifiableSelect {
            value: Some(value), ..
        }) if !preset_chosen => Some(value.to_string()),
        _ => None,
    }
}

pub fn profiles(
    snapshot: &DeviceSnapshot,
    saved: &[TransferProfile],
    unchecked: &[String],
) -> Vec<EqProfile> {
    let in_effect = profile_in_effect(snapshot);
    let equalizer = equalizer(snapshot).map(|(eq, _, _)| eq);
    saved
        .iter()
        .map(|profile| {
            let values = equalizer.map(|eq| device_values(profile, eq.fraction_digits));
            EqProfile {
                name: profile.name.clone().into(),
                curve: match (equalizer, &values) {
                    (Some(eq), Some(values)) => mini_curve(values, eq).into(),
                    _ => Default::default(),
                },
                bands: profile.volume_adjustments.len() as i32,
                in_effect: in_effect.as_deref() == Some(profile.name.as_str()),
                export: !unchecked.contains(&profile.name),
            }
        })
        .collect()
}

/// The saved profile a new one of this name replaces, ignoring case as the mockup does.
pub fn existing_name<'a>(names: &'a [String], name: &str) -> Option<&'a str> {
    let name = name.to_lowercase();
    names
        .iter()
        .find(|n| n.to_lowercase() == name)
        .map(String::as_str)
}

/// The old profiles this device can take: not moved yet, and with as many bands as it has, since the library turns
/// down values for another number of bands.
pub fn movable_legacy<'a>(
    legacy: &'a [(String, LegacyEqualizerProfile)],
    snapshot: &DeviceSnapshot,
) -> Vec<&'a (String, LegacyEqualizerProfile)> {
    let Some((eq, _, _)) = equalizer(snapshot) else {
        return Vec::new();
    };
    if !snapshot
        .settings
        .contains_key(&SettingId::CustomEqualizerProfile)
    {
        return Vec::new();
    }
    let names = saved_names(snapshot);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    crate::model::equalizer::unmigrated(legacy, &names)
        .into_iter()
        .filter(|(_, profile)| profile.volume_offsets.len() == eq.band_hz.len())
        .collect()
}

/// The old profiles to move, drawn on this device's equalizer.
pub fn legacy_items(
    legacy: &[(String, LegacyEqualizerProfile)],
    snapshot: &DeviceSnapshot,
) -> Vec<EqProfile> {
    let Some((eq, _, _)) = equalizer(snapshot) else {
        return Vec::new();
    };
    movable_legacy(legacy, snapshot)
        .into_iter()
        .map(|(name, profile)| EqProfile {
            name: name.clone().into(),
            curve: mini_curve(&profile.volume_offsets, eq).into(),
            bands: profile.volume_offsets.len() as i32,
            in_effect: false,
            export: false,
        })
        .collect()
}

/// What importing the text would do with each profile; None when it is not a profiles file.
pub fn import_items(text: &str, snapshot: &DeviceSnapshot) -> Option<Vec<ImportItem>> {
    let profiles = parse_profiles(text)?;
    let names = saved_names(snapshot);
    let names: Vec<&str> = names.iter().map(String::as_str).collect();
    let equalizer = equalizer(snapshot).map(|(eq, _, _)| eq);
    let statuses = preview_import(&profiles, &names, equalizer);
    Some(
        profiles
            .iter()
            .zip(statuses)
            .map(|(profile, status)| ImportItem {
                name: profile.name.clone().into(),
                curve: equalizer
                    .filter(|_| status.is_valid())
                    .map(|eq| mini_curve(&device_values(profile, eq.fraction_digits), eq))
                    .unwrap_or_default()
                    .into(),
                status: match status {
                    ImportStatus::New => 0,
                    ImportStatus::Replaces => 1,
                    ImportStatus::WrongBandCount => 2,
                    ImportStatus::OutOfRange => 3,
                },
                bands: profile.volume_adjustments.len() as i32,
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn equalizer() -> Equalizer {
        Equalizer {
            band_hz: vec![100, 200, 400, 800, 1600, 3200, 6400, 12800].into(),
            fraction_digits: 1,
            min: -120,
            max: 134,
        }
    }

    #[test]
    fn numbers_follow_the_language() {
        let ru = Numbers { comma: true };
        let en = Numbers::default();
        assert_eq!("+13,4", ru.signed(134, 1));
        assert_eq!("\u{2212}12.0", en.signed(-120, 1));
        assert_eq!("0.0", en.signed(0, 1));
        assert_eq!("-1,5", ru.field(-15, 1));
        assert_eq!("0", ru.field(0, 1));
        let band = band(1600, 25, 1, ru);
        assert_eq!(
            ("1,6", true, "2,5"),
            (band.frequency.as_str(), band.khz, band.text.as_str())
        );
    }

    #[test]
    fn the_guides_are_the_ends_their_halves_and_zero() {
        let values: Vec<i32> = lines(&equalizer(), Numbers::default())
            .iter()
            .map(|l| l.value)
            .collect();
        assert_eq!(vec![134, 70, 0, -60, -120], values);
    }

    #[test]
    fn the_chart_puts_each_band_over_its_column() {
        let (line, area) = chart(&[-120, 134], -120, 134);
        // two columns: their middles are at a quarter and three quarters of the width
        assert!(line.starts_with("M 150.0 188.0"), "{line}");
        assert!(line.ends_with("450.0 12.0"), "{line}");
        assert!(area.ends_with("L 450.0 200.0 L 150.0 200.0 Z"), "{area}");
        assert_eq!((String::new(), String::new()), chart(&[], -6, 6));
    }

    #[test]
    fn small_curves_keep_at_least_two_decibels_each_way() {
        let flat_ish = mini_curve(&[0, 10, 0, 0, 0, 0, 0, 0], &equalizer());
        let steep = mini_curve(&[0, 100, 0, 0, 0, 0, 0, 0], &equalizer());
        // 1 dB of 2 dB reaches a quarter of the height above the middle; 10 dB of 10 dB reaches the top
        assert!(flat_ish.contains(" 27.0 "), "{flat_ish}");
        assert!(steep.contains(" 4.0 "), "{steep}");
        assert_eq!("", mini_curve(&[0], &equalizer()));
    }

    #[test]
    fn a_new_name_replaces_a_saved_one_whatever_its_case() {
        let names = vec!["Evening".to_string()];
        assert_eq!(Some("Evening"), existing_name(&names, "evening"));
        assert_eq!(None, existing_name(&names, "Morning"));
    }
}
