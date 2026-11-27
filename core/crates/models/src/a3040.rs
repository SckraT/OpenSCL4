use crate::{
    AmbientSoundModeCycle, BatteryLevels, ButtonAction, DeviceState, EqualizerSetting,
    FirmwareVersion, LimitHighVolume, ModelDriver, ModelError, RedactedSerial, SettingsSnapshot,
    SoundMode, SoundModeControls, VolumeLimitRefreshRate, apply_drc, band_byte, band_gain,
    battery_percent,
};

/// Body length of the measured A3040 state response.
pub const STATE_LENGTH: usize = 142;
const BATTERY: usize = 0;
const FIRMWARE: std::ops::Range<usize> = 2..7;
const SERIAL: std::ops::Range<usize> = 7..23;
/// Body offsets of the preset id in the measured state layout.
pub const A3040_EQUALIZER_PRESET: std::ops::Range<usize> = 23..25;
/// Body offsets of the ten band gains in the measured state layout.
pub const A3040_EQUALIZER_BANDS: std::ops::Range<usize> = 25..35;
/// Body offset of the double-press button action.
pub const A3040_BUTTON_CONFIGURATION: usize = 47;
/// Body offset of the ambient sound mode cycle flags.
pub const A3040_SOUND_MODE_CYCLE: usize = 48;
/// Body offsets of the mode and its five controls in the measured layout.
pub const A3040_SOUND_MODE_FIELDS: std::ops::Range<usize> = 49..55;
/// Body offset of the LDAC flag.
pub const A3040_LDAC: usize = 61;
/// Body offset of the dual-connections enabled flag.
pub const A3040_DUAL_CONNECTIONS: usize = 62;
/// Body offsets of the auto power off flag and duration index.
pub const A3040_AUTO_POWER_OFF: std::ops::Range<usize> = 65..67;
/// Body offsets of the high-volume limiter fields.
pub const A3040_LIMIT_HIGH_VOLUME: std::ops::Range<usize> = 67..70;
/// Body offset of the voice prompt flag.
pub const A3040_VOICE_PROMPT: usize = 70;
/// Body offset of the low battery prompt flag.
pub const A3040_LOW_BATTERY_PROMPT: usize = 71;
const HEAR_ID_VOLUME: std::ops::Range<usize> = 78..98;
const HEAR_ID_TIME: std::ops::Range<usize> = 98..102;
const HEAR_ID_TYPE: usize = 102;
const HEAR_ID_CUSTOM: std::ops::Range<usize> = 103..123;
const HEAR_ID_GENRE: std::ops::Range<usize> = 133..135;
const HEAR_ID_VOLUME_BANDS: usize = 10;
const HEAR_ID_CHANNELS: usize = 2;

const BATTERY_SCALE: u8 = 5;
/// Number of equalizer bands carried by the model's curve.
pub const EQUALIZER_BAND_COUNT: usize = 10;
/// Length of the measured equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 93;

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
    if !state_serial(body)?.identifies(ModelDriver::A3040) {
        return Err(ModelError::UnknownModel);
    }
    let sound_mode_fields: [u8; 6] = body[A3040_SOUND_MODE_FIELDS]
        .try_into()
        .map_err(|_| ModelError::IncompleteState)?;
    let sound_mode = SoundMode::decode(sound_mode_fields[0])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    // The legacy structure reads the packed noise-canceling controls and clamps
    // them to 1–5 (manual) and 0–5 (adaptive).
    let packed = sound_mode_fields[1];
    let manual_noise_canceling = ((packed >> 4).clamp(1, 5)) << 4;
    let adaptive_noise_canceling = (packed & 0x0f).clamp(0, 5);
    let transparency_mode = match sound_mode_fields[2] {
        0 | 1 => sound_mode_fields[2],
        _ => 0,
    };
    let noise_canceling_mode = match sound_mode_fields[3] {
        0 | 1 => sound_mode_fields[3],
        _ => 0,
    };
    let wind_noise_reduction = u8::from(sound_mode_fields[4] == 1);
    let manual_transparency = sound_mode_fields[5];
    let auto_power_off = match body[A3040_AUTO_POWER_OFF.start] {
        0 => Some(crate::AutoPowerOff {
            enabled: false,
            duration: body[A3040_AUTO_POWER_OFF.start + 1],
        }),
        1 => Some(crate::AutoPowerOff {
            enabled: true,
            duration: body[A3040_AUTO_POWER_OFF.start + 1],
        }),
        _ => None,
    };
    Ok(DeviceState {
        model: ModelDriver::A3040,
        battery: BatteryLevels {
            device: battery_percent(body[BATTERY], BATTERY_SCALE, 0)?,
            left: None,
            right: None,
            case: None,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::A3040([
            manual_noise_canceling | adaptive_noise_canceling,
            transparency_mode,
            noise_canceling_mode,
            wind_noise_reduction,
            manual_transparency,
        ])),
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off,
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
        settings: SettingsSnapshot {
            button_double_press: ButtonAction::decode(body[A3040_BUTTON_CONFIGURATION]),
            ambient_sound_mode_cycle: Some(AmbientSoundModeCycle::decode(
                body[A3040_SOUND_MODE_CYCLE],
            )),
            limit_high_volume: limit_high_volume(body),
            dual_connections: Some(body[A3040_DUAL_CONNECTIONS] == 1),
            ldac: Some(body[A3040_LDAC] == 1),
            auto_play_pause: None,
            battery_alert: None,
            voice_prompt: Some(body[A3040_VOICE_PROMPT] == 1),
            low_battery_prompt: Some(body[A3040_LOW_BATTERY_PROMPT] == 1),
        },
    })
}

