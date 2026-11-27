use std::ops::Range;

use crate::{
    AutoPowerOff, BatteryLevels, DeviceState, EqualizerSetting, ModelDriver, ModelError, SoundMode,
    SoundModeControls, ambient_sound_mode_controls, battery_percent,
};

/// Body length of the A3931/A3935 state response.
pub const STATE_LENGTH: usize = 44;
/// Command of the family's equalizer write; the body carries the curve and its
/// DRC echo for both channels.
pub const EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x83];
/// Body offsets of the sound mode and its controls in the measured layout.
pub const SOUND_MODE_FIELDS: Range<usize> = 36..40;
/// Body offsets of the equalizer preset id in the measured layout.
pub const EQUALIZER_PRESET: Range<usize> = 6..8;
/// Body offsets of the first equalizer channel in the measured layout.
pub const EQUALIZER_CHANNEL_1: Range<usize> = 8..16;
/// Body offsets of the second equalizer channel in the measured layout.
pub const EQUALIZER_CHANNEL_2: Range<usize> = 16..24;
/// Body offsets of the auto power off value in the measured layout.
pub const AUTO_POWER_OFF: Range<usize> = 42..44;

const SOUND_MODE: usize = SOUND_MODE_FIELDS.start;
const SOUND_MODE_CONTROLS: Range<usize> = 37..40;
/// Number of equalizer bands carried by the family's curves.
pub const EQUALIZER_BAND_COUNT: usize = 8;
/// Number of entries in the family's auto power off duration table.
pub const AUTO_POWER_OFF_DURATION_COUNT: u8 = 4;
const BATTERY_SCALE: u8 = 5;

pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(model: ModelDriver, body: &[u8]) -> Result<DeviceState, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    let sound_mode = SoundMode::decode(body[SOUND_MODE])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    let [noise_canceling, transparency, custom] = body
        .get(SOUND_MODE_CONTROLS)
        .and_then(|controls| controls.try_into().ok())
        .ok_or(ModelError::IncompleteState)?;
    let controls = ambient_sound_mode_controls(noise_canceling, transparency, custom)?;
    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[2], BATTERY_SCALE, 0)?,
            right: battery_percent(body[3], BATTERY_SCALE, 0)?,
            case: None,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::Ambient(controls)),
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: auto_power_off(body),
        firmware_device: None,
        firmware_left: None,
        firmware_right: None,
    })
}

/// Reads the first equalizer channel. The legacy state modifier always writes
/// both channels with the same curve, and only the first channel is exposed.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(EQUALIZER_CHANNEL_1)?.try_into().ok()?;
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

/// Reads auto power off only when both the flag and the duration index are
/// recognized. An unparsed field is not reported as a confirmed value.
fn auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    let enabled = match body[AUTO_POWER_OFF.start] {
        0 => false,
        1 => true,
        _ => return None,
    };
    let duration = body[AUTO_POWER_OFF.start + 1];
    if duration >= AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

/// Builds the family's equalizer write from the requested curve.
///
/// The legacy `set_equalizer_with_drc` builder writes the preset and both
/// channels, then appends the DRC-applied curve of each channel. The state
/// modifier applies the same curve to both channels, so the request is
/// duplicated here.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    _state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable()
        || setting.volume_adjustments[EQUALIZER_BAND_COUNT..]
            .iter()
            .any(|gain| *gain != 0)
    {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let mut curve = [0; EQUALIZER_BAND_COUNT];
    curve.copy_from_slice(&setting.volume_adjustments[..EQUALIZER_BAND_COUNT]);
    let drc = apply_drc(curve);
    let mut body = Vec::with_capacity(2 + 4 * EQUALIZER_BAND_COUNT);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    for channel in [curve, curve] {
        body.extend(channel.map(EqualizerSetting::band_byte));
    }
    for channel in [drc, drc] {
        body.extend(channel.map(EqualizerSetting::band_byte));
    }
    Ok(body)
}

/// The legacy `CommonVolumeAdjustments::apply_drc` transform for eight bands.
///
/// The coefficients, the two band subtractions and the divide-then-scale step
/// are copied from the legacy structure, including its rounding, so the write
/// body reproduces the legacy encoder byte for byte.
fn apply_drc(curve: [i16; EQUALIZER_BAND_COUNT]) -> [i16; EQUALIZER_BAND_COUNT] {
    const SMALLER_COEFFICIENT: f64 = 0.85;
    const LARGER_COEFFICIENT: f64 = 0.95;
    const BAND_COEFFICIENTS: [[f64; EQUALIZER_BAND_COUNT]; EQUALIZER_BAND_COUNT] = [
        [
            1.26,
            -0.71 * SMALLER_COEFFICIENT,
            0.177,
            -0.0494,
            0.0345,
            -0.0197,
            0.0075,
            -0.00217,
        ],
        [
            -0.71 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            0.0,
            0.204,
            -0.068,
            0.045,
            -0.0235,
            0.0075,
        ],
        [
            0.177,
            -0.81 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            -0.81 * SMALLER_COEFFICIENT,
            0.208,
            -0.07,
            0.045,
            -0.0197,
        ],
        [
            -0.0494,
            0.204,
            0.0,
            1.73 * LARGER_COEFFICIENT,
            -0.82 * SMALLER_COEFFICIENT,
            0.208,
            -0.068,
            0.0345,
        ],
        [
            0.0345,
            -0.068,
            0.208,
            -0.82 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            0.0,
            0.204,
            -0.0494,
        ],
        [
            -0.0197,
            0.045,
            -0.07,
            0.208,
            -0.81 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            -0.81 * SMALLER_COEFFICIENT,
            0.177,
        ],
        [
            0.0075,
            -0.0235,
            0.045,
            -0.068,
            0.204,
            0.0,
            1.83 * LARGER_COEFFICIENT,
            -0.71 * SMALLER_COEFFICIENT,
        ],
        [
            -0.00217,
            0.0075,
            -0.0197,
            0.0345,
            -0.0494,
            0.177,
            -0.71 * SMALLER_COEFFICIENT,
            1.5,
        ],
    ];
    // The legacy structure stores tenths of a decibel and divides by
    // `10^FRACTION_DIGITS` before applying the coefficients.
    let adjustments = curve.map(|gain| f64::from(gain) / 10.0);
    let lows_subtraction = adjustments[2] * 0.81 * SMALLER_COEFFICIENT;
    let highs_subtraction = adjustments[5] * 0.81 * SMALLER_COEFFICIENT;
    std::array::from_fn(|band| {
        let coefficients = BAND_COEFFICIENTS[band];
        let multiplied = coefficients
            .iter()
            .enumerate()
            .fold(0.0, |acc, (index, coefficient)| {
                if (band == 1 || band == 3) && index == 2 {
                    acc - lows_subtraction
                } else if (band == 4 || band == 6) && index == 5 {
                    acc - highs_subtraction
                } else {
                    acc + adjustments[index] * coefficient
                }
            });
        // Match the legacy `divide afterwards` step before scaling back to
        // tenths of a decibel, including its floating-point rounding.
        let reduced = multiplied / 10.0;
        let value = (reduced * 10.0).round() as i16;
        value.clamp(crate::EQUALIZER_MIN_GAIN, crate::EQUALIZER_MAX_GAIN)
    })
}
