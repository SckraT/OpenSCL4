//! The sound modes as one scale from 1 to 11, as android/app/src/main/java/com/oppzippy/openscq30/ui/sound/
//! SoundModeScale.kt: 1 to 5 transparency (1 the strongest), 6 normal, 7 to 11 noise canceling (11 the strongest).

use crate::settings::{Setting, SettingId, Value};

use super::SettingMap;

pub const SCALE_MIN: i32 = 1;
pub const SCALE_NORMAL: i32 = 6;
pub const SCALE_MAX: i32 = 11;

/// The names a device gives its ambient sound modes. Rules compare against these rather than against literals, so a
/// name no device uses cannot be written by accident.
pub mod ambient_mode {
    pub const NOISE_CANCELING: &str = "NoiseCanceling";
    pub const TRANSPARENCY: &str = "Transparency";
    pub const NORMAL: &str = "Normal";

    /// Every name a rule may read. The shared examples must cover each one.
    pub const ALL: &[&str] = &[NOISE_CANCELING, TRANSPARENCY, NORMAL];
}

/// The names a device gives its noise canceling mode. The mode whose strength the user sets is called `Manual` on
/// some devices and `Custom` on others, and both mean the same thing to a client: the device is not choosing.
pub mod noise_canceling_mode {
    pub const MANUAL: &str = "Manual";
    pub const CUSTOM: &str = "Custom";
    pub const ADAPTIVE: &str = "Adaptive";

    /// The names that mean the user sets the strength.
    pub const STRENGTH_IS_SET_BY_USER: &[&str] = &[MANUAL, CUSTOM];

    /// Every name a rule may read. The shared examples must cover each one.
    pub const ALL: &[&str] = &[MANUAL, CUSTOM, ADAPTIVE];
}

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
        && [
            ambient_mode::TRANSPARENCY,
            ambient_mode::NORMAL,
            ambient_mode::NOISE_CANCELING,
        ]
        .iter()
        .all(|mode| options.iter().any(|o| o == mode))
}

/// Whether the adaptive noise canceling level is worth showing. The device picks that level itself, and only while
/// it is in the adaptive mode; in the custom mode it keeps reporting the level it last chose, which never changes on
/// its own. The level also belongs to noise cancelling alone: transparency and the normal mode have no level, and the
/// device keeps reporting whatever it chose while noise cancelling was on. A device that has no mode to be adaptive
/// in keeps showing it, since there the value is all there is.
pub fn shows_adaptive_noise_canceling(settings: &SettingMap) -> bool {
    match settings.get(&SettingId::AmbientSoundMode) {
        Some(Setting::Select { value, .. }) if value != ambient_mode::NOISE_CANCELING => false,
        _ => match settings.get(&SettingId::NoiseCancelingMode) {
            Some(Setting::Select { value, .. }) => value == noise_canceling_mode::ADAPTIVE,
            _ => true,
        },
    }
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
        ambient_mode::TRANSPARENCY => Some(SCALE_NORMAL - strength(SettingId::ManualTransparency)),
        ambient_mode::NORMAL => Some(SCALE_NORMAL),
        ambient_mode::NOISE_CANCELING => {
            Some(SCALE_NORMAL + strength(SettingId::ManualNoiseCanceling))
        }
        _ => None,
    }
}

/// The settings that put the device at a scale position, sent together.
pub fn sound_mode_scale_settings(position: i32) -> Vec<(SettingId, Value)> {
    let value = position.clamp(SCALE_MIN, SCALE_MAX);
    let mode = |name: &'static str| (SettingId::AmbientSoundMode, Value::String(name.into()));
    match value.cmp(&SCALE_NORMAL) {
        std::cmp::Ordering::Less => vec![
            mode(ambient_mode::TRANSPARENCY),
            (
                SettingId::ManualTransparency,
                Value::I32(SCALE_NORMAL - value),
            ),
        ],
        std::cmp::Ordering::Greater => vec![
            mode(ambient_mode::NOISE_CANCELING),
            (
                SettingId::ManualNoiseCanceling,
                Value::I32(value - SCALE_NORMAL),
            ),
        ],
        std::cmp::Ordering::Equal => vec![mode(ambient_mode::NORMAL)],
    }
}

