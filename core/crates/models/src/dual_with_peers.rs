//! Drivers for the dual-battery models with a conditional peer list.
//!
//! The layouts and write formats come from the legacy drivers in
//! `lib/src/devices/soundcore/{a3330,a3388,a3876,d1101}` and their shared
//! modules. The measured bodies are documented in `core/data/README.md`.

use std::ops::Range;

use crate::{
    BatteryLevels, ColorfulLights, ColorfulLightsMode, DeviceState, EqualizerSetting,
    FirmwareVersion, HostDevice, ModelDriver, ModelError, PairedState, RedactedSerial, RgbColor,
    TwsStatus, battery_percent,
};

/// Serial offset shared by the four measured state layouts.
const SERIAL: Range<usize> = 14..30;
/// Ten-step battery scale with a one-step offset.
const BATTERY_SCALE_TEN: (u8, u8) = (10, 1);
/// The five-step scale of the A3330.
const BATTERY_SCALE_FIVE: (u8, u8) = (5, 0);

/// A measured state layout and the body offsets the virtual transport applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PairedLayout {
    /// Body length of the measured state capture.
    pub state_length: usize,
    /// Shortest body the legacy parser accepts.
    pub parsed_length: usize,
    /// Preset id of the equalizer curve.
    pub equalizer_preset: Range<usize>,
    /// First-channel band gains of the equalizer curve.
    pub equalizer_bands: Range<usize>,
    /// Inverted "disable all buttons" flag, when the model reports it.
    pub buttons_disabled: Option<usize>,
    /// Gaming-mode flag, when the model reports it.
    pub gaming_mode: Option<usize>,
    /// Auto power off value, when the model reports it.
    pub auto_power_off: Option<Range<usize>>,
    /// LDAC flag, when the model reports it.
    pub ldac: Option<usize>,
}

const A3330_LAYOUT: PairedLayout = PairedLayout {
    state_length: 70,
    parsed_length: 67,
    equalizer_preset: 50..52,
    equalizer_bands: 52..62,
    buttons_disabled: Some(48),
    gaming_mode: None,
    auto_power_off: None,
    ldac: None,
};
const A3388_LAYOUT: PairedLayout = PairedLayout {
    state_length: 66,
    parsed_length: 62,
    equalizer_preset: 50..52,
    equalizer_bands: 52..62,
    buttons_disabled: Some(48),
    gaming_mode: None,
    auto_power_off: None,
    ldac: None,
};
const A3876_LAYOUT: PairedLayout = PairedLayout {
    state_length: 94,
    parsed_length: 82,
    equalizer_preset: 36..38,
    equalizer_bands: 38..48,
    buttons_disabled: None,
    gaming_mode: Some(79),
    auto_power_off: Some(76..78),
    ldac: None,
};
const D1101_LAYOUT: PairedLayout = PairedLayout {
    state_length: 69,
    parsed_length: 58,
    equalizer_preset: 30..32,
    equalizer_bands: 32..42,
    buttons_disabled: Some(54),
    gaming_mode: None,
    auto_power_off: Some(55..57),
    ldac: Some(57),
};

/// Returns the measured layout of a dualWithPeers model.
pub const fn layout(model: ModelDriver) -> Option<PairedLayout> {
    match model {
        ModelDriver::A3330 => Some(A3330_LAYOUT),
        ModelDriver::A3388 => Some(A3388_LAYOUT),
        ModelDriver::A3876 => Some(A3876_LAYOUT),
        ModelDriver::D1101 => Some(D1101_LAYOUT),
        _ => None,
    }
}

/// Reads the model-specific serial from the shared state offset.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < SERIAL.end {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the body is the full measured capture of the model.
pub fn has_confirmed_layout(model: ModelDriver, body: &[u8]) -> bool {
    layout(model).is_some_and(|layout| body.len() == layout.state_length)
}

/// Whether the body carries the parsed fields of the model.
fn has_parsed_layout(model: ModelDriver, body: &[u8]) -> bool {
    layout(model).is_some_and(|layout| body.len() >= layout.parsed_length)
}

