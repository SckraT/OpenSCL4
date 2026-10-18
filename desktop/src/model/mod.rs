//! What the app shows, worked out from the device's settings. It is ported from the Android app so that both read and
//! group settings the same way; each module names the Kotlin file it follows, and their tests come along too.

use std::collections::HashMap;

use openscq30_lib::settings::{Setting, SettingId};

pub mod battery;
pub mod device_art;
pub mod devices;
pub mod equalizer;
pub mod groups;
pub mod languages;
pub mod legacy;
pub mod overview;
pub mod presets;
pub mod quick_toggles;
pub mod sound_mode;

/// The device's current settings.
pub type SettingMap = HashMap<SettingId, Setting>;
