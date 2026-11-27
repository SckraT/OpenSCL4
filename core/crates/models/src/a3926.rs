//! Soundcore Life Dot 2S (A3926).
//!
//! The state carries two battery levels, a two-channel eight-band equalizer
//! configuration, the basic hear-id block and the button configuration. The
//! legacy driver has no sound modes, so the new core reads none. The equalizer
//! write is the basic hear-id variant of the shared `03 87` builder.

use crate::{
    DeviceState, EqualizerSetting, EqualizerWrite, ModelDriver, ModelError,
    separate_identity::{
        self, AGE_RANGE, EQUALIZER_BAND_COUNT, GENDER, HEAR_ID_TIME, HEAR_ID_VOLUME,
    },
};

/// Body length of the derived A3926 state.
pub const STATE_LENGTH: usize = 59;
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    body.len() == STATE_LENGTH
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    let equalizer = separate_identity::decode_equalizer(body);
    separate_identity::shared_state(ModelDriver::A3926, body, None, None, equalizer)
}

/// Builds the basic hear-id equalizer write from a fresh state response.
///
/// The legacy state modifier adapts the state's basic hear-id block to the
/// custom hear-id builder: it keeps the two volume channels and the measurement
/// time, uses the default type and genre, and writes no custom channel.
pub fn equalizer_write(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<EqualizerWrite, ModelError> {
    if !has_confirmed_layout(state) {
        return Err(ModelError::IncompleteState);
    }
    let gender = state[GENDER];
    let age_range = state[AGE_RANGE];
    let volume = separate_identity::hear_id_volume(state, HEAR_ID_VOLUME.start)?;
    let time = separate_identity::hear_id_time(state, HEAR_ID_TIME.start)?;
    separate_identity::hear_id_equalizer_write(
        setting,
        separate_identity::HearIdFields {
            gender,
            age_range,
            favorite_music_genre: [0, 0],
            volume,
            time,
            hear_id_type: 0,
            custom_volume: [u8::MAX; EQUALIZER_BAND_COUNT * 2],
        },
    )
}
