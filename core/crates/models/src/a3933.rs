use std::ops::Range;

use crate::{
    BatteryLevels, DeviceState, EqualizerSetting, FirmwareVersion, ModelDriver, ModelError,
    RedactedSerial, SoundMode, SoundModeControls, ambient_sound_mode_controls, battery_percent,
};

/// Body length of the measured A3933/A3939 state with the optional extra fields.
pub const STATE_LENGTH: usize = 132;
/// Body length of the same state without the optional extra fields block.
pub const STATE_LENGTH_WITHOUT_EXTRA_FIELDS: usize = 122;
/// Command of the family's ten-band equalizer write.
pub const EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x81];
/// Body offsets of the two firmware versions in the measured layout.
pub const FIRMWARE: Range<usize> = 6..16;
/// Body offsets of the model serial in the measured layout.
pub const SERIAL: Range<usize> = 16..32;
/// Body offsets of the equalizer preset id in the measured layout.
pub const EQUALIZER_PRESET: Range<usize> = 32..34;
/// Body offsets of the first equalizer channel in the measured layout.
pub const EQUALIZER_CHANNEL_1: Range<usize> = 34..44;
/// Body offsets of the second equalizer channel in the measured layout.
pub const EQUALIZER_CHANNEL_2: Range<usize> = 44..54;
/// Body offsets of the sound mode and its controls in the measured layout.
pub const SOUND_MODE_FIELDS: Range<usize> = 116..120;
/// Body offset of the gaming-mode flag inside the optional extra fields.
pub const GAMING_MODE: usize = 124;
/// Body offset of the case battery level inside the optional extra fields.
pub const CASE_BATTERY_LEVEL: usize = 125;

const SOUND_MODE: usize = SOUND_MODE_FIELDS.start;
const SOUND_MODE_CONTROLS: Range<usize> = 117..120;
const BATTERY_SCALE: u8 = 5;
/// Number of equalizer bands carried by the family's curves.
pub const EQUALIZER_BAND_COUNT: usize = 10;

pub fn has_confirmed_layout(body: &[u8]) -> bool {
    matches!(body.len(), STATE_LENGTH | STATE_LENGTH_WITHOUT_EXTRA_FIELDS)
}

/// Reads the model-specific serial from the measured state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

pub fn decode_state(model: ModelDriver, body: &[u8]) -> Result<DeviceState, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(model) {
        return Err(ModelError::UnknownModel);
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
            case: case_battery_level(body)?,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::Ambient(controls)),
        equalizer: equalizer_setting(body),
        gaming_mode: gaming_mode(body),
        auto_power_off: None,
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[FIRMWARE.start..FIRMWARE.start + 5])?,
        firmware_right: FirmwareVersion::parse(&body[FIRMWARE.start + 5..FIRMWARE.end])?,
    })
}

/// Reads the case battery level only when the optional extra fields are present.
fn case_battery_level(body: &[u8]) -> Result<Option<u8>, ModelError> {
    if body.len() != STATE_LENGTH {
        return Ok(None);
    }
    battery_percent(body[CASE_BATTERY_LEVEL], BATTERY_SCALE, 0)
}

/// Reads the gaming-mode flag only from the layout that carries the extra fields.
fn gaming_mode(body: &[u8]) -> Option<bool> {
    if body.len() != STATE_LENGTH {
        return None;
    }
    match body[GAMING_MODE] {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
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

/// Builds the family's equalizer write from the requested curve.
///
/// The legacy `set_equalizer` builder writes the preset and both channels; the
/// state modifier applies the same curve to both, so the request is duplicated
/// here.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    _state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable() {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let mut body = Vec::with_capacity(2 + 2 * EQUALIZER_BAND_COUNT);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    for _channel in 0..2 {
        body.extend(
            setting.volume_adjustments[..EQUALIZER_BAND_COUNT]
                .iter()
                .map(|gain| EqualizerSetting::band_byte(*gain)),
        );
    }
    Ok(body)
}
