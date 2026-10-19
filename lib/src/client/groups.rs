//! The device's settings by what they are for, as the Android app groups them
//! (android/app/src/main/java/com/oppzippy/openscq30/ui/sound/SettingGroups.kt and SoundCapabilities.kt). Every
//! setting belongs to exactly one group, whose page shows it.

use super::battery::BOTH_EARBUDS_SETTING_IDS;

/// The groups in the order the app lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SettingGroup {
    SoundModes,
    Equalizer,
    SoundEffects,
    Controls,
    Comfort,
    Power,
    Connections,
    Case,
    Sleep,
    Lights,
    Alarms,
    About,
    Other,
}

impl SettingGroup {
    pub const ALL: [Self; 13] = [
        Self::SoundModes,
        Self::Equalizer,
        Self::SoundEffects,
        Self::Controls,
        Self::Comfort,
        Self::Power,
        Self::Connections,
        Self::Case,
        Self::Sleep,
        Self::Lights,
        Self::Alarms,
        Self::About,
        Self::Other,
    ];
}

/// Sound mode settings, which the sound mode page shows together.
pub const SOUND_MODE_SETTING_IDS: &[&str] = &[
    "ambientSoundMode",
    "transparencyMode",
    "noiseCancelingMode",
    "customNoiseCanceling",
    "adaptiveNoiseCanceling",
    "manualNoiseCanceling",
    "manualTransparency",
    "airplaneMode",
    "windNoiseSuppression",
    "windNoiseDetected",
    "adaptiveNoiseCancelingSensitivityLevel",
    "multiSceneNoiseCanceling",
    "transportationMode",
    "environmentDetection",
    "realTimeAdaptiveNoiseCanceling",
    "ancPersonalizedToEarCanal",
];

/// What the case shows and does, including the case's own information.
pub const CASE_SETTING_IDS: &[&str] = &[
    "caseBatteryLevel",
    "caseLanguage",
    "caseSerialNumber",
    "caseFirmwareVersion",
    "atmospheric",
    "remoteCamera",
    "findDevice",
    "caseSpatialAudio",
    "airPressure",
];

/// Settings that belong somewhere other than their library category says.
fn group_by_id(id: &str) -> Option<SettingGroup> {
    if SOUND_MODE_SETTING_IDS.contains(&id) {
        return Some(SettingGroup::SoundModes);
    }
    if CASE_SETTING_IDS.contains(&id) {
        return Some(SettingGroup::Case);
    }
    Some(match id {
        // the sleep earbuds turn noise canceling on and off with a toggle of its own
        "noiseCanceling" => SettingGroup::SoundModes,
        // how the sound gets to the earbuds and what is done to it
        "ldac"
        | "gamingMode"
        | "spatialAudio"
        | "spatialAudioMode"
        | "spatialAudioMusicMode"
        | "immersiveExperience"
        | "dolbyAudio"
        | "surroundSound" => SettingGroup::SoundEffects,
        // feedback for, and a lock on, the touch controls
        "touchTone" | "touchLock" => SettingGroup::Controls,
        // wearing the earbuds, talking with them in and how loud they get
        "wearingDetection"
        | "autoPlayPause"
        | "wearingTone"
        | "easyChat"
        | "easyChatWaitTime"
        | "sideTone"
        | "soundLeakCompensation"
        | "volumeBalance"
        | "volume" => SettingGroup::Comfort,
        "autoPowerOff"
        | "autoPowerOffPrompt"
        | "powerOff"
        | "lowBatteryPrompt"
        | "voicePrompt"
        | "listeningModePrompt"
        | "noiseCancelingPrompt" => SettingGroup::Power,
        "dualConnections" | "dualConnectionsDevices" => SettingGroup::Connections,
        "autoStopTimer"
        | "autoStopTimerDuration"
        | "autoSwitchOnceAsleep"
        | "listeningMode"
        | "defaultListeningMode"
        | "incomingCallsDuringBluetoothMode" => SettingGroup::Sleep,
        _ => return None,
    })
}

/// Button actions, the mode cycle and resetting them.
pub fn is_control(id: &str) -> bool {
    id.contains("Press")
        || id.contains("Slide")
        || matches!(
            id,
            "singlePress"
                | "doublePress"
                | "buttonsEnabled"
                | "resetButtonsToDefault"
                | "normalModeInCycle"
                | "transparencyModeInCycle"
                | "noiseCancelingModeInCycle"
        )
}

/// The group of a setting: by what the setting is first, then by its library category. Going by the setting first
/// also keeps the Liberty 4 Pro's spatial audio, which the library lists under its case too, off the case page.
pub fn setting_group(id: &str, category: Option<&str>) -> SettingGroup {
    if let Some(group) = group_by_id(id) {
        return group;
    }
    if is_control(id) {
        return SettingGroup::Controls;
    }
    match category {
        Some("soundModes") => SettingGroup::SoundModes,
        Some("equalizer" | "equalizerImportExport") => SettingGroup::Equalizer,
        Some("buttonConfiguration") => SettingGroup::Controls,
        Some("limitHighVolume") => SettingGroup::Comfort,
        Some("dualConnections") => SettingGroup::Connections,
        Some("case") => SettingGroup::Case,
        Some("lights") => SettingGroup::Lights,
        Some("alarms") => SettingGroup::Alarms,
        Some("deviceInformation") => SettingGroup::About,
        _ => SettingGroup::Other,
    }
}

