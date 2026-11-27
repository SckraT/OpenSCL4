//! State layout of the Soundcore Liberty 4 (A3952).

use std::ops::Range;

use crate::{
    DeviceState, ModelDriver, ModelError,
    dual::{self, DualLayout},
};

/// Body length the legacy A3952 state writer produces.
pub const STATE_LENGTH: usize = 140;
/// Body length of the recorded A3952 state, which carries thirteen trailing bytes
/// the legacy parser does not read.
pub const STATE_LENGTH_WITH_TRAILING_DATA: usize = 153;

/// Body offsets of the six-byte sound-mode block.
pub const A3952_SOUND_MODE_FIELDS: Range<usize> = 121..127;
/// Body offset of the shared ambient sound mode cycle.
pub const A3952_AMBIENT_SOUND_MODE_CYCLE: usize = 120;
/// Body offsets of the auto power off pair.
pub const A3952_AUTO_POWER_OFF: Range<usize> = 138..140;
/// Body offset of the charging-case level.
pub const A3952_CASE_BATTERY: usize = 130;

pub const LAYOUT: DualLayout = DualLayout {
    state_length: STATE_LENGTH,
    confirmed_lengths: &[STATE_LENGTH, STATE_LENGTH_WITH_TRAILING_DATA],
    battery: 2,
    battery_scale: 5,
    tws_status: 0,
    firmware: 6,
    equalizer_preset: 32,
    equalizer_bands: 34,
    case_battery: A3952_CASE_BATTERY,
    case_battery_scale: 5,
    case_battery_offset: 0,
    sound_modes: A3952_SOUND_MODE_FIELDS.start,
    ambient_cycle: A3952_AMBIENT_SOUND_MODE_CYCLE,
    tws_cycle: false,
    auto_power_off: A3952_AUTO_POWER_OFF.start,
    gaming_mode: None,
    // The measured block carries one unexposed byte, which the write preserves.
    sound_mode_tail: None,
};

pub fn state_serial(body: &[u8]) -> Result<crate::RedactedSerial<'_>, ModelError> {
    dual::state_serial(body)
}

pub fn has_confirmed_layout(body: &[u8]) -> bool {
    dual::has_confirmed_layout(body, &LAYOUT)
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    dual::decode_state(ModelDriver::A3952, body, &LAYOUT)
}

pub fn sound_mode_write(
    mode: crate::SoundMode,
    block: [u8; dual::SOUND_MODE_BLOCK],
) -> Result<[u8; dual::SOUND_MODE_BLOCK], ModelError> {
    dual::sound_mode_write(mode, block, &LAYOUT)
}
