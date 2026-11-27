use crate::{
    AmbientSoundModeCycle, AutoPowerOff, BatteryLevels, ButtonAction, DeviceState,
    EqualizerSetting, FirmwareVersion, LimitHighVolume, ModelDriver, ModelError, PeerDevice,
    PeersPage, RedactedSerial, SettingsSnapshot, SoundMode, SoundModeControls,
    VolumeLimitRefreshRate, battery_percent,
};

const BATTERY: usize = 0;
const BATTERY_SCALE: u8 = 10;
const BATTERY_OFFSET: u8 = 1;
const FIRMWARE: std::ops::Range<usize> = 2..7;
const SERIAL: std::ops::Range<usize> = 7..23;
const SOUND_MODE: usize = 69;
const SOUND_MODE_CONTROLS: std::ops::Range<usize> = 70..75;
/// Body length of the measured A3062 pilot state.
pub const STATE_LENGTH: usize = 95;
/// Body length of the legacy state parser: the measured state has eight trailing
/// bytes the legacy driver does not read.
pub const PARSED_LENGTH: usize = 87;
/// Body offsets of the mode and its controls in the measured A3062 state layout.
pub const A3062_SOUND_MODE_FIELDS: std::ops::Range<usize> = SOUND_MODE..SOUND_MODE_CONTROLS.end;
/// Body offsets of the preset id in the measured A3062 state layout.
pub const A3062_EQUALIZER_PRESET: std::ops::Range<usize> = 23..25;
/// Body offsets of the band gains in the measured A3062 state layout.
pub const A3062_EQUALIZER_BANDS: std::ops::Range<usize> = 25..35;
/// Body offset of the double-press button action.
pub const A3062_BUTTON_CONFIGURATION: usize = 67;
/// Body offset of the ambient sound mode cycle flags.
pub const A3062_SOUND_MODE_CYCLE: usize = 68;
/// Body offset of the low battery prompt flag.
pub const A3062_LOW_BATTERY_PROMPT: usize = 76;
/// Body offset of the Dolby Audio flag.
pub const A3062_DOLBY_AUDIO: usize = 77;
/// Body offset of the LDAC flag.
pub const A3062_LDAC: usize = 78;
/// Body offset of the dual-connections enabled flag.
pub const A3062_DUAL_CONNECTIONS: usize = 79;
/// Body offsets of the auto power off flag and duration index.
pub const A3062_AUTO_POWER_OFF: std::ops::Range<usize> = 80..82;
/// Body offsets of the high-volume limiter fields.
pub const A3062_LIMIT_HIGH_VOLUME: std::ops::Range<usize> = 82..85;
/// Body offset of the side tone flag.
pub const A3062_SIDE_TONE: usize = 85;
/// Body offset of the ambient sound mode voice prompt flag.
pub const A3062_VOICE_PROMPT: usize = 86;
const HEAR_ID_VOLUME: std::ops::Range<usize> = 38..48;
const HEAR_ID_TIME: std::ops::Range<usize> = 48..52;
const HEAR_ID_TYPE: usize = 52;
const HEAR_ID_CUSTOM: std::ops::Range<usize> = 53..63;
const HEAR_ID_GENRE: std::ops::Range<usize> = 63..65;
/// Length of the measured A3062 equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 53;

pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if body.len() < PARSED_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[SERIAL])
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < PARSED_LENGTH {
        return Err(ModelError::IncompleteState);
    }
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
            device: battery_percent(body[BATTERY], BATTERY_SCALE, BATTERY_OFFSET)?,
            left: None,
            right: None,
            case: None,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: body
            .get(SOUND_MODE_CONTROLS)
            .and_then(|controls| controls.try_into().ok())
            .map(SoundModeControls::A3062),
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: auto_power_off(body),
        firmware_device: FirmwareVersion::parse(&body[FIRMWARE])?,
        firmware_left: None,
        firmware_right: None,
        settings: settings(body),
    })
}

/// Reads the equalizer curve from the measured state layout. A band block that
/// starts with the absent marker reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3062_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; 10] = body.get(A3062_EQUALIZER_BANDS)?.try_into().ok()?;
    if bands[0] == u8::MAX {
        return None;
    }
    Some(EqualizerSetting {
        preset_id: u16::from_le_bytes(preset_id),
        volume_adjustments: bands.map(EqualizerSetting::band_gain),
    })
}

