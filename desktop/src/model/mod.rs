//! What the app shows, worked out from the device's settings. It is ported from the Android app so that both read and
//! group settings the same way; each module names the Kotlin file it follows, and their tests come along too.

pub use openscq30_lib::client::{
    SettingMap, battery, equalizer, groups, presets, quick_toggles, sound_mode,
};

pub mod device_art;
pub mod devices;
pub mod languages;
pub mod legacy;
pub mod overview;
