use crate::{
    AutoPowerOff, BatteryLevels, DeviceState, EqualizerSetting, FirmwareVersion, ModelDriver,
    ModelError, RedactedSerial, SoundMode, SoundModeControls, battery_percent,
};

/// Body length of the measured A3028/A3029 state without extra fields.
pub const STATE_LENGTH: usize = 60;
/// Body length of the same state with the documented extra fields.
pub const STATE_LENGTH_WITH_EXTRA_FIELDS: usize = 67;
/// The family's eight-band equalizer write command.
pub const A3028_EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x81];
/// Body offsets of the sound mode and its controls in the measured layout.
pub const A3028_SOUND_MODE_FIELDS: std::ops::Range<usize> = 35..39;
/// Body offsets of the equalizer preset id in the measured layout.
pub const A3028_EQUALIZER_PRESET: std::ops::Range<usize> = 2..4;
/// Body offsets of the eight equalizer band gains in the measured layout.
pub const A3028_EQUALIZER_BANDS: std::ops::Range<usize> = 4..12;
/// Body offsets of the auto power off value inside the optional extra fields.
pub const A3028_AUTO_POWER_OFF: std::ops::Range<usize> = 63..65;

const SOUND_MODE: usize = A3028_SOUND_MODE_FIELDS.start;
const SOUND_MODE_CONTROLS: std::ops::Range<usize> = 36..39;
const FIRMWARE: std::ops::Range<usize> = 39..44;
const SERIAL: std::ops::Range<usize> = 44..60;
const BATTERY_SCALE: u8 = 5;
/// Number of equalizer bands carried by the family's curve.
pub const EQUALIZER_BAND_COUNT: usize = 8;
/// Number of entries in the family's half-hour auto power off table.
pub const AUTO_POWER_OFF_DURATION_COUNT: u8 = 4;

/// Reads the model-specific serial from the measured state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

pub fn has_confirmed_layout(body: &[u8]) -> bool {
    matches!(body.len(), STATE_LENGTH | STATE_LENGTH_WITH_EXTRA_FIELDS)
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
        sound_mode_controls: Some(SoundModeControls::A3028([
            noise_canceling,
            transparency,
            custom,
        ])),
        equalizer: equalizer_setting(body),
        auto_power_off: auto_power_off(body),
        gaming_mode: None,
        wearing_detection: None,
        dual_connections: None,
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
    })
}

/// Reads the eight-band curve. A band block that starts with the absent marker
/// reports no curve. The shared curve keeps the family's two fixed invisible
/// bands at zero, as the legacy preset settings do.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3028_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(A3028_EQUALIZER_BANDS)?.try_into().ok()?;
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

/// Reads auto power off only when the documented extra fields are present and
/// both the flag and the duration index are recognized. An unparsed field is
/// not reported as a confirmed value.
fn auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    if body.len() != STATE_LENGTH_WITH_EXTRA_FIELDS {
        return None;
    }
    let enabled = match body[A3028_AUTO_POWER_OFF.start] {
        0 => false,
        1 => true,
        _ => return None,
    };
    let duration = body[A3028_AUTO_POWER_OFF.start + 1];
    if duration >= AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

pub fn sound_mode_write(mode: SoundMode, controls: [u8; 3]) -> Result<[u8; 4], ModelError> {
    let [noise_canceling @ 0..=3, transparency @ 0..=1, custom] = controls else {
        return Err(ModelError::InvalidSoundModeWrite);
    };
    if mode == SoundMode::AirplaneMode {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    // The legacy structure clamps this unexposed field; echo its effective value.
    let custom = if custom == u8::MAX {
        custom
    } else {
        custom.min(10)
    };
    Ok([mode.wire(), noise_canceling, transparency, custom])
}

/// Builds the eight-band equalizer write; the shared fixed bands must stay zero.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    _state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable()
        || setting.volume_adjustments[EQUALIZER_BAND_COUNT..]
            .iter()
            .any(|gain| *gain != 0)
    {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let mut body = Vec::with_capacity(2 + EQUALIZER_BAND_COUNT);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend(
        setting.volume_adjustments[..EQUALIZER_BAND_COUNT]
            .iter()
            .map(|gain| EqualizerSetting::band_byte(*gain)),
    );
    Ok(body)
}
