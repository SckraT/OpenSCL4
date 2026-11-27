//! Soundcore Life 2 Neo (A3033) driver layout.
//!
//! No capture of this model exists in the repository. The 34-byte reference
//! body is derived from the legacy serializer: the single battery, the
//! eight-band equalizer, the firmware, the synthetic serial and the
//! wearing-detection flag. The legacy serializer writes the battery pair as
//! charging-then-level, while its parser - like the parser of the sibling
//! A3027 - reads level-then-charging; the reference follows the parser, which
//! is the path real device responses take.

use std::ops::Range;

use crate::{
    BatteryLevels, DeviceState, EqualizerSetting, FirmwareVersion, ModelDriver, ModelError,
    RedactedSerial, battery_percent,
};

/// Body length of the derived state.
pub const STATE_LENGTH: usize = 34;
const BATTERY_SCALE: u8 = 5;
const FIRMWARE: Range<usize> = 12..17;
const SERIAL: Range<usize> = 17..33;

/// Body offsets of the preset id in the derived layout.
pub const A3033_EQUALIZER_PRESET: Range<usize> = 2..4;
/// Body offsets of the eight band gains in the derived layout.
pub const A3033_EQUALIZER_BANDS: Range<usize> = 4..12;
/// Body offset of the wearing-detection flag.
pub const A3033_WEARING_DETECTION: usize = 33;
/// Number of equalizer bands carried by the model's curve.
pub const EQUALIZER_BAND_COUNT: usize = 8;

/// Reads the model-specific serial from the derived state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the body has the full derived layout a write may be admitted from.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(model: ModelDriver, body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(model) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: battery_percent(body[0], BATTERY_SCALE, 0)?,
            left: None,
            right: None,
            case: None,
        },
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: None,
        wearing_detection: recognized_flag(body[A3033_WEARING_DETECTION]),
        dual_connections: None,
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
    })
}

/// Reads an eight-band curve. A band block that starts with the absent marker
/// reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3033_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(A3033_EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    let mut volume_adjustments = [0; crate::EQUALIZER_BANDS];
    for (gain, byte) in volume_adjustments.iter_mut().zip(bands) {
        *gain = EqualizerSetting::band_gain(byte);
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments,
    })
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

/// Builds the eight-band equalizer write; the shared curve keeps the two bands
/// the model does not carry at zero, as the legacy eight-band builder does.
pub fn equalizer_write_body(setting: EqualizerSetting) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable()
        || setting.volume_adjustments[EQUALIZER_BAND_COUNT..]
            .iter()
            .any(|gain| *gain != 0)
    {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let mut body = Vec::with_capacity(2 + EQUALIZER_BAND_COUNT);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend(
        setting.volume_adjustments[..EQUALIZER_BAND_COUNT]
            .iter()
            .map(|gain| EqualizerSetting::band_byte(*gain)),
    );
    Ok(body)
}
