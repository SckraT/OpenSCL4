//! Soundcore Q11i (A3005) driver layout.
//!
//! The captured state body is 55 bytes. The legacy parser reads a 48-byte
//! prefix: the single battery, the firmware, the synthetic serial, the ten-band
//! equalizer, six unknown bytes, the dual-connections enabled flag, four unknown
//! bytes and the auto power off value. The remaining seven bytes come from the
//! same capture and are preserved but not interpreted; the legacy driver ignores
//! them as well.
//!
//! The model has no sound modes. Its writes are the DRC equalizer, the
//! dual-connections flag and the auto power off value in half-hour steps.

use std::ops::Range;

use crate::{
    AutoPowerOff, BatteryLevels, DeviceState, EqualizerSetting, FirmwareVersion, ModelDriver,
    ModelError, RedactedSerial, battery_percent,
};

/// Body length of the captured Q11i state.
pub const STATE_LENGTH: usize = 55;
/// Prefix consumed by the legacy state parser.
pub const PARSED_LENGTH: usize = 48;
const BATTERY_SCALE: u8 = 10;
const BATTERY_OFFSET: u8 = 1;
const FIRMWARE: Range<usize> = 2..7;
const SERIAL: Range<usize> = 7..23;

/// Body offsets of the preset id in the captured layout.
pub const A3005_EQUALIZER_PRESET: Range<usize> = 23..25;
/// Body offsets of the ten band gains in the captured layout.
pub const A3005_EQUALIZER_BANDS: Range<usize> = 25..35;
/// Body offset of the dual-connections enabled flag.
pub const A3005_DUAL_CONNECTIONS: usize = 41;
/// Body offsets of the auto power off enabled flag and duration index.
pub const A3005_AUTO_POWER_OFF: Range<usize> = 46..48;
/// Number of equalizer bands carried by the model's curve.
pub const EQUALIZER_BAND_COUNT: usize = crate::EQUALIZER_BANDS;

/// Reads the model-specific serial from the measured state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < PARSED_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the body has the full captured layout a write may be admitted from.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(model: ModelDriver, body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < PARSED_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(model) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: battery_percent(body[0], BATTERY_SCALE, BATTERY_OFFSET)?,
            left: None,
            right: None,
            case: None,
        },
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: auto_power_off(body),
        wearing_detection: None,
        dual_connections: recognized_flag(body[A3005_DUAL_CONNECTIONS]),
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
    })
}

/// Reads a ten-band curve. A band block that starts with the absent marker
/// reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3005_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(A3005_EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

/// Reads auto power off only with a recognized flag and duration index. An
/// unparsed field is not reported as a confirmed value, matching the A3028
/// family decoder.
fn auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    let enabled = recognized_flag(body[A3005_AUTO_POWER_OFF.start])?;
    let duration = body[A3005_AUTO_POWER_OFF.start + 1];
    if duration >= crate::AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

/// The legacy `take_bool` accepts only `1` as enabled. The decoder reports
/// anything else as unrecognized instead of a confirmed disabled value.
fn recognized_flag(byte: u8) -> Option<bool> {
    match byte {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}
