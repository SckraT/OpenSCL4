use std::{collections::HashMap, str::FromStr};

use openscq30_lib::{
    client::{
        self, availability, battery,
        controls::{ControlSide as LibControlSide, SettingControl, setting_control},
        device_art, equalizer, groups, presets, quick_toggles, sound_mode,
    },
    settings::SettingId,
};

use crate::{device::SettingIdValuePair, serializable};

fn settings(values: HashMap<String, serializable::Setting>) -> client::SettingMap {
    values
        .into_iter()
        .filter_map(|(id, value)| SettingId::from_str(&id).ok().map(|id| (id, value.0)))
        .collect()
}

/// The earbud a configurable gesture belongs to, mirroring the shared type with generated bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ControlSide {
    Left,
    Right,
}

impl From<ControlSide> for LibControlSide {
    fn from(side: ControlSide) -> Self {
        match side {
            ControlSide::Left => Self::Left,
            ControlSide::Right => Self::Right,
        }
    }
}

#[uniffi::export]
pub fn client_earbud_control_ids(side: ControlSide) -> Vec<String> {
    LibControlSide::from(side)
        .settings()
        .map(|id| id.to_string())
        .collect()
}

/// The illustration of a device model, mirroring the shared type with generated bindings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DeviceArt {
    Headphones,
    MotionPlus,
    OpenClip,
    OpenHook,
    SportHook,
    WingedBuds,
    CompactBuds,
    ClassicStem,
    LibertyStem,
    OpenStem,
}

impl From<device_art::DeviceArt> for DeviceArt {
    fn from(art: device_art::DeviceArt) -> Self {
        match art {
            device_art::DeviceArt::Headphones => Self::Headphones,
            device_art::DeviceArt::MotionPlus => Self::MotionPlus,
            device_art::DeviceArt::OpenClip => Self::OpenClip,
            device_art::DeviceArt::OpenHook => Self::OpenHook,
            device_art::DeviceArt::SportHook => Self::SportHook,
            device_art::DeviceArt::WingedBuds => Self::WingedBuds,
            device_art::DeviceArt::CompactBuds => Self::CompactBuds,
            device_art::DeviceArt::ClassicStem => Self::ClassicStem,
            device_art::DeviceArt::LibertyStem => Self::LibertyStem,
            device_art::DeviceArt::OpenStem => Self::OpenStem,
        }
    }
}

/// What kind of device a model is, for filtering the model list.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum DeviceKind {
    Earbuds,
    OverEar,
    OpenEar,
    Speaker,
}

impl From<device_art::DeviceKind> for DeviceKind {
    fn from(kind: device_art::DeviceKind) -> Self {
        match kind {
            device_art::DeviceKind::Earbuds => Self::Earbuds,
            device_art::DeviceKind::OverEar => Self::OverEar,
            device_art::DeviceKind::OpenEar => Self::OpenEar,
            device_art::DeviceKind::Speaker => Self::Speaker,
        }
    }
}

#[derive(uniffi::Record)]
pub struct ClientDeviceArt {
    pub art: DeviceArt,
    pub kind: DeviceKind,
    pub models: Vec<String>,
}

#[uniffi::export]
pub fn client_device_art_catalog() -> Vec<ClientDeviceArt> {
    device_art::DeviceArt::ALL
        .into_iter()
        .map(|art| ClientDeviceArt {
            art: art.into(),
            kind: art.kind().into(),
            models: art.model_names().collect(),
        })
        .collect()
}

#[derive(uniffi::Record)]
pub struct ClientSettingCategory {
    pub category: String,
    pub ids: Vec<String>,
}

#[derive(uniffi::Record)]
pub struct ClientSettingGroup {
    pub group: String,
    pub ids: Vec<String>,
}

#[uniffi::export]
pub fn client_group_settings(categories: Vec<ClientSettingCategory>) -> Vec<ClientSettingGroup> {
    let entries = categories
        .iter()
        .flat_map(|entry| {
            entry
                .ids
                .iter()
                .map(|id| (id.as_str(), entry.category.as_str()))
        })
        .collect::<Vec<_>>();
    groups::group_settings(&entries)
        .into_iter()
        .map(|(group, ids)| ClientSettingGroup {
            group: format!("{group:?}"),
            ids: ids.into_iter().map(str::to_owned).collect(),
        })
        .collect()
}

#[uniffi::export]
pub fn client_both_earbuds_setting_ids() -> Vec<String> {
    battery::BOTH_EARBUDS_SETTING_IDS
        .iter()
        .map(|id| (*id).into())
        .collect()
}