/// Selects a model from a measured state body.
pub fn identify_state(body: &[u8]) -> Option<ModelDriver> {
    let serial = state_serial(body).ok()?;
    [
        ModelDriver::A3330,
        ModelDriver::A3388,
        ModelDriver::A3876,
        ModelDriver::D1101,
    ]
    .into_iter()
    .find(|model| has_confirmed_layout(*model, body) && serial.identifies(*model))
}

pub fn decode_state(model: ModelDriver, body: &[u8]) -> Result<DeviceState, ModelError> {
    let layout = layout(model).ok_or(ModelError::UnknownModel)?;
    if !has_parsed_layout(model, body) {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(model) {
        return Err(ModelError::UnknownModel);
    }
    let (scale, offset) = match model {
        ModelDriver::A3330 => BATTERY_SCALE_FIVE,
        _ => BATTERY_SCALE_TEN,
    };
    let (firmware_left, firmware_right) = dual_firmware(&body[4..14])?;
    let case = match model {
        ModelDriver::A3330 | ModelDriver::A3388 => battery_percent(body[35], scale, offset)?,
        _ => None,
    };
    let buttons_enabled = layout.buttons_disabled.map(|offset| body[offset] != 1);
    let gaming_mode = layout.gaming_mode.map(|offset| body[offset] == 1);
    let auto_power_off = layout
        .auto_power_off
        .clone()
        .map(|range| crate::AutoPowerOff {
            enabled: body[range.start] == 1,
            duration: body[range.start + 1],
        });
    let ldac = layout.ldac.map(|offset| body[offset] == 1);
    let (touch_tone, low_battery_prompt, voice_prompt) = match model {
        ModelDriver::A3330 => (Some(body[45] == 1), Some(body[46] == 1), None),
        ModelDriver::A3388 => (Some(body[45] == 1), Some(body[46] == 1), None),
        ModelDriver::D1101 => (None, Some(body[52] == 1), None),
        ModelDriver::A3876 => (None, None, Some(body[75] == 1)),
        _ => unreachable!("only dualWithPeers models reach this decoder"),
    };
    let dual_connections_enabled = match model {
        ModelDriver::A3330 => Some(body[47] == 1),
        ModelDriver::A3388 => Some(body[47] == 1),
        ModelDriver::A3876 => Some(body[81] == 1),
        ModelDriver::D1101 => Some(body[53] == 1),
        _ => unreachable!("only dualWithPeers models reach this decoder"),
    };
    let tws = TwsStatus {
        connected: body[1] == 1,
        host: match body[0] {
            0 => HostDevice::Left,
            1 => HostDevice::Right,
            _ => return Err(ModelError::InvalidStateValue),
        },
    };
    let colorful_lights = (model == ModelDriver::A3876).then(|| ColorfulLights {
        enabled: body[67] == 1,
        brightness: body[68].clamp(1, 10),
        auto_lights_off_minutes: body[69].clamp(1, 120),
        mode: ColorfulLightsMode::decode(body[70]),
        color: RgbColor {
            red: body[71],
            green: body[72],
            blue: body[73],
        },
    });
    let volume_balance = (model == ModelDriver::A3876).then(|| volume_balance(body[80]));

    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[2], scale, offset)?,
            right: battery_percent(body[3], scale, offset)?,
            case,
        },
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: equalizer_setting(body, &layout),
        gaming_mode,
        auto_power_off,
        firmware_device: None,
        firmware_left,
        firmware_right,
        paired: Some(PairedState {
            tws,
            buttons_enabled,
            ldac,
            colorful_lights,
            volume_balance,
            low_battery_prompt,
            touch_tone,
            voice_prompt,
            dual_connections_enabled,
        }),
    })
}

/// Reads the first equalizer channel. A block starting with the absent marker
/// reports no curve.
fn equalizer_setting(body: &[u8], layout: &PairedLayout) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(layout.equalizer_preset.clone())?.try_into().ok()?;
    let bands: [u8; 10] = body.get(layout.equalizer_bands.clone())?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

