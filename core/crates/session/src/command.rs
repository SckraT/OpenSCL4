use openscl4_models::{
    A3949_BUTTON_COMMAND, A3949_DISABLED_ACTION, A3949_RESET_BUTTONS_COMMAND,
    A3949_TOUCH_TONE_COMMAND, A3949Button, AUTO_POWER_OFF_COMMAND, AUTO_POWER_OFF_DURATION_COUNT,
    AutoPowerOff, AutoPowerOffSetting, Capabilities, EqualizerSetting, GAMING_MODE_COMMAND,
    ModelError, SOUND_MODE_COMMAND,
};

use crate::{DeviceState, ModelDriver, SessionError, SoundMode, SoundModeSetting};

/// Identifier for one accepted command in this session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandId(pub u64);

/// Enabled fields of one quick preset, applied as one confirmed operation.
///
/// A preset that changes the equalizer replaces earlier deferred live curves.
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

    /// Whether an enabled sound-mode field needs a capability the model lacks.
    fn mode_unsupported(self, capabilities: Capabilities) -> bool {
        (self.sound_mode.is_some() && !capabilities.positioned_sound_mode_write)
            || (self.ambient_sound_mode.is_some() && !capabilities.ambient_sound_mode_write)
    }
}

/// A discrete operation accepted by the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionCommand {
    /// Set the sound mode and its position on the device's scale.
    SetSoundMode(SoundModeSetting),
    /// Set the ambient mode on a device without a position scale.
    SetAmbientSoundMode(SoundMode),
    /// Enable or disable gaming mode on a supported device.
    SetGamingMode(bool),
    /// Enable or disable the touch tone on a supported device.
    SetTouchTone(bool),
    /// Change one button's action for the earbuds currently in use.
    ///
    /// `None` disables the button, matching the measured disabled action.
    SetButtonAction {
        /// The button whose action changes.
        button: A3949Button,
        /// The action id to apply, or none to disable the button.
        action: Option<u8>,
    },
    /// Reset all button actions to the device defaults.
    ResetButtons,
    /// Set the auto power off value on a device that reports its extra fields.
    SetAutoPowerOff(AutoPowerOffSetting),
    /// Apply the enabled fields of one quick preset, one write at a time.
    ApplyQuickPreset(QuickPresetValues),
}

impl SessionCommand {
    pub(crate) fn validate(self, model: ModelDriver) -> Result<(), ModelError> {
        let capabilities = model.capabilities();
        match self {
            Self::SetSoundMode(_) if !capabilities.positioned_sound_mode_write => {
                Err(ModelError::InvalidSoundModeWrite)
            }
            Self::SetAmbientSoundMode(_) if !capabilities.ambient_sound_mode_write => {
                Err(ModelError::InvalidSoundModeWrite)
            }
            Self::SetGamingMode(_) if !capabilities.gaming_mode_write => {
                Err(ModelError::InvalidGamingModeWrite)
            }
            Self::SetTouchTone(_) if !capabilities.touch_tone_write => {
                Err(ModelError::InvalidTouchToneWrite)
            }
            Self::SetButtonAction { action, .. }
                if !capabilities.button_configuration_write
                    || action.is_some_and(|action| action >= A3949_DISABLED_ACTION) =>
            {
                Err(ModelError::InvalidButtonWrite)
            }
            Self::ResetButtons if !capabilities.button_reset => Err(ModelError::InvalidButtonReset),
            Self::SetAutoPowerOff(AutoPowerOffSetting::Enabled(index))
                if !capabilities.auto_power_off_write || index >= AUTO_POWER_OFF_DURATION_COUNT =>
            {
                Err(ModelError::InvalidAutoPowerOffWrite)
            }
            Self::SetAutoPowerOff(_) if !capabilities.auto_power_off_write => {
                Err(ModelError::InvalidAutoPowerOffWrite)
            }
            Self::ApplyQuickPreset(preset) if preset.mode_unsupported(capabilities) => {
                Err(ModelError::InvalidSoundModeWrite)
            }
            Self::ApplyQuickPreset(preset)
                if preset
                    .equalizer
                    .is_some_and(|setting| !model.equalizer_is_writable(setting)) =>
            {
                Err(ModelError::InvalidEqualizerWrite)
            }
            _ => Ok(()),
        }
    }

    /// Whether the current device state makes this command unavailable.
    ///
    /// A rejected command is completed with [`CommandError::Unavailable`]
    /// before any frame is sent, so the device state is preserved.
    pub(crate) fn unavailable(self, state: DeviceState) -> bool {
        match self {
            Self::ApplyQuickPreset(preset) => {
                preset.equalizer.is_some() && !state.model.equalizer_write_available(&state)
            }
            Self::SetButtonAction { .. } => state.tws.is_none(),
            _ => false,
        }
    }

