//! State layout of the Soundcore Sport X20 (A3968).

use std::ops::Range;

use crate::{
    DeviceState, ModelDriver, ModelError,
    dual::{self, DualLayout},
};

/// Body length the legacy A3968 state writer produces.
pub const STATE_LENGTH: usize = 130;
/// Body length of the recorded A3968 state, which carries thirteen reserved
/// trailing bytes the legacy parser does not read.
pub const STATE_LENGTH_WITH_RESERVED: usize = 143;

/// Body offsets of the six-byte sound-mode block.
pub const A3968_SOUND_MODE_FIELDS: Range<usize> = 117..123;
/// Body offset of the TWS-aware ambient sound mode cycle.
pub const A3968_AMBIENT_SOUND_MODE_CYCLE: usize = 116;
/// Body offsets of the auto power off pair.
pub const A3968_AUTO_POWER_OFF: Range<usize> = 128..130;
/// Body offset of the charging-case level.
pub const A3968_CASE_BATTERY: usize = 37;

pub const LAYOUT: DualLayout = DualLayout {
    state_length: STATE_LENGTH,
    confirmed_lengths: &[STATE_LENGTH, STATE_LENGTH_WITH_RESERVED],
    battery: 2,
    battery_scale: 5,
    tws_status: 0,
    firmware: 6,
    equalizer_preset: 38,
    equalizer_bands: 40,
    case_battery: A3968_CASE_BATTERY,
    case_battery_scale: 5,
    case_battery_offset: 0,
    sound_modes: A3968_SOUND_MODE_FIELDS.start,
    ambient_cycle: A3968_AMBIENT_SOUND_MODE_CYCLE,
    tws_cycle: true,
    auto_power_off: A3968_AUTO_POWER_OFF.start,
    gaming_mode: None,
    // The legacy writer replaces the unexposed last byte with 0xFF.
    sound_mode_tail: Some(0xff),
};

pub fn state_serial(body: &[u8]) -> Result<crate::RedactedSerial<'_>, ModelError> {
    dual::state_serial(body)
}

pub fn has_confirmed_layout(body: &[u8]) -> bool {
    dual::has_confirmed_layout(body, &LAYOUT)
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    dual::decode_state(ModelDriver::A3968, body, &LAYOUT)
}

pub fn sound_mode_write(
    mode: crate::SoundMode,
    block: [u8; dual::SOUND_MODE_BLOCK],
) -> Result<[u8; dual::SOUND_MODE_BLOCK], ModelError> {
    dual::sound_mode_write(mode, block, &LAYOUT)
}
