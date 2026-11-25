use std::{collections::VecDeque, time::Duration};

use openscl4_models::{
    DeviceState, IDENTITY_COMMAND, ModelDriver, ModelError, SOUND_MODE_COMMAND, STATE_COMMAND,
    registry,
};
use openscl4_protocol::{Direction, Frame};
use openscl4_transport::TransportError;

use crate::{
    CommandError, CommandId, CommandOutcome, CommandResult, IoStage, SessionCommand, SessionError,
};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const SEND_TIMEOUT: Duration = Duration::from_secs(2);
const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);
const READ_BACK_INTERVAL: Duration = Duration::from_millis(100);
const QUEUE_TIMEOUT: Duration = Duration::from_secs(5);
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

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

/// An immutable session snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SessionView {
    /// Increases when a new state is published.
    pub revision: u64,
    /// The current connection or confirmed device state.
    pub connection: ConnectionState,
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
    Close,
    Failed(SessionError),
    CleanupFailed(SessionError),
}

pub enum Effect {
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

#[derive(Clone, Copy)]
struct ModeWrite {
    id: CommandId,
    command: SessionCommand,
    model: ModelDriver,
    stage: WriteStage,
    may_have_written: bool,
}

/// The state transition logic has no runtime or transport access.
pub struct SessionMachine {
    view: SessionView,
    reply_deadline: Option<Duration>,
    receive_cutoff: u64,
    mode_write: Option<ModeWrite>,
    queue: VecDeque<QueuedCommand>,
}

impl SessionMachine {
    pub fn new() -> Self {
        Self {
            view: SessionView {
                revision: 0,
                connection: ConnectionState::Connecting,
            },
            reply_deadline: None,
            receive_cutoff: 0,
            mode_write: None,
            queue: VecDeque::new(),
        }
    }

    pub fn view(&self) -> SessionView {
        self.view
    }