/// Parses the two five-byte firmware strings of the paired state.
fn dual_firmware(
    bytes: &[u8],
) -> Result<(Option<FirmwareVersion>, Option<FirmwareVersion>), ModelError> {
    let left = FirmwareVersion::parse(&bytes[..5])?;
    let right = FirmwareVersion::parse(&bytes[5..])?;
    if left.is_none() && right.is_none() {
        return Err(ModelError::InvalidFirmware);
    }
    Ok((left, right))
}

/// Decodes the volume balance byte as the legacy structure does, clamping to
/// the six-step range.
fn volume_balance(byte: u8) -> i8 {
    i8::try_from(byte)
        .unwrap_or(100)
        .saturating_sub(100)
        .clamp(-6, 6)
}

/// Builds an equalizer write body from the model's legacy builder.
///
/// The A3330 and A3876 repeat the adjusted curve as the DRC block; the A3388
/// and D1101 send the same curve for the left and right channels.
pub fn equalizer_write_body(
    model: ModelDriver,
    setting: EqualizerSetting,
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable() {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let bands = setting.volume_adjustments.map(EqualizerSetting::band_byte);
    let mut body = Vec::with_capacity(22);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend_from_slice(&bands);
    match model {
        ModelDriver::A3330 | ModelDriver::A3876 => body.extend_from_slice(&drc_bands(bands)),
        ModelDriver::A3388 | ModelDriver::D1101 => body.extend_from_slice(&bands),
        _ => return Err(ModelError::InvalidEqualizerWrite),
    }
    Ok(body)
}

/// Applies the legacy DRC transform, which affects the first eight bands.
fn drc_bands(bands: [u8; 10]) -> [u8; 10] {
    let adjustments: [f64; 10] =
        bands.map(|byte| f64::from(EqualizerSetting::band_gain(byte)) / 10.0);
    const SMALLER_COEFFICIENT: f64 = 0.85;
    const LARGER_COEFFICIENT: f64 = 0.95;
    let lows_subtraction = adjustments[2] * 0.81 * SMALLER_COEFFICIENT;
    let highs_subtraction = adjustments[5] * 0.81 * SMALLER_COEFFICIENT;
    const BAND_COEFFICIENTS: [[f64; 8]; 8] = [
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
    let multiplied: [f64; 8] = std::array::from_fn(|band| {
        let coefficients = BAND_COEFFICIENTS[band];
        adjustments
            .iter()
            .take(8)
            .enumerate()
            .fold(0.0, |acc, (index, adjustment)| {
                if (band == 1 || band == 3) && index == 2 {
                    acc - lows_subtraction
                } else if (band == 4 || band == 6) && index == 5 {
                    acc - highs_subtraction
                } else {
                    acc + adjustment * coefficients[index]
                }
            })
    });
    // The transform only affects the first eight bands; the remaining band
    // values are the legacy defaults.
    let mut gains = [0_i16; 10];
    for (gain, value) in gains.iter_mut().zip(multiplied) {
        *gain = ((value / 10.0) * 10.0).round() as i16;
    }
    gains[9] = -120;
    gains.map(|gain| EqualizerSetting::band_byte(gain.clamp(-120, 134)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixed example of the legacy `set_equalizer_with_drc` test.
    const LEGACY_CURVE: [i16; 10] = [-60, 60, 23, 120, 22, -120, -4, 16, 0, -120];

    #[test]
    fn drc_transform_matches_the_legacy_eight_band_example() {
        let gains = LEGACY_CURVE.map(EqualizerSetting::band_byte);
        assert_eq!(
            drc_bands(gains),
            [109, 134, 112, 136, 123, 102, 126, 121, 120, 0]
        );
    }

    #[test]
    fn drc_transform_matches_the_legacy_a3876_custom_curve() {
        let gains = [0, 0, 0, 0, 0, 0, 0, 60, 0, 0_i16].map(EqualizerSetting::band_byte);
        assert_eq!(
            drc_bands(gains),
            [120, 120, 120, 120, 120, 121, 116, 129, 120, 0]
        );
    }
}