/// Reads auto power off. The duration index must be in the family's half-hour
/// table; an unrecognized value is not reported as a confirmed setting.
fn auto_power_off(body: &[u8]) -> Option<AutoPowerOff> {
    let [enabled, duration]: [u8; 2] = body.get(A3062_AUTO_POWER_OFF)?.try_into().ok()?;
    let enabled = match enabled {
        0 => false,
        1 => true,
        _ => return None,
    };
    if duration >= crate::AUTO_POWER_OFF_DURATION_COUNT {
        return None;
    }
    Some(AutoPowerOff { enabled, duration })
}

/// Reads the confirmed flags and button configuration the legacy driver exposes.
fn settings(body: &[u8]) -> SettingsSnapshot {
    let enabled = |offset: usize| body.get(offset).map(|byte| *byte == 1);
    let limit_high_volume = body
        .get(A3062_LIMIT_HIGH_VOLUME)
        .and_then(|fields| <[u8; 3]>::try_from(fields).ok())
        .map(|[enabled, db_limit, refresh_rate]| LimitHighVolume {
            enabled: enabled == 1,
            db_limit,
            refresh_rate: VolumeLimitRefreshRate::decode(refresh_rate),
        });
    SettingsSnapshot {
        button_double_press: body
            .get(A3062_BUTTON_CONFIGURATION)
            .and_then(|byte| ButtonAction::decode(*byte)),
        ambient_sound_mode_cycle: body
            .get(A3062_SOUND_MODE_CYCLE)
            .map(|byte| AmbientSoundModeCycle::decode(*byte)),
        limit_high_volume,
        dual_connections: enabled(A3062_DUAL_CONNECTIONS),
        ldac: enabled(A3062_LDAC),
        low_battery_prompt: enabled(A3062_LOW_BATTERY_PROMPT),
        dolby_audio: enabled(A3062_DOLBY_AUDIO),
        side_tone: enabled(A3062_SIDE_TONE),
        voice_prompt: enabled(A3062_VOICE_PROMPT),
    }
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

/// Builds the measured three-byte button-configuration write.
///
/// The measured write repeats two zero bytes and the action; `0x0F` disables
/// the double-press action.
pub fn button_configuration_write(action: Option<ButtonAction>) -> [u8; 3] {
    [0, 0, action.map_or(0x0f, ButtonAction::wire)]
}

/// Builds the ambient sound mode cycle write body.
pub fn sound_mode_cycle_write(cycle: AmbientSoundModeCycle) -> [u8; 1] {
    [cycle.encode()]
}

/// Builds the measured high-volume limiter write body.
///
/// The legacy builder uses a five-decibel grid between 75 and 100 dB.
pub fn limit_high_volume_write(enabled: bool, db_limit: u8) -> Result<[u8; 2], ModelError> {
    if !(75..=100).contains(&db_limit) || !db_limit.is_multiple_of(5) {
        return Err(ModelError::InvalidLimitHighVolumeWrite);
    }
    Ok([u8::from(enabled), db_limit])
}

/// Builds the high-volume limiter refresh-rate write body.
pub fn limit_high_volume_refresh_rate_write(refresh_rate: VolumeLimitRefreshRate) -> [u8; 1] {
    [refresh_rate.encode()]
}

/// Builds the measured A3062 equalizer write from a fresh state response.
///
/// The write repeats the requested curve, carries the hear-id fields of the
/// current state with hear-id disabled so the curve takes effect, and ends
/// with the active curve again.
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
    // The favorite music genre precedes the curve on the write.
    body.extend_from_slice(&state[HEAR_ID_GENRE]);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&[0, 0]);
    // The measured write disables hear id so the requested curve is in effect.
    body.push(0);
    body.extend_from_slice(&state[HEAR_ID_VOLUME]);
    body.extend_from_slice(&state[HEAR_ID_TIME]);
    body.push(state[HEAR_ID_TYPE]);
    body.extend_from_slice(&state[HEAR_ID_CUSTOM]);
    // The active curve repeats after the hear-id fields.
    body.extend_from_slice(&bands);
    body.push(0);
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}

