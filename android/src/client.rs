use std::{collections::HashMap, str::FromStr};

use openscq30_lib::{
    client::{self, availability, battery, equalizer, groups, presets, quick_toggles, sound_mode},
    settings::SettingId,
};

use crate::{device::SettingIdValuePair, serializable};

fn settings(values: HashMap<String, serializable::Setting>) -> client::SettingMap {
    values
        .into_iter()
        .filter_map(|(id, value)| SettingId::from_str(&id).ok().map(|id| (id, value.0)))
        .collect()
}

#[uniffi::export]
pub fn client_setting_group(id: String, category: Option<String>) -> String {
    format!("{:?}", groups::setting_group(&id, category.as_deref()))
}

#[uniffi::export]
pub fn client_is_control(id: String) -> bool {
    groups::is_control(&id)
}

#[uniffi::export]
pub fn client_sound_mode_setting_ids() -> Vec<String> {
    groups::SOUND_MODE_SETTING_IDS
        .iter()
        .map(|id| (*id).into())
        .collect()
}

#[uniffi::export]
pub fn client_case_setting_ids() -> Vec<String> {
    groups::CASE_SETTING_IDS
        .iter()
        .map(|id| (*id).into())
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

#[uniffi::export]
pub fn client_is_preset_applied(
    preset: serializable::QuickPreset,
    values: HashMap<String, serializable::Setting>,
) -> bool {
    presets::is_preset_applied(&preset.0, &settings(values))
}

#[uniffi::export]
pub fn client_presets_use_both_earbud_settings(values: Vec<serializable::QuickPreset>) -> bool {
    presets::presets_use_both_earbud_settings(
        &values
            .into_iter()
            .map(|preset| preset.0)
            .collect::<Vec<_>>(),
    )
}

#[uniffi::export]
pub fn client_preset_status(
    preset: serializable::QuickPreset,
    values: HashMap<String, serializable::Setting>,
    last_applied: Option<String>,
) -> String {
    format!(
        "{:?}",
        presets::preset_status(&preset.0, &settings(values), last_applied.as_deref())
    )
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
