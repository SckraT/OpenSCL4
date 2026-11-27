//! Soundcore Liberty 2 Pro (A3909).
//!
//! The state carries the shared family battery prefix, the equalizer preset id
//! and the button configuration. It has no equalizer band gains and no sound
//! modes: the legacy driver derives the curve from a fixed preset table, so the
//! new core reads neither the curve nor a sound mode for this model.

use crate::{DeviceState, ModelDriver, ModelError, separate_identity};

/// Body length of the captured A3909 state.
///
/// The legacy capture carries one trailing byte that its parser leaves unread.
pub const STATE_LENGTH: usize = 36;
/// Body length of the fields the legacy parser consumes.
pub const PARSED_STATE_LENGTH: usize = 35;
pub fn has_confirmed_layout(body: &[u8]) -> bool {
    matches!(body.len(), PARSED_STATE_LENGTH | STATE_LENGTH)
}

pub fn decode_state(body: &[u8]) -> Result<DeviceState, ModelError> {
    if body.len() < PARSED_STATE_LENGTH {
        return Err(ModelError::IncompleteState);
    }
    separate_identity::shared_state(ModelDriver::A3909, body, None, None, None)
}

/// The A3909 has a preset-only equalizer that the state does not expose as a
/// curve, so the new core reads no equalizer value and rejects a curve write.
pub fn equalizer_write() -> Result<crate::EqualizerWrite, ModelError> {
    Err(ModelError::InvalidEqualizerWrite)
}
