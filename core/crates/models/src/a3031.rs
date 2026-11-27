use crate::{
    AutoPowerOff, BatteryLevels, DeviceState, EqualizerSetting, ModelDriver, ModelError, SoundMode,
    SoundModeControls, battery_percent,
};

/// Body length of the measured A3031 state.
pub const STATE_LENGTH: usize = 44;
/// Body offsets of the TWS status in the measured layout.
pub const A3031_TWS_STATUS: std::ops::Range<usize> = 0..2;
/// Body offset of the TWS-connected flag in the measured layout.
pub const A3031_TWS_CONNECTED: usize = 1;
/// Body offsets of the sound mode and its controls in the measured layout.
pub const A3031_SOUND_MODE_FIELDS: std::ops::Range<usize> = 36..40;
/// Body offsets of the equalizer preset id in the measured layout.
pub const A3031_EQUALIZER_PRESET: std::ops::Range<usize> = 6..8;
/// Body offsets of the two equalizer channels in the measured layout.
pub const A3031_EQUALIZER_CHANNEL_1: std::ops::Range<usize> = 8..16;
/// Body offsets of the second equalizer channel in the measured layout.
pub const A3031_EQUALIZER_CHANNEL_2: std::ops::Range<usize> = 16..24;
/// Body offsets of the auto power off value in the measured layout.
pub const A3031_AUTO_POWER_OFF: std::ops::Range<usize> = 42..44;

const BATTERY_LEVELS: std::ops::Range<usize> = 2..4;
const SOUND_MODE: usize = A3031_SOUND_MODE_FIELDS.start;
const SOUND_MODE_CONTROLS: std::ops::Range<usize> = 37..40;
const BATTERY_SCALE: u8 = 5;
/// Number of bands carried by each of the two channel blocks.
const EQUALIZER_BANDS_PER_CHANNEL: usize = 8;
/// Length of the `02 81` body: the preset and both channel blocks.
const EQUALIZER_WRITE_LENGTH: usize = 2 + 2 * EQUALIZER_BANDS_PER_CHANNEL;

/// Whether the state response has the measured length.
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    if body[A3031_TWS_STATUS.start] > 1 {
        return Err(ModelError::InvalidTwsStatus);
    }
    let sound_mode = SoundMode::decode(body[SOUND_MODE])?;
    if sound_mode == SoundMode::AirplaneMode {
        return Err(ModelError::UnknownSoundMode);
    }
    let [noise_canceling, transparency, custom] = body
        .get(SOUND_MODE_CONTROLS)
        .and_then(|controls| controls.try_into().ok())
        .ok_or(ModelError::IncompleteState)?;
    if noise_canceling > 3 || transparency > 1 {
        return Err(ModelError::UnknownSoundMode);
    }
    let custom = if custom == u8::MAX {
        custom
    } else {
        custom.min(10)
    };
    // The legacy equalizer setting is unavailable while TWS is disconnected.
    let tws_connected = body[A3031_TWS_CONNECTED] == 1;
    let equalizer = tws_connected.then(|| equalizer_setting(body)).flatten();
    Ok(DeviceState {
        model: ModelDriver::A3031,
        battery: BatteryLevels {
            device: None,
            left: battery_percent(body[BATTERY_LEVELS.start], BATTERY_SCALE, 0)?,
            right: battery_percent(body[BATTERY_LEVELS.start + 1], BATTERY_SCALE, 0)?,
            case: None,
        },
        sound_mode: Some(sound_mode),
        sound_mode_controls: Some(SoundModeControls::Ambient([
            noise_canceling,
            transparency,
            custom,
        ])),
        equalizer,
        gaming_mode: None,
        auto_power_off: Some(AutoPowerOff {
            enabled: body[A3031_AUTO_POWER_OFF.start] == 1,
            duration: body[A3031_AUTO_POWER_OFF.start + 1],
        }),
        wearing_detection: None,
        tws_connected: Some(tws_connected),
        firmware_device: None,
        firmware_left: None,
        firmware_right: None,
    })
}

/// Reads the curve of the first channel. A band block that starts with the
/// absent marker reports no curve. The shared curve keeps the fixed invisible
/// bands at zero, as the legacy TWS preset settings do.
fn equalizer_setting(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(A3031_EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BANDS_PER_CHANNEL] =
        body.get(A3031_EQUALIZER_CHANNEL_1)?.try_into().ok()?;
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

/// Builds the two-channel eight-band equalizer write.
///
/// The legacy TWS equalizer applies the same curve to both channels, and the
/// session exposes only the first one. The write is unavailable while TWS is
/// disconnected, matching the legacy setting handler.
pub fn equalizer_write_body(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<Vec<u8>, ModelError> {
    if !has_confirmed_layout(state) {
        return Err(ModelError::IncompleteState);
    }
    if state[A3031_TWS_CONNECTED] != 1 {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    if !setting.is_writable()
        || setting.volume_adjustments[EQUALIZER_BANDS_PER_CHANNEL..]
            .iter()
            .any(|gain| *gain != 0)
    {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let bands = setting.volume_adjustments[..EQUALIZER_BANDS_PER_CHANNEL]
        .iter()
        .map(|gain| EqualizerSetting::band_byte(*gain))
        .collect::<Vec<_>>();
    let mut body = Vec::with_capacity(EQUALIZER_WRITE_LENGTH);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    Ok(body)
}