#[uniffi::export]
pub fn client_equalizer_opens_its_page(ids: Vec<String>, one_earbud_only: bool) -> bool {
    groups::equalizer_opens_its_page(
        &ids.iter().map(String::as_str).collect::<Vec<_>>(),
        one_earbud_only,
    )
}

#[uniffi::export]
pub fn client_sound_mode_scale_limits() -> Vec<i32> {
    vec![
        sound_mode::SCALE_MIN,
        sound_mode::SCALE_NORMAL,
        sound_mode::SCALE_MAX,
    ]
}

#[uniffi::export]
pub fn client_supports_sound_mode_scale(
    model: String,
    values: HashMap<String, serializable::Setting>,
) -> bool {
    sound_mode::supports_sound_mode_scale(&model, &settings(values))
}

/// The settings a client draws, keyed by the id it reports them under. The adaptive noise canceling level reads as
/// the status saying the device adjusts it automatically; a setting with nothing to draw is left out.
#[uniffi::export]
pub fn client_displayed_settings(
    values: HashMap<String, serializable::Setting>,
) -> HashMap<String, serializable::Setting> {
    let mut displayed = HashMap::new();
    let known = values
        .into_iter()
        .filter_map(|(name, setting)| {
            let Ok(id) = SettingId::from_str(&name) else {
                displayed.insert(name, setting);
                return None;
            };
            Some((id, setting.0))
        })
        .collect::<client::SettingMap>();
    displayed.extend(known.keys().filter_map(|id| {
        let setting = sound_mode::displayed_setting(&known, *id)?;
        Some((id.to_string(), serializable::Setting(setting)))
    }));
    displayed
}

/// The settings shown with the sound mode itself, in the order both clients use.
#[uniffi::export]
pub fn client_sound_mode_setting_ids() -> Vec<String> {
    sound_mode::MODE_SETTING_IDS
        .iter()
        .map(ToString::to_string)
        .collect()
}

#[uniffi::export]
pub fn client_visible_sound_mode_settings(
    values: HashMap<String, serializable::Setting>,
    ids: Vec<String>,
    scale: bool,
) -> Vec<String> {
    let values = settings(values);
    ids.into_iter()
        .filter(|id| {
            SettingId::from_str(id)
                .is_ok_and(|id| sound_mode::mode_setting_is_visible(&values, id, scale))
        })
        .collect()
}

#[uniffi::export]
pub fn client_sound_mode_scale_value(
    values: HashMap<String, serializable::Setting>,
) -> Option<i32> {
    sound_mode::sound_mode_scale_value(&settings(values))
}

#[uniffi::export]
pub fn client_sound_mode_scale_settings(position: i32) -> Vec<SettingIdValuePair> {
    sound_mode::sound_mode_scale_settings(position)
        .into_iter()
        .map(|(setting, value)| SettingIdValuePair {
            setting: serializable::SettingId(setting),
            value: serializable::Value(value),
        })
        .collect()
}

#[uniffi::export]
pub fn client_battery_progress(raw: String) -> Option<f32> {
    battery::battery_progress(&raw)
}

#[uniffi::export]
pub fn client_is_tws(values: HashMap<String, serializable::Setting>) -> bool {
    battery::is_tws(&settings(values))
}

#[uniffi::export]
pub fn client_is_only_one_earbud_connected(values: HashMap<String, serializable::Setting>) -> bool {
    battery::is_only_one_earbud_connected(&settings(values))
}

#[derive(Clone, Copy, Debug, uniffi::Enum)]
pub enum ClientSettingAvailability {
    Available,
    ReadOnly {
        reason: serializable::AvailabilityReason,
    },
    Unavailable {
        reason: serializable::AvailabilityReason,
    },
}

impl From<availability::SettingAvailability> for ClientSettingAvailability {
    fn from(availability: availability::SettingAvailability) -> Self {
        match availability {
            availability::SettingAvailability::Available => Self::Available,
            availability::SettingAvailability::ReadOnly { reason } => Self::ReadOnly {
                reason: reason.into(),
            },
            availability::SettingAvailability::Unavailable { reason } => Self::Unavailable {
                reason: reason.into(),
            },
        }
    }
}

#[uniffi::export]
pub fn client_setting_availability(
    values: HashMap<String, serializable::Setting>,
    setting_id: String,
) -> ClientSettingAvailability {
    availability::setting_availability(&settings(values), &setting_id).into()
}

#[derive(Clone, Copy, uniffi::Enum)]
pub enum ClientSettingControl {
    Disabled,
    Hidden,
}

#[derive(uniffi::Record)]
pub struct ClientSettingsPresentation {
    pub availability: HashMap<String, ClientSettingAvailability>,
    /// Editors whose dependencies disable or hide them.
    pub controls: HashMap<String, ClientSettingControl>,
}