/// Reads the ten-band curve. A band block that starts with the absent marker
/// reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3040_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(A3040_EQUALIZER_BANDS)?.try_into().ok()?;
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
        body.get(A3040_LIMIT_HIGH_VOLUME)?.try_into().ok()?;
    Some(LimitHighVolume {
        enabled: enabled == 1,
        db_limit,
        refresh_rate: VolumeLimitRefreshRate::decode(refresh_rate),
    })
}

/// Builds the six-byte sound-mode write from the latest measured controls.
pub fn sound_mode_write(mode: SoundMode, controls: [u8; 5]) -> Result<[u8; 6], ModelError> {
    let [
        packed,
        transparency_mode @ 0..=1,
        noise_canceling_mode @ 0..=1,
        wind @ 0..=1,
        manual_transparency,
    ] = controls
    else {
        return Err(ModelError::InvalidSoundModeWrite);
    };
    if mode == SoundMode::AirplaneMode || !(1..=5).contains(&(packed >> 4)) || (packed & 0x0f) > 5 {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    Ok([
        mode.wire(),
        packed,
        transparency_mode,
        noise_canceling_mode,
        wind,
        manual_transparency,
    ])
}

/// Builds the measured equalizer write from a fresh state response.
///
/// The write repeats the requested curve in the model's two channel slots,
/// carries the hear-id fields of the current state with hear-id disabled, and
/// ends with two copies of the curve after dynamic range compression.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable() || !has_confirmed_layout(state) {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let bands = setting.volume_adjustments.map(band_byte);
    let mut body = Vec::with_capacity(EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend_from_slice(&state[HEAR_ID_GENRE]);
    // The requested curve fills the channel 1 block and the following block that
    // the measured write repeats it in.
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&[0, 1]);
    // The measured write disables hear id so the requested curve is in effect.
    body.push(0);
    for channel in 0..HEAR_ID_CHANNELS {
        let start = HEAR_ID_VOLUME.start + channel * HEAR_ID_VOLUME_BANDS;
        body.extend_from_slice(&hear_id_block(&state[start..start + HEAR_ID_VOLUME_BANDS]));
    }
    body.extend_from_slice(&state[HEAR_ID_TIME]);
    body.push(mapped_hear_id_type(state[HEAR_ID_TYPE]));
    // The measured write repeats the first custom hear-id channel in both slots.
    let custom = hear_id_block(&state[HEAR_ID_CUSTOM][..HEAR_ID_VOLUME_BANDS]);
    body.extend_from_slice(&custom);
    body.extend_from_slice(&custom);
    let compressed = apply_drc(setting.volume_adjustments).map(band_byte);
    body.extend_from_slice(&compressed);
    body.extend_from_slice(&compressed);
    body.push(0);
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}

/// Encodes one optional ten-byte hear-id block the way the legacy builder does:
/// the absent marker keeps nine `FF` bytes and a zero last byte.
fn hear_id_block(block: &[u8]) -> [u8; HEAR_ID_VOLUME_BANDS] {
    let mut bytes = [0xff; HEAR_ID_VOLUME_BANDS];
    if block.first() != Some(&0xff) {
        bytes.copy_from_slice(block);
    } else {
        bytes[HEAR_ID_VOLUME_BANDS - 1] = 0;
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

// The measured state carries two hear-id channels; the write body repeats the
// first channel, so the second is not otherwise used.
const _: () = assert!(HEAR_ID_CHANNELS * HEAR_ID_VOLUME_BANDS == 20);
