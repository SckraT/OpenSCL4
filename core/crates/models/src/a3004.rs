//! Soundcore Q20i (A3004) driver layout.
//!
//! The captured state body is 61 bytes. The legacy parser reads a 39-byte
//! prefix: the single battery, the firmware, the synthetic serial, the ten-band
//! equalizer and the four sound-mode bytes. The remaining 22 bytes come from
//! the same capture and are preserved but not interpreted; the legacy driver
//! ignores them as well.
//!
//! The model exposes the ambient sound mode only. Its noise-canceling and
//! transparency submodes are not available in the legacy settings, so the
//! captured control bytes are echoed unchanged when the ambient mode changes.

use std::ops::Range;

use crate::{
    BatteryLevels, DeviceState, EqualizerSetting, FirmwareVersion, ModelDriver, ModelError,
    RedactedSerial, SoundMode, SoundModeControls, battery_percent,
};

/// Body length of the captured Q20i state.
pub const STATE_LENGTH: usize = 61;
/// Prefix consumed by the legacy state parser.
pub const PARSED_LENGTH: usize = 39;
const BATTERY_SCALE: u8 = 5;
const FIRMWARE: Range<usize> = 2..7;
const SERIAL: Range<usize> = 7..23;
const SOUND_MODE: usize = 35;

/// Body offsets of the ambient mode and the preserved control bytes.
pub const A3004_SOUND_MODE_FIELDS: Range<usize> = SOUND_MODE..39;
/// Body offsets of the preset id in the captured layout.
pub const A3004_EQUALIZER_PRESET: Range<usize> = 23..25;
/// Body offsets of the ten band gains in the captured layout.
pub const A3004_EQUALIZER_BANDS: Range<usize> = 25..35;
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
    let sound_mode = SoundMode::decode(body[SOUND_MODE])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    let controls: [u8; 3] = body
        .get(SOUND_MODE + 1..A3004_SOUND_MODE_FIELDS.end)
        .and_then(|controls| controls.try_into().ok())
        .ok_or(ModelError::IncompleteState)?;
    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: battery_percent(body[0], BATTERY_SCALE, 0)?,
            left: None,
            right: None,
            case: None,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::A3004(controls)),
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: None,
        wearing_detection: None,
        dual_connections: None,
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
    })
}

/// Reads the ten-band curve. A band block that starts with the absent marker
/// reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3004_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(A3004_EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

/// Builds the four-byte ambient-mode write, echoing the unexposed controls.
pub fn sound_mode_write(mode: SoundMode, controls: [u8; 3]) -> Result<[u8; 4], ModelError> {
    if mode == SoundMode::AirplaneMode {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    let [noise_canceling, transparency, custom] = controls;
    Ok([mode.wire(), noise_canceling, transparency, custom])
}
