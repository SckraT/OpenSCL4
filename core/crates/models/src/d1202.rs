//! Driver for the earbuds that share the legacy `D1202` driver.
//!
//! The layouts and every command body come from
//! `lib/src/devices/soundcore/d1202/` and its packet builders. The measured
//! bodies and their provenance are documented in `core/data/README.md`.
//!
//! Both public codes report the same four-digit model number `1202` in the
//! state serial, so the wire format names the family but not the code. The code
//! is selected from the recorded Soundcore RFCOMM service UUID, which is
//! connection metadata rather than a protocol response.

use std::ops::Range;

use crate::{
    AutoPowerOff, BatteryLevels, ConnectionSettings, DecibelReadingRefreshRate, DeviceState,
    EqualizerSetting, FirmwareVersion, HostDevice, LimitHighVolume, ModelDriver, ModelError,
    RedactedSerial, SoundMode, SoundModeControls, SpatialAudio, SpatialAudioMode, battery_percent,
};

/// Body length of both measured state layouts.
pub const STATE_LENGTH: usize = 152;
/// Body length parsed by the legacy state parser; the measured bodies carry
/// thirteen trailing bytes the legacy parser ignores.
const PARSED_LENGTH: usize = 139;
/// The state serial the family reports; both public codes use the same value.
pub const MODEL_NUMBER: &[u8; 4] = b"1202";
/// Body offsets of the state serial.
pub const SERIAL: Range<usize> = 16..32;
/// Body offsets of the equalizer preset id in the measured state layout.
pub const EQUALIZER_PRESET: Range<usize> = 38..40;
/// Body offsets of the first equalizer channel in the measured state layout.
pub const EQUALIZER_BANDS: Range<usize> = 40..50;
/// Body offset of the ambient sound mode.
pub const SOUND_MODE: usize = 119;
/// Body offsets of the remaining sound-mode controls.
pub const SOUND_MODE_CONTROLS: Range<usize> = 120..127;
/// Body offsets of the resumed sound-mode controls.
pub const SOUND_MODE_FIELDS: Range<usize> = SOUND_MODE..SOUND_MODE_CONTROLS.end;
/// Body offsets of the auto power off value.
pub const AUTO_POWER_OFF: Range<usize> = 132..134;
/// Body offsets of the spatial audio setting.
pub const SPATIAL_AUDIO: Range<usize> = 137..139;
/// Body offsets of the hear-id favorite music genre.
pub const HEAR_ID_GENRE: Range<usize> = 107..109;
const HEAR_ID_VOLUME: Range<usize> = 62..82;
const HEAR_ID_TIME: Range<usize> = 82..86;
const HEAR_ID_TYPE: usize = 86;
const HEAR_ID_CUSTOM: Range<usize> = 87..107;
const TWS_HOST: usize = 0;
const TWS_CONNECTED: usize = 1;
const BATTERY_LEVELS: Range<usize> = 2..4;
const FIRMWARE_LEFT: Range<usize> = 6..11;
const FIRMWARE_RIGHT: Range<usize> = 11..16;
const CASE_BATTERY: usize = 37;
const TOUCH_TONE: usize = 128;
const LOW_BATTERY_PROMPT: usize = 129;
const LDAC: usize = 130;
const DUAL_CONNECTIONS: usize = 131;
const LIMIT_HIGH_VOLUME: Range<usize> = 134..137;
/// Ten-step battery scale with a one-step offset.
const BATTERY_SCALE: (u8, u8) = (10, 1);
/// Length of the measured equalizer write body.
pub const EQUALIZER_WRITE_LENGTH: usize = 114;

/// The family's equalizer write command from the legacy packet builder.
pub const EQUALIZER_COMMAND: [u8; 2] = [0x03, 0x87];
/// The family's spatial audio write command from the legacy packet builder.
pub const SPATIAL_AUDIO_COMMAND: [u8; 2] = [0x10, 0x81];
/// Number of entries in the family's ten-to-sixty minute auto power off table.
pub const AUTO_POWER_OFF_DURATION_COUNT: u8 = 4;

/// Recorded Soundcore RFCOMM service UUID of the P31i (`D1202`) driver.
///
/// The value is the one recorded by the inherited device faker research
/// configuration; it shares the vendor prefix and the four-digit model number
/// with the R60i NC value below.
pub const D1202_SERVICE_ID: [u8; 16] = [
    0x0c, 0xf1, 0x2d, 0x31, 0xfa, 0xc3, 0x45, 0x53, 0xbd, 0x80, 0xd6, 0x83, 0x2e, 0x7d, 0x12, 0x02,
];
/// Recorded Soundcore RFCOMM service UUID of the R60i NC (`D1202C`) driver.
///
/// The value is recorded in the inherited device faker research configuration
/// and is confirmed by the Windows discovery log of a physical R60i NC
/// reported in upstream OpenSCQ30 issue #350.
pub const D1202C_SERVICE_ID: [u8; 16] = [
    0x0c, 0xf1, 0x2d, 0x31, 0xfa, 0xc3, 0x45, 0x53, 0xbd, 0x80, 0xd6, 0x83, 0x2e, 0x71, 0x20, 0x2c,
];

