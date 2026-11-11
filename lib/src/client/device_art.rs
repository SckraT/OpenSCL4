//! Shared device types and illustration choices. Visually similar models share one illustration.

/// What kind of device a model is, for filtering the model list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Earbuds,
    OverEar,
    OpenEar,
    Speaker,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

    /// The name shared by Android drawables and the converted desktop SVGs.
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

    /// Model names as exposed to clients, including the manufacturer prefix.
    pub fn model_names(self) -> impl Iterator<Item = String> {
        self.model_codes()
            .iter()
            .map(|code| format!("Soundcore{code}"))
    }

    fn model_codes(self) -> &'static [&'static str] {
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
        .find(|art| art.model_codes().contains(&code))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for model in crate::DeviceModel::VARIANTS {
            let name: &'static str = model.into();
            if name.starts_with("Soundcore") && name != "SoundcoreDevelopment" {
                assert!(device_art(name).is_some(), "{name}");
            }
        }
    }
}
