//! The device art (ui/art, converted from the Android drawables) as images for the window.

use std::{cell::RefCell, collections::HashMap};

use crate::model::device_art::{DeviceArt, EarbudsColor};

macro_rules! device_svgs {
    ($($art:ident => $name:literal),* $(,)?) => {
        fn svg(art: DeviceArt, color: EarbudsColor) -> &'static [u8] {
            match (art, color) {
                $(
                    (DeviceArt::$art, EarbudsColor::Silver) =>
                        include_bytes!(concat!("../ui/art/sound_art_", $name, "_silver.svg")),
                    (DeviceArt::$art, EarbudsColor::Black) =>
                        include_bytes!(concat!("../ui/art/sound_art_", $name, "_black.svg")),
                )*
            }
        }
    };
}

device_svgs! {
    Headphones => "headphones",
    MotionPlus => "motion_plus",
    OpenClip => "open_clip",
    OpenHook => "open_hook",
    SportHook => "sport_hook",
    WingedBuds => "buds_winged",
    CompactBuds => "buds_compact",
    ClassicStem => "stem_classic",
    LibertyStem => "stem_premium",
    OpenStem => "stem_open",
}

thread_local! {
    static IMAGES: RefCell<HashMap<(DeviceArt, EarbudsColor), slint::Image>> = RefCell::default();
}

/// The illustration in the given color. Images are made once and shared, as the window shows them again and again.
pub fn image(art: DeviceArt, color: EarbudsColor) -> slint::Image {
    IMAGES.with(|images| {
        images
            .borrow_mut()
            .entry((art, color))
            .or_insert_with(|| {
                slint::Image::load_from_svg_data(svg(art, color)).unwrap_or_else(|err| {
                    tracing::error!("device art {art:?} {color:?} did not load: {err:?}");
                    slint::Image::default()
                })
            })
            .clone()
    })
}
