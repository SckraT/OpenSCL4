use crate::{
    BatteryLevels, DeviceState, FirmwareVersion, ModelDriver, ModelError, RedactedSerial,
    SoundMode, SoundModeControls, a3028, battery_percent,
};

/// Body length of the measured A3027/A3030 state without the optional touch flag.
pub const STATE_LENGTH: usize = 61;
/// Body length of the same state with the documented optional touch flag.
pub const STATE_LENGTH_WITH_TOUCH: usize = 62;
/// Body offset of the wearing-detection flag in the measured layout.
pub const A3027_WEARING_DETECTION: usize = 60;

const SOUND_MODE: usize = a3028::A3028_SOUND_MODE_FIELDS.start;
const SOUND_MODE_CONTROLS: std::ops::Range<usize> = 36..39;
const FIRMWARE: std::ops::Range<usize> = 39..44;
const SERIAL: std::ops::Range<usize> = 44..60;
const BATTERY_SCALE: u8 = 5;

/// Reads the model-specific serial from the measured state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the state response has one of the two documented lengths.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    matches!(body.len(), STATE_LENGTH | STATE_LENGTH_WITH_TOUCH)
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
    // Legacy `SoundModes::take` accepts the documented ANC and transparency
    // values and clamps the unexposed custom value.
    if noise_canceling > 3 || transparency > 1 {
        return Err(ModelError::UnknownSoundMode);
    }
    let custom = if custom == u8::MAX {
        custom
    } else {
        custom.min(10)
    };
    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: battery_percent(body[0], BATTERY_SCALE, 0)?,
            left: None,
            right: None,
            case: None,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::Ambient([
            noise_canceling,
            transparency,
            custom,
        ])),
        equalizer: a3028::equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: None,
        wearing_detection: Some(body[A3027_WEARING_DETECTION] == 1),
        tws_connected: None,
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
    })
}

/// Builds the family sound-mode write; the controls come from the latest state.
pub fn sound_mode_write(mode: SoundMode, controls: [u8; 3]) -> Result<[u8; 4], ModelError> {
    a3028::sound_mode_write(mode, controls)
}

/// Builds the single-channel eight-band equalizer write; the shared fixed bands
/// must stay zero.
pub fn equalizer_write_body(
    setting: crate::EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    a3028::equalizer_write_body(setting, state)
}
