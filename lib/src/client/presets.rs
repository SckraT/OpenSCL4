//! Quick preset states, as android/app/src/main/java/com/oppzippy/openscq30/ui/sound/SoundPresetsPage.kt.

use indexmap::IndexMap;

use crate::{
    settings::{Setting, SettingId},
    storage::{QuickPreset, QuickPresetField},
};

use super::{
    SettingMap,
    availability::{SettingAvailability, setting_availability},
};

/// One field per setting. Old presets can repeat ids shared by categories; activation applies the last enabled
/// value, so that is the one that counts.
pub fn preset_fields(preset: &QuickPreset) -> Vec<&QuickPresetField> {
    let mut ids = Vec::new();
    for field in &preset.fields {
        if !ids.contains(&field.setting_id) {
            ids.push(field.setting_id);
        }
    }
    ids.into_iter()
        .map(|id| {
            let fields: Vec<_> = preset
                .fields
                .iter()
                .filter(|f| f.setting_id == id)
                .collect();
            fields
                .iter()
                .rev()
                .find(|f| f.is_enabled)
                .copied()
                .unwrap_or(fields[fields.len() - 1])
        })
        .collect()
}

/// Fields that will be written, in device write order. Later enabled duplicates replace the value at the first
/// enabled occurrence. Resolve duplicates before choosing between a factory equalizer profile and custom settings.
pub(crate) fn fields_to_apply(
    preset: &QuickPreset,
    mut is_action: impl FnMut(SettingId) -> bool,
) -> Vec<&QuickPresetField> {
    let mut fields: IndexMap<_, _> = preset
        .fields
        .iter()
        .filter(|field| field.is_enabled)
        .map(|field| (field.setting_id, field))
        .collect();
    fields.retain(|id, _| !is_action(*id));
    if fields
        .get(&SettingId::PresetEqualizerProfile)
        .is_some_and(|field| {
            field
                .value
                .try_as_optional_str()
                .is_ok_and(|name| name.is_some())
        })
    {
        // Writing a custom curve or profile would replace the factory profile.
        fields.shift_remove(&SettingId::CustomEqualizerProfile);
        fields.shift_remove(&SettingId::VolumeAdjustments);
    }
    fields.into_values().collect()
}

fn effective_fields<'a>(
    preset: &'a QuickPreset,
    settings: &SettingMap,
) -> Vec<&'a QuickPresetField> {
    fields_to_apply(preset, |id| {
        settings.get(&id).is_some_and(Setting::is_action)
    })
}

fn matches_fields(fields: &[&QuickPresetField], settings: &SettingMap) -> bool {
    !fields.is_empty()
        && fields.iter().all(|field| {
            settings
                .get(&field.setting_id)
                .is_some_and(|setting| setting.matches_value(&field.value))
        })
}

/// Whether the device currently has the values this preset would apply. Several presets can match at once.
pub fn is_preset_applied(preset: &QuickPreset, settings: &SettingMap) -> bool {
    matches_fields(&effective_fields(preset, settings), settings)
}

/// Whether a preset would write fields that cannot be changed in the device's current state.
pub fn preset_uses_blocked_settings(preset: &QuickPreset, settings: &SettingMap) -> bool {
    effective_fields(preset, settings).into_iter().any(|field| {
        let id: &'static str = field.setting_id.into();
        !matches!(
            setting_availability(settings, id),
            SettingAvailability::Available
        )
    })
}

/// Whether any preset has enabled fields that cannot be changed in the device's current state.
pub fn presets_use_blocked_settings(presets: &[QuickPreset], settings: &SettingMap) -> bool {
    presets
        .iter()
        .any(|preset| preset_uses_blocked_settings(preset, settings))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresetStatus {
    /// The preset the user applied last, and the device still has all of its values.
    Applied,
    /// The device has the preset's values, but it wasn't the last one applied.
    MatchesCurrent,
    NotApplied,
}

pub struct PresetSummary<'a> {
    pub fields: Vec<&'a QuickPresetField>,
    pub status: PresetStatus,
}

