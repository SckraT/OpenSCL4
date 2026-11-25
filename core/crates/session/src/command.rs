use openscl4_models::ModelError;

use crate::{DeviceState, ModelDriver, SessionError, SoundMode, SoundModeSetting};

/// Identifier for one accepted command in this session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CommandId(pub u64);

/// A discrete operation accepted by the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionCommand {
    /// Set the sound mode and its position on the device's scale.
    SetSoundMode(SoundModeSetting),
    /// Set the ambient mode on a device without a position scale.
    SetAmbientSoundMode(SoundMode),
}

impl SessionCommand {
    pub(crate) fn supported_by(self, model: ModelDriver) -> bool {
        let capabilities = model.capabilities();
        match self {
            Self::SetSoundMode(_) => capabilities.positioned_sound_mode_write,
            Self::SetAmbientSoundMode(_) => capabilities.ambient_sound_mode_write,
        }
    }

    pub(crate) fn write_body(self, state: DeviceState) -> Result<Vec<u8>, ModelError> {
        match self {
            Self::SetSoundMode(setting) => Ok(state.sound_mode_write(setting)?.bytes().to_vec()),
            Self::SetAmbientSoundMode(mode) => Ok(state.ambient_sound_mode_write(mode)?.to_vec()),
        }
    }

    pub(crate) fn verify_mode(self, state: DeviceState) -> Result<(), CommandError> {
        match self {
            Self::SetSoundMode(requested) => {
                let observed = state
                    .sound_mode_setting()
                    .ok_or(ModelError::InvalidSoundModeWrite)?;
                if observed != requested {
                    return Err(CommandError::ModeMismatch {
                        requested,
                        observed,
                    });
                }
            }
            Self::SetAmbientSoundMode(requested) => {
                let observed = state.sound_mode.ok_or(ModelError::InvalidSoundModeWrite)?;
                if observed != requested {
                    return Err(CommandError::AmbientModeMismatch {
                        requested,
                        observed,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Outcome backed by a device response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    /// A read-back confirmed the requested value.
    Applied(DeviceState),
}

/// Failure to accept a command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum DispatchError {
    /// The session has not completed its initial state read.
    #[error("session is not ready")]
    NotReady,
    /// The bounded command queue is full.
    #[error("session command queue is full")]
    QueueFull,
    /// The session is closing or has ended.
    #[error("session closed")]
    Closed,
}

/// Failure of an accepted command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// The command expired before it could be sent.
    #[error("session command expired in the queue")]
    QueueTimeout,
    /// The requested device value could not be encoded.
    #[error(transparent)]
    Model(#[from] ModelError),
    /// The read-back did not match the requested mode or position.
    #[error(
        "sound mode did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    ModeMismatch {
        /// The mode requested by the command.
        requested: SoundModeSetting,
        /// The mode and position reported by the device.
        observed: SoundModeSetting,
    },
    /// The read-back did not match the requested mode on a device without a scale.
    #[error(
        "ambient sound mode did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    AmbientModeMismatch {
        /// The mode requested by the command.
        requested: SoundMode,
        /// The mode reported by the device.
        observed: SoundMode,
    },
    /// The device link was lost before the command could be confirmed.
    #[error("device disconnected")]
    Disconnected,
    /// The write may have reached the device before confirmation was interrupted.
    #[error("device write could not be confirmed: {0}")]
    Unconfirmed(SessionError),
    /// An error ended the session before the write was attempted.
    #[error(transparent)]
    Session(#[from] SessionError),
}

/// Confirmed result for an accepted command.
pub type CommandResult = Result<CommandOutcome, CommandError>;