#[cfg(test)]
mod tests {
    use crate::settings::{Range, Select};

    use super::*;

    const MODES: [&str; 4] = [
        ambient_mode::NOISE_CANCELING,
        ambient_mode::TRANSPARENCY,
        ambient_mode::NORMAL,
        "AirplaneMode",
    ];

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
            &device(ambient_mode::NORMAL, 3, 2)
        ));
        // the device offers only the strength of the current mode
        assert!(supports_sound_mode_scale(
            "SoundcoreA3954",
            &without(
                device(ambient_mode::NORMAL, 3, 2),
                SettingId::ManualTransparency
            )
        ));
        assert!(!supports_sound_mode_scale(
            "SoundcoreA3951",
            &device(ambient_mode::NORMAL, 3, 2)
        ));
        let mut no_normal = device(ambient_mode::NORMAL, 3, 2);
        no_normal.insert(
            SettingId::AmbientSoundMode,
            select(
                &[ambient_mode::NOISE_CANCELING, ambient_mode::TRANSPARENCY],
                ambient_mode::NOISE_CANCELING,
            ),
        );
        assert!(!supports_sound_mode_scale("SoundcoreA3954", &no_normal));
    }

    #[test]
    fn the_current_mode_is_a_position_on_the_scale() {
        assert_eq!(
            Some(6),
            sound_mode_scale_value(&device(ambient_mode::NORMAL, 3, 2))
        );
        assert_eq!(
            Some(9),
            sound_mode_scale_value(&device(ambient_mode::NOISE_CANCELING, 3, 2))
        );
        assert_eq!(
            Some(11),
            sound_mode_scale_value(&device(ambient_mode::NOISE_CANCELING, 5, 2))
        );
        assert_eq!(
            Some(4),
            sound_mode_scale_value(&device(ambient_mode::TRANSPARENCY, 3, 2))
        );
        assert_eq!(
            Some(1),
            sound_mode_scale_value(&device(ambient_mode::TRANSPARENCY, 3, 5))
        );
        assert_eq!(None, sound_mode_scale_value(&device("AirplaneMode", 3, 2)));
    }

    #[test]
    fn a_mode_without_its_strength_is_at_the_weakest_position() {
        let nc = without(
            device(ambient_mode::NOISE_CANCELING, 3, 2),
            SettingId::ManualNoiseCanceling,
        );
        let transparency = without(
            device(ambient_mode::TRANSPARENCY, 3, 2),
            SettingId::ManualTransparency,
        );
        assert_eq!(Some(7), sound_mode_scale_value(&nc));
        assert_eq!(Some(5), sound_mode_scale_value(&transparency));
    }

    #[test]
    fn a_position_sets_the_mode_and_its_strength() {
        assert_eq!(
            vec![
                (
                    SettingId::AmbientSoundMode,
                    Value::String(ambient_mode::TRANSPARENCY.into())
                ),
                (SettingId::ManualTransparency, Value::I32(5)),
            ],
            sound_mode_scale_settings(1)
        );
        assert_eq!(
            vec![(
                SettingId::AmbientSoundMode,
                Value::String(ambient_mode::NORMAL.into())
            )],
            sound_mode_scale_settings(6)
        );
        assert_eq!(
            vec![
                (
                    SettingId::AmbientSoundMode,
                    Value::String(ambient_mode::NOISE_CANCELING.into())
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
                ambient_mode::TRANSPARENCY => ambient_mode::TRANSPARENCY,
                ambient_mode::NOISE_CANCELING => ambient_mode::NOISE_CANCELING,
                _ => ambient_mode::NORMAL,
            };
            let values = device(
                mode,
                if mode == ambient_mode::NOISE_CANCELING {
                    strength.unwrap()
                } else {
                    1
                },
                if mode == ambient_mode::TRANSPARENCY {
                    strength.unwrap()
                } else {
                    1
                },
            );
            assert_eq!(Some(position), sound_mode_scale_value(&values));
        }
    }
}
