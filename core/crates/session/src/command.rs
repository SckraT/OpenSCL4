use openscl4_models::{
    AUTO_POWER_OFF_COMMAND, AUTO_POWER_OFF_DURATION_COUNT, AmbientSoundModeCycle, AutoPowerOff,
    AutoPowerOffSetting, BUTTON_CONFIGURATION_COMMAND, ButtonAction, Capabilities,
    DOLBY_AUDIO_COMMAND, DUAL_CONNECTIONS_COMMAND, EqualizerSetting, GAMING_MODE_COMMAND,
    LDAC_COMMAND, LDAC_STATE_COMMAND, LIMIT_HIGH_VOLUME_COMMAND,
    LIMIT_HIGH_VOLUME_REFRESH_RATE_COMMAND, LOW_BATTERY_PROMPT_COMMAND, LimitHighVolume,
    ModelError, SIDE_TONE_COMMAND, SOUND_MODE_COMMAND, SOUND_MODE_CYCLE_COMMAND,
    VOICE_PROMPT_COMMAND, VolumeLimitRefreshRate, button_configuration_write,
    limit_high_volume_refresh_rate_write, limit_high_volume_write, sound_mode_cycle_write,
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
    /// Set the auto power off value on a device that reports its extra fields.
    SetAutoPowerOff(AutoPowerOffSetting),
    /// Set the double-press button action.
    SetButtonDoublePress(Option<ButtonAction>),
    /// Set which modes the ambient-mode cycle includes.
    SetAmbientSoundModeCycle(AmbientSoundModeCycle),
    /// Enable or disable the high-volume limiter and set its limit.
    SetLimitHighVolume {
        /// Whether the limiter is enabled.
        enabled: bool,
        /// The limit in decibels.
        db_limit: u8,
    },
    /// Set how often the high-volume limiter refreshes its level reading.
    SetLimitHighVolumeRefreshRate(VolumeLimitRefreshRate),
    /// Enable or disable simultaneous connections to two devices.
    SetDualConnections(bool),
    /// Enable or disable LDAC.
    SetLdac(bool),
    /// Enable or disable the low battery prompt.
    SetLowBatteryPrompt(bool),
    /// Enable or disable Dolby Audio.
    SetDolbyAudio(bool),
    /// Enable or disable side tone.
    SetSideTone(bool),
    /// Enable or disable the ambient sound mode voice prompt.
    SetVoicePrompt(bool),
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
            Self::SetAutoPowerOff(AutoPowerOffSetting::Enabled(index))
                if !capabilities.auto_power_off_write || index >= AUTO_POWER_OFF_DURATION_COUNT =>
            {
                Err(ModelError::InvalidAutoPowerOffWrite)
            }
            Self::SetAutoPowerOff(_) if !capabilities.auto_power_off_write => {
                Err(ModelError::InvalidAutoPowerOffWrite)
            }
            Self::SetButtonDoublePress(_) if !capabilities.button_configuration_write => {
                Err(ModelError::InvalidButtonConfigurationWrite)
            }
            Self::SetAmbientSoundModeCycle(_) if !capabilities.ambient_sound_mode_cycle_write => {
                Err(ModelError::InvalidAmbientSoundModeCycleWrite)
            }
            Self::SetLimitHighVolume { .. } | Self::SetLimitHighVolumeRefreshRate(_)
                if !capabilities.limit_high_volume_write =>
            {
                Err(ModelError::InvalidLimitHighVolumeWrite)
            }
            Self::SetDualConnections(_) if !capabilities.dual_connections_write => {
                Err(ModelError::InvalidDualConnectionsWrite)
            }
            Self::SetLdac(_) if !capabilities.ldac_write => Err(ModelError::InvalidLdacWrite),
            Self::SetLowBatteryPrompt(_) if !capabilities.low_battery_prompt_write => {
                Err(ModelError::InvalidLowBatteryPromptWrite)
            }
            Self::SetDolbyAudio(_) if !capabilities.dolby_audio_write => {
                Err(ModelError::InvalidDolbyAudioWrite)
            }
            Self::SetSideTone(_) if !capabilities.side_tone_write => {
                Err(ModelError::InvalidSideToneWrite)
            }
            Self::SetVoicePrompt(_) if !capabilities.voice_prompt_write => {
                Err(ModelError::InvalidVoicePromptWrite)
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
    /// The value a read-back must report for a high-volume limiter write.
    pub expected_limit_high_volume: Option<LimitHighVolume>,
}

/// The confirmation path of a write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirmation {
    /// The device acknowledges the write with an empty response, then a state
    /// response is read back.
    Acknowledged,
    /// The write has no acknowledgement; a state response is read back directly.
    StateRead,
    /// The write has no acknowledgement; the dedicated request confirms the flag.
    Targeted([u8; 2]),
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
    /// A double-press button action change.
    ButtonConfiguration(Option<ButtonAction>),
    /// An ambient sound mode cycle change.
    AmbientSoundModeCycle(AmbientSoundModeCycle),
    /// A high-volume limiter enable flag and limit change.
    LimitHighVolume {
        /// Whether the limiter is enabled.
        enabled: bool,
        /// The limit in decibels.
        db_limit: u8,
    },
    /// A high-volume limiter refresh-rate change.
    LimitHighVolumeRefreshRate(VolumeLimitRefreshRate),
    /// A dual-connections flag change.
    DualConnections(bool),
    /// An LDAC flag change.
    Ldac(bool),
    /// A low battery prompt flag change.
    LowBatteryPrompt(bool),
    /// A Dolby Audio flag change.
    DolbyAudio(bool),
    /// A side tone flag change.
    SideTone(bool),
    /// An ambient sound mode voice prompt flag change.
    VoicePrompt(bool),
    /// An equalizer curve change.
    Equalizer(EqualizerSetting),
}