/// Selects the family code from the recorded connection metadata.
pub fn identify_service(service: &[u8; 16]) -> Option<ModelDriver> {
    if *service == D1202_SERVICE_ID {
        Some(ModelDriver::D1202)
    } else if *service == D1202C_SERVICE_ID {
        Some(ModelDriver::D1202C)
    } else {
        None
    }
}

/// Returns the recorded service UUID of a family code.
pub fn service_id(model: ModelDriver) -> Option<[u8; 16]> {
    match model {
        ModelDriver::D1202 => Some(D1202_SERVICE_ID),
        ModelDriver::D1202C => Some(D1202C_SERVICE_ID),
        _ => None,
    }
}

/// Reads the state serial shared by both public codes.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < SERIAL.end {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

/// Whether the state body is the measured full capture of either code.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

/// Whether both public codes report this serial.
pub fn serial_is_family(serial: &RedactedSerial<'_>) -> bool {
    serial.starts_with(MODEL_NUMBER)
}

pub fn decode_state(model: ModelDriver, body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < PARSED_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    if !serial_is_family(&state_serial(body)?) {
        return Err(ModelError::UnknownModel);
    }
    let host_device = match body[TWS_HOST] {
        0 => HostDevice::Left,
        1 => HostDevice::Right,
        _ => return Err(ModelError::InvalidStateValue),
    };
    let sound_mode = SoundMode::decode(body[SOUND_MODE])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    let controls: [u8; 7] = body[SOUND_MODE_CONTROLS]
        .try_into()
        .map_err(|_| ModelError::IncompleteState)?;
    Ok(DeviceState {
        model,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[BATTERY_LEVELS.start], BATTERY_SCALE.0, BATTERY_SCALE.1)?,
            right: battery_percent(
                body[BATTERY_LEVELS.start + 1],
                BATTERY_SCALE.0,
                BATTERY_SCALE.1,
            )?,
            case: battery_percent(body[CASE_BATTERY], BATTERY_SCALE.0, BATTERY_SCALE.1)?,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::D1202(controls)),
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: auto_power_off(body),
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[FIRMWARE_LEFT])?,
        firmware_right: FirmwareVersion::parse(&body[FIRMWARE_RIGHT])?,
        spatial_audio: Some(spatial_audio(body)),
        connection: Some(ConnectionSettings {
            tws_connected: body[TWS_CONNECTED] == 1,
            host_device,
            dual_connections_enabled: body[DUAL_CONNECTIONS] == 1,
            ldac: body[LDAC] == 1,
            touch_tone: body[TOUCH_TONE] == 1,
            low_battery_prompt: body[LOW_BATTERY_PROMPT] == 1,
            limit_high_volume: limit_high_volume(body),
        }),
    })
}

/// Reads the first equalizer channel. A block that starts with the absent
/// marker reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; 10] = body.get(EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

/// Reads the spatial audio setting. The legacy structure clamps an
/// unrecognized mode to its default, so the same value is reported.
fn spatial_audio(body: &[u8]) -> SpatialAudio {
    let [enabled, mode] = [body[SPATIAL_AUDIO.start], body[SPATIAL_AUDIO.start + 1]];
    SpatialAudio {
        enabled: enabled == 1,
        mode: SpatialAudioMode::decode(mode),
    }
}

/// Reads the high-volume limit. The legacy structure clamps an unrecognized
/// refresh rate to its default.
fn limit_high_volume(body: &[u8]) -> LimitHighVolume {
    LimitHighVolume {
        enabled: body[LIMIT_HIGH_VOLUME.start] == 1,
        db_limit: body[LIMIT_HIGH_VOLUME.start + 1],
        refresh_rate: DecibelReadingRefreshRate::decode(body[LIMIT_HIGH_VOLUME.start + 2]),
    }
}

/// Reads auto power off only when both the flag and the duration index are
/// recognized. An unparsed field is not reported as a confirmed value.
fn auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    let enabled = match *body.get(AUTO_POWER_OFF.start)? {
        0 => false,
        1 => true,
        _ => return None,
    };
    let duration = *body.get(AUTO_POWER_OFF.start + 1)?;
    if duration >= AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

