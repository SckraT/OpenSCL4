//! Soundcore Liberty Air 2 Pro (A3951).
//!
//! The state carries two battery levels, a two-channel eight-band equalizer
//! configuration, the custom hear-id block, the button configuration, the sound
//! modes, the side tone, wearing detection and the touch tone. The driver reads
//! the LDAC state with a separate `01 7F` request and exposes the LDAC flag for
//! writing through the legacy flag module.

use crate::{
    DeviceState, EqualizerSetting, EqualizerWrite, ModelDriver, ModelError, SoundMode,
    SoundModeControls, separate_identity,
};

/// Body length of the derived A3951 state without optional fields.
pub const STATE_LENGTH: usize = 84;
/// Body length with the optional hear-id preset.
pub const STATE_LENGTH_WITH_HEAR_ID_PRESET: usize = 86;
/// Body length with the optional hear-id preset and new battery levels.
pub const STATE_LENGTH_WITH_NEW_BATTERY: usize = 88;
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    matches!(
        body.len(),
        STATE_LENGTH | STATE_LENGTH_WITH_HEAR_ID_PRESET | STATE_LENGTH_WITH_NEW_BATTERY
    )
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if !has_confirmed_layout(body) {
        return Err(ModelError::IncompleteState);
    }
    let equalizer = separate_identity::decode_equalizer(body);
    let (sound_mode, controls) = separate_identity::decode_sound_modes(body)?;
    separate_identity::shared_state(
        ModelDriver::A3951,
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

/// The A3951 offers all three ambient modes.
pub const fn supports_ambient_mode(mode: SoundMode) -> bool {
    matches!(
        mode,
        SoundMode::NoiseCanceling | SoundMode::Transparency | SoundMode::Normal
    )
}
