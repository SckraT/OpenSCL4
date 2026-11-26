use crate::{
    BatteryLevels, DeviceState, EqualizerSetting, FirmwareVersion, ModelDriver, ModelError,
    RedactedSerial, SoundMode, SoundModeControls, battery_percent,
};

const SOUND_MODE: usize = 69;
const SOUND_MODE_CONTROLS: std::ops::Range<usize> = 70..75;
/// Body offsets of the preset id in the measured A3062 state layout.
pub const A3062_EQUALIZER_PRESET: std::ops::Range<usize> = 23..25;
/// Body offsets of the band gains in the measured A3062 state layout.
pub const A3062_EQUALIZER_BANDS: std::ops::Range<usize> = 25..35;
const HEAR_ID_VOLUME: std::ops::Range<usize> = 38..48;
const HEAR_ID_TIME: std::ops::Range<usize> = 48..52;
const HEAR_ID_TYPE: usize = 52;
const HEAR_ID_CUSTOM: std::ops::Range<usize> = 53..63;
const HEAR_ID_GENRE: std::ops::Range<usize> = 63..65;
/// Length of the measured A3062 equalizer write body.
const EQUALIZER_WRITE_LENGTH: usize = 53;

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
        sound_mode: Some(sound_mode),
        sound_mode_controls: body
            .get(SOUND_MODE_CONTROLS)
            .and_then(|controls| controls.try_into().ok())
            .map(SoundModeControls::A3062),
        equalizer: equalizer_setting(body),
        firmware_device: FirmwareVersion::parse(&body[2..7])?,
        firmware_left: None,
        firmware_right: None,
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

/// Builds the measured A3062 equalizer write from a fresh state response.
///
/// The write repeats the requested curve, carries the hear-id fields of the
/// current state with hear-id disabled so the curve takes effect, and ends
/// with the active curve again.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
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

#[cfg(test)]
mod tests {
    use super::*;

    const STATE: &[u8] = include_bytes!("../../../data/a3062-pilot-state.bin");

    /// One complete inbound frame is nine header bytes, the body, and a checksum.
    fn state_body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], [0x01, 0x01]);
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
}
