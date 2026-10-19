//! The sound modes as one scale from 1 to 11, as android/app/src/main/java/com/oppzippy/openscq30/ui/sound/
//! SoundModeScale.kt: 1 to 5 transparency (1 the strongest), 6 normal, 7 to 11 noise canceling (11 the strongest).

use crate::settings::{Setting, SettingId, Value};

use super::SettingMap;

pub const SCALE_MIN: i32 = 1;
pub const SCALE_NORMAL: i32 = 6;
pub const SCALE_MAX: i32 = 11;

// Models that keep the mode and its strength as one position from 1 to 11, which is what the scale shows. They offer
// only the strength of the current mode, so support can not be told from the settings alone.
const SCALE_MODELS: &[&str] = &["SoundcoreA3954"];

fn ambient(settings: &SettingMap) -> Option<(&[std::borrow::Cow<'static, str>], &str)> {
    match settings.get(&SettingId::AmbientSoundMode) {
        Some(Setting::Select { setting, value }) => Some((&setting.options, value)),
        _ => None,
    }
}

/// Whether the device can be shown on one scale: transparency, normal and noise canceling on one slider.
pub fn supports_sound_mode_scale(model: &str, settings: &SettingMap) -> bool {
    let Some((options, _)) = ambient(settings) else {
        return false;
    };
    SCALE_MODELS.contains(&model)
        && ["Transparency", "Normal", "NoiseCanceling"]
            .iter()
            .all(|mode| options.iter().any(|o| o == mode))
}

/// The scale position of the current mode, or None in other modes such as airplane mode.
pub fn sound_mode_scale_value(settings: &SettingMap) -> Option<i32> {
    let (_, mode) = ambient(settings)?;
    // a mode without a strength is at the weakest one, which is also what the device picks when switching to it
    let strength = |id| match settings.get(&id) {
        Some(Setting::I32Range { value, .. }) => *value,
        _ => 1,
    };
    match mode {
        "Transparency" => Some(SCALE_NORMAL - strength(SettingId::ManualTransparency)),
        "Normal" => Some(SCALE_NORMAL),
        "NoiseCanceling" => Some(SCALE_NORMAL + strength(SettingId::ManualNoiseCanceling)),
        _ => None,
    }
}

/// The settings that put the device at a scale position, sent together.
pub fn sound_mode_scale_settings(position: i32) -> Vec<(SettingId, Value)> {
    let value = position.clamp(SCALE_MIN, SCALE_MAX);
    let mode = |name: &'static str| (SettingId::AmbientSoundMode, Value::String(name.into()));
    match value.cmp(&SCALE_NORMAL) {
        std::cmp::Ordering::Less => vec![
            mode("Transparency"),
            (
                SettingId::ManualTransparency,
                Value::I32(SCALE_NORMAL - value),
            ),
        ],
        std::cmp::Ordering::Greater => vec![
            mode("NoiseCanceling"),
            (
                SettingId::ManualNoiseCanceling,
                Value::I32(value - SCALE_NORMAL),
            ),
        ],
        std::cmp::Ordering::Equal => vec![mode("Normal")],
    }
}

#[cfg(test)]
mod tests {
    use crate::settings::{Range, Select};

    use super::*;

    const MODES: [&str; 4] = ["NoiseCanceling", "Transparency", "Normal", "AirplaneMode"];

    fn select(options: &[&'static str], value: &'static str) -> Setting {
        Setting::Select {
            setting: Select {
                options: options.iter().map(|o| (*o).into()).collect(),
                localized_options: options.iter().map(|o| (*o).to_string()).collect(),
            },
            value: value.into(),
        }
    }

    fn strength(value: i32) -> Setting {
        Setting::I32Range {
            setting: Range {
                range: 1..=5,
                step: 1,
            },
            value,
        }
    }

    fn device(mode: &'static str, noise_canceling: i32, transparency: i32) -> SettingMap {
        SettingMap::from([
            (SettingId::AmbientSoundMode, select(&MODES, mode)),
            (SettingId::ManualNoiseCanceling, strength(noise_canceling)),
            (SettingId::ManualTransparency, strength(transparency)),
        ])
    }

    fn without(mut settings: SettingMap, id: SettingId) -> SettingMap {
        settings.remove(&id);
        settings
    }

    #[test]
    fn the_scale_is_for_models_with_one_slider_and_all_three_modes() {
        assert!(supports_sound_mode_scale(
            "SoundcoreA3954",
            &device("Normal", 3, 2)
        ));
        // the device offers only the strength of the current mode
        assert!(supports_sound_mode_scale(
            "SoundcoreA3954",
            &without(device("Normal", 3, 2), SettingId::ManualTransparency)
        ));
        assert!(!supports_sound_mode_scale(
            "SoundcoreA3951",
            &device("Normal", 3, 2)
        ));
        let mut no_normal = device("Normal", 3, 2);
        no_normal.insert(
            SettingId::AmbientSoundMode,
            select(&["NoiseCanceling", "Transparency"], "NoiseCanceling"),
        );
        assert!(!supports_sound_mode_scale("SoundcoreA3954", &no_normal));
    }

    #[test]
    fn the_current_mode_is_a_position_on_the_scale() {
        assert_eq!(Some(6), sound_mode_scale_value(&device("Normal", 3, 2)));
        assert_eq!(
            Some(9),
            sound_mode_scale_value(&device("NoiseCanceling", 3, 2))
        );
        assert_eq!(
            Some(11),
            sound_mode_scale_value(&device("NoiseCanceling", 5, 2))
        );
        assert_eq!(
            Some(4),
            sound_mode_scale_value(&device("Transparency", 3, 2))
        );
        assert_eq!(
            Some(1),
            sound_mode_scale_value(&device("Transparency", 3, 5))
        );
        assert_eq!(None, sound_mode_scale_value(&device("AirplaneMode", 3, 2)));
    }

    #[test]
    fn a_mode_without_its_strength_is_at_the_weakest_position() {
        let nc = without(
            device("NoiseCanceling", 3, 2),
            SettingId::ManualNoiseCanceling,
        );
        let transparency = without(device("Transparency", 3, 2), SettingId::ManualTransparency);
        assert_eq!(Some(7), sound_mode_scale_value(&nc));
        assert_eq!(Some(5), sound_mode_scale_value(&transparency));
    }

    #[test]
    fn a_position_sets_the_mode_and_its_strength() {
        assert_eq!(
            vec![
                (
                    SettingId::AmbientSoundMode,
                    Value::String("Transparency".into())
                ),
                (SettingId::ManualTransparency, Value::I32(5)),
            ],
            sound_mode_scale_settings(1)
        );
        assert_eq!(
            vec![(SettingId::AmbientSoundMode, Value::String("Normal".into()))],
            sound_mode_scale_settings(6)
        );
        assert_eq!(
            vec![
                (
                    SettingId::AmbientSoundMode,
                    Value::String("NoiseCanceling".into())
                ),
                (SettingId::ManualNoiseCanceling, Value::I32(1)),
            ],
            sound_mode_scale_settings(7)
        );
    }

    #[test]
    fn every_position_reads_back_as_itself() {
        for position in SCALE_MIN..=SCALE_MAX {
            let settings = sound_mode_scale_settings(position);
            let Value::String(mode) = &settings[0].1 else {
                panic!("the mode comes first")
            };
            let strength = settings.get(1).map(|(_, value)| match value {
                Value::I32(value) => *value,
                _ => panic!("the strength is a number"),
            });
            let mode: &'static str = match mode.as_ref() {
                "Transparency" => "Transparency",
                "NoiseCanceling" => "NoiseCanceling",
                _ => "Normal",
            };
            let values = device(
                mode,
                if mode == "NoiseCanceling" {
                    strength.unwrap()
                } else {
                    1
                },
                if mode == "Transparency" {
                    strength.unwrap()
                } else {
                    1
                },
            );
            assert_eq!(Some(position), sound_mode_scale_value(&values));
        }
    }
}
