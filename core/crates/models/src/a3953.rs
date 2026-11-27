//! A3953 (Liberty 4) driver taken from the legacy `devices/soundcore/a3953` driver.
//!
//! Body offsets are the legacy parser's positions in the `01 01` state body; the
//! legacy code cites the decompiled application's own array indices, which are
//! nine bytes further into the complete frame.

use crate::{
    BatteryLevels, DeviceState, EqualizerSetting, FirmwareVersion, ModelDriver, ModelError,
    RedactedSerial, SoundMode, SoundModeControls, SpatialAudioState, battery_percent,
    canonical_band_bytes, drc_band_bytes,
};
/// Body length of the measured A3953 pilot state.
pub const STATE_LENGTH: usize = 158;
/// Body offsets of the sound mode and its controls in the measured layout.
pub const A3953_SOUND_MODE_FIELDS: std::ops::Range<usize> = 121..127;
/// Body offsets of the spatial-audio controls in the measured layout.
pub const A3953_SPATIAL_AUDIO: std::ops::Range<usize> = 146..149;
/// Body offset of the press sensitivity value in the measured layout.
pub const A3953_PRESS_SENSITIVITY: usize = 155;
/// Body offsets of the equalizer preset id in the measured layout.
pub const A3953_EQUALIZER_PRESET: std::ops::Range<usize> = 32..34;
/// Body offsets of the equalizer band gains in the measured layout.
pub const A3953_EQUALIZER_BANDS: std::ops::Range<usize> = 34..44;
/// Number of equalizer bands the driver exposes from the ten-band curve.
pub const VISIBLE_EQUALIZER_BANDS: usize = 8;

const SOUND_MODE: usize = A3953_SOUND_MODE_FIELDS.start;
const CASE_BATTERY: usize = 136;
const BATTERY_SCALE: u8 = 5;
/// Body offset of the one-byte hear-id initialization status.
const HEAR_ID_INITIALIZED: usize = 54;
/// Body offsets of the two hear-id adjustment channels.
const HEAR_ID_VOLUME: std::ops::Range<usize> = 56..76;
/// Body offsets of the big-endian hear-id timestamp.
const HEAR_ID_TIME: std::ops::Range<usize> = 76..80;
/// Body offset of the hear-id type.
const HEAR_ID_TYPE: usize = 80;
/// Body offsets of the two hear-id custom adjustment channels.
const HEAR_ID_CUSTOM: std::ops::Range<usize> = 81..101;
/// Body offsets of the two-byte favorite music genre.
const HEAR_ID_GENRE: std::ops::Range<usize> = 101..103;
/// Length of the documented `03 87` equalizer body.
const EQUALIZER_WRITE_LENGTH: usize = 93;

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
    if !state_serial(body)?.identifies(ModelDriver::A3953) {
        return Err(ModelError::UnknownModel);
    }
    let sound_mode = SoundMode::decode(body[SOUND_MODE])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    let controls: [u8; 6] = body[A3953_SOUND_MODE_FIELDS.clone()]
        .try_into()
        .map_err(|_| ModelError::IncompleteState)?;
    Ok(DeviceState {
        model: ModelDriver::A3953,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[2], BATTERY_SCALE, 0)?,
            right: battery_percent(body[3], BATTERY_SCALE, 0)?,
            case: battery_percent(body[CASE_BATTERY], BATTERY_SCALE, 0)?,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::A3953(controls)),
        equalizer: equalizer_setting(body),
        gaming_mode: None,
        auto_power_off: None,
        spatial_audio: spatial_audio(body),
        press_sensitivity: press_sensitivity(body),
        firmware_device: None,
        firmware_left: FirmwareVersion::parse(&body[6..11])?,
        firmware_right: FirmwareVersion::parse(&body[11..16])?,
    })
}

/// Reads the spatial-audio controls only when every field is a documented value.
fn spatial_audio(body: &[u8]) -> Option<SpatialAudioState> {
    let bytes: [u8; 3] = body.get(A3953_SPATIAL_AUDIO)?.try_into().ok()?;
    SpatialAudioState::decode(bytes).ok()
}

/// Reads the press sensitivity only when it is inside the documented 0-4 range.
fn press_sensitivity(body: &[u8]) -> Option<u8> {
    match body.get(A3953_PRESS_SENSITIVITY) {
        Some(&value) if value <= 4 => Some(value),
        _ => None,
    }
}

