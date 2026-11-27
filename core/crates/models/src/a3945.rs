use crate::{
    BatteryLevels, DeviceState, EQUALIZER_BANDS, EqualizerSetting, FirmwareVersion, ModelDriver,
    ModelError, RedactedSerial, battery_percent,
};

/// Body bytes documented by the A3945 state parser.
pub const STATE_LENGTH: usize = 72;
/// Body offsets of the equalizer preset id in the measured layout.
pub const A3945_EQUALIZER_PRESET: std::ops::Range<usize> = 32..34;
/// Body offsets of the first equalizer channel in the measured layout.
pub const A3945_EQUALIZER_LEFT_BANDS: std::ops::Range<usize> = 34..44;
/// Body offsets of the second equalizer channel in the measured layout.
pub const A3945_EQUALIZER_RIGHT_BANDS: std::ops::Range<usize> = 44..54;

/// Body offset of the TWS-connected flag shared by the family's state layout.
const TWS_CONNECTED: usize = 1;
/// Body offset of the gaming-mode flag in the measured layout.
pub const A3945_GAMING_MODE: usize = 68;
const CASE_BATTERY: usize = 69;

const SERIAL: std::ops::Range<usize> = 16..32;
const FIRMWARE_LEFT: std::ops::Range<usize> = 6..11;
const FIRMWARE_RIGHT: std::ops::Range<usize> = 11..16;
const BATTERY_LEFT: usize = 2;
const BATTERY_RIGHT: usize = 3;
const BATTERY_SCALE: u8 = 5;
const CASE_SCALE: u8 = 5;
const EQUALIZER_CHANNELS: usize = 2;
/// Length of the two-channel equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 2 + EQUALIZER_BANDS * EQUALIZER_CHANNELS;

/// Reads the model-specific serial from the state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the body carries the bytes the A3945 parser documents.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() >= STATE_LENGTH
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(ModelDriver::A3945) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model: ModelDriver::A3945,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[BATTERY_LEFT], BATTERY_SCALE, 0)?,
            right: battery_percent(body[BATTERY_RIGHT], BATTERY_SCALE, 0)?,
            case: battery_percent(body[CASE_BATTERY], CASE_SCALE, 0)?,
        },
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: equalizer_setting(body),
        gaming_mode: match body[A3945_GAMING_MODE] {
            0 => Some(false),
            1 => Some(true),
            _ => None,
        },
        auto_power_off: None,
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[FIRMWARE_LEFT])?,
        firmware_right: FirmwareVersion::parse(&body[FIRMWARE_RIGHT])?,
    })
}

/// Reads the shared two-channel curve. The new state carries one curve, so a
/// state whose two channels carry different curves is reported as no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3945_EQUALIZER_PRESET)?.try_into().ok()?;
    let left: [u8; EQUALIZER_BANDS] = body.get(A3945_EQUALIZER_LEFT_BANDS)?.try_into().ok()?;
    let right: [u8; EQUALIZER_BANDS] = body.get(A3945_EQUALIZER_RIGHT_BANDS)?.try_into().ok()?;
    if left[0] == u8::MAX || right[0] == u8::MAX || left != right {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: left.map(EqualizerSetting::band_gain),
    })
}

/// Builds the legacy `02 81` equalizer write: the preset followed by the
/// requested curve twice, once for each channel.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if state.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    // The legacy TWS module ignores equalizer changes while the earbuds are not
    // connected to each other, so the session refuses the write instead.
    if state[TWS_CONNECTED] != 1 {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    if !setting.is_writable() {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let curve: [u8; EQUALIZER_BANDS] = setting.volume_adjustments.map(EqualizerSetting::band_byte);
    let mut body = Vec::with_capacity(EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    for _ in 0..EQUALIZER_CHANNELS {
        body.extend_from_slice(&curve);
    }
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}
