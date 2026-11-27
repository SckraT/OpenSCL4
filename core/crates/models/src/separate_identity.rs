//! Shared layout of the separate-identity TWS family.
//!
//! The family reads its state with a `01 01` request and reads its firmware and
//! serial with a separate `01 05` identity request. The state body carries no
//! serial number, so the model is selected from the identity response only.
//! A3031, A3931 and A3935 use the same family but are not ported yet.

use std::ops::Range;

use crate::{
    BatteryLevels, DeviceState, EqualizerSetting, EqualizerWrite, FirmwareVersion, ModelDriver,
    ModelError, SoundMode, SoundModeControls, battery_percent,
};

/// Body offsets of the TWS status (host device and connection flag).
pub const TWS_STATUS: Range<usize> = 0..2;
/// Body offsets of the left and right battery levels and charging flags.
pub const BATTERY: Range<usize> = 2..6;
/// Body offset of the gender byte.
pub const GENDER: usize = 24;
/// Body offset of the age range byte.
pub const AGE_RANGE: usize = 25;
/// Body offset of the custom hear-id fields.
pub const CUSTOM_HEAR_ID: usize = 26;
/// Body offset of the sound modes on the models that report them.
pub const SOUND_MODES: Range<usize> = 77..81;
/// Body offsets of the preset id inside the equalizer configuration.
pub const EQUALIZER_PRESET: Range<usize> = 6..8;
/// Body offsets of the first eight-band channel curve.
pub const EQUALIZER_BANDS: Range<usize> = 8..16;
/// Body offsets of the first hear-id volume channel.
pub const HEAR_ID_VOLUME: Range<usize> = 27..43;
/// Body offsets of the hear-id measurement time.
pub const HEAR_ID_TIME: Range<usize> = 43..47;
/// Body offset of the hear-id type.
pub const HEAR_ID_TYPE: usize = 47;
/// Body offset of the favorite music genre.
pub const HEAR_ID_GENRE: usize = 48;
/// Body offsets of the custom hear-id volume channel.
pub const HEAR_ID_CUSTOM: Range<usize> = 49..65;

/// Body length of a `01 05` identity response: two versions and a serial.
pub const IDENTITY_LENGTH: usize = 26;
/// Number of equalizer bands carried by the family's state.
pub const EQUALIZER_BAND_COUNT: usize = 8;
const BATTERY_SCALE: u8 = 5;

/// Reads the two five-byte firmware versions at the head of an identity body.
///
/// An absent version is encoded as five zero or five `FF` bytes. A body where
/// both versions are absent is not a usable identity.
pub fn decode_identity_firmware(body: &[u8]) -> Result<crate::IdentityFirmware, ModelError> {
    if body.len() != IDENTITY_LENGTH {
        return Err(ModelError::InvalidIdentity);
    }
    let left = FirmwareVersion::parse(&body[0..5])?;
    let right = FirmwareVersion::parse(&body[5..10])?;
    if left.is_none() && right.is_none() {
        return Err(ModelError::InvalidIdentity);
    }
    Ok(crate::IdentityFirmware { left, right })
}

/// Reads a separately reported LDAC flag from a `01 7F` response body.
pub fn decode_ldac_state(body: &[u8]) -> Result<bool, ModelError> {
    match body {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(ModelError::InvalidLdacState),
    }
}

/// Reads the left and right battery levels from the measured layout.
pub fn decode_battery(body: &[u8]) -> Result<BatteryLevels, ModelError> {
    let levels = body
        .get(BATTERY.start..BATTERY.start + 2)
        .ok_or(ModelError::IncompleteState)?;
    Ok(BatteryLevels {
        device: None,
        left: battery_percent(levels[0], BATTERY_SCALE, 0)?,
        right: battery_percent(levels[1], BATTERY_SCALE, 0)?,
        case: None,
    })
}

