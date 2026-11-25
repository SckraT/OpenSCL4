use crate::{
    BatteryLevels, DeviceState, FirmwareVersion, ModelDriver, ModelError, RedactedSerial,
    SoundMode, battery_percent,
};

const SOUND_MODE: usize = 69;

pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() <= SOUND_MODE {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[7..23])
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if !state_serial(body)?.identifies(ModelDriver::A3062) {
        return Err(ModelError::UnknownModel);
    }
    let sound_mode = SoundMode::decode(body[SOUND_MODE])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    Ok(DeviceState {
        model: ModelDriver::A3062,
        battery: BatteryLevels {
            device: battery_percent(body[0], 10, 1)?,
            left: None,
            right: None,
            case: None,
        },
        sound_mode,
        sound_mode_controls: None,
        firmware_device: FirmwareVersion::parse(&body[2..7])?,
        firmware_left: None,
        firmware_right: None,
    })
}
