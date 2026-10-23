use std::collections::HashMap;

use crate::settings::{Setting, SettingId};

pub mod availability;
pub mod battery;
pub mod equalizer;
pub mod groups;
pub mod presets;
pub mod quick_toggles;
pub mod sound_mode;

pub type SettingMap = HashMap<SettingId, Setting>;

#[cfg(test)]
mod contract_tests;