/// Reads the equalizer curve from the measured layout.
///
/// The state carries one eight-band curve per channel. Legacy settings expose
/// the first channel only, and a band block that starts with the absent marker
/// reports no curve. The shared ten-band curve keeps the family's two fixed
/// invisible bands at zero.
pub fn decode_equalizer(body: &[u8]) -> Option<EqualizerSetting> {
    let preset_id: [u8; 2] = body.get(EQUALIZER_PRESET)?.try_into().ok()?;
    let bands: [u8; EQUALIZER_BAND_COUNT] = body.get(EQUALIZER_BANDS)?.try_into().ok()?;
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

/// Reads the ambient mode and the sound-mode controls that a write must echo.
///
/// Legacy parsers fall back to the default value for an unknown mode byte. A
/// confirmed state instead rejects an unrecognized ambient, noise-canceling or
/// transparency value. The unexposed custom noise-canceling value keeps the
/// legacy clamp.
pub fn decode_sound_modes(body: &[u8]) -> Result<(SoundMode, [u8; 4]), ModelError> {
    let fields: [u8; 4] = body
        .get(SOUND_MODES)
        .and_then(|fields| fields.try_into().ok())
        .ok_or(ModelError::IncompleteState)?;
    let [ambient, noise_canceling, transparency, custom] = fields;
    let sound_mode = SoundMode::decode(ambient)?;
    if sound_mode == SoundMode::AirplaneMode || noise_canceling > 3 || transparency > 1 {
        return Err(ModelError::UnknownSoundMode);
    }
    let custom = if custom == u8::MAX {
        custom
    } else {
        custom.min(10)
    };
    Ok((sound_mode, [ambient, noise_canceling, transparency, custom]))
}

/// Builds the four-byte sound-mode write from a fresh state response.
pub fn ambient_sound_mode_write(
    model: ModelDriver,
    mode: SoundMode,
    controls: [u8; 4],
) -> Result<Vec<u8>, ModelError> {
    if !model.supports_ambient_mode(mode) {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    let [_, noise_canceling, transparency, custom] = controls;
    if noise_canceling > 3 || transparency > 1 {
        return Err(ModelError::InvalidSoundModeWrite);
    }
    Ok(vec![mode.wire(), noise_canceling, transparency, custom])
}

/// Builds the custom hear-id equalizer write from a fresh state response.
///
/// Both custom hear-id models share this layout: the write repeats the
/// requested curve, the state's gender and age range, its hear-id volume, time,
/// type, music genre and custom channel, and ends with the compressed curve.
pub fn custom_hear_id_equalizer_write(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<EqualizerWrite, ModelError> {
    let gender = *state.get(GENDER).ok_or(ModelError::IncompleteState)?;
    let age_range = *state.get(AGE_RANGE).ok_or(ModelError::IncompleteState)?;
    let genre = *state
        .get(HEAR_ID_GENRE)
        .ok_or(ModelError::IncompleteState)?;
    let hear_id_type = *state.get(HEAR_ID_TYPE).ok_or(ModelError::IncompleteState)?;
    let custom_volume: [u8; 16] = state
        .get(HEAR_ID_CUSTOM)
        .and_then(|values| values.try_into().ok())
        .ok_or(ModelError::IncompleteState)?;
    let volume = hear_id_volume(state, HEAR_ID_VOLUME.start)?;
    let time = hear_id_time(state, HEAR_ID_TIME.start)?;
    hear_id_equalizer_write(
        setting,
        HearIdFields {
            gender,
            age_range,
            favorite_music_genre: [genre, 0],
            volume,
            time,
            hear_id_type,
            custom_volume,
        },
    )
}

/// Reads the two eight-band hear-id volume channels from a state body.
pub fn hear_id_volume(body: &[u8], start: usize) -> Result<[u8; 16], ModelError> {
    body.get(start..start + 16)
        .and_then(|values| values.try_into().ok())
        .ok_or(ModelError::IncompleteState)
}

/// Reads four bytes of a state body as the hear-id time.
pub fn hear_id_time(body: &[u8], start: usize) -> Result<[u8; 4], ModelError> {
    body.get(start..start + 4)
        .and_then(|values| values.try_into().ok())
        .ok_or(ModelError::IncompleteState)
}

/// Encodes the eight-band family curve and rejects the two fixed bands.
pub fn band_bytes(setting: EqualizerSetting) -> Result<[u8; EQUALIZER_BAND_COUNT], ModelError> {
    if !setting.is_writable()
        || setting.volume_adjustments[EQUALIZER_BAND_COUNT..]
            .iter()
            .any(|gain| *gain != 0)
    {
        return Err(ModelError::InvalidEqualizerWrite);
    }
    let mut bands = [0; EQUALIZER_BAND_COUNT];
    for (band, gain) in bands.iter_mut().zip(setting.volume_adjustments) {
        *band = EqualizerSetting::band_byte(gain);
    }
    Ok(bands)
}

/// Hear-id fields echoed from a fresh state response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HearIdFields {
    /// Gender byte of the state.
    pub gender: u8,
    /// Age range byte of the state; `0xFF` means the device has no hear id.
    pub age_range: u8,
    /// Favorite music genre, written as a little-endian pair.
    pub favorite_music_genre: [u8; 2],
    /// Two eight-band hear-id volume channels.
    pub volume: [u8; 16],
    /// Measurement time.
    pub time: [u8; 4],
    /// Hear-id type byte.
    pub hear_id_type: u8,
    /// Two eight-band custom channels.
    pub custom_volume: [u8; 16],
}

/// Builds the equalizer write shared by the basic and custom hear-id variants.
///
/// The body repeats the requested curve for both channels and carries the
/// hear-id fields of the fresh state, so a write does not change them. With no
/// hear-id support (`age_range` of `0xFF`) the legacy builder instead sends
/// `03 86` with the hear-id blocks replaced by absent markers. The trailing
/// active curve is the legacy dynamic-range-compression output of the request.
pub fn hear_id_equalizer_write(
    setting: EqualizerSetting,
    fields: HearIdFields,
) -> Result<EqualizerWrite, ModelError> {
    let bands = band_bytes(setting)?;
    let supports_hear_id = fields.age_range != u8::MAX;

    let mut body = Vec::with_capacity(76);
    body.extend_from_slice(&setting.preset_id.to_le_bytes());
    if supports_hear_id {
        body.extend_from_slice(&fields.favorite_music_genre);
    }
    body.extend_from_slice(&bands);
    body.extend_from_slice(&bands);
    body.push(if supports_hear_id {
        fields.gender
    } else {
        u8::MAX
    });
    body.push(if supports_hear_id {
        fields.age_range
    } else {
        u8::MAX
    });
    body.push(0);
    body.extend_from_slice(if supports_hear_id {
        &fields.volume
    } else {
        &[u8::MAX; 16]
    });
    body.extend_from_slice(if supports_hear_id {
        &fields.time
    } else {
        &[0; 4]
    });
    body.push(fields.hear_id_type);
    body.extend_from_slice(if supports_hear_id {
        &fields.custom_volume
    } else {
        &[u8::MAX; 16]
    });
    let compressed = apply_drc(
        setting.volume_adjustments[..EQUALIZER_BAND_COUNT]
            .try_into()
            .expect("the family curve has eight active bands"),
    )
    .map(EqualizerSetting::band_byte);
    body.extend_from_slice(&compressed);
    body.extend_from_slice(&compressed);

    let command = if supports_hear_id {
        crate::EQUALIZER_COMMAND
    } else {
        crate::EQUALIZER_WITHOUT_HEAR_ID_COMMAND
    };
    Ok(EqualizerWrite::new(command, body))
}

/// Applies the legacy dynamic-range-compression curve to the eight active bands.
///
/// The legacy equalizer state modifier ends the equalizer write with this curve,
/// which is the one the device puts in effect. The values are tenths of a
/// decibel, as in the state.
pub fn apply_drc(bands: [i16; 8]) -> [i16; 8] {
    const SMALLER_COEFFICIENT: f64 = 0.85;
    const LARGER_COEFFICIENT: f64 = 0.95;
    const BAND_COEFFICIENTS: [[f64; 8]; 8] = [
        [
            1.26,
            -0.71 * SMALLER_COEFFICIENT,
            0.177,
            -0.0494,
            0.0345,
            -0.0197,
            0.0075,
            -0.00217,
        ],
        [
            -0.71 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            0.0,
            0.204,
            -0.068,
            0.045,
            -0.0235,
            0.0075,
        ],
        [
            0.177,
            -0.81 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            -0.81 * SMALLER_COEFFICIENT,
            0.208,
            -0.07,
            0.045,
            -0.0197,
        ],
        [
            -0.0494,
            0.204,
            0.0,
            1.73 * LARGER_COEFFICIENT,
            -0.82 * SMALLER_COEFFICIENT,
            0.208,
            -0.068,
            0.0345,
        ],
        [
            0.0345,
            -0.068,
            0.208,
            -0.82 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            0.0,
            0.204,
            -0.0494,
        ],
        [
            -0.0197,
            0.045,
            -0.07,
            0.208,
            -0.81 * SMALLER_COEFFICIENT,
            1.73 * LARGER_COEFFICIENT,
            -0.81 * SMALLER_COEFFICIENT,
            0.177,
        ],
        [
            0.0075,
            -0.0235,
            0.045,
            -0.068,
            0.204,
            0.0,
            1.83 * LARGER_COEFFICIENT,
            -0.71 * SMALLER_COEFFICIENT,
        ],
        [
            -0.00217,
            0.0075,
            -0.0197,
            0.0345,
            -0.0494,
            0.177,
            -0.71 * SMALLER_COEFFICIENT,
            1.5,
        ],
    ];

    let adjustments = bands.map(|band| f64::from(band) / 10.0);
    let lows_subtraction = adjustments[2] * 0.81 * SMALLER_COEFFICIENT;
    let highs_subtraction = adjustments[5] * 0.81 * SMALLER_COEFFICIENT;

    std::array::from_fn(|coefficient_index| {
        let coefficients = BAND_COEFFICIENTS[coefficient_index];
        let multiplied = adjustments
            .iter()
            .enumerate()
            .fold(0.0, |acc, (index, adjustment)| {
                if (coefficient_index == 1 || coefficient_index == 3) && index == 2 {
                    acc - lows_subtraction
                } else if (coefficient_index == 4 || coefficient_index == 6) && index == 5 {
                    acc - highs_subtraction
                } else {
                    acc + adjustment * coefficients[index]
                }
            });
        let band = multiplied / 10.0;
        (band * 10.0).round() as i16
    })
}

/// Builds the state of a model whose body starts with the shared family prefix.
pub fn shared_state(
    model: ModelDriver,
    body: &[u8],
    sound_mode: Option<SoundMode>,
    sound_mode_controls: Option<SoundModeControls>,
    equalizer: Option<EqualizerSetting>,
) -> Result<DeviceState, ModelError> {
    Ok(DeviceState {
        model,
        battery: decode_battery(body)?,
        sound_mode,
        sound_mode_controls,
        equalizer,
        gaming_mode: None,
        auto_power_off: None,
        ldac: None,
        firmware_device: None,
        firmware_left: None,
        firmware_right: None,
    })
}
