use std::{collections::VecDeque, time::Duration};

use openscl4_models::{
    DeviceState, EqualizerSetting, IDENTITY_COMMAND, ModelDriver, ModelError, STATE_COMMAND,
    registry,
};
use openscl4_protocol::{Direction, Frame};
use openscl4_transport::TransportError;

use crate::{
    CommandError, CommandId, CommandOutcome, CommandResult, IoStage, PresetFieldMismatch,
    QuickPresetValues, SessionCommand, SessionError, WriteTarget,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const SEND_TIMEOUT: Duration = Duration::from_secs(2);
const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);
const READ_BACK_INTERVAL: Duration = Duration::from_millis(100);
const QUEUE_TIMEOUT: Duration = Duration::from_secs(5);
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const RECOVERY_WINDOW: Duration = Duration::from_secs(30);
const RETRY_INTERVAL: Duration = Duration::from_millis(250);
/// The interval a live-equalizer value waits between writes, inside the
/// 50–80 ms window agreed with the Sans-IO session contract.
const EQ_WRITE_INTERVAL: Duration = Duration::from_millis(80);

/// Chooses which layer manages recovery after losing a confirmed connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReconnectPolicy {
    /// End the session with the transport failure so the caller can reconnect.
    External,
    /// Retry within a bounded window and synchronize state before accepting writes.
    Internal,
}

/// Current stage of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// The transport provider is opening the link.
    Connecting,
    /// The session is waiting for the identification response.
    ProbingIdentity,
    /// A read-only state probe after the identity query failed.
    ProbingState(SessionError),
    /// The model is known and the initial state read is in progress.
    SyncingState(ModelDriver),
    /// A validated state response has been parsed for the current link.
    Ready(DeviceState),
    /// The identity or state format is not recognized, so only safe reads are available.
    Unidentified(UnidentifiedState),
    /// Recovery is in progress and the most recent confirmed snapshot is still available.
    Reconnecting {
        /// Snapshot from the last ready connection.
        last_confirmed: DeviceState,
        /// Why recovery started.
        reason: SessionError,
    },
    /// The link has been released.
    Closed,
    /// The session ended with a typed failure.
    Failed(SessionError),
}

impl ConnectionState {
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Closed | Self::Failed(_))
    }
}

/// Safe metadata for a device the session can read but not drive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnidentifiedState {
    /// The registered model, when identity was recognized but its state was not.
    pub model: Option<ModelDriver>,
    /// Why the session cannot confirm a device state.
    pub reason: UnidentifiedReason,
}

/// The safe cause of a read-only session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnidentifiedReason {
    /// An explicit identity names no registered model, or the probed state
    /// selects none after an unavailable identity response.
    UnknownIdentity,
    /// The registered model's state did not match its measured format.
    IncompatibleState,
}

/// An immutable session snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionView {
    /// Increases when a new state is published.
    pub revision: u64,
    /// Identifies the current connection attempt, starting with generation one.
    pub generation: u64,
    /// The current connection or confirmed device state.
    pub connection: ConnectionState,
    /// The most recent confirmed device snapshot, retained through link loss.
    pub last_confirmed: Option<DeviceState>,
}

pub struct QueuedCommand {
    pub id: CommandId,
    pub command: SessionCommand,
    pub submitted_at: Duration,
}

impl QueuedCommand {
    fn expires_at(&self) -> Duration {
        self.submitted_at + QUEUE_TIMEOUT
    }
}

pub enum Input {
    Opened,
    Released,
    RetryElapsed,
    RequestSent {
        receive_cutoff: u64,
    },
    RequestFailed {
        error: SessionError,
        send_started: bool,
    },
    Frame {
        frame: Frame,
        first_read: u64,
    },
    TimerElapsed,
    Command(QueuedCommand),
    /// A live value replaced the pending equalizer curve.
    EqLive {
        after_command: CommandId,
        setting: EqualizerSetting,
    },
    Close,
    Failed(SessionError),
    CleanupFailed(SessionError),
}

pub enum Effect {
    Open {
        deadline: Duration,
    },
    Release {
        deadline: Duration,
        interrupted_preset: Option<(CommandId, CommandError)>,
    },
    RetryAt(Duration),
    Request {
        command: [u8; 2],
        body: Vec<u8>,
        deadline: Duration,
    },
    Receive(Option<Duration>),
    Complete(CommandId, CommandResult),
    Stop,
}

impl Effect {
    fn request(now: Duration, command: [u8; 2], body: Vec<u8>) -> Self {
        Self::Request {
            command,
            body,
            deadline: now + SEND_TIMEOUT,
        }
    }