/// Builds the eight-byte sound-mode write from the fresh state's controls.
pub fn ambient_sound_mode_write(mode: SoundMode, controls: [u8; 7]) -> Result<[u8; 8], ModelError> {
    if mode == SoundMode::AirplaneMode {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    let mut body = [0; 8];
    body[0] = mode.wire();
    body[1..].copy_from_slice(&controls);
    Ok(body)
}

/// Builds the measured equalizer write from a fresh state response.
///
/// The write carries both channels, repeats the state's hear-id fields with
/// hear-id disabled so the requested curve is in effect, and ends with the
/// interleaved active curve the legacy builder emits.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable() {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    if state.len() < HEAR_ID_GENRE.end {
        return Err(ModelError::IncompleteState);
    }
    let bands = setting.volume_adjustments.map(EqualizerSetting::band_byte);
    let mut body = Vec::with_capacity(EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend_from_slice(&state[HEAR_ID_GENRE]);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&[0, 0]);
    body.push(0);
    body.extend_from_slice(&state[HEAR_ID_VOLUME]);
    body.extend_from_slice(&state[HEAR_ID_TIME]);
    body.push(state[HEAR_ID_TYPE]);
    body.extend_from_slice(&state[HEAR_ID_CUSTOM]);
    for band in bands {
        body.extend_from_slice(&[band, 0]);
    }
    for band in bands {
        body.extend_from_slice(&[band, 0]);
    }
    body.extend_from_slice(&[0, 0]);
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}

/// The wire command, body and expected value of a spatial audio change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpatialAudioWrite {
    command: [u8; 2],
    body: Vec<u8>,
    expected: SpatialAudio,
}

impl SpatialAudioWrite {
    /// The command that carries the change.
    pub const fn command(&self) -> [u8; 2] {
        self.command
    }

    /// The command body to transmit.
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    /// The value a fresh state response must report.
    pub const fn expected(&self) -> SpatialAudio {
        self.expected
    }
}

/// Builds a spatial audio change from a fresh state response.
///
/// Enabling uses the family's `10 81` command. Disabling follows the legacy
/// state modifier and the official app: it sends the current equalizer
/// configuration with the spatial setting cleared rather than a `10 81` with
/// the enable flag unset.
pub fn spatial_audio_write(
    setting: SpatialAudio,
    state: &DeviceState,
    raw_state: &[u8],
) -> Result<SpatialAudioWrite, ModelError> {
    if setting.enabled {
        let mut body = Vec::with_capacity(3);
        body.extend_from_slice(&[1, 0, setting.mode.wire()]);
        return Ok(SpatialAudioWrite {
            command: SPATIAL_AUDIO_COMMAND,
            body,
            expected: SpatialAudio {
                enabled: true,
                mode: setting.mode,
            },
        });
    }
    let current = state
        .spatial_audio
        .ok_or(ModelError::InvalidSpatialAudioWrite)?;
    let equalizer = state
        .equalizer
        .ok_or(ModelError::InvalidSpatialAudioWrite)?;
    Ok(SpatialAudioWrite {
        command: EQUALIZER_COMMAND,
        body: equalizer_write_body(equalizer, raw_state)?,
        expected: SpatialAudio {
            enabled: false,
            mode: current.mode,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measured P31i state body from `core/data/d1202-pilot-state.bin`.
    const P31I: &[u8] = include_bytes!("../../../data/d1202-pilot-state.bin");
    /// The measured R60i NC state body from `core/data/d1202c-pilot-state.bin`.
    const R60I: &[u8] = include_bytes!("../../../data/d1202c-pilot-state.bin");

    fn body(fixture: &[u8]) -> &[u8] {
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn both_measured_bodies_carry_the_same_model_number() {
        for fixture in [P31I, R60I] {
            let serial = state_serial(body(fixture)).expect("family serial");
            assert!(serial_is_family(&serial));
            assert!(serial.starts_with(MODEL_NUMBER));
        }
    }

    #[test]
    fn equalizer_write_matches_the_legacy_builder_example() {
        // The state of the legacy `set_preset_eq` test: the curve block starts
        // with the present marker and both channels carry the same values.
        let state = body(P31I);
        let setting = EqualizerSetting {
            preset_id: 1,
            volume_adjustments: [40, 10, 20, 20, 40, 40, 40, 20, 0, -120],
        };
        let mut expected = vec![0x01, 0x00, 0x00, 0x00];
        let bands = [160, 130, 140, 140, 160, 160, 160, 140, 120, 0];
        expected.extend_from_slice(&bands);
        expected.extend_from_slice(&bands);
        expected.extend_from_slice(&[0, 0, 0]);
        expected.extend_from_slice(&state[HEAR_ID_VOLUME]);
        expected.extend_from_slice(&state[HEAR_ID_TIME]);
        expected.push(state[HEAR_ID_TYPE]);
        expected.extend_from_slice(&state[HEAR_ID_CUSTOM]);
        for band in bands {
            expected.extend_from_slice(&[band, 0]);
        }
        for band in bands {
            expected.extend_from_slice(&[band, 0]);
        }
        expected.extend_from_slice(&[0, 0]);
        assert_eq!(expected.len(), EQUALIZER_WRITE_LENGTH);
        assert_eq!(
            equalizer_write_body(setting, state).expect("measured write"),
            expected
        );
    }

    #[test]
    fn equalizer_write_rejects_unrepresentable_gains() {
        let setting = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [135, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        assert_eq!(
            equalizer_write_body(setting, body(P31I)),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }
}