/// The device's settings by group, each once and in the device's order; groups without settings are left out.
/// `settings` lists each setting with the category it was found in, so a setting in two categories comes twice.
pub fn group_settings<'a>(settings: &[(&'a str, &str)]) -> Vec<(SettingGroup, Vec<&'a str>)> {
    // as on Android, a setting listed in several categories goes by the last of them
    let category_of = |id: &str| {
        settings
            .iter()
            .rev()
            .find(|(other, _)| *other == id)
            .map(|(_, c)| *c)
    };
    let mut seen = Vec::new();
    let mut grouped: Vec<(SettingGroup, Vec<&'a str>)> = Vec::new();
    for (id, _) in settings {
        if seen.contains(id) {
            continue;
        }
        seen.push(*id);
        let group = setting_group(id, category_of(id));
        match grouped.iter_mut().find(|(g, _)| *g == group) {
            Some((_, ids)) => ids.push(id),
            None => grouped.push((group, vec![id])),
        }
    }
    grouped.sort_by_key(|(group, _)| *group);
    grouped
}

/// Whether the equalizer opens the equalizer page, which shows only some kinds of equalizer settings; a device with
/// others gets the plain list of them. While only one earbud is connected the device offers no equalizer settings,
/// but the page still says why, and saved profiles can still be imported and exported there.
pub fn equalizer_opens_its_page(category_setting_ids: &[&str], one_earbud_only: bool) -> bool {
    if category_setting_ids.is_empty() {
        one_earbud_only
    } else {
        category_setting_ids
            .iter()
            .all(|id| BOTH_EARBUDS_SETTING_IDS.contains(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group_of(id: &str) -> SettingGroup {
        setting_group(id, Some("miscellaneous"))
    }

    #[test]
    fn the_liberty_four_pro_miscellaneous_settings_all_find_a_group() {
        let expected = [
            ("ldac", SettingGroup::SoundEffects),
            ("autoPowerOff", SettingGroup::Power),
            ("lowBatteryPrompt", SettingGroup::Power),
            ("spatialAudio", SettingGroup::SoundEffects),
            ("spatialAudioMode", SettingGroup::SoundEffects),
            ("spatialAudioMusicMode", SettingGroup::SoundEffects),
            ("easyChat", SettingGroup::Comfort),
            ("easyChatWaitTime", SettingGroup::Comfort),
            ("soundLeakCompensation", SettingGroup::Comfort),
            ("wearingDetection", SettingGroup::Comfort),
        ];
        for (id, group) in expected {
            assert_eq!(group, group_of(id), "{id}");
        }
    }

    #[test]
    fn what_a_setting_is_for_comes_before_its_library_category() {
        // the library also lists the Liberty 4 Pro's spatial audio under its case
        assert_eq!(
            SettingGroup::SoundEffects,
            setting_group("spatialAudio", Some("case"))
        );
        assert_eq!(
            SettingGroup::Case,
            setting_group("caseBatteryLevel", Some("deviceInformation"))
        );
        assert_eq!(SettingGroup::Controls, group_of("touchTone"));
        assert_eq!(
            SettingGroup::Controls,
            setting_group("leftSinglePress", Some("buttonConfiguration"))
        );
    }

    #[test]
    fn settings_without_a_group_of_their_own_go_by_their_category() {
        assert_eq!(
            SettingGroup::Comfort,
            setting_group("limitHighVolumeDbLimit", Some("limitHighVolume"))
        );
        assert_eq!(
            SettingGroup::About,
            setting_group("serialNumber", Some("deviceInformation"))
        );
        assert_eq!(
            SettingGroup::Equalizer,
            setting_group(
                "importCustomEqualizerProfiles",
                Some("equalizerImportExport")
            )
        );
        assert_eq!(
            SettingGroup::Lights,
            setting_group("lightsColor", Some("lights"))
        );
        assert_eq!(SettingGroup::Other, group_of("somethingNew"));
        assert_eq!(SettingGroup::Other, setting_group("somethingNew", None));
    }

    #[test]
    fn the_sleep_earbuds_get_sleep_and_power_groups() {
        assert_eq!(SettingGroup::Sleep, group_of("autoStopTimer"));
        assert_eq!(SettingGroup::Sleep, group_of("listeningMode"));
        assert_eq!(SettingGroup::Power, group_of("noiseCancelingPrompt"));
        assert_eq!(SettingGroup::SoundModes, group_of("noiseCanceling"));
    }

    #[test]
    fn every_setting_is_shown_once_in_the_device_order() {
        let settings = [
            ("spatialAudio", "case"),
            ("wearingDetection", "miscellaneous"),
            ("ldac", "miscellaneous"),
            ("spatialAudio", "miscellaneous"),
        ];
        let grouped = group_settings(&settings);
        let ids = |group| {
            grouped
                .iter()
                .find(|(g, _)| *g == group)
                .map(|(_, ids)| ids.clone())
        };
        assert_eq!(
            Some(vec!["spatialAudio", "ldac"]),
            ids(SettingGroup::SoundEffects)
        );
        assert_eq!(Some(vec!["wearingDetection"]), ids(SettingGroup::Comfort));
        assert_eq!(None, ids(SettingGroup::Case));
        assert_eq!(
            vec![SettingGroup::SoundEffects, SettingGroup::Comfort],
            grouped.iter().map(|(g, _)| *g).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_equalizer_opens_its_page_when_the_page_shows_all_of_its_settings() {
        assert!(equalizer_opens_its_page(
            &["presetEqualizerProfile", "volumeAdjustments"],
            false
        ));
        assert!(!equalizer_opens_its_page(
            &["volumeAdjustments", "someOtherSetting"],
            false
        ));
    }

    #[test]
    fn the_equalizer_opens_its_page_while_one_earbud_is_connected() {
        // the device offers no equalizer settings then, but profiles can still be imported and exported on the page
        assert!(equalizer_opens_its_page(&[], true));
        assert!(!equalizer_opens_its_page(&[], false));
        assert!(!equalizer_opens_its_page(
            &["volumeAdjustments", "someOtherSetting"],
            true
        ));
    }
}
