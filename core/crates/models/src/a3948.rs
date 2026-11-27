use crate::{
    BatteryLevels, DeviceState, EQUALIZER_BANDS, EqualizerSetting, FirmwareVersion, ModelDriver,
    ModelError, RedactedSerial, battery_percent, drc,
};

/// Body bytes documented by the A3948 state parser, including its trailing
/// padding.
pub const STATE_LENGTH: usize = 88;
/// Body offsets of the equalizer preset id in the measured layout.
pub const A3948_EQUALIZER_PRESET: std::ops::Range<usize> = 32..34;
/// Body offsets of the ten equalizer band gains in the measured layout.
pub const A3948_EQUALIZER_BANDS: std::ops::Range<usize> = 34..44;

/// Body offset of the TWS-connected flag shared by the family's state layout.
const TWS_CONNECTED: usize = 1;
const SERIAL: std::ops::Range<usize> = 16..32;
const FIRMWARE_LEFT: std::ops::Range<usize> = 6..11;
const FIRMWARE_RIGHT: std::ops::Range<usize> = 11..16;
const BATTERY_LEFT: usize = 2;
const BATTERY_RIGHT: usize = 3;
const BATTERY_SCALE: u8 = 5;
/// Length of the DRC equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 2 + EQUALIZER_BANDS * 2;

/// Reads the model-specific serial from the state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the body carries the bytes the A3948 parser documents.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(ModelDriver::A3948) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model: ModelDriver::A3948,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[BATTERY_LEFT], BATTERY_SCALE, 0)?,
            right: battery_percent(body[BATTERY_RIGHT], BATTERY_SCALE, 0)?,
            case: None,
        },
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: None,
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[FIRMWARE_LEFT])?,
        firmware_right: FirmwareVersion::parse(&body[FIRMWARE_RIGHT])?,
    })
}

/// Reads the single-channel curve. A band block that starts with the absent
/// marker reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3948_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BANDS] = body.get(A3948_EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

/// Builds the legacy `02 83` equalizer write: the preset, the requested curve,
/// and the DRC copy of the same curve.
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
    body.extend_from_slice(&curve);
    body.extend(
        drc::apply_drc(setting.volume_adjustments)
            .iter()
            .map(|gain| EqualizerSetting::band_byte(*gain)),
    );
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}
