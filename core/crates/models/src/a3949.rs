use crate::{
    BatteryLevels, DeviceState, FirmwareVersion, ModelDriver, ModelError, RedactedSerial,
    battery_percent,
};

const STATE_LENGTH: usize = 88;
const BATTERY_SCALE: u8 = 5;

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
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[6..11])?,
        firmware_right: FirmwareVersion::parse(&body[11..16])?,
    })
}