    fn read(now: Duration, command: [u8; 2]) -> Self {
        Self::request(now, command, Vec::new())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteStage {
    ReadCurrent,
    Ack,
    ReadBack {
        deadline: Option<Duration>,
        observed: Option<DeviceState>,
    },
    WaitForReadBack {
        deadline: Duration,
        observed: DeviceState,
    },
}

/// A deferred live-equalizer curve waiting for its write interval.
#[derive(Clone, Copy)]
struct PendingEq {
    after_command: CommandId,
    setting: EqualizerSetting,
    ready_at: Duration,
}

/// One in-flight device write: a discrete command, one field of a quick preset,
/// or a coalesced live-equalizer value.
#[derive(Clone, Copy)]
struct ActiveWrite {
    /// The submitter's command id, or none for a coalesced live-equalizer write.
    id: Option<CommandId>,
    /// The setting this write changes.
    target: WriteTarget,
    /// The preset fields still to apply after this one, or none outside a
    /// preset. The preset's last field carries the marker with every field
    /// consumed.
    preset: Option<QuickPresetValues>,
    confirmed_preset: Option<DeviceState>,
    model: ModelDriver,
    stage: WriteStage,
    may_have_written: bool,
}

impl ActiveWrite {
    fn interrupted_preset(self, error: SessionError) -> Option<CommandError> {
        Some(CommandError::PresetInterrupted {
            error,
            may_have_written: self.may_have_written,
            confirmed: self.confirmed_preset?,
        })
    }
}

#[derive(Clone, Copy)]
struct Recovery {
    deadline: Duration,
    last_confirmed: DeviceState,
    reason: SessionError,
}

/// The state transition logic has no runtime or transport access.
pub struct SessionMachine {
    view: SessionView,
    reconnect_policy: ReconnectPolicy,
    recovery: Option<Recovery>,
    reply_deadline: Option<Duration>,
    receive_cutoff: u64,
    active: Option<ActiveWrite>,
    pending_eq: Option<PendingEq>,
    last_eq_preset: CommandId,
    queue: VecDeque<QueuedCommand>,
}

impl SessionMachine {
    pub fn new(reconnect_policy: ReconnectPolicy) -> Self {
        Self {
            view: SessionView {
                revision: 0,
                generation: 0,
                connection: ConnectionState::Connecting,
                last_confirmed: None,
            },
            reconnect_policy,
            recovery: None,
            reply_deadline: None,
            receive_cutoff: 0,
            active: None,
            pending_eq: None,
            last_eq_preset: CommandId(0),
            queue: VecDeque::new(),
        }
    }

    pub fn view(&self) -> SessionView {
        match self.recovery {
            Some(recovery) => SessionView {
                connection: ConnectionState::Reconnecting {
                    last_confirmed: recovery.last_confirmed,
                    reason: recovery.reason,
                },
                ..self.view
            },
            None => self.view,
        }
    }

    pub fn open(&mut self, now: Duration) -> Effect {
        self.view.generation += 1;
        self.transition(ConnectionState::Connecting);
        self.limit_to_recovery(Effect::Open {
            deadline: now + CONNECT_TIMEOUT,
        })
    }

    pub fn input_from(&mut self, generation: u64, now: Duration, input: Input) -> Option<Effect> {
        if generation != self.view.generation {
            return None;
        }
        Some(self.input(now, input))
    }

    pub fn input(&mut self, now: Duration, input: Input) -> Effect {
        if let Some(recovery) = self.recovery
            && now >= recovery.deadline
            && !matches!(input, Input::Close | Input::CleanupFailed(_))
        {
            return self.stop_with(recovery.reason);
        }

        let effect = match input {
            Input::Opened => {
                self.transition(ConnectionState::ProbingIdentity);
                Effect::read(now, IDENTITY_COMMAND)
            }
            Input::Released => Effect::RetryAt(now + RETRY_INTERVAL),
            Input::RetryElapsed => self.open(now),
            Input::RequestSent { receive_cutoff } => {
                self.mark_write_attempt();
                self.receive_cutoff = receive_cutoff;
                let timeout = now + REPLY_TIMEOUT;
                let deadline = match &mut self.active {
                    Some(ActiveWrite {
                        stage: WriteStage::ReadBack { deadline, .. },
                        ..
                    }) => *deadline.get_or_insert(timeout),
                    _ => timeout,
                };
                self.reply_deadline = Some(deadline);
                self.next_effect(now)
            }
            Input::RequestFailed {
                error,
                send_started,
            } => {
                if send_started {
                    self.mark_write_attempt();
                }
                if error == SessionError::Closed {
                    self.input(now, Input::Close)
                } else {
                    self.fail(now, error)
                }
            }
            Input::TimerElapsed => self.on_timer(now),
            Input::Command(command) => {
                if command.command.replaces_pending_equalizer()
                    && let ConnectionState::Ready(device) = self.view.connection
                    && command.command.validate(device.model).is_ok()
                    && now < command.expires_at()
                {
                    self.last_eq_preset = command.id;
                    if self
                        .pending_eq
                        .is_some_and(|pending| pending.after_command < command.id)
                    {
                        self.pending_eq = None;
                    }
                }
                self.queue.push_back(command);
                self.next_effect(now)
            }
            Input::EqLive {
                after_command,
                setting,
            } => {
                if after_command >= self.last_eq_preset
                    && let ConnectionState::Ready(device) = self.view.connection
                    && device.model.capabilities().equalizer_write
                    && setting.is_writable()
                {
                    // Rapid values coalesce: the latest curve keeps the interval
                    // anchored at the first value.
                    let ready_at = self
                        .pending_eq
                        .map_or(now + EQ_WRITE_INTERVAL, |pending| pending.ready_at);
                    self.pending_eq = Some(PendingEq {
                        after_command,
                        setting,
                        ready_at,
                    });
                }
                if self.active.is_none()
                    && matches!(self.view.connection, ConnectionState::Ready(_))
                {
                    self.on_timer(now)
                } else {
                    self.next_effect(now)
                }
            }
            Input::Frame { frame, first_read } => self.on_frame(now, frame, first_read),
            Input::Close => {
                self.recovery = None;
                self.transition(ConnectionState::Closed);
                Effect::Stop
            }
            Input::CleanupFailed(_)
                if matches!(self.view.connection, ConnectionState::Failed(_)) =>
            {
                Effect::Stop
            }
            Input::CleanupFailed(error) => {
                self.stop_with(self.recovery.map_or(error, |recovery| recovery.reason))
            }
            Input::Failed(error) => self.fail(now, error),
        };
        self.limit_to_recovery(effect)
    }

    pub fn next_effect(&mut self, now: Duration) -> Effect {
        if let Some(command) = self.queue.front()
            && now >= command.expires_at()
        {
            let id = command.id;
            self.queue.pop_front();
            return Effect::Complete(id, Err(CommandError::QueueTimeout));
        }

        if self.active.is_none()
            && let ConnectionState::Ready(device) = self.view.connection
            && let Some(command) = self.queue.pop_front()
        {
            if let Err(error) = command.command.validate(device.model) {
                return Effect::Complete(command.id, Err(error.into()));
            }
            let (target, preset) = match command.command {
                SessionCommand::SetSoundMode(setting) => (WriteTarget::SoundMode(setting), None),
                SessionCommand::SetAmbientSoundMode(mode) => (WriteTarget::AmbientMode(mode), None),
                SessionCommand::SetGamingMode(enabled) => (WriteTarget::GamingMode(enabled), None),
                SessionCommand::ApplyQuickPreset(preset) => match preset.first_field() {
                    Some((target, rest)) => (target, Some(rest)),
                    None => {
                        // A preset with no enabled fields confirms the current state.
                        return Effect::Complete(command.id, Ok(CommandOutcome::Applied(device)));
                    }
                },
            };
            self.active = Some(ActiveWrite {
                id: Some(command.id),
                target,
                preset,
                confirmed_preset: None,
                model: device.model,
                stage: WriteStage::ReadCurrent,
                may_have_written: false,
            });
            return Effect::read(now, device.model.state_command());
        }

        let next_deadline = self
            .reply_deadline
            .into_iter()
            .chain(self.queue.front().map(QueuedCommand::expires_at))
            .chain(
                self.active
                    .is_none()
                    .then(|| self.pending_eq.as_ref().map(|pending| pending.ready_at))
                    .flatten(),
            )
            .min();
        Effect::Receive(next_deadline)
    }

    fn on_timer(&mut self, now: Duration) -> Effect {
        if self.reply_deadline.is_none_or(|deadline| now < deadline) {
            if self.active.is_none()
                && self.queue.is_empty()
                && let ConnectionState::Ready(device) = self.view.connection
                && let Some(pending) = self.pending_eq.take_if(|pending| now >= pending.ready_at)
            {
                self.active = Some(ActiveWrite {
                    id: None,
                    target: WriteTarget::Equalizer(pending.setting),
                    preset: None,
                    confirmed_preset: None,
                    model: device.model,
                    stage: WriteStage::ReadCurrent,
                    may_have_written: false,
                });
                return Effect::read(now, device.model.state_command());
            }
            return self.next_effect(now);
        }

        if let Some(write) = self.active {
            match write.stage {
                WriteStage::Ack => return self.read_back(now, write, None, None),
                WriteStage::WaitForReadBack { deadline, observed } if now < deadline => {
                    return self.read_back(now, write, Some(deadline), Some(observed));
                }
                WriteStage::ReadBack {
                    observed: Some(observed),
                    ..
                }
                | WriteStage::WaitForReadBack { observed, .. } => {
                    return self.finish_write(now, write, observed);
                }
                _ => {}
            }
        }
        match self.view.connection {
            ConnectionState::ProbingIdentity => {
                self.probe_state(now, SessionError::Timeout(IoStage::Receive))
            }
            ConnectionState::ProbingState(identity_error) => self.fail(now, identity_error),
            _ => self.fail(now, SessionError::Timeout(IoStage::Receive)),
        }
    }

    fn on_frame(&mut self, now: Duration, frame: Frame, first_read: u64) -> Effect {
        // A frame and its receive timer can become ready in the same runtime poll.
        if self.reply_deadline.is_some_and(|deadline| now >= deadline) {
            return self.on_timer(now);
        }
        if self.reply_deadline.is_none() || frame.direction != Direction::Inbound {
            return self.next_effect(now);
        }

        if let Some(write) = self.active {
            if first_read <= self.receive_cutoff {
                return self.next_effect(now);
            }
            return match write.stage {
                WriteStage::ReadCurrent if frame.command == write.model.state_command() => {
                    self.send_write(now, write, &frame.body)
                }
                WriteStage::Ack
                    if frame.command == write.target.command() && frame.body.is_empty() =>
                {
                    self.read_back(now, write, None, None)
                }
                WriteStage::ReadBack {
                    deadline: Some(deadline),
                    ..
                } if frame.command == write.model.state_command() => {
                    self.observe_write(now, deadline, write, &frame.body)
                }
                _ => self.next_effect(now),
            };
        }

        match self.view.connection {
            ConnectionState::ProbingIdentity
                if frame.command == IDENTITY_COMMAND && first_read > self.receive_cutoff =>
            {
                match ModelDriver::identify(&frame.body) {
                    Ok(model) => {
                        self.transition(ConnectionState::SyncingState(model));
                        Effect::read(now, model.state_command())
                    }
                    // A received identity naming no registered model is final:
                    // the state layout must not select a driver for it.
                    Err(ModelError::UnknownModel) => self.unidentified(
                        now,
                        UnidentifiedState {
                            model: None,
                            reason: UnidentifiedReason::UnknownIdentity,
                        },
                    ),
                    Err(error) => self.probe_state(now, error.into()),
                }
            }
            ConnectionState::ProbingState(_)
                if frame.command == STATE_COMMAND && first_read > self.receive_cutoff =>
            {
                match registry::identify_state(&frame.body) {
                    Ok(model) => self.confirm_state(now, model, &frame.body),
                    // The requested safe read was answered but selects no pilot.
                    Err(_) => self.unidentified(
                        now,
                        UnidentifiedState {
                            model: None,
                            reason: UnidentifiedReason::UnknownIdentity,
                        },
                    ),
                }
            }
            ConnectionState::SyncingState(model)
                if frame.command == model.state_command() && first_read > self.receive_cutoff =>
            {
                self.confirm_state(now, model, &frame.body)
            }
            _ => self.next_effect(now),
        }
    }

    fn probe_state(&mut self, now: Duration, reason: SessionError) -> Effect {
        self.transition(ConnectionState::ProbingState(reason));
        Effect::read(now, STATE_COMMAND)
    }

    fn confirm_state(&mut self, now: Duration, model: ModelDriver, body: &[u8]) -> Effect {
        match model.decode_confirmed_state(body) {
            Some(state) => {
                self.recovery = None;
                self.transition(ConnectionState::Ready(state));
                self.next_effect(now)
            }
            None => self.unidentified(
                now,
                UnidentifiedState {
                    model: Some(model),
                    reason: UnidentifiedReason::IncompatibleState,
                },
            ),
        }
    }

    /// Publishes a read-only view and keeps only safe reads available.
    fn unidentified(&mut self, now: Duration, state: UnidentifiedState) -> Effect {
        // A read-only connection cannot recover a confirmed snapshot by retrying.
        self.recovery = None;
        self.transition(ConnectionState::Unidentified(state));
        self.next_effect(now)
    }

    fn send_write(&mut self, now: Duration, write: ActiveWrite, body: &[u8]) -> Effect {
        let Some(state) = write.model.decode_confirmed_state(body) else {
            // A matching command with an unusable payload keeps the confirmed
            // state and the original deadline; the command waits for a valid reply.
            return self.next_effect(now);
        };
        if self.view.connection == ConnectionState::Ready(state) {
            self.reply_deadline = None;
        } else {
            self.transition(ConnectionState::Ready(state));
        }

        match write.target.write_body(state, body) {
            Ok(body) => {
                self.active = Some(ActiveWrite {
                    stage: WriteStage::Ack,
                    ..write
                });
                Effect::request(now, write.target.command(), body)
            }
            Err(error) => self.reject_write(now, write, error),
        }
    }

    /// Completes a write that could not be encoded without touching the device.
    fn reject_write(&mut self, now: Duration, write: ActiveWrite, error: ModelError) -> Effect {
        self.active = None;
        match write.id {
            Some(id) => Effect::Complete(
                id,
                Err(write
                    .interrupted_preset(error.into())
                    .unwrap_or_else(|| error.into())),
            ),
            None => self.next_effect(now),
        }
    }

    fn read_back(
        &mut self,
        now: Duration,
        write: ActiveWrite,
        deadline: Option<Duration>,
        observed: Option<DeviceState>,
    ) -> Effect {
        self.reply_deadline = None;
        self.active = Some(ActiveWrite {
            stage: WriteStage::ReadBack { deadline, observed },
            ..write
        });
        let send_deadline = now + SEND_TIMEOUT;
        Effect::Request {
            command: write.model.state_command(),
            body: Vec::new(),
            deadline: deadline.map_or(send_deadline, |deadline| deadline.min(send_deadline)),
        }
    }

    fn observe_write(
        &mut self,
        now: Duration,
        deadline: Duration,
        write: ActiveWrite,
        body: &[u8],
    ) -> Effect {
        let Some(state) = write.model.decode_confirmed_state(body) else {
            // Keep the confirmed state and the original deadline for a valid reply.
            return self.next_effect(now);
        };
        if write.target.matches(state) {
            return self.finish_write(now, write, state);
        }

        // A device can acknowledge the write before the new value appears in a state response.
        self.active = Some(ActiveWrite {
            stage: WriteStage::WaitForReadBack {
                deadline,
                observed: state,
            },
            ..write
        });
        self.reply_deadline = Some((now + READ_BACK_INTERVAL).min(deadline));
        self.next_effect(now)
    }

    fn finish_write(&mut self, now: Duration, write: ActiveWrite, state: DeviceState) -> Effect {
        self.transition(ConnectionState::Ready(state));

        if !write.target.matches(state) {
            self.active = None;
            return match write.id {
                Some(id) => Effect::Complete(id, Err(write_failure(write, state))),
                None => self.continue_live_eq(now),
            };
        }

        if let Some(rest) = write.preset
            && let Some((target, rest)) = rest.first_field()
        {
            // The preset confirmed this field; the next one starts fresh.
            self.active = Some(ActiveWrite {
                target,
                preset: Some(rest),
                confirmed_preset: Some(state),
                stage: WriteStage::ReadCurrent,
                may_have_written: false,
                ..write
            });
            return Effect::read(now, write.model.state_command());
        }

        self.active = None;
        match write.id {
            Some(id) => Effect::Complete(id, Ok(CommandOutcome::Applied(state))),
            None => self.continue_live_eq(now),
        }
    }

    /// Continues after a live-equalizer write reconciled with the confirmed state.
    ///
    /// A value that became ready while the write was in flight waits a full
    /// interval after this write, keeping the interval between live writes.
    fn continue_live_eq(&mut self, now: Duration) -> Effect {
        if let Some(pending) = &mut self.pending_eq
            && pending.ready_at <= now
        {
            pending.ready_at = now + EQ_WRITE_INTERVAL;
        }
        self.next_effect(now)
    }

    fn mark_write_attempt(&mut self) {
        if let Some(write) = &mut self.active
            && write.stage == WriteStage::Ack
        {
            write.may_have_written = true;
        }
    }

    pub fn terminal_command_error(&self, id: CommandId) -> CommandError {
        let failure = match self.view.connection {
            ConnectionState::Failed(error) => error,
            _ => SessionError::Closed,
        };
        let write = self.active.filter(|write| write.id == Some(id));
        if let Some(partial) = write.and_then(|write| write.interrupted_preset(failure)) {
            return partial;
        }
        if matches!(
            failure,
            SessionError::Transport {
                error: TransportError::Disconnected,
                ..
            }
        ) {
            CommandError::Disconnected
        } else if write.is_some_and(|write| write.may_have_written) {
            CommandError::Unconfirmed(failure)
        } else {
            failure.into()
        }
    }

    fn fail(&mut self, now: Duration, error: SessionError) -> Effect {
        if self.reconnect_policy == ReconnectPolicy::Internal
            && self.recovery.is_none()
            && let ConnectionState::Ready(last_confirmed) = self.view.connection
            && matches!(
                error,
                SessionError::Transport {
                    error: TransportError::Disconnected | TransportError::Closed,
                    ..
                }
            )
        {
            self.recovery = Some(Recovery {
                deadline: now + RECOVERY_WINDOW,
                last_confirmed,
                reason: error,
            });
        }

        if self.recovery.is_some() && retryable(error) {
            // The deferred live value does not survive a lost connection; applying
            // it again takes a new user action.
            let interrupted_preset = self
                .active
                .take()
                .and_then(|write| Some((write.id?, write.interrupted_preset(error)?)));
            self.pending_eq = None;
            self.queue.clear();
            self.receive_cutoff = 0;
            self.transition(ConnectionState::Connecting);
            return Effect::Release {
                deadline: now + CLOSE_TIMEOUT,
                interrupted_preset,
            };
        }

        self.stop_with(error)
    }

    fn stop_with(&mut self, error: SessionError) -> Effect {
        self.recovery = None;
        self.transition(ConnectionState::Failed(error));
        Effect::Stop
    }

    fn limit_to_recovery(&self, mut effect: Effect) -> Effect {
        if let Some(recovery) = self.recovery {
            match &mut effect {
                Effect::Open { deadline }
                | Effect::Release { deadline, .. }
                | Effect::RetryAt(deadline)
                | Effect::Request { deadline, .. } => {
                    *deadline = (*deadline).min(recovery.deadline);
                }
                Effect::Receive(deadline) => {
                    *deadline =
                        Some(deadline.map_or(recovery.deadline, |at| at.min(recovery.deadline)));
                }
                _ => {}
            }
        }
        effect
    }

    fn transition(&mut self, connection: ConnectionState) {
        self.reply_deadline = None;
        if let ConnectionState::Ready(state) = connection {
            self.view.last_confirmed = Some(state);
        }
        self.view.connection = connection;
        self.view.revision += 1;
    }
}

fn retryable(error: SessionError) -> bool {
    matches!(
        error,
        SessionError::Timeout(_)
            | SessionError::Transport {
                error: TransportError::Disconnected
                    | TransportError::Closed
                    | TransportError::DeviceNotFound
                    | TransportError::Unavailable
                    | TransportError::Busy
                    | TransportError::Io,
                ..
            }
    )
}

/// The typed failure for a write whose read-back did not match the request.
///
/// A preset field failure carries the last confirmed snapshot, so the fields
/// confirmed before it stay observable; a live-equalizer write has no
/// submitter and never reaches this mapping.
fn write_failure(write: ActiveWrite, state: DeviceState) -> CommandError {
    match write.target {
        WriteTarget::SoundMode(requested) => {
            let Some(observed) = state.sound_mode_setting() else {
                return CommandError::Model(ModelError::InvalidSoundModeWrite);
            };
            if write.preset.is_some() {
                CommandError::PresetMismatch {
                    field: PresetFieldMismatch::SoundMode {
                        requested,
                        observed,
                    },
                    confirmed: state,
                }
            } else {
                CommandError::ModeMismatch {
                    requested,
                    observed,
                }
            }
        }
        WriteTarget::AmbientMode(requested) => {
            let Some(observed) = state.sound_mode else {
                return CommandError::Model(ModelError::InvalidSoundModeWrite);
            };
            if write.preset.is_some() {
                CommandError::PresetMismatch {
                    field: PresetFieldMismatch::AmbientMode {
                        requested,
                        observed,
                    },
                    confirmed: state,
                }
            } else {
                CommandError::AmbientModeMismatch {
                    requested,
                    observed,
                }
            }
        }
        WriteTarget::Equalizer(requested) => CommandError::PresetMismatch {
            field: PresetFieldMismatch::Equalizer {
                requested,
                observed: state.equalizer,
            },
            confirmed: state,
        },
        WriteTarget::GamingMode(requested) => CommandError::GamingModeMismatch {
            requested,
            observed: state.gaming_mode,
        },
    }
}

#[cfg(test)]
mod tests {
    use openscl4_models::{EQUALIZER_COMMAND, SOUND_MODE_COMMAND, SoundMode, SoundModeSetting};
    use openscl4_protocol::encode_frame;

    use super::*;

    const STATE: &[u8] = include_bytes!("../../../data/a3954-pilot-state.bin");
    const A3062_STATE: &[u8] = include_bytes!("../../../data/a3062-pilot-state.bin");
    const A3062_NORMAL: &[u8] = include_bytes!("../../../data/a3062-mode-normal.bin");
    const A3062_TRANSPARENCY: &[u8] = include_bytes!("../../../data/a3062-mode-transparency.bin");
    // The a3062-anc/transparency frame from conformance.json.
    const A3062_TRANSPARENCY_WRITE: &[u8] =
        b"\x08\xee\0\0\0\x06\x81\x10\0\x01\x51\x01\x01\0\x05\xe6";
    const A3949_STATE: &[u8] = include_bytes!("../../../data/a3949-pilot-state.bin");

    #[test]
    fn frame_and_timer_from_the_prior_generation_are_ignored_during_sync() {
        let mut machine = SessionMachine::new(ReconnectPolicy::Internal);
        let now = Duration::ZERO;
        let identity = b"04.2904.29A3954DEMO0000000";
        let state = state_body(STATE);
        let sent = || Input::RequestSent { receive_cutoff: 0 };

        assert!(matches!(machine.open(now), Effect::Open { .. }));
        let old = machine.view().generation;
        machine.input(now, Input::Opened);
        machine.input(now, sent());
        machine.input(now, inbound(IDENTITY_COMMAND, identity));
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &state));
        assert!(matches!(
            machine.view().connection,
            ConnectionState::Ready(_)
        ));

        let reason = SessionError::Transport {
            stage: IoStage::Receive,
            error: TransportError::Disconnected,
        };
        assert!(matches!(
            machine.input(now, Input::Failed(reason)),
            Effect::Release { .. }
        ));
        assert!(matches!(
            machine.input(now, Input::Released),
            Effect::RetryAt(_)
        ));
        let now = RETRY_INTERVAL;
        assert!(matches!(
            machine.input(now, Input::RetryElapsed),
            Effect::Open { .. }
        ));
        let new = machine.view().generation;
        assert!(new > old);
        machine.input(now, Input::Opened);
        machine.input(now, sent());
        machine.input(now, inbound(IDENTITY_COMMAND, identity));
        machine.input(now, sent());

        let reconnecting = machine.view();
        assert!(
            machine
                .input_from(old, now, inbound(STATE_COMMAND, &state))
                .is_none()
        );
        assert!(machine.input_from(old, now, Input::TimerElapsed).is_none());
        assert_eq!(machine.view(), reconnecting);
        assert!(
            machine
                .input_from(new, now, inbound(STATE_COMMAND, &state))
                .is_some()
        );
        assert!(matches!(
            machine.view().connection,
            ConnectionState::Ready(_)
        ));
    }