impl WriteTarget {
    /// The wire command that carries this write for the selected model.
    pub(crate) fn command(self, model: ModelDriver) -> [u8; 2] {
        match self {
            Self::SoundMode(_) | Self::AmbientMode(_) => SOUND_MODE_COMMAND,
            Self::GamingMode(_) => GAMING_MODE_COMMAND,
            Self::AutoPowerOff(_) => AUTO_POWER_OFF_COMMAND,
            Self::ButtonConfiguration(_) => BUTTON_CONFIGURATION_COMMAND,
            Self::AmbientSoundModeCycle(_) => SOUND_MODE_CYCLE_COMMAND,
            Self::LimitHighVolume { .. } => LIMIT_HIGH_VOLUME_COMMAND,
            Self::LimitHighVolumeRefreshRate(_) => LIMIT_HIGH_VOLUME_REFRESH_RATE_COMMAND,
            Self::DualConnections(_) => DUAL_CONNECTIONS_COMMAND,
            Self::Ldac(_) => LDAC_COMMAND,
            Self::LowBatteryPrompt(_) => LOW_BATTERY_PROMPT_COMMAND,
            Self::DolbyAudio(_) => DOLBY_AUDIO_COMMAND,
            Self::SideTone(_) => SIDE_TONE_COMMAND,
            Self::VoicePrompt(_) => VOICE_PROMPT_COMMAND,
            Self::Equalizer(_) => model.equalizer_command(),
        }
    }

    /// How the device reports the write's result.
    pub(crate) fn confirmation(self) -> Confirmation {
        match self {
            // The legacy driver writes the flag with `send_without_response`;
            // LDAC is confirmed with the dedicated `01 7F` request.
            Self::Ldac(_) => Confirmation::Targeted(LDAC_STATE_COMMAND),
            Self::LowBatteryPrompt(_)
            | Self::DolbyAudio(_)
            | Self::SideTone(_)
            | Self::VoicePrompt(_) => Confirmation::StateRead,
            _ => Confirmation::Acknowledged,
        }
    }

    /// The flag value a dedicated confirmation request must report.
    pub(crate) fn targeted_expected(self) -> Option<bool> {
        match self {
            Self::Ldac(enabled) => Some(enabled),
            _ => None,
        }
    }

    /// Builds the write body from a fresh state response and its raw bytes.
    ///
    /// The equalizer write echoes the state's hear-id fields, so it needs the
    /// raw response next to the decoded one. The high-volume limiter resolves
    /// the field it leaves unchanged from the same response.
    pub(crate) fn write_body(
        self,
        state: DeviceState,
        raw_state: &[u8],
    ) -> Result<ResolvedWrite, ModelError> {
        let mut resolved = ResolvedWrite {
            body: Vec::new(),
            expected_auto_power_off: None,
            expected_limit_high_volume: None,
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
            Self::ButtonConfiguration(action) => {
                if !state.model.capabilities().button_configuration_write {
                    return Err(ModelError::InvalidButtonConfigurationWrite);
                }
                button_configuration_write(action).to_vec()
            }
            Self::AmbientSoundModeCycle(cycle) => {
                if state.settings.ambient_sound_mode_cycle.is_none() {
                    return Err(ModelError::InvalidAmbientSoundModeCycleWrite);
                }
                sound_mode_cycle_write(cycle).to_vec()
            }
            Self::LimitHighVolume { enabled, db_limit } => {
                let current = state
                    .settings
                    .limit_high_volume
                    .ok_or(ModelError::InvalidLimitHighVolumeWrite)?;
                resolved.expected_limit_high_volume = Some(LimitHighVolume {
                    enabled,
                    db_limit,
                    refresh_rate: current.refresh_rate,
                });
                limit_high_volume_write(enabled, db_limit)?.to_vec()
            }
            Self::LimitHighVolumeRefreshRate(refresh_rate) => {
                let current = state
                    .settings
                    .limit_high_volume
                    .ok_or(ModelError::InvalidLimitHighVolumeWrite)?;
                resolved.expected_limit_high_volume = Some(LimitHighVolume {
                    enabled: current.enabled,
                    db_limit: current.db_limit,
                    refresh_rate,
                });
                limit_high_volume_refresh_rate_write(refresh_rate).to_vec()
            }
            Self::DualConnections(enabled) => {
                if state.settings.dual_connections.is_none() {
                    return Err(ModelError::InvalidDualConnectionsWrite);
                }
                vec![u8::from(enabled)]
            }
            Self::Ldac(enabled) => {
                if state.settings.ldac.is_none() {
                    return Err(ModelError::InvalidLdacWrite);
                }
                vec![u8::from(enabled)]
            }
            Self::LowBatteryPrompt(enabled) => {
                if state.settings.low_battery_prompt.is_none() {
                    return Err(ModelError::InvalidLowBatteryPromptWrite);
                }
                vec![u8::from(enabled)]
            }
            Self::DolbyAudio(enabled) => {
                if state.settings.dolby_audio.is_none() {
                    return Err(ModelError::InvalidDolbyAudioWrite);
                }
                vec![u8::from(enabled)]
            }
            Self::SideTone(enabled) => {
                if state.settings.side_tone.is_none() {
                    return Err(ModelError::InvalidSideToneWrite);
                }
                vec![u8::from(enabled)]
            }
            Self::VoicePrompt(enabled) => {
                if state.settings.voice_prompt.is_none() {
                    return Err(ModelError::InvalidVoicePromptWrite);
                }
                vec![u8::from(enabled)]
            }
            Self::Equalizer(setting) => state.model.equalizer_write_body(setting, raw_state)?,
        };
        Ok(resolved)
    }