/// Reads the eight exposed bands of the ten-band curve. A channel that starts
/// with the absent marker reports no curve.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3953_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; VISIBLE_EQUALIZER_BANDS] = body
        .get(A3953_EQUALIZER_BANDS)?
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

/// Builds the six-byte `06 81` body from the measured controls.
///
/// The frame was captured for the same state in the legacy driver tests; the
/// driver preserves the unexposed fields and clamps the two documented ranges
/// exactly as the legacy structure does when it serializes.
pub fn ambient_sound_mode_write(mode: SoundMode, controls: [u8; 6]) -> Result<[u8; 6], ModelError> {
    if mode == SoundMode::AirplaneMode {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    let [
        _current,
        manual_auto,
        trans_option,
        automation,
        wind,
        sensitivity,
    ] = controls;
    let manual = (manual_auto >> 4).clamp(1, 6);
    let auto = manual_auto & 0x0f;
    Ok([
        mode.wire(),
        (manual << 4) | auto,
        trans_option.clamp(0, 2),
        automation,
        wind & 1,
        sensitivity,
    ])
}

/// Encodes the documented `10 81` spatial-audio body.
pub fn spatial_audio_write(setting: SpatialAudioState) -> [u8; 3] {
    setting.bytes()
}

/// Encodes the documented one-byte `04 85` press-sensitivity body.
pub fn press_sensitivity_write(value: u8) -> Result<[u8; 1], ModelError> {
    if value > 4 {
        return Err(ModelError::InvalidPressSensitivityWrite);
    }
    Ok([value])
}

/// Builds the documented `03 87` equalizer write from a fresh state response.
///
/// The body carries the requested ten-band curve twice, then echoes the state's
/// hear-id block with hear id disabled so the requested curve takes effect, and
/// ends with the driver-compensated curve for both channels.
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
    // The favorite music genre precedes the curve on the write.
    body.extend_from_slice(&state[HEAR_ID_GENRE]);
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    // 255 marks a state without initialized hear-id data, matching the capture.
    let hear_id_initialized =
        state[HEAR_ID_INITIALIZED] != u8::MAX && state[HEAR_ID_INITIALIZED] != u8::MAX - 1;
    if hear_id_initialized {
        body.extend_from_slice(&[0, 0]);
    } else {
        body.extend_from_slice(&[u8::MAX, u8::MAX]);
    }
    // The driver never enables hear id on a write.
    body.push(0);
    for channel in state[HEAR_ID_VOLUME].chunks(10) {
        body.extend_from_slice(&hear_id_channel_bytes(channel));
    }
    body.extend_from_slice(&state[HEAR_ID_TIME]);
    body.push(match state[HEAR_ID_TYPE] {
        0..=2 => state[HEAR_ID_TYPE],
        _ => 0,
    });
    for channel in state[HEAR_ID_CUSTOM].chunks(10) {
        body.extend_from_slice(&hear_id_channel_bytes(channel));
    }
    body.extend_from_slice(&drc_band_bytes(setting.volume_adjustments));
    body.extend_from_slice(&drc_band_bytes(setting.volume_adjustments));
    body.push(0);
    debug_assert_eq!(body.len(), EQUALIZER_WRITE_LENGTH);
    Ok(body)
}

/// Re-encodes one hear-id channel, keeping the driver's absent-channel marker.
fn hear_id_channel_bytes(channel: &[u8]) -> [u8; crate::EQUALIZER_BANDS] {
    if channel.first() == Some(&u8::MAX) {
        let mut bytes = [u8::MAX; crate::EQUALIZER_BANDS];
        bytes[crate::EQUALIZER_BANDS - 1] = 0;
        bytes
    } else {
        canonical_band_bytes(channel)
    }
}

#[cfg(test)]
mod tests {
    use crate::{SpatialAudioContentMode, SpatialAudioMode};

    use super::*;

    /// The measured `01 01` body from the legacy driver's `parses_known_packet`
    /// test, with the serial replaced by a synthetic model-prefixed value.
    const STATE: &[u8] = include_bytes!("../../../data/a3953-pilot-state.bin");

    fn state_body(fixture: &[u8]) -> &[u8] {
        &fixture[9..fixture.len() - 1]
    }