/// A card describes the fields that activation would write and whether the device has those values.
pub fn preset_summary<'a>(
    preset: &'a QuickPreset,
    settings: &SettingMap,
    last_applied: Option<&str>,
) -> PresetSummary<'a> {
    let fields = effective_fields(preset, settings);
    let status = if !matches_fields(&fields, settings) {
        PresetStatus::NotApplied
    } else if last_applied == Some(preset.name.as_str()) {
        PresetStatus::Applied
    } else {
        PresetStatus::MatchesCurrent
    };
    PresetSummary { fields, status }
}

#[cfg(test)]
mod tests {
    use crate::settings::{Setting, SettingId};

    use super::*;

    const WEAR: SettingId = SettingId::WearingDetection;
    const VOLUME: SettingId = SettingId::LimitHighVolume;
    const SPATIAL: SettingId = SettingId::SpatialAudio;

    fn field(setting_id: SettingId, value: bool, is_enabled: bool) -> QuickPresetField {
        QuickPresetField {
            setting_id,
            value: value.into(),
            is_enabled,
        }
    }

    fn preset(name: &str, fields: Vec<QuickPresetField>) -> QuickPreset {
        QuickPreset {
            name: name.to_string(),
            fields,
        }
    }

    fn toggles(values: &[(SettingId, bool)]) -> SettingMap {
        values
            .iter()
            .map(|(id, value)| (*id, Setting::Toggle { value: *value }))
            .collect()
    }

    #[test]
    fn an_empty_preset_is_not_applied() {
        assert!(!is_preset_applied(
            &preset("Empty", vec![]),
            &SettingMap::new()
        ));
    }

    #[test]
    fn a_preset_with_only_disabled_fields_is_not_applied() {
        let p = preset("Disabled", vec![field(WEAR, true, false)]);
        assert!(!is_preset_applied(&p, &toggles(&[(WEAR, true)])));
    }

    #[test]
    fn an_enabled_field_must_be_available_and_match() {
        let p = preset("Matching", vec![field(WEAR, true, true)]);
        assert!(!is_preset_applied(&p, &SettingMap::new()));
        assert!(!is_preset_applied(&p, &toggles(&[(WEAR, false)])));
        assert!(is_preset_applied(&p, &toggles(&[(WEAR, true)])));
    }

    #[test]
    fn a_disabled_mismatch_does_not_invalidate_an_applied_preset() {
        let p = preset(
            "Partial",
            vec![field(WEAR, true, true), field(VOLUME, false, false)],
        );
        assert!(is_preset_applied(
            &p,
            &toggles(&[(WEAR, true), (VOLUME, true)])
        ));
    }

    #[test]
    fn all_enabled_fields_must_match() {
        let p = preset(
            "Multiple",
            vec![field(WEAR, true, true), field(VOLUME, true, true)],
        );
        assert!(!is_preset_applied(
            &p,
            &toggles(&[(WEAR, true), (VOLUME, false)])
        ));
    }

    #[test]
    fn duplicate_category_fields_give_one_entry() {
        let spatial = field(SPATIAL, true, true);
        let volume = field(VOLUME, false, true);
        let p = preset(
            "Shared categories",
            vec![spatial.clone(), volume.clone(), spatial.clone()],
        );
        assert_eq!(vec![&spatial, &volume], preset_fields(&p));
    }

    #[test]
    fn the_last_enabled_duplicate_is_what_activation_sets() {
        let first = field(SPATIAL, true, true);
        let last = field(SPATIAL, false, true);
        let p = preset("Ordered values", vec![first, last.clone()]);
        assert_eq!(vec![&last], preset_fields(&p));
        assert!(is_preset_applied(&p, &toggles(&[(SPATIAL, false)])));
        assert!(!is_preset_applied(&p, &toggles(&[(SPATIAL, true)])));
    }