/// Presentation for the snapshot, plus requested settings that may be absent from it.
#[uniffi::export]
pub fn client_settings_presentation(
    values: HashMap<String, serializable::Setting>,
    additional_ids: Vec<String>,
) -> ClientSettingsPresentation {
    let ids: Vec<_> = values.keys().cloned().chain(additional_ids).collect();
    let values = settings(values);
    let mut availability = HashMap::with_capacity(ids.len());
    let mut controls = HashMap::new();
    for id in ids {
        let status = availability::setting_availability(&values, &id).into();
        match setting_control(&values, &id) {
            SettingControl::Enabled => {}
            SettingControl::Disabled => {
                controls.insert(id.clone(), ClientSettingControl::Disabled);
            }
            SettingControl::Hidden => {
                controls.insert(id.clone(), ClientSettingControl::Hidden);
            }
        }
        availability.insert(id, status);
    }
    ClientSettingsPresentation {
        availability,
        controls,
    }
}

#[uniffi::export]
pub fn client_presets_use_blocked_settings(
    values: Vec<serializable::QuickPreset>,
    settings_by_id: HashMap<String, serializable::Setting>,
) -> bool {
    presets::presets_use_blocked_settings(
        &values
            .into_iter()
            .map(|preset| preset.0)
            .collect::<Vec<_>>(),
        &settings(settings_by_id),
    )
}

#[uniffi::export]
pub fn client_category_needs_both_earbuds(category: String) -> bool {
    battery::category_needs_both_earbuds(&category)
}

#[derive(uniffi::Enum)]
pub enum ClientBatteryLabel {
    Left,
    Right,
    Case,
    Battery,
}

#[derive(uniffi::Record)]
pub struct ClientBattery {
    pub label: ClientBatteryLabel,
    pub value: String,
    pub progress: Option<f32>,
}

impl From<battery::DeviceBattery> for ClientBattery {
    fn from(value: battery::DeviceBattery) -> Self {
        Self {
            label: match value.label {
                battery::BatteryLabel::Left => ClientBatteryLabel::Left,
                battery::BatteryLabel::Right => ClientBatteryLabel::Right,
                battery::BatteryLabel::Case => ClientBatteryLabel::Case,
                battery::BatteryLabel::Battery => ClientBatteryLabel::Battery,
            },
            value: value.value,
            progress: value.progress,
        }
    }
}

impl From<ClientBattery> for battery::DeviceBattery {
    fn from(value: ClientBattery) -> Self {
        Self {
            label: match value.label {
                ClientBatteryLabel::Left => battery::BatteryLabel::Left,
                ClientBatteryLabel::Right => battery::BatteryLabel::Right,
                ClientBatteryLabel::Case => battery::BatteryLabel::Case,
                ClientBatteryLabel::Battery => battery::BatteryLabel::Battery,
            },
            value: value.value,
            progress: value.progress,
        }
    }
}

#[uniffi::export]
pub fn client_device_batteries(
    values: HashMap<String, serializable::Setting>,
) -> Vec<ClientBattery> {
    battery::device_batteries(&settings(values))
        .into_iter()
        .map(Into::into)
        .collect()
}

#[uniffi::export]
pub fn client_with_missing_earbud(
    batteries: Vec<ClientBattery>,
    one_earbud_only: bool,
) -> Vec<ClientBattery> {
    battery::with_missing_earbud(
        batteries.into_iter().map(Into::into).collect(),
        one_earbud_only,
    )
    .into_iter()
    .map(Into::into)
    .collect()
}

#[derive(uniffi::Record)]
pub struct ClientTransferProfile {
    pub name: String,
    pub volume_adjustments: Vec<f64>,
}

impl From<ClientTransferProfile> for equalizer::TransferProfile {
    fn from(value: ClientTransferProfile) -> Self {
        Self {
            name: value.name,
            volume_adjustments: value.volume_adjustments,
        }
    }
}

#[uniffi::export]
pub fn client_parse_profiles(text: String) -> Option<Vec<ClientTransferProfile>> {
    equalizer::parse_profiles(&text).map(|profiles| {
        profiles
            .into_iter()
            .map(|profile| ClientTransferProfile {
                name: profile.name,
                volume_adjustments: profile.volume_adjustments,
            })
            .collect()
    })
}

#[uniffi::export]
pub fn client_profiles_json(profiles: Vec<ClientTransferProfile>) -> String {
    equalizer::profiles_json(&profiles.into_iter().map(Into::into).collect::<Vec<_>>())
}