    /// Whether accepting this command replaces a deferred live-equalizer value.
    pub(crate) fn replaces_pending_equalizer(self) -> bool {
        matches!(self, Self::ApplyQuickPreset(preset) if preset.equalizer.is_some())
    }
}

/// A requested write resolved against a fresh state response.
pub struct ResolvedWrite {
    /// The command body to transmit.
    pub body: Vec<u8>,
    /// The value a read-back must report for an auto power off write.
    pub expected_auto_power_off: Option<AutoPowerOff>,
}

/// One in-flight device write: a discrete command, one field of a quick preset,
/// or a coalesced live-equalizer value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WriteTarget {
    /// A positioned sound-mode change.
    SoundMode(SoundModeSetting),
    /// An ambient-mode change without a scale.
    AmbientMode(SoundMode),
    /// A gaming-mode flag change.
    GamingMode(bool),
    /// An auto power off value change.
    AutoPowerOff(AutoPowerOffSetting),
    /// An equalizer curve change.
    Equalizer(EqualizerSetting),
    /// A touch-tone flag change.
    TouchTone(bool),
    /// One button's action change.
    ButtonAction {
        /// The button whose action changes.
        button: A3949Button,
        /// The requested action id, or none to disable the button.
        action: Option<u8>,
    },
    /// The reset-buttons action.
    ResetButtons,
}

impl WriteTarget {
    /// The wire command that carries this write for the selected model.
    pub(crate) fn command(self, model: ModelDriver) -> [u8; 2] {
        match self {
            Self::SoundMode(_) | Self::AmbientMode(_) => SOUND_MODE_COMMAND,
            Self::GamingMode(_) => GAMING_MODE_COMMAND,
            Self::AutoPowerOff(_) => AUTO_POWER_OFF_COMMAND,
            Self::Equalizer(_) => model.equalizer_command(),
            Self::TouchTone(_) => A3949_TOUCH_TONE_COMMAND,
            Self::ButtonAction { .. } => A3949_BUTTON_COMMAND,
            Self::ResetButtons => A3949_RESET_BUTTONS_COMMAND,
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
    ) -> Result<ResolvedWrite, ModelError> {
        let mut resolved = ResolvedWrite {
            body: Vec::new(),
            expected_auto_power_off: None,
        };
        resolved.body = match self {
            Self::SoundMode(setting) => state.sound_mode_write(setting)?.bytes().to_vec(),
            Self::AmbientMode(mode)
                if matches!(state.model, ModelDriver::A3028 | ModelDriver::A3029) =>
            {
                state.family_sound_mode_write(mode)?.to_vec()
            }
            Self::AmbientMode(mode) => state.ambient_sound_mode_write(mode)?.to_vec(),
            Self::GamingMode(enabled) => state.gaming_mode_write(enabled)?.to_vec(),
            Self::AutoPowerOff(setting) => {
                let write = state.auto_power_off_write(setting)?;
                resolved.expected_auto_power_off = Some(write.expected());
                write.body().to_vec()
            }
            Self::Equalizer(setting) => state.model.equalizer_write_body(setting, raw_state)?,
            Self::TouchTone(enabled) => state.touch_tone_write(enabled)?.to_vec(),
            Self::ButtonAction { button, action } => state.button_write(button, action)?.to_vec(),
            Self::ResetButtons => {
                state.button_reset()?;
                Vec::new()
            }
        };
        Ok(resolved)
    }

    /// Whether the confirmed state already carries this write's requested value.
    pub(crate) fn matches(
        self,
        state: DeviceState,
        expected_auto_power_off: Option<AutoPowerOff>,
    ) -> bool {
        match self {
            Self::SoundMode(setting) => state.sound_mode_setting() == Some(setting),
            Self::AmbientMode(mode) => state.sound_mode == Some(mode),
            Self::GamingMode(enabled) => state.gaming_mode == Some(enabled),
            Self::AutoPowerOff(_) => expected_auto_power_off
                .is_some_and(|expected| state.auto_power_off == Some(expected)),
            Self::Equalizer(setting) => state.equalizer == Some(setting),
            Self::TouchTone(enabled) => state.touch_tone == Some(enabled),
            Self::ButtonAction { button, action } => {
                state.button_write_matches(button, action) == Some(true)
            }
            // The reset action has no measured post-reset state; its outcome is
            // confirmed by the device acknowledgement instead.
            Self::ResetButtons => true,
        }
    }

    /// Whether an empty acknowledgement completes this target without a value
    /// read-back. Only actions that have no measured post-action state use it.
    pub(crate) fn confirms_with_ack(self) -> bool {
        matches!(self, Self::ResetButtons)
    }
}

/// Outcome backed by a device response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandOutcome {
    /// A read-back confirmed the requested value.
    Applied(DeviceState),
    /// The device acknowledged an action that has no readable setting.
    ///
    /// The snapshot is the fresh state read after the acknowledgement.
    Accepted(DeviceState),
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
    /// The device identity or state is not recognized; only safe reads are available.
    #[error("device is read-only: its identity or state is not recognized")]
    ReadOnly,
    /// The connected model does not support equalizer writes.
    #[error("the connected model does not support equalizer writes")]
    UnsupportedEqualizer,
    /// A gain in the live-equalizer curve is outside the writable range.
    #[error("equalizer gain is outside the writable range")]
    InvalidEqualizer,
}

