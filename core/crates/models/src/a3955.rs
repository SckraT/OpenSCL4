use crate::{
    AUTO_POWER_OFF_DURATION_COUNT, AutoPowerOff, BatteryLevels, DeviceState, EqualizerSetting,
    FirmwareVersion, ImmersiveExperience, ModelDriver, ModelError, RedactedSerial,
    SoundModeControls, battery_percent, equalizer,
};

/// Body length of the measured P40i state.
pub const STATE_LENGTH: usize = 143;
const BATTERY_SCALE: u8 = 5;
const SERIAL: std::ops::Range<usize> = 16..32;
const FIRMWARE_LEFT: std::ops::Range<usize> = 6..11;
const FIRMWARE_RIGHT: std::ops::Range<usize> = 11..16;
const CASE_BATTERY: usize = 37;
/// Body offsets of the seven extended sound-mode fields.
pub const A3955_SOUND_MODE_FIELDS: std::ops::Range<usize> = 119..126;
/// Body offset of the auto power off flag and duration index.
pub const A3955_AUTO_POWER_OFF: std::ops::Range<usize> = 139..141;
/// Body offsets of the equalizer preset id.
pub const A3955_EQUALIZER_PRESET: std::ops::Range<usize> = 38..40;
/// Body offsets of both equalizer channels.
pub const A3955_EQUALIZER_BANDS: std::ops::Range<usize> = 40..60;
/// Length of the measured equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 93;

/// Body offset of the hear-id initialization status; `0xff` means the device
/// has no stored hear-id profile.
const HEAR_ID_STATUS: usize = 60;
const HEAR_ID_VOLUME: std::ops::Range<usize> = 62..82;
const HEAR_ID_TIME: std::ops::Range<usize> = 82..86;
const HEAR_ID_TYPE: usize = 86;
const HEAR_ID_CUSTOM: std::ops::Range<usize> = 87..107;
const HEAR_ID_MUSIC_GENRE: std::ops::Range<usize> = 107..109;

/// The measured P40i immersive-experience write command.
pub const IMMERSIVE_EXPERIENCE_COMMAND: [u8; 2] = [0x12, 0x81];

/// Reads the model-specific serial from the measured state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Decodes the measured P40i state.
pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(ModelDriver::A3955) {
        return Err(ModelError::UnknownModel);
    }
    Ok(DeviceState {
        model: ModelDriver::A3955,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[2], BATTERY_SCALE, 0)?,
            right: battery_percent(body[3], BATTERY_SCALE, 0)?,
            case: battery_percent(body[CASE_BATTERY], BATTERY_SCALE, 0)?,
        },
        sound_mode: Some(super::decode_extended_sound_mode(ModelDriver::A3955, body)?),
        sound_mode_controls: body
            .get(A3955_SOUND_MODE_FIELDS)
            .and_then(|controls| controls.try_into().ok())
            .map(SoundModeControls::Extended),
        equalizer: equalizer_setting(body),
        // The legacy P40i state does not report a gaming-mode flag.
        gaming_mode: None,
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
    let preset_id: [u8; 2] = body.get(A3955_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; 10] = body
        .get(A3955_EQUALIZER_BANDS.start..A3955_EQUALIZER_BANDS.start + 10)?
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
    let enabled = match body[A3955_AUTO_POWER_OFF.start] {
        0 => false,
        1 => true,
        _ => return None,
    };
    let duration = body[A3955_AUTO_POWER_OFF.start + 1];
    if duration >= AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

/// Builds the measured equalizer write body.
///
/// The write carries both channels with the requested curve, keeps the hear-id
/// fields of the fresh state with hear-id disabled, and repeats the requested
/// curve after the DRC transform as the active curve. A state without a stored
/// hear-id profile echoes the absent marker and clears the profile fields.
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
    let hear_id_initialized = state[HEAR_ID_STATUS] != u8::MAX;
    let mut body = Vec::with_capacity(EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    if hear_id_initialized {
        body.extend_from_slice(&state[HEAR_ID_MUSIC_GENRE]);
    } else {
        // The legacy parser replaces an uninitialized hear-id profile with its
        // default, whose favorite music genre is zero.
        body.extend_from_slice(&[0, 0]);
    }
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    if hear_id_initialized {
        body.extend_from_slice(&[0, 0]);
    } else {
        body.extend_from_slice(&[u8::MAX, u8::MAX]);
    }
    // The measured write disables hear id so the requested curve takes effect.
    body.push(0);
    if hear_id_initialized {
        body.extend_from_slice(&state[HEAR_ID_VOLUME]);
        body.extend_from_slice(&state[HEAR_ID_TIME]);
        body.push(state[HEAR_ID_TYPE]);
        body.extend_from_slice(&state[HEAR_ID_CUSTOM]);
    } else {
        // The legacy parser replaces an uninitialized hear-id profile with its
        // default: two absent curves, no time, the initial type and two absent
        // custom curves. An absent curve keeps nine 0xff bytes and a zero tail,
        // exactly like the legacy builder.
        push_absent_curves(&mut body, 2);
        body.extend_from_slice(&[0, 0, 0, 0]);
        body.push(0);
        push_absent_curves(&mut body, 2);
    }
    let active_curve = equalizer::drc_bytes(setting.volume_adjustments);
    body.extend_from_slice(&active_curve);
    body.extend_from_slice(&active_curve);
    body.push(0);
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}

/// Builds the one-byte immersive-experience write body.
pub fn immersive_experience_write(experience: ImmersiveExperience) -> [u8; 1] {
    [experience.wire()]
}

/// Appends absent hear-id curves: nine `0xff` bytes and a zero tail per
/// channel, exactly like the legacy outbound builder.
fn push_absent_curves(body: &mut Vec<u8>, channels: usize) {
    for _ in 0..channels {
        body.extend_from_slice(&[u8::MAX; 9]);
        body.push(0);
    }
}