    #[test]
    fn keeps_the_deadline_from_send_success_across_unrelated_frames() {
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        let request = machine.input(Duration::ZERO, Input::Opened);
        assert!(
            matches!(request, Effect::Request { command: IDENTITY_COMMAND, body, deadline } if body.is_empty() && deadline == SEND_TIMEOUT)
        );

        let sent = Duration::from_secs(1);
        let expected = sent + REPLY_TIMEOUT;
        let receive = machine.input(sent, Input::RequestSent { receive_cutoff: 0 });
        assert!(matches!(receive, Effect::Receive(Some(deadline)) if deadline == expected));

        let receive = machine.input(sent + Duration::from_millis(500), inbound([0, 0], &[]));
        assert!(matches!(receive, Effect::Receive(Some(deadline)) if deadline == expected));

        let fallback = machine.input(expected, Input::TimerElapsed);
        assert!(matches!(
            fallback,
            Effect::Request {
                command: STATE_COMMAND,
                ..
            }
        ));
        machine.input(expected, Input::RequestSent { receive_cutoff: 0 });
        let stopped = machine.input(expected + REPLY_TIMEOUT, Input::TimerElapsed);
        assert!(matches!(stopped, Effect::Stop));
        assert_eq!(
            machine.view().connection,
            ConnectionState::Failed(SessionError::Timeout(IoStage::Receive))
        );
    }

