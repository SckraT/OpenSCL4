//! Whether a setting can be changed in the device's current state, as the device reports it.

use super::{
    SettingMap,
    battery::{BOTH_EARBUDS_SETTING_IDS, is_only_one_earbud_connected},
};

/// Why a setting cannot be changed in the current state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvailabilityReason {
    RequiresBothEarbuds,
}

/// What a setting allows in the device's current state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingAvailability {
    Available,
    ReadOnly { reason: AvailabilityReason },
    Unavailable { reason: AvailabilityReason },
}

/// The availability of a setting reported by the device's current state.
pub fn setting_availability(settings: &SettingMap, setting_id: &str) -> SettingAvailability {
    if is_only_one_earbud_connected(settings) {
        if BOTH_EARBUDS_SETTING_IDS.contains(&setting_id) {
            return SettingAvailability::Unavailable {
                reason: AvailabilityReason::RequiresBothEarbuds,
            };
        }
        if setting_id == "ldac" {
            return SettingAvailability::ReadOnly {
                reason: AvailabilityReason::RequiresBothEarbuds,
            };
        }
    }
    SettingAvailability::Available
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Setting, SettingId};

    fn information(value: &str) -> Setting {
        Setting::Information {
            value: value.to_string(),
            translated_value: value.to_string(),
        }
    }

    fn one_earbud() -> SettingMap {
        SettingMap::from([(SettingId::TwsStatus, information("Disconnected"))])
    }

    fn both_earbuds() -> SettingMap {
        SettingMap::from([(SettingId::TwsStatus, information("Connected"))])
    }

    fn with_multipoint(settings: &mut SettingMap, enabled: bool) {
        settings.insert(
            SettingId::DualConnections,
            Setting::Toggle { value: enabled },
        );
    }

    #[test]
    fn ldac_can_be_read_but_not_changed_with_one_earbud() {
        assert_eq!(
            SettingAvailability::ReadOnly {
                reason: AvailabilityReason::RequiresBothEarbuds
            },
            setting_availability(&one_earbud(), "ldac")
        );
    }

    #[test]
    fn equalizer_settings_are_unavailable_with_one_earbud() {
        for id in BOTH_EARBUDS_SETTING_IDS {
            assert_eq!(
                SettingAvailability::Unavailable {
                    reason: AvailabilityReason::RequiresBothEarbuds
                },
                setting_availability(&one_earbud(), id),
                "{id}"
            );
        }
    }

    #[test]
    fn every_other_setting_stays_available_with_one_earbud() {
        for id in [
            "ambientSoundMode",
            "transparencyMode",
            "noiseCanceling",
            "autoPowerOff",
            "wearingDetection",
            "limitHighVolume",
            "easyChat",
            "soundLeakCompensation",
            "dualConnections",
            "leftSinglePress",
            "voicePrompt",
            "findDevice",
            "fitTest",
            "somethingNew",
        ] {
            assert_eq!(
                SettingAvailability::Available,
                setting_availability(&one_earbud(), id),
                "{id}"
            );
        }
    }

    #[test]
    fn everything_is_available_with_both_earbuds() {
        for id in BOTH_EARBUDS_SETTING_IDS.into_iter().chain(["ldac"]) {
            assert_eq!(
                SettingAvailability::Available,
                setting_availability(&both_earbuds(), id),
                "{id}"
            );
        }
    }

    #[test]
    fn models_without_the_tws_status_keep_every_setting_available() {
        let settings = SettingMap::new();
        for id in BOTH_EARBUDS_SETTING_IDS.into_iter().chain(["ldac"]) {
            assert_eq!(
                SettingAvailability::Available,
                setting_availability(&settings, id),
                "{id}"
            );
        }
    }

    #[test]
    fn multipoint_does_not_gate_ldac_or_the_equalizer() {
        for enabled in [false, true] {
            let mut settings = one_earbud();
            with_multipoint(&mut settings, enabled);
            assert_eq!(
                SettingAvailability::ReadOnly {
                    reason: AvailabilityReason::RequiresBothEarbuds
                },
                setting_availability(&settings, "ldac"),
                "multipoint {enabled}"
            );
            for id in BOTH_EARBUDS_SETTING_IDS {
                assert_eq!(
                    SettingAvailability::Unavailable {
                        reason: AvailabilityReason::RequiresBothEarbuds
                    },
                    setting_availability(&settings, id),
                    "{id} with multipoint {enabled}"
                );
            }
        }
    }
}