/// One quick-preset field whose read-back did not match the request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PresetFieldMismatch {
    /// The positioned sound-mode field.
    #[error("sound mode (requested {requested:?}, observed {observed:?})")]
    SoundMode {
        /// The mode requested by the preset field.
        requested: SoundModeSetting,
        /// The mode and position reported by the device.
        observed: SoundModeSetting,
    },
    /// The ambient-mode field.
    #[error("ambient mode (requested {requested:?}, observed {observed:?})")]
    AmbientMode {
        /// The mode requested by the preset field.
        requested: SoundMode,
        /// The mode reported by the device.
        observed: SoundMode,
    },
    /// The equalizer field.
    #[error("equalizer (requested {requested:?}, observed {observed:?})")]
    Equalizer {
        /// The curve requested by the preset field.
        requested: EqualizerSetting,
        /// The curve reported by the device, if it reported one.
        observed: Option<EqualizerSetting>,
    },
}

/// Failure of an accepted command.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CommandError {
    /// The device reported a different gaming-mode flag at the confirmation deadline.
    #[error(
        "gaming mode did not match the device read-back (requested {requested}, observed {observed:?})"
    )]
    GamingModeMismatch {
        /// The requested flag.
        requested: bool,
        /// The last flag reported by the device.
        observed: Option<bool>,
    },
    /// The command expired before it could be sent.
    #[error("session command expired in the queue")]
    QueueTimeout,
    /// The requested device value could not be encoded.
    #[error(transparent)]
    Model(#[from] ModelError),
    /// The read-back did not match the requested auto power off value.
    #[error(
        "auto power off did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    AutoPowerOffMismatch {
        /// The value requested by the command.
        requested: AutoPowerOffSetting,
        /// The value reported by the device, if it reported a recognized one.
        observed: Option<AutoPowerOff>,
    },
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
    /// The read-back did not match the requested touch-tone flag.
    #[error(
        "touch tone did not match the device read-back (requested {requested}, observed {observed:?})"
    )]
    TouchToneMismatch {
        /// The requested flag.
        requested: bool,
        /// The flag reported by the device, if any.
        observed: Option<bool>,
    },
    /// The read-back did not match the requested button action.
    #[error(
        "button action did not match the device read-back (button {}, requested {requested:?}, observed {observed:?})",
        button.code()
    )]
    ButtonMismatch {
        /// The button that was written.
        button: A3949Button,
        /// The action requested for the button, or none for disabled.
        requested: Option<u8>,
        /// The action reported by the device, or none for disabled.
        observed: Option<u8>,
    },
    /// A quick-preset field did not confirm; fields confirmed before it stay
    /// applied in the confirmed snapshot.
    #[error("quick preset stopped at a field that did not match the read-back: {field}")]
    PresetMismatch {
        /// The field that did not confirm, with the requested and observed values.
        field: PresetFieldMismatch,
        /// The snapshot with every field confirmed before the failure.
        confirmed: DeviceState,
    },
    /// A preset stopped after confirming at least one field.
    #[error("quick preset interrupted: {error}")]
    PresetInterrupted {
        /// Why the next field could not complete.
        #[source]
        error: SessionError,
        /// Whether the unfinished write may have reached the device.
        may_have_written: bool,
        /// The snapshot from the confirmation of the last completed field.
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
    /// The current device state makes the requested setting unavailable.
    #[error("the setting is not available in the current device state")]
    Unavailable,
}

/// Confirmed result for an accepted command.
pub type CommandResult = Result<CommandOutcome, CommandError>;