    pub fn input(&mut self, now: Duration, input: Input) -> Effect {
        match input {
            Input::Opened => {
                self.transition(ConnectionState::ProbingIdentity);
                Effect::read(now, IDENTITY_COMMAND)
            }
            Input::RequestSent { receive_cutoff } => {
                self.mark_write_attempt();
                self.receive_cutoff = receive_cutoff;
                let timeout = now + REPLY_TIMEOUT;
                let deadline = match &mut self.mode_write {
                    Some(ModeWrite {
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
                    self.fail(error)
                }
            }
            Input::TimerElapsed => self.on_timer(now),
            Input::Command(command) => {
                self.queue.push_back(command);
                self.next_effect(now)
            }
            Input::Frame { frame, first_read } => self.on_frame(now, frame, first_read),
            Input::Close => {
                self.transition(ConnectionState::Closed);
                Effect::Stop
            }
            Input::CleanupFailed(_)
                if matches!(self.view.connection, ConnectionState::Failed(_)) =>
            {
                Effect::Stop
            }
            Input::Failed(error) | Input::CleanupFailed(error) => self.fail(error),
        }
    }

    pub fn next_effect(&mut self, now: Duration) -> Effect {
        if let Some(command) = self.queue.front()
            && now >= command.expires_at()
        {
            let id = command.id;
            self.queue.pop_front();
            return Effect::Complete(id, Err(CommandError::QueueTimeout));
        }

        if self.mode_write.is_none()
            && let ConnectionState::Ready(device) = self.view.connection
            && let Some(command) = self.queue.pop_front()
        {
            if !command.command.supported_by(device.model) {
                return Effect::Complete(command.id, Err(ModelError::InvalidSoundModeWrite.into()));
            }
            self.mode_write = Some(ModeWrite {
                id: command.id,
                command: command.command,
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
            .min();
        Effect::Receive(next_deadline)
    }

    fn on_timer(&mut self, now: Duration) -> Effect {
        if self.reply_deadline.is_none_or(|deadline| now < deadline) {
            return self.next_effect(now);
        }

        if let Some(write) = self.mode_write {
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
                    return self.finish_write(write, observed);
                }
                _ => {}
            }
        }
        match self.view.connection {
            ConnectionState::ProbingIdentity => {
                self.probe_state(now, SessionError::Timeout(IoStage::Receive))
            }
            ConnectionState::ProbingState(identity_error) => self.fail(identity_error),
            _ => self.fail(SessionError::Timeout(IoStage::Receive)),
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

        if let Some(write) = self.mode_write {
            if first_read <= self.receive_cutoff {
                return self.next_effect(now);
            }
            return match write.stage {
                WriteStage::ReadCurrent if frame.command == write.model.state_command() => {
                    self.send_write(now, write, &frame.body)
                }
                WriteStage::Ack if frame.command == SOUND_MODE_COMMAND && frame.body.is_empty() => {
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
            ConnectionState::ProbingIdentity if frame.command == IDENTITY_COMMAND => {
                match ModelDriver::identify(&frame.body) {
                    Ok(model) => {
                        self.transition(ConnectionState::SyncingState(model));
                        Effect::read(now, model.state_command())
                    }
                    Err(error) => self.probe_state(now, error.into()),
                }
            }
            ConnectionState::ProbingState(identity_error)
                if frame.command == STATE_COMMAND && first_read > self.receive_cutoff =>
            {
                match registry::identify_state(&frame.body) {
                    Ok(model) => self.confirm_state(now, model, &frame.body),
                    Err(_) => self.fail(identity_error),
                }
            }
            ConnectionState::SyncingState(model) if frame.command == model.state_command() => {
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
        match model.decode_state(body) {
            Ok(state) => {
                self.transition(ConnectionState::Ready(state));
                self.next_effect(now)
            }
            Err(error) => self.fail(error.into()),
        }
    }

    fn send_write(&mut self, now: Duration, write: ModeWrite, body: &[u8]) -> Effect {
        let state = match write.model.decode_state(body) {
            Ok(state) => state,
            Err(error) => return self.fail(error.into()),
        };
        if self.view.connection == ConnectionState::Ready(state) {
            self.reply_deadline = None;
        } else {
            self.transition(ConnectionState::Ready(state));
        }

        match write.command.write_body(state) {
            Ok(body) => {
                self.mode_write = Some(ModeWrite {
                    stage: WriteStage::Ack,
                    ..write
                });
                Effect::request(now, SOUND_MODE_COMMAND, body)
            }
            Err(error) => {
                self.mode_write = None;
                Effect::Complete(write.id, Err(error.into()))
            }
        }
    }

    fn read_back(
        &mut self,
        now: Duration,
        write: ModeWrite,
        deadline: Option<Duration>,
        observed: Option<DeviceState>,
    ) -> Effect {
        self.reply_deadline = None;
        self.mode_write = Some(ModeWrite {
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
        write: ModeWrite,
        body: &[u8],
    ) -> Effect {
        let state = match write.model.decode_state(body) {
            Ok(state) => state,
            Err(error) => return self.fail(error.into()),
        };
        if write.command.verify_mode(state).is_ok() {
            return self.finish_write(write, state);
        }

        // A device can acknowledge the write before the new value appears in a state response.
        self.mode_write = Some(ModeWrite {
            stage: WriteStage::WaitForReadBack {
                deadline,
                observed: state,
            },
            ..write
        });
        self.reply_deadline = Some((now + READ_BACK_INTERVAL).min(deadline));
        self.next_effect(now)
    }

    fn finish_write(&mut self, write: ModeWrite, state: DeviceState) -> Effect {
        self.mode_write = None;
        self.transition(ConnectionState::Ready(state));

        let result = write
            .command
            .verify_mode(state)
            .map(|()| CommandOutcome::Applied(state));
        Effect::Complete(write.id, result)
    }

    fn mark_write_attempt(&mut self) {
        if let Some(write) = &mut self.mode_write
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
        if matches!(
            failure,
            SessionError::Transport {
                error: TransportError::Disconnected,
                ..
            }
        ) {
            CommandError::Disconnected
        } else if self
            .mode_write
            .is_some_and(|write| write.id == id && write.may_have_written)
        {
            CommandError::Unconfirmed(failure)
        } else {
            failure.into()
        }
    }

    fn fail(&mut self, error: SessionError) -> Effect {
        self.transition(ConnectionState::Failed(error));
        Effect::Stop
    }

    fn transition(&mut self, connection: ConnectionState) {
        self.reply_deadline = None;
        self.view.connection = connection;
        self.view.revision += 1;
    }
}

#[cfg(test)]
mod tests {
    use openscl4_models::{SoundMode, SoundModeSetting};
    use openscl4_protocol::encode_frame;

    use super::*;

    const STATE: &[u8] = include_bytes!("../../../data/a3954-pilot-state.bin");
    const A3062_STATE: &[u8] = include_bytes!("../../../data/a3062-pilot-state.bin");
    const A3062_NORMAL: &[u8] = include_bytes!("../../../data/a3062-mode-normal.bin");
    const A3062_TRANSPARENCY: &[u8] = include_bytes!("../../../data/a3062-mode-transparency.bin");
    // The a3062-anc/transparency frame from conformance.json.
    const A3062_TRANSPARENCY_WRITE: &[u8] =
        b"\x08\xee\0\0\0\x06\x81\x10\0\x01\x51\x01\x01\0\x05\xe6";

    #[test]
    fn keeps_the_deadline_from_send_success_across_unrelated_frames() {
        let mut machine = SessionMachine::new();
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
        let mut machine = SessionMachine::new();
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
        let mut machine = SessionMachine::new();
        machine.mode_write = Some(ModeWrite {
            id,
            command: SessionCommand::SetSoundMode(requested),
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
        let mut machine = SessionMachine::new();
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
                if completed_id == id && state.sound_mode == SoundMode::Transparency
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
        let mut machine = SessionMachine::new();
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
    fn a_cancelled_send_marks_only_the_attempted_write_as_unconfirmed() {
        let attempted = CommandId(1);
        let queued = CommandId(2);
        for send_started in [false, true] {
            let mut machine = SessionMachine::new();
            machine.mode_write = Some(ModeWrite {
                id: attempted,
                command: SessionCommand::SetSoundMode(SoundModeSetting::NORMAL),
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

    fn inbound(command: [u8; 2], body: &[u8]) -> Input {
        Input::Frame {
            frame: Frame {
                direction: Direction::Inbound,
                command,
                body: bytes::Bytes::copy_from_slice(body),
            },
            first_read: 1,
        }
    }
}
