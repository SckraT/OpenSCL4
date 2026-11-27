use crate::{
    AmbientSoundModeCycle, BatteryLevels, ButtonAction, DeviceState, EqualizerSetting,
    FirmwareVersion, LimitHighVolume, ModelDriver, ModelError, RedactedSerial, SettingsSnapshot,
    SoundMode, SoundModeControls, VolumeLimitRefreshRate, apply_drc, band_byte, band_gain,
    battery_percent,
};

/// Body length of the measured A3035 state response.
pub const STATE_LENGTH: usize = 95;
const BATTERY: usize = 0;
const FIRMWARE: std::ops::Range<usize> = 2..7;
const SERIAL: std::ops::Range<usize> = 7..23;
/// Body offsets of the preset id in the measured state layout.
pub const A3035_EQUALIZER_PRESET: std::ops::Range<usize> = 23..25;
/// Body offsets of the ten band gains in the measured state layout.
pub const A3035_EQUALIZER_BANDS: std::ops::Range<usize> = 25..35;
const HEAR_ID_VOLUME: std::ops::Range<usize> = 38..48;
const HEAR_ID_TIME: std::ops::Range<usize> = 48..52;
const HEAR_ID_TYPE: usize = 52;
const HEAR_ID_CUSTOM: std::ops::Range<usize> = 53..63;
const HEAR_ID_GENRE: usize = 63;
/// Body offset of the double-press button action.
pub const A3035_BUTTON_CONFIGURATION: usize = 67;
/// Body offset of the ambient sound mode cycle flags.
pub const A3035_SOUND_MODE_CYCLE: usize = 68;
/// Body offsets of the mode and its five controls in the measured layout.
pub const A3035_SOUND_MODE_FIELDS: std::ops::Range<usize> = 69..75;
/// Body offset of the auto play/pause flag.
pub const A3035_AUTO_PLAY_PAUSE: usize = 76;
/// Body offset of the LDAC flag.
pub const A3035_LDAC: usize = 81;
/// Body offset of the dual-connections enabled flag.
pub const A3035_DUAL_CONNECTIONS: usize = 82;
/// Body offsets of the auto power off flag and duration index.
pub const A3035_AUTO_POWER_OFF: std::ops::Range<usize> = 85..87;
/// Body offsets of the high-volume limiter fields.
pub const A3035_LIMIT_HIGH_VOLUME: std::ops::Range<usize> = 87..90;
/// Body offset of the ambient sound mode voice prompt flag.
pub const A3035_VOICE_PROMPT: usize = 90;
/// Body offset of the power-on battery notice flag.
pub const A3035_BATTERY_ALERT: usize = 91;

const BATTERY_SCALE: u8 = 5;
/// Number of equalizer bands carried by the family's curve.
pub const EQUALIZER_BAND_COUNT: usize = 10;
/// Length of the measured equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 53;

pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the body has the exact measured state layout.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !state_serial(body)?.identifies(ModelDriver::A3035) {
        return Err(ModelError::UnknownModel);
    }
    let sound_mode_fields: [u8; 6] = body[A3035_SOUND_MODE_FIELDS]
        .try_into()
        .map_err(|_| ModelError::IncompleteState)?;
    let sound_mode = SoundMode::decode(sound_mode_fields[0])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    // The legacy structure reads the packed controls and clamps both values to 1–5.
    let packed = sound_mode_fields[1];
    let custom_noise_canceling = ((packed >> 4).clamp(1, 5)) << 4;
    let adaptive_noise_canceling = (packed & 0x0f).clamp(1, 5);
    let noise_canceling_mode = match sound_mode_fields[3] {
        0 | 1 => sound_mode_fields[3],
        _ => 0,
    };
    let wind_noise_reduction = u8::from(sound_mode_fields[4] == 1);
    let custom_transparency = sound_mode_fields[5].clamp(1, 5);
    let auto_power_off = match body[A3035_AUTO_POWER_OFF.start] {
        0 => Some(crate::AutoPowerOff {
            enabled: false,
            duration: body[A3035_AUTO_POWER_OFF.start + 1],
        }),
        1 => Some(crate::AutoPowerOff {
            enabled: true,
            duration: body[A3035_AUTO_POWER_OFF.start + 1],
        }),
        _ => None,
    };
    Ok(DeviceState {
        model: ModelDriver::A3035,
        battery: BatteryLevels {
            device: battery_percent(body[BATTERY], BATTERY_SCALE, 0)?,
            left: None,
            right: None,
            case: None,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::A3035([
            custom_noise_canceling | adaptive_noise_canceling,
            noise_canceling_mode,
            wind_noise_reduction,
            custom_transparency,
        ])),
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off,
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
        settings: SettingsSnapshot {
            button_double_press: ButtonAction::decode(body[A3035_BUTTON_CONFIGURATION]),
            ambient_sound_mode_cycle: Some(AmbientSoundModeCycle::decode(
                body[A3035_SOUND_MODE_CYCLE],
            )),
            limit_high_volume: limit_high_volume(body),
            dual_connections: Some(body[A3035_DUAL_CONNECTIONS] == 1),
            ldac: Some(body[A3035_LDAC] == 1),
            auto_play_pause: Some(body[A3035_AUTO_PLAY_PAUSE] == 1),
            battery_alert: Some(body[A3035_BATTERY_ALERT] == 1),
            voice_prompt: Some(body[A3035_VOICE_PROMPT] == 1),
            low_battery_prompt: None,
        },
    })
}

