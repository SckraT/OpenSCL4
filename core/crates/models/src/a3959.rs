use crate::{
    AutoPowerOff, BatteryLevels, DeviceState, EQUALIZER_BANDS, EqualizerSetting, FirmwareVersion,
    ModelDriver, ModelError, RedactedSerial, SoundMode, SoundModeControls, battery_percent, drc,
};

/// Body bytes documented by the A3959 state parser, including its trailing
/// padding.
pub const STATE_LENGTH: usize = 91;
/// Body offsets of the equalizer preset id in the measured layout.
pub const A3959_EQUALIZER_PRESET: std::ops::Range<usize> = 32..34;
/// Body offsets of the ten equalizer band gains in the measured layout.
pub const A3959_EQUALIZER_BANDS: std::ops::Range<usize> = 34..44;
/// Body offsets of the seven sound-mode bytes in the measured layout.
pub const A3959_SOUND_MODE_FIELDS: std::ops::Range<usize> = 64..71;
/// Body offsets of the two auto power off bytes in the measured layout.
pub const A3959_AUTO_POWER_OFF: std::ops::Range<usize> = 75..77;
/// Body offset of the gaming-mode flag in the measured layout.
pub const A3959_GAMING_MODE: usize = 78;

/// Number of entries in the family's ten-minute-step auto power off table.
pub const AUTO_POWER_OFF_DURATION_COUNT: u8 = 4;

/// The gaming-mode flag exists only on devices with both firmware versions at
/// or above `01.60`.
const GAMING_MODE_MIN_FIRMWARE: (u8, u8) = (1, 60);

/// Body offset of the TWS-connected flag shared by the family's state layout.
const TWS_CONNECTED: usize = 1;
const SOUND_MODE: usize = A3959_SOUND_MODE_FIELDS.start;
const MANUAL_ADAPTIVE: usize = 65;
const NOISE_CANCELING_MODE: usize = 67;
const WIND_NOISE: usize = 68;
const ADAPTIVE_SENSITIVITY: usize = 69;
const MULTI_SCENE: usize = 70;
const AUTO_POWER_OFF_ENABLED: usize = A3959_AUTO_POWER_OFF.start;
const AUTO_POWER_OFF_DURATION: usize = A3959_AUTO_POWER_OFF.start + 1;
const SERIAL: std::ops::Range<usize> = 16..32;
const FIRMWARE_LEFT: std::ops::Range<usize> = 6..11;
const FIRMWARE_RIGHT: std::ops::Range<usize> = 11..16;
const BATTERY_LEFT: usize = 2;
const BATTERY_RIGHT: usize = 3;
const BATTERY_SCALE: u8 = 10;
/// Length of the DRC equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 2 + EQUALIZER_BANDS * 2;
/// Number of sound-mode controls preserved by an ambient-mode write.
pub const SOUND_MODE_CONTROL_COUNT: usize = 5;

/// Reads the model-specific serial from the state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the body carries the bytes the A3959 parser documents.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(ModelDriver::A3959) {
        return Err(ModelError::UnknownModel);
    }
    let sound_mode = SoundMode::decode(body[SOUND_MODE])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    // The legacy structure clamps the packed manual and adaptive strengths to
    // the five-step range and reports an unrecognized mode as its default, so
    // an unrecognized mode byte is not a confirmed state for a write.
    if body[NOISE_CANCELING_MODE] > 2 || body[MULTI_SCENE] > 3 {
        return Err(ModelError::UnknownSoundMode);
    }
    let packed = body[MANUAL_ADAPTIVE];
    let manual_adaptive = ((packed >> 4).clamp(1, 5) << 4) | (packed & 0x0F).clamp(1, 5);
    // The legacy structure keeps only the suppression and detection bits.
    let wind_noise = body[WIND_NOISE] & 0b11;
    let firmware_left = FirmwareVersion::parse(&body[FIRMWARE_LEFT])?;
    let firmware_right = FirmwareVersion::parse(&body[FIRMWARE_RIGHT])?;
    Ok(DeviceState {
        model: ModelDriver::A3959,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[BATTERY_LEFT], BATTERY_SCALE, 0)?,
            right: battery_percent(body[BATTERY_RIGHT], BATTERY_SCALE, 0)?,
            case: None,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::A3959([
            manual_adaptive,
            body[NOISE_CANCELING_MODE],
            wind_noise,
            body[ADAPTIVE_SENSITIVITY],
            body[MULTI_SCENE],
        ])),
        equalizer: equalizer_setting(body),
        gaming_mode: gaming_mode(body, firmware_left, firmware_right),
        auto_power_off: auto_power_off(body),
        firmware_device: None,
        firmware_left,
        firmware_right,
    })
}

/// Reads the single-channel curve. A band block that starts with the absent
/// marker reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3959_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BANDS] = body.get(A3959_EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

/// Reads the gaming-mode flag only when both earbuds' firmware supports it and
/// the reported byte is a recognized boolean.
fn gaming_mode(
    body: &[u8],
    firmware_left: Option<FirmwareVersion>,
    firmware_right: Option<FirmwareVersion>,
) -> Option<bool> {
    let oldest = match (firmware_left, firmware_right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(firmware), None) | (None, Some(firmware)) => Some(firmware),
        (None, None) => None,
    }?;
    if !oldest.at_least(GAMING_MODE_MIN_FIRMWARE.0, GAMING_MODE_MIN_FIRMWARE.1) {
        return None;
    }
    match body[A3959_GAMING_MODE] {
        0 => Some(false),
        1 => Some(true),
        _ => None,
    }
}

/// Reads auto power off only when both bytes are recognized. An unparsed field
/// is not reported as a confirmed value.
fn auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    let enabled = match body[AUTO_POWER_OFF_ENABLED] {
        0 => false,
        1 => true,
        _ => return None,
    };
    let duration = body[AUTO_POWER_OFF_DURATION];
    if duration >= AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

/// Builds the legacy `06 81` ambient-mode write: the mode twice with the
/// remaining five controls of the fresh state in between.
pub fn sound_mode_write(
    mode: SoundMode,
    controls: [u8; SOUND_MODE_CONTROL_COUNT],
) -> Result<[u8; SOUND_MODE_CONTROL_COUNT + 2], ModelError> {
    if mode == SoundMode::AirplaneMode {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    let [
        manual_adaptive,
        noise_canceling_mode @ 0..=2,
        wind_noise,
        sensitivity,
        multi_scene @ 0..=3,
    ] = controls
    else {
        return Err(ModelError::InvalidSoundModeWrite);
    };
    Ok([
        mode.wire(),
        manual_adaptive,
        mode.wire(),
        noise_canceling_mode,
        wind_noise,
        sensitivity,
        multi_scene,
    ])
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
