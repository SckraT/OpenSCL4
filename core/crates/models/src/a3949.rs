use crate::{
    BatteryLevels, DeviceState, FirmwareVersion, ModelDriver, ModelError, RedactedSerial,
    battery_percent,
};

/// Body length of the measured A3949 pilot state.
pub const STATE_LENGTH: usize = 88;
const BATTERY_SCALE: u8 = 5;

/// Body offset of the measured A3949 gaming-mode flag.
pub const A3949_GAMING_MODE: usize = 71;

pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[16..32])
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if !state_serial(body)?.identifies(ModelDriver::A3949) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model: ModelDriver::A3949,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[2], BATTERY_SCALE, 0)?,
            right: battery_percent(body[3], BATTERY_SCALE, 0)?,
            case: None,
        },
        sound_mode: None,
        sound_mode_controls: None,
        equalizer: None,
        gaming_mode: match (body.len(), body[A3949_GAMING_MODE]) {
            (STATE_LENGTH, 0) => Some(false),
            (STATE_LENGTH, 1) => Some(true),
            _ => None,
        },
        auto_power_off: None,
        tws_connected: None,
        case_features: None,
        spatial_audio: None,
        sound_leak_compensation: None,
        case_language: None,
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[6..11])?,
        firmware_right: FirmwareVersion::parse(&body[11..16])?,
        firmware_case: None,
    })
}
