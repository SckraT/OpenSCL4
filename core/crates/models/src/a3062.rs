use crate::{
    BatteryLevels, DeviceState, FirmwareVersion, ModelDriver, ModelError, RedactedSerial,
    SoundMode, SoundModeControls, battery_percent,
};

const SOUND_MODE: usize = 69;
const SOUND_MODE_CONTROLS: std::ops::Range<usize> = 70..75;

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
        sound_mode_controls: body
            .get(SOUND_MODE_CONTROLS)
            .and_then(|controls| controls.try_into().ok())
            .map(SoundModeControls::A3062),
        firmware_device: FirmwareVersion::parse(&body[2..7])?,
        firmware_left: None,
        firmware_right: None,
    })
}

pub fn ambient_sound_mode_write(mode: SoundMode, controls: [u8; 5]) -> Result<[u8; 6], ModelError> {
    let [strength, 1, anc @ 0..=1, wind @ 0..=1, transparency @ 1..=5] = controls else {
        return Err(ModelError::InvalidSoundModeWrite);
    };
    if mode == SoundMode::AirplaneMode
        || !(1..=5).contains(&(strength >> 4))
        || !(1..=5).contains(&(strength & 0x0f))
    {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    Ok([mode.wire(), strength, 1, anc, wind, transparency])
}