    /// Whether the confirmed state already carries this write's requested value.
    pub(crate) fn matches(
        self,
        state: DeviceState,
        expected_auto_power_off: Option<AutoPowerOff>,
        expected_limit_high_volume: Option<LimitHighVolume>,
    ) -> bool {
        match self {
            Self::SoundMode(setting) => state.sound_mode_setting() == Some(setting),
            Self::AmbientMode(mode) => state.sound_mode == Some(mode),
            Self::GamingMode(enabled) => state.gaming_mode == Some(enabled),
            Self::AutoPowerOff(_) => expected_auto_power_off
                .is_some_and(|expected| state.auto_power_off == Some(expected)),
            Self::ButtonConfiguration(action) => state.settings.button_double_press == action,
            Self::AmbientSoundModeCycle(cycle) => {
                state.settings.ambient_sound_mode_cycle == Some(cycle)
            }
            Self::LimitHighVolume { .. } | Self::LimitHighVolumeRefreshRate(_) => {
                expected_limit_high_volume
                    .is_some_and(|expected| state.settings.limit_high_volume == Some(expected))
            }
            Self::DualConnections(enabled) => state.settings.dual_connections == Some(enabled),
            Self::Ldac(enabled) => state.settings.ldac == Some(enabled),
            Self::LowBatteryPrompt(enabled) => state.settings.low_battery_prompt == Some(enabled),
            Self::DolbyAudio(enabled) => state.settings.dolby_audio == Some(enabled),
            Self::SideTone(enabled) => state.settings.side_tone == Some(enabled),
            Self::VoicePrompt(enabled) => state.settings.voice_prompt == Some(enabled),
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
    /// The read-back did not match the requested high-volume limiter value.
    #[error(
        "high-volume limiter did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    LimitHighVolumeMismatch {
        /// The value requested by the command.
        requested: Option<LimitHighVolume>,
        /// The value reported by the device, if it reported one.
        observed: Option<LimitHighVolume>,
    },
    /// The read-back did not match the requested double-press button action.
    #[error(
        "button configuration did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    ButtonConfigurationMismatch {
        /// The action requested by the command.
        requested: Option<ButtonAction>,
        /// The action reported by the device, if it reported one.
        observed: Option<ButtonAction>,
    },
    /// The read-back did not match the requested ambient sound mode cycle.
    #[error(
        "ambient sound mode cycle did not match the device read-back (requested {requested:?}, observed {observed:?})"
    )]
    AmbientSoundModeCycleMismatch {
        /// The flags requested by the command.
        requested: AmbientSoundModeCycle,
        /// The flags reported by the device, if it reported them.
        observed: Option<AmbientSoundModeCycle>,
    },
    /// The read-back did not match the requested dual-connections flag.
    #[error(
        "dual connections did not match the device read-back (requested {requested}, observed {observed:?})"
    )]
    DualConnectionsMismatch {
        /// The flag requested by the command.
        requested: bool,
        /// The flag reported by the device, if it reported one.
        observed: Option<bool>,
    },
    /// The dedicated LDAC request did not report the requested flag.
    #[error(
        "LDAC did not match the device read-back (requested {requested}, observed {observed:?})"
    )]
    LdacMismatch {
        /// The flag requested by the command.
        requested: bool,
        /// The last flag reported by the dedicated request.
        observed: Option<bool>,
    },
    /// The read-back did not match the requested boolean flag.
    #[error(
        "{flag} did not match the device read-back (requested {requested}, observed {observed:?})"
    )]
    FlagMismatch {
        /// The stable flag name.
        flag: &'static str,
        /// The flag requested by the command.
        requested: bool,
        /// The flag reported by the device, if it reported one.
        observed: Option<bool>,
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
}

/// Confirmed result for an accepted command.
pub type CommandResult = Result<CommandOutcome, CommandError>;
