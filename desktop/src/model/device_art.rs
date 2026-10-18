//! Unbranded illustrations of the devices, the Android app's (res/drawable/sound_art_*), chosen per model as
//! android/app/src/main/java/com/oppzippy/openscq30/ui/sound/SoundDeviceArt.kt does. Visually similar models share one.

/// What kind of device a model is, for filtering the model list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Earbuds,
    OverEar,
    OpenEar,
    Speaker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum EarbudsColor {
    #[default]
    Silver,
    Black,
}

impl DeviceArt {
    pub const ALL: [Self; 10] = [
        Self::Headphones,
        Self::MotionPlus,
        Self::OpenClip,
        Self::OpenHook,
        Self::SportHook,
        Self::WingedBuds,
        Self::CompactBuds,
        Self::ClassicStem,
        Self::LibertyStem,
        Self::OpenStem,
    ];

    pub fn kind(self) -> DeviceKind {
        match self {
            Self::Headphones => DeviceKind::OverEar,
            Self::MotionPlus => DeviceKind::Speaker,
            Self::OpenClip | Self::OpenHook => DeviceKind::OpenEar,
            Self::SportHook
            | Self::WingedBuds
            | Self::CompactBuds
            | Self::ClassicStem
            | Self::LibertyStem
            | Self::OpenStem => DeviceKind::Earbuds,
        }
    }

    /// The Android drawable name, sound_art_<name>_<color>.xml.
    pub fn drawable_name(self) -> &'static str {
        match self {
            Self::Headphones => "headphones",
            Self::MotionPlus => "motion_plus",
            Self::OpenClip => "open_clip",
            Self::OpenHook => "open_hook",
            Self::SportHook => "sport_hook",
            Self::WingedBuds => "buds_winged",
            Self::CompactBuds => "buds_compact",
            Self::ClassicStem => "stem_classic",
            Self::LibertyStem => "stem_premium",
            Self::OpenStem => "stem_open",
        }
    }

    fn models(self) -> &'static [&'static str] {
        match self {
            Self::Headphones => &[
                "A3004", "A3005", "A3027", "A3028", "A3029", "A3030", "A3031", "A3033", "A3035",
                "A3040", "A3062",
            ],
            Self::MotionPlus => &["A3116"],
            Self::OpenClip => &["A3330", "A3388", "D1101"],
            Self::OpenHook => &["A3876"],
            Self::SportHook => &["A3968"],
            Self::WingedBuds => &["A3909", "A3926", "A3930", "A3931", "A3935", "A3952"],
            Self::CompactBuds => &["A3936", "A3948", "D1301"],
            Self::ClassicStem => &[
                "A3933", "A3939", "A3944", "A3949", "A3959", "D1202", "D1202C",
            ],
            Self::LibertyStem => &["A3947", "A3951", "A3953", "A3954", "A3955", "A3957"],
            Self::OpenStem => &["A3945"],
        }
    }
}

/// The illustration of a device model such as "SoundcoreA3954", or None for models without one.
pub fn device_art(model: &str) -> Option<DeviceArt> {
    let code = model.strip_prefix("Soundcore")?;
    DeviceArt::ALL
        .into_iter()
        .find(|art| art.models().contains(&code))
}

/// The color chosen in the settings, or, when following the theme, silver in light themes and black in dark themes,
/// so the illustration stands out from the card.
pub fn art_color(dark_theme: bool, chosen: Option<EarbudsColor>) -> EarbudsColor {
    chosen.unwrap_or(if dark_theme {
        EarbudsColor::Black
    } else {
        EarbudsColor::Silver
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_theme_picks_the_color_unless_one_is_chosen() {
        assert_eq!(EarbudsColor::Silver, art_color(false, None));
        assert_eq!(EarbudsColor::Black, art_color(true, None));
        assert_eq!(
            EarbudsColor::Black,
            art_color(false, Some(EarbudsColor::Black))
        );
        assert_eq!(
            EarbudsColor::Silver,
            art_color(true, Some(EarbudsColor::Silver))
        );
    }

    #[test]
    fn models_use_the_illustration_of_their_shape() {
        assert_eq!(Some(DeviceArt::LibertyStem), device_art("SoundcoreA3954"));
        assert_eq!(Some(DeviceArt::Headphones), device_art("SoundcoreA3028"));
        assert_eq!(Some(DeviceArt::MotionPlus), device_art("SoundcoreA3116"));
        assert_eq!(Some(DeviceArt::ClassicStem), device_art("SoundcoreD1202C"));
        assert_eq!(Some(DeviceArt::OpenStem), device_art("SoundcoreA3945"));
    }

    #[test]
    fn models_are_grouped_by_kind() {
        let kind = |model| device_art(model).map(DeviceArt::kind);
        assert_eq!(Some(DeviceKind::Earbuds), kind("SoundcoreA3954"));
        assert_eq!(Some(DeviceKind::Earbuds), kind("SoundcoreA3968"));
        assert_eq!(Some(DeviceKind::OverEar), kind("SoundcoreA3028"));
        assert_eq!(Some(DeviceKind::OpenEar), kind("SoundcoreA3330"));
        assert_eq!(Some(DeviceKind::OpenEar), kind("SoundcoreA3876"));
        assert_eq!(Some(DeviceKind::Speaker), kind("SoundcoreA3116"));
    }

    #[test]
    fn unknown_models_have_no_illustration() {
        assert_eq!(None, device_art("SoundcoreA9999"));
        assert_eq!(None, device_art("A3954"));
    }

    #[test]
    fn every_library_model_has_an_illustration() {
        use strum::VariantArray;
        for model in openscq30_lib::DeviceModel::VARIANTS {
            let name: &'static str = model.into();
            if name.starts_with("Soundcore") && name != "SoundcoreDevelopment" {
                assert!(device_art(name).is_some(), "{name}");
            }
        }
    }
}
