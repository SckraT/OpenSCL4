//! A3957 (Liberty 5) driver taken from the legacy `devices/soundcore/a3957` driver.
//!
//! Body offsets are the legacy parser's positions in the `01 01` state body.

use crate::{
    BatteryLevels, DeviceState, EqualizerSetting, FirmwareVersion, ModelDriver, ModelError,
    RedactedSerial, SoundMode, battery_percent, canonical_band_bytes,
};

/// Body length of the measured A3957 pilot state.
pub const STATE_LENGTH: usize = 154;
/// Body offsets of the sound-mode controls in the measured layout.
pub const A3957_SOUND_MODE_FIELDS: std::ops::Range<usize> = 119..126;
/// Body offset of the measured gaming-mode flag.
pub const A3957_GAMING_MODE: usize = 146;
/// Body offsets of the equalizer preset id in the measured layout.
pub const A3957_EQUALIZER_PRESET: std::ops::Range<usize> = 38..40;
/// Body offsets of the equalizer band gains in the measured layout.
pub const A3957_EQUALIZER_BANDS: std::ops::Range<usize> = 40..50;
/// Number of equalizer bands the driver exposes from the ten-band curve.
pub const VISIBLE_EQUALIZER_BANDS: usize = 8;

const SOUND_MODE: usize = A3957_SOUND_MODE_FIELDS.start;
const BATTERY_SCALE: u8 = 10;
/// The measured battery scale counts from one, so raw level zero is one tenth.
const BATTERY_OFFSET: u8 = 1;
const CASE_BATTERY: usize = 37;
/// Body offsets of the two hear-id adjustment channels.
const HEAR_ID_VOLUME: std::ops::Range<usize> = 62..82;
/// Body offsets of the big-endian hear-id timestamp.
const HEAR_ID_TIME: std::ops::Range<usize> = 82..86;
/// Body offset of the hear-id type.
const HEAR_ID_TYPE: usize = 86;
/// Body offsets of the two hear-id custom adjustment channels.
const HEAR_ID_CUSTOM: std::ops::Range<usize> = 87..107;
/// Body offsets of the two-byte favorite music genre.
const HEAR_ID_GENRE: std::ops::Range<usize> = 107..109;
/// Length of the documented `03 87` equalizer body.
const EQUALIZER_WRITE_LENGTH: usize = 114;

/// Reads the model-specific serial from the measured state layout.
pub fn state_serial(body: &[u8]) -> Result<RedactedSerial<'_>, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    RedactedSerial::parse(&body[16..32])
}

/// Whether the body has the one state layout measured for this model.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if !state_serial(body)?.identifies(ModelDriver::A3957) {
        return Err(ModelError::UnknownModel);
    }
    let sound_mode = SoundMode::decode(body[SOUND_MODE])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    Ok(DeviceState {
        model: ModelDriver::A3957,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[2], BATTERY_SCALE, BATTERY_OFFSET)?,
            right: battery_percent(body[3], BATTERY_SCALE, BATTERY_OFFSET)?,
            case: battery_percent(body[CASE_BATTERY], BATTERY_SCALE, BATTERY_OFFSET)?,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(crate::SoundModeControls::A3957(
            body[A3957_SOUND_MODE_FIELDS.clone()]
                .try_into()
                .map_err(|_| ModelError::IncompleteState)?,
        )),
        equalizer: equalizer_setting(body),
        gaming_mode: gaming_mode(body),
        auto_power_off: None,
        spatial_audio: None,
        press_sensitivity: None,
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[6..11])?,
        firmware_right: FirmwareVersion::parse(&body[11..16])?,
    })
}

/// Reads the gaming-mode flag only for the two documented values.
fn gaming_mode(body: &[u8]) -> Option<bool> {
    match body.get(A3957_GAMING_MODE) {
        Some(0) => Some(false),
        Some(1) => Some(true),
        _ => None,
    }
}

/// Reads the eight exposed bands of the ten-band curve. A channel that starts
/// with the absent marker reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3957_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; VISIBLE_EQUALIZER_BANDS] = body
        .get(A3957_EQUALIZER_BANDS)?
        .get(..VISIBLE_EQUALIZER_BANDS)?
        .try_into()
        .ok()?;
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

/// Builds the documented `03 87` equalizer write from a fresh state response.
///
/// The body carries the requested curve, the state's hear-id block with hear id
/// disabled, and the driver-compensated active curve.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !setting.is_writable()
        || setting.volume_adjustments[VISIBLE_EQUALIZER_BANDS..]
            .iter()
            .any(|gain| *gain != 0)
    {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    if !has_confirmed_layout(state) {
        return Err(ModelError::IncompleteState);
    }
    let bands = setting.volume_adjustments.map(EqualizerSetting::band_byte);
    let mut body = Vec::with_capacity(EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend_from_slice(&state[HEAR_ID_GENRE]);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&[0, 0]);
    // The driver never enables hear id on a write.
    body.push(0);
    for channel in state[HEAR_ID_VOLUME].chunks(10) {
        body.extend_from_slice(&hear_id_channel_bytes(channel, false));
    }
    body.extend_from_slice(&state[HEAR_ID_TIME]);
    body.push(match state[HEAR_ID_TYPE] {
        0..=2 => state[HEAR_ID_TYPE],
        _ => 0,
    });
    for channel in state[HEAR_ID_CUSTOM].chunks(10) {
        body.extend_from_slice(&hear_id_channel_bytes(channel, false));
    }
    // Both channels carry the same requested curve; the driver interleaves it
    // with zero bytes for the compensated block.
    for _ in 0..2 {
        for band in bands {
            body.extend_from_slice(&[band, 0]);
        }
    }
    body.extend_from_slice(&[0, 0]);
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}

