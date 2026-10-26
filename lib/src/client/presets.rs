//! Quick preset states, as android/app/src/main/java/com/oppzippy/openscq30/ui/sound/SoundPresetsPage.kt.

use crate::{
    settings::Value,
    storage::{QuickPreset, QuickPresetField},
};

use super::{
    SettingMap,
    availability::{SettingAvailability, setting_availability},
    battery::BOTH_EARBUDS_SETTING_IDS,
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

/// Whether the device currently has the values of the preset's enabled fields. Several presets can match at once,
/// for example when they enable different subsets of the same settings, so this alone doesn't say which is applied.
pub fn is_preset_applied(preset: &QuickPreset, settings: &SettingMap) -> bool {
    let enabled: Vec<_> = preset_fields(preset)
        .into_iter()
        .filter(|f| f.is_enabled)
        .collect();
    !enabled.is_empty()
        && enabled.iter().all(|field| {
            settings
                .get(&field.setting_id)
                .is_some_and(|setting| Value::from(setting.clone()) == field.value)
        })
}

/// Whether any preset would change settings that are only available while both earbuds are connected.
pub fn presets_use_both_earbud_settings(presets: &[QuickPreset]) -> bool {
    presets.iter().any(|preset| {
        preset_fields(preset).iter().any(|field| {
            let id: &'static str = field.setting_id.into();
            field.is_enabled && BOTH_EARBUDS_SETTING_IDS.contains(&id)
        })
    })
}

/// Whether a preset has enabled fields that cannot be changed in the device's current state, and so would not be
/// applied.
pub fn preset_uses_blocked_settings(preset: &QuickPreset, settings: &SettingMap) -> bool {
    preset_fields(preset)
        .into_iter()
        .filter(|field| field.is_enabled)
        .any(|field| {
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

/// Only the last preset the user applied can be applied. It stops being applied as soon as one of its enabled
/// settings changes, since then the device no longer has its values.
pub fn preset_status(
    preset: &QuickPreset,
    settings: &SettingMap,
    last_applied: Option<&str>,
) -> PresetStatus {
    if !is_preset_applied(preset, settings) {
        PresetStatus::NotApplied
    } else if last_applied == Some(preset.name.as_str()) {
        PresetStatus::Applied
    } else {
        PresetStatus::MatchesCurrent
    }
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
            preset_status(&anc(), &matching_both(), Some("ANC"))
        );
        assert_eq!(
            PresetStatus::MatchesCurrent,
            preset_status(&equalizer(), &matching_both(), Some("ANC"))
        );
        assert_eq!(
            PresetStatus::MatchesCurrent,
            preset_status(&anc(), &matching_both(), None)
        );
    }

    #[test]
    fn the_last_applied_preset_stops_being_applied_when_one_of_its_settings_changes() {
        let changed = toggles(&[(WEAR, false), (VOLUME, false)]);
        assert_eq!(
            PresetStatus::NotApplied,
            preset_status(&anc(), &changed, Some("ANC"))
        );
        assert_eq!(
            PresetStatus::MatchesCurrent,
            preset_status(&equalizer(), &changed, Some("ANC"))
        );
        // changing a setting the preset does not use keeps it applied
        let other = toggles(&[(WEAR, true), (VOLUME, true)]);
        assert_eq!(
            PresetStatus::Applied,
            preset_status(&anc(), &other, Some("ANC"))
        );
    }

    #[test]
    fn presets_with_an_enabled_equalizer_field_need_both_earbuds() {
        let eq = QuickPreset {
            name: "EQ".to_string(),
            fields: vec![QuickPresetField {
                setting_id: SettingId::VolumeAdjustments,
                value: vec![0i16; 8].into(),
                is_enabled: true,
            }],
        };
        assert!(presets_use_both_earbud_settings(&[anc(), eq]));
        assert!(!presets_use_both_earbud_settings(&[anc()]));
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