/// Splits a multiple-packet `0B 01` response into one page.
///
/// Each device entry starts with its total length (including the length byte),
/// a connected flag, the six Bluetooth address bytes and the padded name.
/// Names are padded with zeroes and can end in the middle of a UTF-8
/// character; the valid prefix is kept, as the legacy parser does.
pub fn decode_peers_page(body: &[u8]) -> Result<PeersPage, ModelError> {
    let Some((&total_packets, rest)) = body.split_first() else {
        return Err(ModelError::InvalidPeers);
    };
    let Some((&current_packet_index, mut input)) = rest.split_first() else {
        return Err(ModelError::InvalidPeers);
    };
    let mut devices = Vec::new();
    while !input.is_empty() {
        let (&length, rest) = input.split_first().ok_or(ModelError::InvalidPeers)?;
        let length = usize::from(length);
        if length < 8 || rest.len() < length - 1 {
            return Err(ModelError::InvalidPeers);
        }
        let connected = rest[0] == 1;
        let mac: [u8; 6] = rest[1..7]
            .try_into()
            .map_err(|_| ModelError::InvalidPeers)?;
        let name_field = &rest[7..length - 1];
        let name_bytes = name_field
            .iter()
            .position(|byte| *byte == 0)
            .map_or(name_field, |end| &name_field[..end]);
        let name = match std::str::from_utf8(name_bytes) {
            Ok(name) => name.to_owned(),
            Err(error) => String::from_utf8_lossy(&name_bytes[..error.valid_up_to()]).into_owned(),
        };
        devices.push(PeerDevice {
            connected,
            mac,
            name,
        });
        input = &rest[length - 1..];
    }
    Ok(PeersPage {
        total_packets,
        current_packet_index,
        devices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &[u8] = include_bytes!("../../../data/a3062-pilot-state.bin");
    const PEERS: &[u8] = include_bytes!("../../../data/a3062-dual-connections.bin");

    /// One complete inbound frame is nine header bytes, the body, and a checksum.
    fn state_body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], [0x01, 0x01]);
        &fixture[9..fixture.len() - 1]
    }

    fn peers_body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], [0x0b, 0x01]);
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn reads_the_soundcore_signature_curve_from_the_pilot_state() {
        let state = decode_state(state_body(STATE)).expect("pilot state");
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
            })
        );
    }

    #[test]
    fn reads_the_legacy_flags_from_the_pilot_state() {
        let state = decode_state(state_body(STATE)).expect("pilot state");
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1
            })
        );
        assert_eq!(
            state.settings,
            SettingsSnapshot {
                button_double_press: None,
                ambient_sound_mode_cycle: Some(AmbientSoundModeCycle {
                    noise_canceling: true,
                    transparency: true,
                    normal: false,
                }),
                limit_high_volume: Some(LimitHighVolume {
                    enabled: false,
                    db_limit: 90,
                    refresh_rate: VolumeLimitRefreshRate::RealTime,
                }),
                dual_connections: Some(true),
                ldac: Some(false),
                low_battery_prompt: Some(true),
                dolby_audio: Some(false),
                side_tone: Some(true),
                voice_prompt: Some(true),
            }
        );
    }

    #[test]
    fn absent_band_block_reports_no_curve() {
        let mut body = state_body(STATE).to_vec();
        body[A3062_EQUALIZER_BANDS.start] = u8::MAX;
        let state = decode_state(&body).expect("pilot state with an absent curve");
        assert_eq!(state.equalizer, None);
    }

    #[test]
    fn equalizer_write_matches_the_measured_packet() {
        // The hear-id fields come from a state carrying the captured values.
        let mut state = state_body(STATE).to_vec();
        state[HEAR_ID_VOLUME].copy_from_slice(&[120, 120, 120, 120, 120, 120, 120, 120, 120, 60]);
        state[HEAR_ID_TIME].copy_from_slice(&[0, 0, 0, 0]);
        state[HEAR_ID_TYPE] = 2;
        state[HEAR_ID_CUSTOM].copy_from_slice(&[130, 130, 130, 130, 130, 130, 130, 130, 130, 60]);
        let setting = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [-60, 5, -60, 28, -7, -25, 24, -9, 0, 0],
        };
        let body = equalizer_write_body(setting, &state).expect("measured write");
        assert_eq!(
            body,
            vec![
                254, 254, // preset
                0, 0, // hear-id favorite music genre
                60, 125, 60, 148, 113, 95, 144, 111, 120, 120, // volume adjustments
                0, 0, // unknown
                0, // hear id disabled
                120, 120, 120, 120, 120, 120, 120, 120, 120, 60, // hear-id adjustments
                0, 0, 0, 0, // hear-id time
                2, // hear-id type
                130, 130, 130, 130, 130, 130, 130, 130, 130, 60, // hear-id custom
                60, 125, 60, 148, 113, 95, 144, 111, 120, 120, // active curve
                0,   // unknown
            ]
        );
    }

    #[test]
    fn equalizer_write_rejects_a_state_without_the_hear_id_block() {
        let setting = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; 10],
        };
        let short = &state_body(STATE)[..HEAR_ID_GENRE.end - 1];
        assert_eq!(
            equalizer_write_body(setting, short),
            Err(ModelError::IncompleteState)
        );
    }

    #[test]
    fn equalizer_write_accepts_only_representable_gains() {
        let mut setting = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [-120, 134, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        let state = state_body(STATE);
        let body = equalizer_write_body(setting, state).expect("both bounds are representable");
        assert_eq!(body[4..6], [0, 254]);
        for invalid in [i16::MIN, -121, 135, i16::MAX] {
            setting.volume_adjustments[9] = invalid;
            assert_eq!(
                equalizer_write_body(setting, state),
                Err(ModelError::InvalidEqualizerWrite)
            );
        }
    }

    #[test]
    fn equalizer_write_is_rejected_for_models_without_the_capability() {
        let setting = EqualizerSetting {
            preset_id: 0,
            volume_adjustments: [0; 10],
        };
        assert_eq!(
            ModelDriver::A3954.equalizer_write_body(setting, state_body(STATE)),
            Err(ModelError::InvalidEqualizerWrite)
        );
    }

    #[test]
    fn writes_match_the_legacy_builders() {
        assert_eq!(
            button_configuration_write(Some(ButtonAction::BassUp)),
            [0, 0, 7]
        );
        assert_eq!(button_configuration_write(None), [0, 0, 0x0f]);
        assert_eq!(
            sound_mode_cycle_write(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: true,
                normal: false,
            }),
            [3]
        );
        assert_eq!(limit_high_volume_write(true, 90), Ok([1, 90]));
        assert_eq!(limit_high_volume_write(false, 80), Ok([0, 80]));
        for invalid in [0, 74, 76, 101, 255] {
            assert_eq!(
                limit_high_volume_write(true, invalid),
                Err(ModelError::InvalidLimitHighVolumeWrite)
            );
        }
        assert_eq!(
            limit_high_volume_refresh_rate_write(VolumeLimitRefreshRate::OneMinute),
            [2]
        );
    }

    #[test]
    fn peers_capture_is_masked_and_decoded() {
        let page = decode_peers_page(peers_body(PEERS)).expect("captured peer list");
        assert_eq!(page.total_packets, 1);
        assert_eq!(page.current_packet_index, 1);
        assert_eq!(
            page.devices,
            vec![
                PeerDevice {
                    connected: true,
                    mac: [0x02, 0, 0, 0, 0, 0x01],
                    name: "Desktop".to_owned(),
                },
                PeerDevice {
                    connected: false,
                    mac: [0x02, 0, 0, 0, 0, 0x02],
                    name: "Phone model".to_owned(),
                },
            ]
        );
    }

    #[test]
    fn malformed_peer_pages_are_rejected() {
        for body in [
            Vec::new(),
            vec![1],
            vec![1, 1, 0x28],
            vec![1, 1, 0x07, 0, 0, 0, 0, 0, 0],
        ] {
            assert_eq!(
                decode_peers_page(&body).map_err(|_| ()),
                Err(()),
                "body {body:?}"
            );
        }
    }
}