    #[test]
    fn ack_requests_read_back_and_never_completes_without_a_matching_mode() {
        let now = Duration::ZERO;
        let model = ModelDriver::A3954;
        let state = state_body(STATE);
        let original = model.decode_state(&state).expect("decode baseline fixture");
        assert_eq!(
            original
                .sound_mode_write(SoundModeSetting::NORMAL)
                .expect("measured normal write")
                .bytes(),
            [2, 6, 0, 1],
        );
        let mut current = state.to_vec();
        openscl4_models::SoundModeWrite::decode(&[0, 11, 1, 0])
            .expect("controls changed at the headset")
            .apply_to_state(&mut current)
            .expect("apply changed controls to the state");
        let refreshed = model
            .decode_state(&current)
            .expect("decode refreshed state");
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        machine.input(
            now,
            inbound(IDENTITY_COMMAND, b"04.2904.29A3954DEMO0000000"),
        );
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        machine.input(now, inbound(model.state_command(), &state));

        let id = CommandId(1);
        let requested = SoundModeSetting::NORMAL;
        let refresh = machine.input(
            now,
            Input::Command(QueuedCommand {
                id,
                command: SessionCommand::SetSoundMode(requested),
                submitted_at: now,
            }),
        );
        assert!(
            matches!(refresh, Effect::Request { command, body, .. } if command == model.state_command() && body.is_empty())
        );
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        let write = machine.input(now, inbound(model.state_command(), &current));
        assert!(
            matches!(write, Effect::Request { command: SOUND_MODE_COMMAND, body, .. } if body == [2, 6, 1, 0])
        );
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });

        let read_back = machine.input(now, inbound(SOUND_MODE_COMMAND, &[]));
        assert!(
            matches!(read_back, Effect::Request { command, body, .. } if command == model.state_command() && body.is_empty())
        );
        assert_eq!(machine.view().connection, ConnectionState::Ready(refreshed));

        let waiting = machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        assert!(matches!(waiting, Effect::Receive(Some(_))));
        let retry = machine.input(now, inbound(model.state_command(), &current));
        assert!(matches!(retry, Effect::Receive(Some(deadline)) if deadline == READ_BACK_INTERVAL));
        let completed = machine.input(REPLY_TIMEOUT, Input::TimerElapsed);
        assert!(matches!(
            completed,
            Effect::Complete(completed_id, Err(CommandError::ModeMismatch {requested: expected, observed}))
                if completed_id == id && expected == requested && Some(observed) == original.sound_mode_setting()
        ));
    }

    #[test]
    fn rereads_until_the_written_mode_appears_without_sending_another_write() {
        let original = state_body(STATE);
        let model = ModelDriver::A3954;
        let requested = SoundModeSetting::NORMAL;
        let id = CommandId(1);
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.active = Some(ActiveWrite {
            id: Some(id),
            target: WriteTarget::SoundMode(requested),
            preset: None,
            confirmed_preset: None,
            model,
            stage: WriteStage::ReadBack {
                deadline: None,
                observed: None,
            },
            may_have_written: true,
        });
        machine.input(Duration::ZERO, Input::RequestSent { receive_cutoff: 0 });
        let retry = machine.input(Duration::ZERO, inbound(model.state_command(), &original));
        assert!(matches!(retry, Effect::Receive(Some(deadline)) if deadline == READ_BACK_INTERVAL));

        let request = machine.input(READ_BACK_INTERVAL, Input::TimerElapsed);
        assert!(
            matches!(request, Effect::Request { command, body, deadline }
                if command == model.state_command() && body.is_empty() && deadline == REPLY_TIMEOUT)
        );
        machine.input(READ_BACK_INTERVAL, Input::RequestSent { receive_cutoff: 0 });
        let mut changed = original.to_vec();
        model
            .decode_state(&original)
            .expect("baseline state")
            .sound_mode_write(requested)
            .expect("valid write")
            .apply_to_state(&mut changed)
            .expect("write has appeared");
        let completed = machine.input(READ_BACK_INTERVAL, inbound(model.state_command(), &changed));
        assert!(matches!(
            completed,
            Effect::Complete(completed_id, Ok(CommandOutcome::Applied(state)))
                if completed_id == id && state.sound_mode_setting() == Some(requested)
        ));
    }

    fn state_body(fixture: &[u8]) -> bytes::Bytes {
        let mut decoder = openscl4_protocol::FrameDecoder::default();
        decoder.push(fixture);
        let frame = decoder
            .next_frame()
            .expect("valid state frame")
            .expect("complete state frame");
        assert_eq!(frame.command, STATE_COMMAND);
        assert_eq!(frame.direction, Direction::Inbound);
        frame.body
    }

    #[test]
    fn a3062_write_preserves_fresh_controls_and_waits_for_observed_mode() {
        let now = Duration::ZERO;
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        machine.input(now, Input::Opened);
        machine.input(now, sent());
        machine.input(now, inbound(IDENTITY_COMMAND, &[]));
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &state_body(A3062_STATE)));

        let id = CommandId(1);
        let refresh = machine.input(
            now,
            Input::Command(QueuedCommand {
                id,
                command: SessionCommand::SetAmbientSoundMode(SoundMode::Transparency),
                submitted_at: now,
            }),
        );
        assert!(
            matches!(refresh, Effect::Request { command: STATE_COMMAND, body, .. } if body.is_empty())
        );
        machine.input(now, sent());

        // The two captures of normal mode have different ANC and transparency controls.
        let normal = state_body(A3062_NORMAL);
        let Effect::Request { command, body, .. } =
            machine.input(now, inbound(STATE_COMMAND, &normal))
        else {
            panic!("expected a transparency write after refreshing the state");
        };
        let wire = encode_frame(Direction::Outbound, command, &body).expect("encode mode write");
        assert_eq!(wire.as_ref(), A3062_TRANSPARENCY_WRITE);
        machine.input(now, sent());

        let read_back = machine.input(now, inbound(SOUND_MODE_COMMAND, &[]));
        assert!(
            matches!(read_back, Effect::Request { command: STATE_COMMAND, body, .. } if body.is_empty())
        );
        machine.input(now, sent());
        let waiting = machine.input(now, inbound(STATE_COMMAND, &normal));
        assert!(matches!(waiting, Effect::Receive(Some(at)) if at == READ_BACK_INTERVAL));

        let retry = machine.input(READ_BACK_INTERVAL, Input::TimerElapsed);
        assert!(
            matches!(retry, Effect::Request { command: STATE_COMMAND, body, .. } if body.is_empty())
        );
        machine.input(READ_BACK_INTERVAL, sent());
        let completed = machine.input(
            READ_BACK_INTERVAL,
            inbound(STATE_COMMAND, &state_body(A3062_TRANSPARENCY)),
        );
        assert!(matches!(
            completed,
            Effect::Complete(completed_id, Ok(CommandOutcome::Applied(state)))
                if completed_id == id && state.sound_mode == Some(SoundMode::Transparency)
        ));
    }

    #[test]
    fn a3062_state_probe_rejects_positioned_writes_without_another_request() {
        let mut decoder = openscl4_protocol::FrameDecoder::default();
        decoder.push(A3062_STATE);
        let frame = decoder
            .next_frame()
            .expect("valid state frame")
            .expect("complete state frame");
        let now = Duration::ZERO;
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        let probe = machine.input(now, inbound(IDENTITY_COMMAND, &[]));
        assert!(matches!(
            probe,
            Effect::Request {
                command: STATE_COMMAND,
                ..
            }
        ));
        machine.input(now, Input::RequestSent { receive_cutoff: 1 });
        machine.input(
            now,
            Input::Frame {
                frame,
                first_read: 2,
            },
        );

        let ConnectionState::Ready(snapshot) = machine.view().connection else {
            panic!("expected confirmed A3062 state");
        };
        assert_eq!(snapshot.model, ModelDriver::A3062);
        assert_eq!(snapshot.sound_mode_setting(), None);
        let id = CommandId(1);
        let rejected = machine.input(
            now,
            Input::Command(QueuedCommand {
                id,
                command: SessionCommand::SetSoundMode(SoundModeSetting::NOISE_CANCELING),
                submitted_at: now,
            }),
        );
        assert!(matches!(
            rejected,
            Effect::Complete(completed, Err(CommandError::Model(ModelError::InvalidSoundModeWrite)))
                if completed == id
        ));
        assert_eq!(machine.view().connection, ConnectionState::Ready(snapshot));
    }

    #[test]
    fn a3949_state_probe_enables_reading_and_rejects_sound_mode_writes() {
        let now = Duration::ZERO;
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        machine.input(now, inbound(IDENTITY_COMMAND, &[]));
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        machine.input(now, inbound(STATE_COMMAND, &state_body(A3949_STATE)));

        let ConnectionState::Ready(state) = machine.view().connection else {
            panic!("expected confirmed A3949 state");
        };
        assert_eq!(state.model, ModelDriver::A3949);
        assert_eq!(state.sound_mode, None);

        for (id, command) in [
            (
                CommandId(1),
                SessionCommand::SetSoundMode(SoundModeSetting::NORMAL),
            ),
            (
                CommandId(2),
                SessionCommand::SetAmbientSoundMode(SoundMode::Normal),
            ),
        ] {
            let effect = machine.input(
                now,
                Input::Command(QueuedCommand {
                    id,
                    command,
                    submitted_at: now,
                }),
            );
            assert!(matches!(
                effect,
                Effect::Complete(completed, Err(CommandError::Model(ModelError::InvalidSoundModeWrite)))
                    if completed == id
            ));
        }
        assert_eq!(machine.view().connection, ConnectionState::Ready(state));
    }

    #[test]
    fn a_cancelled_send_marks_only_the_attempted_write_as_unconfirmed() {
        let attempted = CommandId(1);
        let queued = CommandId(2);
        for send_started in [false, true] {
            let mut machine = SessionMachine::new(ReconnectPolicy::External);
            machine.active = Some(ActiveWrite {
                id: Some(attempted),
                target: WriteTarget::SoundMode(SoundModeSetting::NORMAL),
                preset: None,
                confirmed_preset: None,
                model: ModelDriver::A3954,
                stage: WriteStage::Ack,
                may_have_written: false,
            });
            let effect = machine.input(
                Duration::ZERO,
                Input::RequestFailed {
                    error: SessionError::Closed,
                    send_started,
                },
            );

            assert!(matches!(effect, Effect::Stop));
            let expected = if send_started {
                CommandError::Unconfirmed(SessionError::Closed)
            } else {
                CommandError::Session(SessionError::Closed)
            };
            assert_eq!(machine.terminal_command_error(attempted), expected);
            assert_eq!(
                machine.terminal_command_error(queued),
                CommandError::Session(SessionError::Closed)
            );
        }
    }

    fn ready_a3062(reconnect_policy: ReconnectPolicy) -> SessionMachine {
        let now = Duration::ZERO;
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        let mut machine = SessionMachine::new(reconnect_policy);
        machine.input(now, Input::Opened);
        machine.input(now, sent());
        machine.input(now, inbound(IDENTITY_COMMAND, &[]));
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &state_body(A3062_STATE)));
        machine
    }

    /// One band byte in the measured A3062 wire scale.
    fn band_wire(gain: i16) -> u8 {
        (gain + 120) as u8
    }

    fn live_before_command(setting: EqualizerSetting) -> Input {
        Input::EqLive {
            after_command: CommandId(0),
            setting,
        }
    }

    fn curve(preset_id: u16, first_band: i16) -> EqualizerSetting {
        EqualizerSetting {
            preset_id,
            volume_adjustments: [first_band, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        }
    }

    /// Writes the curve into the A3062 state layout: preset at 23, bands at 25.
    fn apply_curve(body: &mut [u8], setting: EqualizerSetting) {
        body[23..25].copy_from_slice(&setting.preset_id.to_le_bytes());
        body[25..35].copy_from_slice(&setting.volume_adjustments.map(band_wire));
    }

    fn a3062_state_with_eq(setting: EqualizerSetting) -> Vec<u8> {
        let mut body = state_body(A3062_STATE).to_vec();
        apply_curve(&mut body, setting);
        body
    }

    fn a3062_transparency_state() -> Vec<u8> {
        let mut body = state_body(A3062_STATE).to_vec();
        body[69] = 1; // Transparency on the A3062 sound-mode byte.
        body
    }

    #[test]
    fn live_eq_coalesces_rapid_values_into_one_write() {
        let mut machine = ready_a3062(ReconnectPolicy::External);
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        let first = curve(0xfefe, 60);
        let second = curve(0xfefe, -60);

        let wait = machine.input(Duration::ZERO, live_before_command(first));
        assert!(matches!(wait, Effect::Receive(Some(at)) if at == EQ_WRITE_INTERVAL));
        // A value inside the interval replaces the pending curve and keeps the deadline.
        let wait = machine.input(Duration::from_millis(20), live_before_command(second));
        assert!(matches!(wait, Effect::Receive(Some(at)) if at == EQ_WRITE_INTERVAL));

        let refresh = machine.input(EQ_WRITE_INTERVAL, Input::TimerElapsed);
        assert!(
            matches!(refresh, Effect::Request { command, body, .. } if command == STATE_COMMAND && body.is_empty())
        );
        machine.input(EQ_WRITE_INTERVAL, sent());
        // The write carries the latest curve, not the first one.
        let Effect::Request {
            command,
            body,
            deadline,
        } = machine.input(
            EQ_WRITE_INTERVAL,
            inbound(STATE_COMMAND, &state_body(A3062_STATE)),
        )
        else {
            panic!("expected a live-equalizer write after the refresh");
        };
        assert_eq!(command, EQUALIZER_COMMAND);
        assert_eq!(deadline, EQ_WRITE_INTERVAL + SEND_TIMEOUT);
        assert_eq!(&body[0..2], &0xfefe_u16.to_le_bytes());
        assert_eq!(body[4..14], second.volume_adjustments.map(band_wire));
        machine.input(EQ_WRITE_INTERVAL, sent());

        let read_back = machine.input(EQ_WRITE_INTERVAL, inbound(EQUALIZER_COMMAND, &[]));
        assert!(
            matches!(read_back, Effect::Request { command, body, .. } if command == STATE_COMMAND && body.is_empty())
        );
        machine.input(EQ_WRITE_INTERVAL, sent());
        // A live write publishes the confirmed state and completes no command.
        let idle = machine.input(
            EQ_WRITE_INTERVAL,
            inbound(STATE_COMMAND, &a3062_state_with_eq(second)),
        );
        assert!(matches!(idle, Effect::Receive(None)));
        assert!(matches!(
            machine.view().connection,
            ConnectionState::Ready(state) if state.equalizer == Some(second)
        ));
    }

    #[test]
    fn preset_with_equalizer_supersedes_the_pending_live_value() {
        let mut machine = ready_a3062(ReconnectPolicy::External);
        let now = Duration::ZERO;
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        let preset_curve = curve(5, -30);
        machine.input(now, live_before_command(curve(0xfefe, 60)));

        let id = CommandId(1);
        let accepted = machine.input(
            now,
            Input::Command(QueuedCommand {
                id,
                command: SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    sound_mode: None,
                    ambient_sound_mode: Some(SoundMode::Transparency),
                    equalizer: Some(preset_curve),
                }),
                submitted_at: now,
            }),
        );
        assert!(
            matches!(accepted, Effect::Request { command, body, .. } if command == STATE_COMMAND && body.is_empty())
        );
        machine.input(now, sent());

        // The ambient field writes first, from a fresh state read. The normal
        // capture carries the controls of the golden transparency write.
        let Effect::Request { command, body, .. } =
            machine.input(now, inbound(STATE_COMMAND, &state_body(A3062_NORMAL)))
        else {
            panic!("expected the ambient-mode write");
        };
        assert_eq!(command, SOUND_MODE_COMMAND);
        let wire = encode_frame(Direction::Outbound, command, &body).expect("encode mode write");
        assert_eq!(wire.as_ref(), A3062_TRANSPARENCY_WRITE);
        machine.input(now, sent());
        machine.input(now, inbound(SOUND_MODE_COMMAND, &[]));
        machine.input(now, sent());
        // The read-back confirms transparency; the preset continues with its curve.
        let next_field = machine.input(now, inbound(STATE_COMMAND, &a3062_transparency_state()));
        assert!(
            matches!(next_field, Effect::Request { command, body, .. } if command == STATE_COMMAND && body.is_empty())
        );
        machine.input(now, sent());
        let Effect::Request { command, body, .. } =
            machine.input(now, inbound(STATE_COMMAND, &a3062_transparency_state()))
        else {
            panic!("expected the preset's equalizer write");
        };
        assert_eq!(command, EQUALIZER_COMMAND);
        assert_eq!(&body[0..2], &5_u16.to_le_bytes());
        machine.input(now, sent());
        machine.input(now, inbound(EQUALIZER_COMMAND, &[]));
        machine.input(now, sent());
        let mut applied = a3062_transparency_state();
        apply_curve(&mut applied, preset_curve);
        let completed = machine.input(now, inbound(STATE_COMMAND, &applied));
        assert!(matches!(
            completed,
            Effect::Complete(completed_id, Ok(CommandOutcome::Applied(state)))
                if completed_id == id
                    && state.sound_mode == Some(SoundMode::Transparency)
                    && state.equalizer == Some(preset_curve)
        ));
        // The superseded live value never writes.
        assert!(matches!(
            machine.input(now, Input::TimerElapsed),
            Effect::Receive(None)
        ));
    }

    #[test]
    fn preset_without_equalizer_keeps_the_pending_live_value() {
        let mut machine = ready_a3062(ReconnectPolicy::External);
        let now = Duration::ZERO;
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        let live = curve(0xfefe, -60);
        machine.input(now, live_before_command(live));

        let id = CommandId(1);
        machine.input(
            now,
            Input::Command(QueuedCommand {
                id,
                command: SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    sound_mode: None,
                    ambient_sound_mode: Some(SoundMode::Transparency),
                    equalizer: None,
                }),
                submitted_at: now,
            }),
        );
        machine.input(now, sent());
        let write = machine.input(now, inbound(STATE_COMMAND, &state_body(A3062_STATE)));
        assert!(matches!(
            write,
            Effect::Request {
                command: SOUND_MODE_COMMAND,
                ..
            }
        ));
        machine.input(now, sent());
        machine.input(now, inbound(SOUND_MODE_COMMAND, &[]));
        machine.input(now, sent());
        let completed = machine.input(now, inbound(STATE_COMMAND, &a3062_transparency_state()));
        assert!(matches!(
            completed,
            Effect::Complete(completed_id, Ok(CommandOutcome::Applied(state)))
                if completed_id == id && state.sound_mode == Some(SoundMode::Transparency)
        ));
        // The deferred live value keeps its interval and writes after the preset.
        let wait = machine.input(now, Input::TimerElapsed);
        assert!(matches!(wait, Effect::Receive(Some(at)) if at == EQ_WRITE_INTERVAL));
        let live_write = machine.input(EQ_WRITE_INTERVAL, Input::TimerElapsed);
        assert!(
            matches!(live_write, Effect::Request { command, body, .. } if command == STATE_COMMAND && body.is_empty())
        );
    }

    #[test]
    fn a_lost_connection_clears_the_pending_live_value() {
        let mut machine = ready_a3062(ReconnectPolicy::Internal);
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        machine.input(Duration::ZERO, live_before_command(curve(0xfefe, 60)));

        let reason = SessionError::Transport {
            stage: IoStage::Receive,
            error: TransportError::Disconnected,
        };
        assert!(matches!(
            machine.input(Duration::ZERO, Input::Failed(reason)),
            Effect::Release { .. }
        ));
        machine.input(Duration::ZERO, Input::Released);
        assert!(matches!(
            machine.input(RETRY_INTERVAL, Input::RetryElapsed),
            Effect::Open { .. }
        ));
        machine.input(RETRY_INTERVAL, Input::Opened);
        machine.input(RETRY_INTERVAL, sent());
        machine.input(RETRY_INTERVAL, inbound(IDENTITY_COMMAND, &[]));
        machine.input(RETRY_INTERVAL, sent());
        machine.input(
            RETRY_INTERVAL,
            inbound(STATE_COMMAND, &state_body(A3062_STATE)),
        );

        // The deferred curve was cleared with the connection.
        let idle = machine.input(RETRY_INTERVAL, Input::TimerElapsed);
        assert!(matches!(idle, Effect::Receive(None)));
    }

    #[test]
    fn a_failed_preset_field_keeps_the_confirmed_fields() {
        let mut machine = ready_a3062(ReconnectPolicy::External);
        let now = Duration::ZERO;
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        let preset_curve = curve(0xfefe, -60);
        let id = CommandId(1);
        machine.input(
            now,
            Input::Command(QueuedCommand {
                id,
                command: SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    sound_mode: None,
                    ambient_sound_mode: Some(SoundMode::Transparency),
                    equalizer: Some(preset_curve),
                }),
                submitted_at: now,
            }),
        );
        // The ambient field confirms first.
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &state_body(A3062_STATE)));
        machine.input(now, sent());
        machine.input(now, inbound(SOUND_MODE_COMMAND, &[]));
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &a3062_transparency_state()));

        // The equalizer field writes, but the read-back never shows the curve.
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &a3062_transparency_state()));
        machine.input(now, sent());

        // The unanswered write ACK and the repeated read-backs each wait their
        // turn on the virtual clock; the state never carries the preset's curve.
        let mut now = REPLY_TIMEOUT;
        loop {
            match machine.input(now, Input::TimerElapsed) {
                Effect::Request { command, body, .. }
                    if command == STATE_COMMAND && body.is_empty() =>
                {
                    machine.input(now, sent());
                    machine.input(now, inbound(STATE_COMMAND, &a3062_transparency_state()));
                }
                Effect::Receive(Some(deadline)) => {
                    now = deadline;
                    continue;
                }
                Effect::Complete(
                    completed_id,
                    Err(CommandError::PresetMismatch {
                        field:
                            PresetFieldMismatch::Equalizer {
                                requested,
                                observed,
                            },
                        confirmed,
                    }),
                ) => {
                    assert_eq!(completed_id, id);
                    assert_eq!(requested, preset_curve);
                    // The fixture reports the Soundcore Signature curve.
                    assert_eq!(
                        observed,
                        Some(EqualizerSetting {
                            preset_id: 0,
                            volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
                        })
                    );
                    // The sound-mode field confirmed before the failure stays applied.
                    assert_eq!(confirmed.sound_mode, Some(SoundMode::Transparency));
                    break;
                }
                _ => panic!("unexpected effect while waiting for the preset deadline"),
            }
            now += READ_BACK_INTERVAL;
        }
    }

    #[test]
    fn unknown_identity_and_state_publish_a_read_only_view() {
        let now = Duration::ZERO;
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        let probe = machine.input(
            now,
            inbound(IDENTITY_COMMAND, b"04.3004.309999UNKNOWN00000"),
        );
        assert!(
            matches!(probe, Effect::Receive(None)),
            "an explicit unknown identity must not probe the state"
        );

        assert_eq!(
            machine.view().connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: None,
                reason: UnidentifiedReason::UnknownIdentity,
            })
        );

        // A valid A3062 state pushed without a request cannot select a driver.
        let view = machine.view();
        machine.input(now, inbound(STATE_COMMAND, &state_body(A3062_STATE)));
        assert_eq!(machine.view(), view);

        let view = machine.view();
        assert!(
            machine
                .input_from(view.generation.wrapping_sub(1), now, Input::TimerElapsed)
                .is_none()
        );
        assert_eq!(machine.view(), view);
    }

    #[test]
    fn an_identity_timeout_then_a_fresh_unrecognized_state_publishes_read_only() {
        let now = Duration::ZERO;
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });

        let probe = machine.input(REPLY_TIMEOUT, Input::TimerElapsed);
        assert!(matches!(
            probe,
            Effect::Request {
                command: STATE_COMMAND,
                ..
            }
        ));
        assert_eq!(
            machine.view().connection,
            ConnectionState::ProbingState(SessionError::Timeout(IoStage::Receive))
        );
        machine.input(REPLY_TIMEOUT, Input::RequestSent { receive_cutoff: 1 });

        machine.input(
            REPLY_TIMEOUT,
            inbound_at(STATE_COMMAND, &state_body(STATE), 2),
        );
        assert_eq!(
            machine.view().connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: None,
                reason: UnidentifiedReason::UnknownIdentity,
            })
        );
    }

    #[test]
    fn an_identity_and_state_timeout_still_fails() {
        let now = Duration::ZERO;
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });

        let probe = machine.input(REPLY_TIMEOUT, Input::TimerElapsed);
        assert!(matches!(
            probe,
            Effect::Request {
                command: STATE_COMMAND,
                ..
            }
        ));
        machine.input(REPLY_TIMEOUT, Input::RequestSent { receive_cutoff: 1 });

        let stopped = machine.input(REPLY_TIMEOUT + REPLY_TIMEOUT, Input::TimerElapsed);
        assert!(matches!(stopped, Effect::Stop));
        assert_eq!(
            machine.view().connection,
            ConnectionState::Failed(SessionError::Timeout(IoStage::Receive))
        );
    }

    #[test]
    fn a3954_state_with_a_contradictory_serial_is_read_only() {
        let now = Duration::ZERO;
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        let mut body = state_body(STATE).to_vec();
        body[16..32].copy_from_slice(b"3949TEST00000000");
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, sent());
        machine.input(
            now,
            inbound(IDENTITY_COMMAND, b"04.2904.29A3954DEMO0000000"),
        );
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &body));

        assert_eq!(
            machine.view().connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3954),
                reason: UnidentifiedReason::IncompatibleState,
            })
        );
    }

    #[test]
    fn a_prefix_valid_unconfirmed_state_layout_is_read_only_for_every_driver() {
        for (model, fixture, identity) in [
            (ModelDriver::A3062, A3062_STATE, b"".as_slice()),
            (ModelDriver::A3949, A3949_STATE, b"".as_slice()),
            (
                ModelDriver::A3954,
                STATE,
                b"04.2904.29A3954DEMO0000000".as_slice(),
            ),
        ] {
            let now = Duration::ZERO;
            let sent = || Input::RequestSent { receive_cutoff: 0 };
            let mut body = state_body(fixture).to_vec();
            body.push(0);
            let mut machine = SessionMachine::new(ReconnectPolicy::External);
            machine.input(now, Input::Opened);
            machine.input(now, sent());
            machine.input(now, inbound(IDENTITY_COMMAND, identity));
            machine.input(now, sent());
            machine.input(now, inbound(model.state_command(), &body));

            assert_eq!(
                machine.view().connection,
                ConnectionState::Unidentified(UnidentifiedState {
                    model: Some(model),
                    reason: UnidentifiedReason::IncompatibleState,
                }),
                "unconfirmed layout accepted for {}",
                model.code()
            );
        }
    }

    #[test]
    fn a_state_buffered_before_its_request_cannot_confirm_the_initial_sync() {
        let now = Duration::ZERO;
        let identity = b"04.2904.29A3954DEMO0000000";
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, Input::RequestSent { receive_cutoff: 1 });

        let stale_identity = machine.input(now, inbound_at(IDENTITY_COMMAND, identity, 1));
        assert!(matches!(stale_identity, Effect::Receive(Some(_))));
        assert_eq!(machine.view().connection, ConnectionState::ProbingIdentity);

        machine.input(now, inbound_at(IDENTITY_COMMAND, identity, 2));
        machine.input(now, Input::RequestSent { receive_cutoff: 3 });

        let stale_state = machine.input(now, inbound_at(STATE_COMMAND, &state_body(STATE), 3));
        assert!(matches!(stale_state, Effect::Receive(Some(_))));
        assert_eq!(
            machine.view().connection,
            ConnectionState::SyncingState(ModelDriver::A3954)
        );

        machine.input(now, inbound_at(STATE_COMMAND, &state_body(STATE)[..50], 4));
        assert_eq!(
            machine.view().connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3954),
                reason: UnidentifiedReason::IncompatibleState,
            })
        );
    }

    #[test]
    fn a_reconnect_with_an_incompatible_state_becomes_read_only_and_keeps_the_snapshot() {
        let now = Duration::ZERO;
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        let identity = b"04.2904.29A3954DEMO0000000";
        let mut machine = SessionMachine::new(ReconnectPolicy::Internal);
        machine.input(now, Input::Opened);
        machine.input(now, sent());
        machine.input(now, inbound(IDENTITY_COMMAND, identity));
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &state_body(STATE)));
        let ConnectionState::Ready(confirmed) = machine.view().connection else {
            panic!("expected a confirmed state");
        };

        let reason = SessionError::Transport {
            stage: IoStage::Receive,
            error: TransportError::Disconnected,
        };
        assert!(matches!(
            machine.input(now, Input::Failed(reason)),
            Effect::Release { .. }
        ));
        machine.input(now, Input::Released);
        machine.input(RETRY_INTERVAL, Input::RetryElapsed);
        machine.input(RETRY_INTERVAL, Input::Opened);
        machine.input(RETRY_INTERVAL, sent());
        machine.input(RETRY_INTERVAL, inbound(IDENTITY_COMMAND, identity));
        machine.input(RETRY_INTERVAL, sent());
        machine.input(
            RETRY_INTERVAL,
            inbound(STATE_COMMAND, &state_body(STATE)[..50]),
        );

        let view = machine.view();
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3954),
                reason: UnidentifiedReason::IncompatibleState,
            })
        );
        assert_eq!(view.last_confirmed, Some(confirmed));
    }

    #[test]
    fn an_active_write_ignores_foreign_and_matching_unusable_state_frames() {
        let now = Duration::ZERO;
        let sent = || Input::RequestSent { receive_cutoff: 0 };
        let state = state_body(STATE);
        let mut machine = SessionMachine::new(ReconnectPolicy::External);
        machine.input(now, Input::Opened);
        machine.input(now, sent());
        machine.input(
            now,
            inbound(IDENTITY_COMMAND, b"04.2904.29A3954DEMO0000000"),
        );
        machine.input(now, sent());
        machine.input(now, inbound(STATE_COMMAND, &state));
        let confirmed = machine.view().connection;

        let id = CommandId(1);
        machine.input(
            now,
            Input::Command(QueuedCommand {
                id,
                command: SessionCommand::SetSoundMode(SoundModeSetting::NORMAL),
                submitted_at: now,
            }),
        );
        machine.input(now, sent());
        let deadline = now + REPLY_TIMEOUT;

        let mut unconfirmed = state.to_vec();
        unconfirmed.push(0);
        // A matching state response with a prefix-valid but unconfirmed layout
        // does not change the confirmed state or the original deadline.
        let unusable = machine.input(
            now + Duration::from_millis(500),
            inbound(STATE_COMMAND, &unconfirmed),
        );
        assert!(matches!(unusable, Effect::Receive(Some(at)) if at == deadline));
        assert_eq!(machine.view().connection, confirmed);

        // A valid response starts the write.
        let write = machine.input(
            now + Duration::from_millis(600),
            inbound(STATE_COMMAND, &state),
        );
        assert!(matches!(
            write,
            Effect::Request {
                command: SOUND_MODE_COMMAND,
                ..
            }
        ));
        machine.input(now + Duration::from_millis(600), sent());
        let write_deadline = now + Duration::from_millis(600) + REPLY_TIMEOUT;

        // A correct frame with an unknown opcode is ignored and the deadline stays.
        let foreign = machine.input(
            now + Duration::from_millis(700),
            inbound([0x7f, 0x74], b"unknown-payload"),
        );
        assert!(matches!(foreign, Effect::Receive(Some(at)) if at == write_deadline));
        assert_eq!(machine.view().connection, confirmed);

        // A valid ACK starts the read-back for the same command.
        let read_back = machine.input(
            now + Duration::from_millis(700),
            inbound(SOUND_MODE_COMMAND, &[]),
        );
        assert!(
            matches!(read_back, Effect::Request { command, body, .. } if command == STATE_COMMAND && body.is_empty())
        );
        machine.input(now + Duration::from_millis(700), sent());
        let read_back_deadline = now + Duration::from_millis(700) + REPLY_TIMEOUT;

        // Foreign frames and a prefix-valid unconfirmed layout keep the confirmed
        // state and the original read-back deadline.
        let foreign = machine.input(
            now + Duration::from_millis(800),
            inbound([0x7f, 0x74], b"unknown-payload"),
        );
        assert!(matches!(foreign, Effect::Receive(Some(at)) if at == read_back_deadline));
        assert_eq!(machine.view().connection, confirmed);

        let ignored = machine.input(
            now + Duration::from_millis(900),
            inbound(STATE_COMMAND, &unconfirmed),
        );
        assert!(matches!(ignored, Effect::Receive(Some(at)) if at == read_back_deadline));
        assert_eq!(machine.view().connection, confirmed);

        // A fresh confirmed read-back completes exactly this command.
        let mut applied = state.to_vec();
        openscl4_models::SoundModeWrite::decode(&[2, 6, 0, 1])
            .expect("measured normal write")
            .apply_to_state(&mut applied)
            .expect("apply the write to the state");
        let completed = machine.input(
            now + Duration::from_millis(1000),
            inbound(STATE_COMMAND, &applied),
        );
        assert!(matches!(
            completed,
            Effect::Complete(completed_id, Ok(CommandOutcome::Applied(state)))
                if completed_id == id && state.sound_mode_setting() == Some(SoundModeSetting::NORMAL)
        ));
        assert!(matches!(
            machine.view().connection,
            ConnectionState::Ready(state) if state.sound_mode_setting() == Some(SoundModeSetting::NORMAL)
        ));
    }

    fn inbound(command: [u8; 2], body: &[u8]) -> Input {
        inbound_at(command, body, 1)
    }

    fn inbound_at(command: [u8; 2], body: &[u8], first_read: u64) -> Input {
        Input::Frame {
            frame: Frame {
                direction: Direction::Inbound,
                command,
                body: bytes::Bytes::copy_from_slice(body),
            },
            first_read,
        }
    }
}