/// Reads the ten-band curve. A band block that starts with the absent marker
/// reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3035_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(A3035_EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(band_gain),
    })
}

fn limit_high_volume(body: &[u8]) -> Option<LimitHighVolume> {
    let [enabled, db_limit, refresh_rate]: [u8; 3] =
        body.get(A3035_LIMIT_HIGH_VOLUME)?.try_into().ok()?;
    Some(LimitHighVolume {
        enabled: enabled == 1,
        db_limit,
        refresh_rate: VolumeLimitRefreshRate::decode(refresh_rate),
    })
}

/// Builds the six-byte sound-mode write from the latest measured controls.
pub fn sound_mode_write(mode: SoundMode, controls: [u8; 4]) -> Result<[u8; 6], ModelError> {
    let [
        packed,
        noise_canceling_mode @ 0..=1,
        wind @ 0..=1,
        custom_transparency @ 1..=5,
    ] = controls
    else {
        return Err(ModelError::InvalidSoundModeWrite);
    };
    if mode == SoundMode::AirplaneMode
        || !(1..=5).contains(&(packed >> 4))
        || !(1..=5).contains(&(packed & 0x0f))
    {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    Ok([
        mode.wire(),
        packed,
        mode.wire(),
        noise_canceling_mode,
        wind,
        custom_transparency,
    ])
}

/// Builds the measured equalizer write from a fresh state response.
///
/// The write carries the requested curve, echoes the state's hear-id fields with
/// hear-id disabled so the curve takes effect, and ends with the curve after the
/// model's dynamic range compression.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable() || !has_confirmed_layout(state) {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let genre = state[HEAR_ID_GENRE];
    let mut body = Vec::with_capacity(EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    // The favorite music genre is a little-endian u16 in the write body.
    body.extend_from_slice(&[genre, 0]);
    body.extend(setting.volume_adjustments.map(band_byte));
    // The measured write disables hear id so the requested curve is in effect.
    body.extend_from_slice(&[0xff, 0xff]);
    body.push(0);
    body.extend_from_slice(&voice_prompt_bytes(&state[HEAR_ID_VOLUME]));
    body.extend_from_slice(&state[HEAR_ID_TIME]);
    body.push(mapped_hear_id_type(state[HEAR_ID_TYPE]));
    body.extend_from_slice(&voice_prompt_bytes(&state[HEAR_ID_CUSTOM]));
    body.extend(apply_drc(setting.volume_adjustments).map(band_byte));
    body.push(0);
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}

/// Encodes an optional ten-byte hear-id block the way the legacy builder does:
/// the absent marker keeps nine `FF` bytes and a zero last byte.
fn voice_prompt_bytes(block: &[u8]) -> [u8; EQUALIZER_BAND_COUNT] {
    let mut bytes = [0xff; EQUALIZER_BAND_COUNT];
    if block.first() != Some(&0xff) {
        bytes.copy_from_slice(block);
    } else {
        bytes[EQUALIZER_BAND_COUNT - 1] = 0;
    }
    bytes
}

/// The legacy parser maps an unknown hear-id type to its default, `Initial`.
const fn mapped_hear_id_type(byte: u8) -> u8 {
    match byte {
        0..=2 => byte,
        _ => 0,
    }
}