#[uniffi::export]
pub fn client_preview_import(
    profiles: Vec<ClientTransferProfile>,
    existing_names: Vec<String>,
    eq: Option<serializable::Equalizer>,
) -> Vec<String> {
    equalizer::preview_import(
        &profiles.into_iter().map(Into::into).collect::<Vec<_>>(),
        &existing_names
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        eq.as_ref().map(|eq| &eq.0),
    )
    .into_iter()
    .map(|status| format!("{status:?}"))
    .collect()
}

#[uniffi::export]
pub fn client_equalizer_device_values(
    profile: ClientTransferProfile,
    fraction_digits: i16,
) -> Vec<i16> {
    equalizer::device_values(&profile.into(), fraction_digits)
}

#[uniffi::export]
pub fn client_preset_fields(
    preset: serializable::QuickPreset,
) -> Vec<serializable::QuickPresetField> {
    presets::preset_fields(&preset.0)
        .into_iter()
        .cloned()
        .map(serializable::QuickPresetField)
        .collect()
}

#[derive(uniffi::Record)]
pub struct ClientPresetSummary {
    pub status: String,
    pub fields: Vec<serializable::QuickPresetField>,
}

#[uniffi::export]
pub fn client_preset_summary(
    preset: serializable::QuickPreset,
    values: HashMap<String, serializable::Setting>,
    last_applied: Option<String>,
) -> ClientPresetSummary {
    let summary = presets::preset_summary(&preset.0, &settings(values), last_applied.as_deref());
    ClientPresetSummary {
        status: format!("{:?}", summary.status),
        fields: summary
            .fields
            .into_iter()
            .cloned()
            .map(serializable::QuickPresetField)
            .collect(),
    }
}

#[uniffi::export]
pub fn client_parse_equalizer_value(
    text: String,
    equalizer: serializable::Equalizer,
) -> Option<i16> {
    equalizer::parse_equalizer_value(&text, &equalizer.0)
}

#[uniffi::export]
pub fn client_format_equalizer_value(value: i16, fraction_digits: i16) -> String {
    equalizer::format_equalizer_value(value, fraction_digits)
}

#[derive(uniffi::Record)]
pub struct ClientFrequency {
    pub value: String,
    pub khz: bool,
}

#[uniffi::export]
pub fn client_equalizer_frequency(hz: u16) -> ClientFrequency {
    let (value, khz) = equalizer::frequency(hz);
    ClientFrequency { value, khz }
}

#[uniffi::export]
pub fn client_quick_toggle_ids(stored: Option<Vec<String>>, available: Vec<String>) -> Vec<String> {
    quick_toggles::quick_toggle_ids(
        stored.as_deref(),
        &available.iter().map(String::as_str).collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art_and_kind(catalog: &[ClientDeviceArt], model: &str) -> Option<(DeviceArt, DeviceKind)> {
        catalog
            .iter()
            .find(|entry| entry.models.iter().any(|name| name == model))
            .map(|entry| (entry.art, entry.kind))
    }

    #[test]
    fn the_catalog_carries_the_shared_illustration_and_kind_as_variants() {
        let catalog = client_device_art_catalog();
        assert_eq!(
            Some((DeviceArt::LibertyStem, DeviceKind::Earbuds)),
            art_and_kind(&catalog, "SoundcoreA3954")
        );
        assert_eq!(
            Some((DeviceArt::Headphones, DeviceKind::OverEar)),
            art_and_kind(&catalog, "SoundcoreA3028")
        );
        assert_eq!(
            Some((DeviceArt::MotionPlus, DeviceKind::Speaker)),
            art_and_kind(&catalog, "SoundcoreA3116")
        );
        assert_eq!(
            Some((DeviceArt::OpenClip, DeviceKind::OpenEar)),
            art_and_kind(&catalog, "SoundcoreA3330")
        );
    }

    #[test]
    fn each_earbud_gets_its_own_control_ids() {
        let left = client_earbud_control_ids(ControlSide::Left);
        let right = client_earbud_control_ids(ControlSide::Right);

        assert!(left.contains(&"leftSinglePress".to_string()));
        assert!(left.contains(&"leftLongPressDuringCall".to_string()));
        assert!(!left.contains(&"rightSinglePress".to_string()));
        assert!(right.contains(&"rightSinglePress".to_string()));
        assert!(right.contains(&"rightLongPressDuringCall".to_string()));
        assert!(!right.contains(&"leftSinglePress".to_string()));
    }

    #[test]
    fn every_illustration_appears_once() {
        let catalog = client_device_art_catalog();
        assert_eq!(device_art::DeviceArt::ALL.len(), catalog.len());
        for art in device_art::DeviceArt::ALL {
            assert_eq!(
                1,
                catalog
                    .iter()
                    .filter(|entry| entry.art == art.into())
                    .count(),
                "{art:?}"
            );
        }
    }
}
