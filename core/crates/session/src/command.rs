use openscl4_models::{EQUALIZER_COMMAND, EqualizerSetting, ModelError, SOUND_MODE_COMMAND};

use crate::{DeviceState, ModelDriver, SessionError, SoundMode, SoundModeSetting};

/// Identifier for one accepted command in this session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CommandId(pub u64);

/// Enabled fields of one quick preset, applied as one confirmed operation.
///
/// A preset replaces a deferred live-equalizer value only when it changes the
/// equalizer itself; a preset with only other fields keeps the pending value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QuickPresetValues {
    /// A positioned sound-mode change, for models that report a scale.
    pub sound_mode: Option<SoundModeSetting>,
    /// An ambient-mode change, for models without a scale.
    pub ambient_sound_mode: Option<SoundMode>,
    /// An equalizer curve change.
    pub equalizer: Option<EqualizerSetting>,
}

impl QuickPresetValues {
    /// The first enabled field in application order, with this field consumed.
    pub(crate) fn first_field(self) -> Option<(WriteTarget, Self)> {
        if let Some(setting) = self.sound_mode {
            return Some((
                WriteTarget::SoundMode(setting),
                Self {
                    sound_mode: None,
                    ..self
                },
            ));
        }
        if let Some(mode) = self.ambient_sound_mode {
            return Some((
                WriteTarget::AmbientMode(mode),
                Self {
                    ambient_sound_mode: None,
                    ..self
                },
            ));
        }
        if let Some(setting) = self.equalizer {
            return Some((
                WriteTarget::Equalizer(setting),
                Self {
                    equalizer: None,
                    ..self
                },
            ));
        }
        None
    }
}

/// A discrete operation accepted by the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionCommand {
    /// Set the sound mode and its position on the device's scale.
    SetSoundMode(SoundModeSetting),
    /// Set the ambient mode on a device without a position scale.
    SetAmbientSoundMode(SoundMode),
    /// Apply the enabled fields of one quick preset, one write at a time.
    ApplyQuickPreset(QuickPresetValues),
}

impl SessionCommand {
    pub(crate) fn supported_by(self, model: ModelDriver) -> bool {
        let capabilities = model.capabilities();
        match self {
            Self::SetSoundMode(_) => capabilities.positioned_sound_mode_write,
            Self::SetAmbientSoundMode(_) => capabilities.ambient_sound_mode_write,
            Self::ApplyQuickPreset(preset) => {
                let mode_unsupported = (preset.sound_mode.is_some()
                    && !capabilities.positioned_sound_mode_write)
                    || (preset.ambient_sound_mode.is_some()
                        && !capabilities.ambient_sound_mode_write);
                let equalizer_unsupported =
                    preset.equalizer.is_some() && !capabilities.equalizer_write;
                !mode_unsupported && !equalizer_unsupported
            }
        }
    }

    /// The typed rejection for a model that cannot accept this command.
    pub(crate) fn unsupported_error(self, model: ModelDriver) -> CommandError {
        let capabilities = model.capabilities();
        match self {
            Self::SetSoundMode(_) | Self::SetAmbientSoundMode(_) => {
                CommandError::Model(ModelError::InvalidSoundModeWrite)
            }
            Self::ApplyQuickPreset(preset) => {
                let mode_unsupported = (preset.sound_mode.is_some()
                    && !capabilities.positioned_sound_mode_write)
                    || (preset.ambient_sound_mode.is_some()
                        && !capabilities.ambient_sound_mode_write);
                if mode_unsupported {
                    CommandError::Model(ModelError::InvalidSoundModeWrite)
                } else {
                    CommandError::Model(ModelError::InvalidEqualizerWrite)
                }
            }
        }
    }

    /// Whether accepting this command replaces a deferred live-equalizer value.
    pub(crate) fn replaces_pending_equalizer(self) -> bool {
        matches!(self, Self::ApplyQuickPreset(preset) if preset.equalizer.is_some())
    }
}

/// One in-flight device write: a discrete command, one field of a quick preset,
/// or a coalesced live-equalizer value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteTarget {
    /// A positioned sound-mode change.
    SoundMode(SoundModeSetting),
    /// An ambient-mode change without a scale.
    AmbientMode(SoundMode),
    /// An equalizer curve change.
    Equalizer(EqualizerSetting),
}

impl WriteTarget {
    /// The wire command that carries this write.
    pub(crate) fn command(self) -> [u8; 2] {
        match self {
            Self::SoundMode(_) | Self::AmbientMode(_) => SOUND_MODE_COMMAND,
            Self::Equalizer(_) => EQUALIZER_COMMAND,
        }
    }

    /// Builds the write body from a fresh state response and its raw bytes.
    ///
    /// The equalizer write echoes the state's hear-id fields, so it needs the
    /// raw response next to the decoded one.
    pub(crate) fn write_body(
        self,
        state: DeviceState,
        raw_state: &[u8],
    ) -> Result<Vec<u8>, CommandError> {
        match self {
            Self::SoundMode(setting) => Ok(state.sound_mode_write(setting)?.bytes().to_vec()),
            Self::AmbientMode(mode) => Ok(state.ambient_sound_mode_write(mode)?.to_vec()),
            Self::Equalizer(setting) => Ok(state.model.equalizer_write_body(setting, raw_state)?),
        }
    }

    /// Whether the confirmed state already carries this write's requested value.
    pub(crate) fn matches(self, state: DeviceState) -> bool {
        match self {
            Self::SoundMode(setting) => state.sound_mode_setting() == Some(setting),
            Self::AmbientMode(mode) => state.sound_mode == Some(mode),
            Self::Equalizer(setting) => state.equalizer == Some(setting),
        }
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
    /// The sound-mode field of a quick preset did not confirm; fields confirmed
    /// before it stay applied in the confirmed snapshot.
    #[error(
        "quick preset sound mode did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    PresetSoundModeMismatch {
        /// The mode requested by the preset field.
        requested: SoundModeSetting,
        /// The mode and position reported by the device.
        observed: SoundModeSetting,
        /// The snapshot with every field confirmed before the failure.
        confirmed: DeviceState,
    },
    /// The ambient-mode field of a quick preset did not confirm; fields
    /// confirmed before it stay applied in the confirmed snapshot.
    #[error(
        "quick preset ambient mode did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    PresetAmbientModeMismatch {
        /// The mode requested by the preset field.
        requested: SoundMode,
        /// The mode reported by the device.
        observed: SoundMode,
        /// The snapshot with every field confirmed before the failure.
        confirmed: DeviceState,
    },
    /// The equalizer field of a quick preset did not confirm; fields confirmed
    /// before it stay applied in the confirmed snapshot.
    #[error(
        "quick preset equalizer did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    PresetEqualizerMismatch {
        /// The curve requested by the preset field.
        requested: EqualizerSetting,
        /// The curve reported by the device, if it reported one.
        observed: Option<EqualizerSetting>,
        /// The snapshot with every field confirmed before the failure.
        confirmed: DeviceState,
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
