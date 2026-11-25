use openscl4_models::ModelError;

use crate::{DeviceState, SessionError, SoundModeSetting};

/// Identifier for one accepted command in this session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CommandId(pub u64);

/// A discrete operation accepted by the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionCommand {
    /// Set the sound mode and its position on the device's scale.
    SetSoundMode(SoundModeSetting),
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
    #[error("sound mode did not match the device read-back")]
    ModeMismatch {
        /// The mode requested by the command.
        requested: SoundModeSetting,
        /// The mode and position reported by the device.
        observed: SoundModeSetting,
    },
    /// An error ended the session before the command was confirmed.
    #[error(transparent)]
    Session(#[from] SessionError),
}

/// Confirmed result for an accepted command.
pub type CommandResult = Result<CommandOutcome, CommandError>;