    #[test]
    fn a_disabled_duplicate_does_not_override_an_enabled_one() {
        let enabled = field(SPATIAL, true, true);
        let p = preset(
            "Mixed flags",
            vec![enabled.clone(), field(SPATIAL, false, false)],
        );
        assert_eq!(vec![&enabled], preset_fields(&p));
        assert!(is_preset_applied(&p, &toggles(&[(SPATIAL, true)])));
    }

    #[test]
    fn duplicates_without_an_enabled_one_stay_disabled() {
        let last = field(SPATIAL, false, false);
        let p = preset(
            "Disabled duplicates",
            vec![field(SPATIAL, true, false), last.clone()],
        );
        assert_eq!(vec![&last], preset_fields(&p));
        assert!(!is_preset_applied(&p, &toggles(&[(SPATIAL, false)])));
    }

    // two presets that enable different settings can both match the device at the same time
    fn anc() -> QuickPreset {
        preset("ANC", vec![field(WEAR, true, true)])
    }
    fn equalizer() -> QuickPreset {
        preset("Equalizer", vec![field(VOLUME, false, true)])
    }
    fn matching_both() -> SettingMap {
        toggles(&[(WEAR, true), (VOLUME, false)])
    }

    #[test]
    fn only_the_last_applied_preset_is_applied_when_several_match() {
        assert_eq!(
            PresetStatus::Applied,
            preset_summary(&anc(), &matching_both(), Some("ANC")).status
        );
        assert_eq!(
            PresetStatus::MatchesCurrent,
            preset_summary(&equalizer(), &matching_both(), Some("ANC")).status
        );
        assert_eq!(
            PresetStatus::MatchesCurrent,
            preset_summary(&anc(), &matching_both(), None).status
        );
    }

    #[test]
    fn the_last_applied_preset_stops_being_applied_when_one_of_its_settings_changes() {
        let changed = toggles(&[(WEAR, false), (VOLUME, false)]);
        assert_eq!(
            PresetStatus::NotApplied,
            preset_summary(&anc(), &changed, Some("ANC")).status
        );
        assert_eq!(
            PresetStatus::MatchesCurrent,
            preset_summary(&equalizer(), &changed, Some("ANC")).status
        );
        // changing a setting the preset does not use keeps it applied
        let other = toggles(&[(WEAR, true), (VOLUME, true)]);
        assert_eq!(
            PresetStatus::Applied,
            preset_summary(&anc(), &other, Some("ANC")).status
        );
    }

    fn one_earbud() -> SettingMap {
        SettingMap::from([(
            SettingId::TwsStatus,
            Setting::Information {
                value: "Disconnected".to_string(),
                translated_value: "Disconnected".to_string(),
            },
        )])
    }

    #[test]
    fn presets_with_blocked_fields_are_reported_from_their_availability() {
        let ldac = QuickPreset {
            name: "LDAC".to_string(),
            fields: vec![field(SettingId::Ldac, true, true)],
        };
        let eq = QuickPreset {
            name: "EQ".to_string(),
            fields: vec![QuickPresetField {
                setting_id: SettingId::VolumeAdjustments,
                value: vec![0i16; 8].into(),
                is_enabled: true,
            }],
        };
        assert!(presets_use_blocked_settings(
            &[anc(), ldac.clone(), eq.clone()],
            &one_earbud()
        ));
        assert!(!presets_use_blocked_settings(&[anc()], &one_earbud()));
        // disabled fields are not applied, so they do not block the preset
        let disabled = QuickPreset {
            name: "Disabled".to_string(),
            fields: vec![field(SettingId::Ldac, true, false)],
        };
        assert!(!presets_use_blocked_settings(&[disabled], &one_earbud()));
        let both = toggles(&[(WEAR, true)]);
        assert!(!presets_use_blocked_settings(&[ldac, eq], &both));
    }
}
