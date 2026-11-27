//! Device session for the A3954 pilot.

#![forbid(unsafe_code)]

mod command;
mod diagnostics;
mod machine;
mod runner;

use std::sync::Arc;

pub(crate) use command::WriteTarget;
pub use command::{
    CommandError, CommandId, CommandOutcome, CommandResult, DispatchError, PresetFieldMismatch,
    QuickPresetValues, SessionCommand,
};
pub use diagnostics::DiagnosticExportError;
pub use machine::{
    ConnectionState, ReconnectPolicy, SessionView, UnidentifiedReason, UnidentifiedState,
};
pub use openscl4_models::{
    AutoPowerOff, AutoPowerOffSetting, CaseFeatures, CaseLanguage, DeviceState, EqualizerSetting,
    ModelDriver, SoundMode, SoundModeSetting, SpatialAudio, SpatialAudioMode,
    SpatialAudioMusicMode, registry,
};
use openscl4_protocol::{DecodeError, EncodeError};
use openscl4_transport::TransportError;
pub use runner::{PendingCommand, SessionHandle, SessionRunner};

/// Stage at which an I/O operation can fail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IoStage {
    /// Opening the transport.
    Connect,
    /// Transmitting a request.
    Send,
    /// Receiving a frame or waiting for its response.
    Receive,
    /// Releasing the transport.
    Close,
}

/// Typed session failure, without device identifiers or raw frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum SessionError {
    /// A transport operation failed.
    #[error("transport failed during {stage:?}: {error}")]
    Transport {
        /// The failed operation.
        stage: IoStage,
        /// Its classified transport error.
        error: TransportError,
    },
    /// A deadline expired.
    #[error("session timed out during {0:?}")]
    Timeout(IoStage),
    /// A request could not be encoded.
    #[error(transparent)]
    Encode(#[from] EncodeError),
    /// A model response did not produce a state.
    #[error(transparent)]
    Model(#[from] openscl4_models::ModelError),
    /// The session ended before a requested observation became available.
    #[error("session closed")]
    Closed,
}

/// Notifications published separately from the last known state.
#[derive(Clone, Debug)]
pub enum SessionEvent {
    /// An updated connection state or confirmed device snapshot.
    ViewChanged(Arc<SessionView>),
    /// A frame was rejected without exposing its bytes.
    FrameRejected(DecodeError),
    /// Closing the transport failed. The final snapshot keeps any earlier failure.
    CleanupFailed(SessionError),
    /// The device confirmation or failure for an accepted command.
    CommandCompleted {
        /// The accepted command.
        id: CommandId,
        /// The command result, also sent directly to its submitter.
        result: CommandResult,
    },
}
