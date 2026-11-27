//! Soundcore Liberty 2 Pro+ (A3930).
//!
//! The state carries two battery levels, a two-channel eight-band equalizer
//! configuration, the custom hear-id block, the button configuration, the sound
//! modes and a side-tone flag. The driver selects an explicit trailing checksum
//! and reads the LDAC state with a separate `01 7F` request. It offers only the
//! normal and transparency ambient modes and does not write the LDAC flag.

use crate::{
    DeviceState, EqualizerSetting, EqualizerWrite, ModelDriver, ModelError, SoundMode,
    SoundModeControls, separate_identity,
};

/// Body length of the derived A3930 state without the optional field.
pub const STATE_LENGTH: usize = 82;
/// Body length with the optional hear-id preset index.
pub const STATE_LENGTH_WITH_PRESET: usize = 84;

pub fn has_confirmed_layout(body: &[u8]) -> bool {
    matches!(body.len(), STATE_LENGTH | STATE_LENGTH_WITH_PRESET)
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    let equalizer = separate_identity::decode_equalizer(body);
    let (sound_mode, controls) = separate_identity::decode_sound_modes(body)?;
    separate_identity::shared_state(
        ModelDriver::A3930,
        body,
        Some(sound_mode),
        Some(SoundModeControls::SeparateIdentity(controls)),
        equalizer,
    )
}

/// Builds the custom hear-id equalizer write from a fresh state response.
pub fn equalizer_write(
    setting: EqualizerSetting,
    state: &[u8],
) -> Result<EqualizerWrite, ModelError> {
    if !has_confirmed_layout(state) {
        return Err(ModelError::IncompleteState);
    }
    separate_identity::custom_hear_id_equalizer_write(setting, state)
}

/// The A3930 offers normal and transparency, but no noise-canceling mode.
pub const fn supports_ambient_mode(mode: SoundMode) -> bool {
    matches!(mode, SoundMode::Normal | SoundMode::Transparency)
}