    #[test]
    fn reads_the_measured_state() {
        let state = decode_state(state_body(STATE)).expect("measured A3953 state");
        assert_eq!(state.model, ModelDriver::A3953);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(100));
        assert_eq!(
            state.firmware_left.map(|v| v.to_string()),
            Some("03.23".into())
        );
        assert_eq!(
            state.firmware_right.map(|v| v.to_string()),
            Some("03.23".into())
        );
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(
            state.spatial_audio,
            Some(SpatialAudioState {
                enabled: false,
                mode: SpatialAudioMode::Fixed,
                content: SpatialAudioContentMode::Music,
            })
        );
        assert_eq!(state.press_sensitivity, Some(0));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0; crate::EQUALIZER_BANDS],
            })
        );
    }

    #[test]
    fn rejects_a_state_with_another_serial_prefix() {
        let mut body = state_body(STATE).to_vec();
        body[16..32].copy_from_slice(b"3957TEST00000000");
        assert_eq!(decode_state(&body), Err(ModelError::UnknownModel));
    }

    #[test]
    fn undocumented_spatial_values_are_unavailable() {
        let mut body = state_body(STATE).to_vec();
        body[A3953_SPATIAL_AUDIO.start + 1] = 7;
        assert_eq!(decode_state(&body).unwrap().spatial_audio, None);
        body[A3953_SPATIAL_AUDIO.start + 1] = 2;
        assert_eq!(
            decode_state(&body).unwrap().spatial_audio.map(|v| v.mode),
            Some(SpatialAudioMode::HeadTracking)
        );
        let mut body = state_body(STATE).to_vec();
        body[A3953_PRESS_SENSITIVITY] = 5;
        assert_eq!(decode_state(&body).unwrap().press_sensitivity, None);
    }

    #[test]
    fn ambient_write_keeps_the_measured_controls() {
        let controls: [u8; 6] = state_body(STATE)[A3953_SOUND_MODE_FIELDS.clone()]
            .try_into()
            .unwrap();
        assert_eq!(
            ambient_sound_mode_write(SoundMode::NoiseCanceling, controls),
            Ok([0, 0x30, 0, 0, 1, 1])
        );
        assert_eq!(
            ambient_sound_mode_write(SoundMode::Transparency, controls),
            Ok([1, 0x30, 0, 0, 1, 1])
        );
        assert_eq!(
            ambient_sound_mode_write(SoundMode::AirplaneMode, controls),
            Err(ModelError::InvalidSoundModeWrite)
        );
    }

    #[test]
    fn spatial_and_press_writes_match_the_documented_formats() {
        let setting = SpatialAudioState {
            enabled: true,
            mode: SpatialAudioMode::HeadTracking,
            content: SpatialAudioContentMode::Movie,
        };
        assert_eq!(spatial_audio_write(setting), [1, 2, 1]);
        assert_eq!(press_sensitivity_write(3), Ok([3]));
        assert_eq!(
            press_sensitivity_write(5),
            Err(ModelError::InvalidPressSensitivityWrite)
        );
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
        assert_eq!(body[2..4], [0, 0]);
        // The requested curve, then the state's initialized-hear-id sentinel.
        assert_eq!(
            body[4..24],
            [
                180, 60, 180, 60, 180, 60, 180, 60, 120, 120, 180, 60, 180, 60, 180, 60, 180, 60,
                120, 120,
            ]
        );
        assert_eq!(body[24..26], [255, 255]);
        assert_eq!(body[26], 0);
        // The stored hear-id channels are echoed with their final byte cleared.
        assert_eq!(
            body[27..47],
            [
                60, 60, 60, 60, 60, 60, 60, 60, 0, 0, 60, 60, 60, 60, 60, 60, 60, 60, 0, 0,
            ]
        );
        assert_eq!(body[47..51], [0, 0, 0, 0]);
        assert_eq!(body[51], 0);
        assert_eq!(
            body[52..72],
            [
                60, 60, 60, 60, 60, 60, 60, 60, 0, 0, 60, 60, 60, 60, 60, 60, 60, 60, 0, 0,
            ]
        );
        // The driver-compensated curve closes the body.
        let compensated = [133, 100, 141, 98, 142, 99, 140, 106, 120, 0];
        assert_eq!(body[72..82], compensated);
        assert_eq!(body[82..92], compensated);
        assert_eq!(body[92], 0);
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