/// Re-encodes one hear-id channel, keeping the driver's absent-channel marker.
fn hear_id_channel_bytes(
    channel: &[u8],
    zero_last_byte_when_absent: bool,
) -> [u8; crate::EQUALIZER_BANDS] {
    if channel.first() == Some(&u8::MAX) {
        let mut bytes = [u8::MAX; crate::EQUALIZER_BANDS];
        if zero_last_byte_when_absent {
            bytes[crate::EQUALIZER_BANDS - 1] = 0;
        }
        bytes
    } else {
        canonical_band_bytes(channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The measured `01 01` body from the legacy driver's
    /// `with_liberty5_packet_from_issue_226` test, with the serial replaced by a
    /// synthetic model-prefixed value.
    const STATE: &[u8] = include_bytes!("../../../data/a3957-pilot-state.bin");

    fn state_body(fixture: &[u8]) -> &[u8] {
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn reads_the_measured_state_with_its_charging_formats() {
        let state = decode_state(state_body(STATE)).expect("measured A3957 state");
        assert_eq!(state.model, ModelDriver::A3957);
        // The measured scale is ten levels counted from one.
        assert_eq!(state.battery.left, Some(90));
        assert_eq!(state.battery.right, Some(90));
        assert_eq!(state.battery.case, Some(30));
        assert_eq!(
            state.firmware_left.map(|v| v.to_string()),
            Some("03.90".into())
        );
        assert_eq!(
            state.firmware_right.map(|v| v.to_string()),
            Some("03.90".into())
        );
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.gaming_mode, Some(false));
        // The capture reports the Soundcore Signature preset: a flat curve.
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0; crate::EQUALIZER_BANDS],
            })
        );
    }

    #[test]
    fn reads_the_documented_sound_settings() {
        // The legacy test asserts these values from the same capture: ambient
        // noise canceling, manual strength 5, manual noise canceling, wind
        // suppression off and transportation plane.
        let state = decode_state(state_body(STATE)).expect("measured A3957 state");
        let Some(crate::SoundModeControls::A3957(controls)) = state.sound_mode_controls else {
            panic!("the measured state carries its seven sound-mode bytes");
        };
        assert_eq!(controls[0], 0);
        assert_eq!(controls[1] >> 4, 5);
        assert_eq!(controls[3], 0);
        assert_eq!(controls[4] & 1, 0);
        assert_eq!(controls[6], 0);
    }

    #[test]
    fn rejects_a_state_with_another_serial_prefix() {
        let mut body = state_body(STATE).to_vec();
        body[16..32].copy_from_slice(b"3953TEST00000000");
        assert_eq!(decode_state(&body), Err(ModelError::UnknownModel));
    }

    #[test]
    fn undocumented_gaming_flags_are_unavailable() {
        let mut body = state_body(STATE).to_vec();
        body[A3957_GAMING_MODE] = 2;
        assert_eq!(decode_state(&body).unwrap().gaming_mode, None);
        body[A3957_GAMING_MODE] = 1;
        assert_eq!(decode_state(&body).unwrap().gaming_mode, Some(true));
    }

    #[test]
    fn equalizer_write_echoes_the_captured_hear_id_block() {
        let state = state_body(STATE);
        let setting = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [60, -60, 60, -60, 60, -60, 60, -60, 0, 0],
        };
        let body = equalizer_write_body(setting, state).expect("documented equalizer write");
        assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
        assert_eq!(body[0..2], [0xfe, 0xfe]);
        // The captured genre, then the requested curve for the first ten bands.
        assert_eq!(body[2..4], [0, 0]);
        assert_eq!(
            body[4..24],
            [
                180, 60, 180, 60, 180, 60, 180, 60, 120, 120, 180, 60, 180, 60, 180, 60, 180, 60,
                120, 120,
            ]
        );
        assert_eq!(body[24..27], [0, 0, 0]);
        // The captured hear-id channels, timestamp and type follow unchanged.
        assert_eq!(body[27..37], canonical_band_bytes(&state[62..72]));
        assert_eq!(body[37..47], canonical_band_bytes(&state[72..82]));
        assert_eq!(body[47..51], state[82..86]);
        assert_eq!(body[51], state[86]);
        assert_eq!(body[52..62], canonical_band_bytes(&state[87..97]));
        assert_eq!(body[62..72], canonical_band_bytes(&state[97..107]));
        // The active curve repeats for both channels, interleaved with zeros.
        let interleaved = [
            180, 0, 60, 0, 180, 0, 60, 0, 180, 0, 60, 0, 180, 0, 60, 0, 120, 0, 120, 0,
        ];
        assert_eq!(body[72..92], interleaved);
        assert_eq!(body[92..112], interleaved);
        assert_eq!(body[112..], [0, 0]);
    }

    #[test]
    fn equalizer_write_rejects_unexposed_bands_and_short_states() {
        let state = state_body(STATE);
        let mut unexposed = [0i16; crate::EQUALIZER_BANDS];
        unexposed[VISIBLE_EQUALIZER_BANDS] = 1;
        assert_eq!(
            equalizer_write_body(
                EqualizerSetting {
                    preset_id: 0,
                    volume_adjustments: unexposed,
                },
                state,
            ),
            Err(ModelError::InvalidEqualizerWrite)
        );
        assert_eq!(
            equalizer_write_body(
                EqualizerSetting {
                    preset_id: 0,
                    volume_adjustments: [0; crate::EQUALIZER_BANDS],
                },
                &state[..STATE_LENGTH - 1],
            ),
            Err(ModelError::IncompleteState)
        );
    }
}
