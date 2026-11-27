//! State layout of the Soundcore Space A40 (A3936).

use std::ops::Range;

use crate::{
    DeviceState, ModelDriver, ModelError,
    dual::{self, DualLayout},
};

/// Confirmed body length of the A3936 state layout.
pub const STATE_LENGTH: usize = 143;

/// Body offsets of the six-byte sound-mode block.
pub const A3936_SOUND_MODE_FIELDS: Range<usize> = 117..123;
/// Body offset of the TWS ambient sound mode cycle.
pub const A3936_AMBIENT_SOUND_MODE_CYCLE: usize = 116;
/// Body offset of the gaming-mode flag.
pub const A3936_GAMING_MODE: usize = 130;
/// Body offsets of the auto power off pair.
pub const A3936_AUTO_POWER_OFF: Range<usize> = 128..130;
/// Body offset of the charging-case level.
pub const A3936_CASE_BATTERY: usize = 124;

pub const LAYOUT: DualLayout = DualLayout {
    state_length: STATE_LENGTH,
    // The legacy writer ends the body with twelve reserved zero bytes.
    confirmed_lengths: &[STATE_LENGTH],
    battery: 2,
    battery_scale: 5,
    tws_status: 0,
    firmware: 6,
    equalizer_preset: 32,
    equalizer_bands: 34,
    case_battery: A3936_CASE_BATTERY,
    case_battery_scale: 10,
    case_battery_offset: 0,
    sound_modes: A3936_SOUND_MODE_FIELDS.start,
    ambient_cycle: A3936_AMBIENT_SOUND_MODE_CYCLE,
    tws_cycle: false,
    auto_power_off: A3936_AUTO_POWER_OFF.start,
    gaming_mode: Some(A3936_GAMING_MODE),
    // The measured block already carries this model's sensitivity byte.
    sound_mode_tail: None,
};

pub fn state_serial(body: &[u8]) -> Result<crate::RedactedSerial<'_>, ModelError> {
    dual::state_serial(body)
}

pub fn has_confirmed_layout(body: &[u8]) -> bool {
    dual::has_confirmed_layout(body, &LAYOUT)
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    dual::decode_state(ModelDriver::A3936, body, &LAYOUT)
}

pub fn sound_mode_write(
    mode: crate::SoundMode,
    block: [u8; dual::SOUND_MODE_BLOCK],
) -> Result<[u8; dual::SOUND_MODE_BLOCK], ModelError> {
    dual::sound_mode_write(mode, block, &LAYOUT)
}
