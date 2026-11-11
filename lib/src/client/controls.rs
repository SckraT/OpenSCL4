use crate::settings::{Setting, SettingId};

use super::SettingMap;

/// How a setting's editor is presented given the other settings in the snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingControl {
    Enabled,
    Disabled,
    Hidden,
}

/// Editor dependencies. A preset can write a parent setting and its dependent settings together.
pub fn setting_control(settings: &SettingMap, id: &str) -> SettingControl {
    if id == "spatialAudioMusicMode"
        && settings
            .get(&SettingId::SpatialAudioMode)
            .is_some_and(|mode| match mode {
                Setting::Select { value, .. } => value != "Music",
                Setting::Information { value, .. } => value != "Music",
                _ => true,
            })
    {
        return SettingControl::Hidden;
    }
    let parent = match id {
        "easyChatWaitTime" => SettingId::EasyChat,
        "limitHighVolumeDbLimit" | "limitHighVolumeRefreshRate" => SettingId::LimitHighVolume,
        "spatialAudioMode" | "spatialAudioMusicMode" => SettingId::SpatialAudio,
        _ => return SettingControl::Enabled,
    };
    if matches!(settings.get(&parent), Some(Setting::Toggle { value: true })) {
        SettingControl::Enabled
    } else {
        SettingControl::Disabled
    }
}

/// The earbud a configurable gesture belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlSide {
    Left,
    Right,
}

impl ControlSide {
    pub const ALL: [Self; 2] = [Self::Left, Self::Right];

    pub fn gestures(self) -> &'static [SettingId] {
        use SettingId::*;
        match self {
            Self::Left => &[
                LeftSinglePress,
                LeftDoublePress,
                LeftTriplePress,
                LeftLongPress,
                LeftSlideUp,
                LeftSlideDown,
            ],
            Self::Right => &[
                RightSinglePress,
                RightDoublePress,
                RightTriplePress,
                RightLongPress,
                RightSlideUp,
                RightSlideDown,
            ],
        }
    }

    pub fn call_gestures(self) -> &'static [SettingId] {
        match self {
            Self::Left => &[
                SettingId::LeftDoublePressDuringCall,
                SettingId::LeftLongPressDuringCall,
            ],
            Self::Right => &[
                SettingId::RightDoublePressDuringCall,
                SettingId::RightLongPressDuringCall,
            ],
        }
    }

    pub fn settings(self) -> impl Iterator<Item = SettingId> {
        self.gestures().iter().chain(self.call_gestures()).copied()
    }
}
