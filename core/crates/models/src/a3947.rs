use crate::{
    AUTO_POWER_OFF_DURATION_COUNT, AutoPowerOff, BatteryLevels, DeviceState, EqualizerSetting,
    FirmwareVersion, ModelDriver, ModelError, RedactedSerial, SoundModeControls, battery_percent,
    equalizer,
};

/// Body length of the measured Liberty 4 NC state.
pub const STATE_LENGTH: usize = 158;
const BATTERY_SCALE: u8 = 5;
const SERIAL: std::ops::Range<usize> = 16..32;
const FIRMWARE_LEFT: std::ops::Range<usize> = 6..11;
const FIRMWARE_RIGHT: std::ops::Range<usize> = 11..16;
const CASE_BATTERY: usize = 139;
/// Body offsets of the seven extended sound-mode fields.
pub const A3947_SOUND_MODE_FIELDS: std::ops::Range<usize> = 126..133;
/// The measured Liberty 4 NC gaming-mode write command.
pub const A3947_GAMING_MODE_COMMAND: [u8; 2] = [0x10, 0x85];
/// Body offset of the measured gaming-mode flag.
pub const A3947_GAMING_MODE: usize = 143;
/// Body offset of the auto power off flag and duration index.
pub const A3947_AUTO_POWER_OFF: std::ops::Range<usize> = 154..156;
/// Body offsets of the equalizer preset id.
pub const A3947_EQUALIZER_PRESET: std::ops::Range<usize> = 37..39;
/// Body offsets of both equalizer channels.
pub const A3947_EQUALIZER_BANDS: std::ops::Range<usize> = 39..59;
/// Length of the measured equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 93;

const HEAR_ID_MUSIC_GENRE: usize = 106;
const HEAR_ID_VOLUME: std::ops::Range<usize> = 61..81;
const HEAR_ID_TIME: std::ops::Range<usize> = 81..85;
const HEAR_ID_TYPE: usize = 85;
const HEAR_ID_CUSTOM: std::ops::Range<usize> = 86..106;

/// Reads the model-specific serial from the measured state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Decodes the measured Liberty 4 NC state.
pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(ModelDriver::A3947) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model: ModelDriver::A3947,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[2], BATTERY_SCALE, 0)?,
            right: battery_percent(body[3], BATTERY_SCALE, 0)?,
            case: battery_percent(body[CASE_BATTERY], BATTERY_SCALE, 0)?,
        },
        sound_mode: Some(super::decode_extended_sound_mode(ModelDriver::A3947, body)?),
        sound_mode_controls: body
            .get(A3947_SOUND_MODE_FIELDS)
            .and_then(|controls| controls.try_into().ok())
            .map(SoundModeControls::Extended),
        equalizer: equalizer_setting(body),
        gaming_mode: match body[A3947_GAMING_MODE] {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        auto_power_off: auto_power_off(body),
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[FIRMWARE_LEFT])?,
        firmware_right: FirmwareVersion::parse(&body[FIRMWARE_RIGHT])?,
        tws_connected: Some(body[1] == 1),
    })
}

/// Reads the equalizer curve of the first channel. A band block that starts
/// with the absent marker reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3947_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; 10] = body
        .get(A3947_EQUALIZER_BANDS.start..A3947_EQUALIZER_BANDS.start + 10)?
        .try_into()
        .ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

/// Reads auto power off only when both the flag and the duration index are
/// recognized.
fn auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    let enabled = match body[A3947_AUTO_POWER_OFF.start] {
        0 => false,
        1 => true,
        _ => return None,
    };
    let duration = body[A3947_AUTO_POWER_OFF.start + 1];
    if duration >= AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

/// Builds the measured equalizer write body.
///
/// The write carries both channels with the requested curve, keeps the hear-id
/// block of the fresh state with hear-id disabled so the curve is in effect, and
/// repeats the requested curve after the DRC transform as the active curve.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if state.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if state[1] != 1 {
        // The legacy TWS equalizer module is unavailable while the earbuds are
        // not both connected.
        return Err(ModelError::InvalidEqualizerWrite);
    }
    if !setting.is_writable() {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let bands = setting.volume_adjustments.map(EqualizerSetting::band_byte);
    let active_curve = equalizer::drc_bytes(setting.volume_adjustments);
    let mut body = Vec::with_capacity(EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    // The legacy structure keeps the one-byte music genre followed by a zero.
    body.extend_from_slice(&[state[HEAR_ID_MUSIC_GENRE], 0]);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&[0, 0]);
    // The measured write disables hear id so the requested curve takes effect.
    body.push(0);
    body.extend_from_slice(&state[HEAR_ID_VOLUME]);
    body.extend_from_slice(&state[HEAR_ID_TIME]);
    body.push(state[HEAR_ID_TYPE]);
    body.extend_from_slice(&state[HEAR_ID_CUSTOM]);
    body.extend_from_slice(&active_curve);
    body.extend_from_slice(&active_curve);
    body.push(0);
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}
