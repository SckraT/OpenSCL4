use std::{collections::VecDeque, time::Duration};

use openscl4_models::{
    DeviceState, IDENTITY_COMMAND, ModelDriver, SOUND_MODE_COMMAND, SoundModeSetting,
};
use openscl4_protocol::{Direction, Frame};
use openscl4_transport::TransportError;

use crate::{
    CommandError, CommandId, CommandOutcome, CommandResult, IoStage, SessionCommand, SessionError,
};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const SEND_TIMEOUT: Duration = Duration::from_secs(2);
const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);
const QUEUE_TIMEOUT: Duration = Duration::from_secs(5);
pub const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

/// Current stage of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionState {
    /// The transport provider is opening the link.
    Connecting,
    /// The session is waiting for the identification response.
    ProbingIdentity,
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
    ReadBack,
}

#[derive(Clone, Copy)]
struct ModeWrite {
    id: CommandId,
    requested: SoundModeSetting,
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
                self.reply_deadline = Some(now + REPLY_TIMEOUT);
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
            let SessionCommand::SetSoundMode(requested) = command.command;
            self.mode_write = Some(ModeWrite {
                id: command.id,
                requested,
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
        if self.reply_deadline.is_some_and(|deadline| now >= deadline) {
            if let Some(write) = self.mode_write
                && write.stage == WriteStage::Ack
            {
                return self.read_back(now, write);
            }
            return self.fail(SessionError::Timeout(IoStage::Receive));
        }
        self.next_effect(now)
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
                    self.read_back(now, write)
                }
                WriteStage::ReadBack if frame.command == write.model.state_command() => {
                    self.finish_write(write, &frame.body)
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
                    Err(error) => self.fail(error.into()),
                }
            }
            ConnectionState::SyncingState(model) if frame.command == model.state_command() => {
                match model.decode_state(&frame.body) {
                    Ok(state) => {
                        self.transition(ConnectionState::Ready(state));
                        self.next_effect(now)
                    }
                    Err(error) => self.fail(error.into()),
                }
            }
            _ => self.next_effect(now),
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

        match state.sound_mode_write(write.requested) {
            Ok(command) => {
                self.mode_write = Some(ModeWrite {
                    stage: WriteStage::Ack,
                    ..write
                });
                Effect::request(now, SOUND_MODE_COMMAND, command.bytes().to_vec())
            }
            Err(error) => {
                self.mode_write = None;
                Effect::Complete(write.id, Err(error.into()))
            }
        }
    }

    fn read_back(&mut self, now: Duration, write: ModeWrite) -> Effect {
        self.reply_deadline = None;
        self.mode_write = Some(ModeWrite {
            stage: WriteStage::ReadBack,
            ..write
        });
        Effect::read(now, write.model.state_command())
    }

    fn finish_write(&mut self, write: ModeWrite, body: &[u8]) -> Effect {
        let state = match write.model.decode_state(body) {
            Ok(state) => state,
            Err(error) => return self.fail(error.into()),
        };
        let observed = state.sound_mode_setting();
        self.mode_write = None;
        self.transition(ConnectionState::Ready(state));

        let result = if observed == write.requested {
            Ok(CommandOutcome::Applied(state))
        } else {
            Err(CommandError::ModeMismatch {
                requested: write.requested,
                observed,
            })
        };
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
    use super::*;

    const STATE: &[u8] = include_bytes!("../../../data/a3954-pilot-state.bin");

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

        let stopped = machine.input(expected, Input::TimerElapsed);
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
        let mut decoder = openscl4_protocol::FrameDecoder::default();
        decoder.push(STATE);
        let state = decoder
            .next_frame()
            .expect("valid baseline frame")
            .expect("complete baseline frame")
            .body;
        let original = model.decode_state(&state).expect("decode baseline fixture");
        assert_eq!(
            original
                .sound_mode_write(SoundModeSetting::NORMAL)
                .expect("measured normal write")
                .bytes(),
            [2, 6, 0, 1, 255],
        );
        let mut current = state.to_vec();
        openscl4_models::SoundModeWrite::decode(&[0, 11, 1, 0, 255])
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
            matches!(write, Effect::Request { command: SOUND_MODE_COMMAND, body, .. } if body == [2, 6, 1, 0, 255])
        );
        machine.input(now, Input::RequestSent { receive_cutoff: 0 });

        let read_back = machine.input(now, inbound(SOUND_MODE_COMMAND, &[]));
        assert!(
            matches!(read_back, Effect::Request { command, body, .. } if command == model.state_command() && body.is_empty())
        );
        assert_eq!(machine.view().connection, ConnectionState::Ready(refreshed));

        let waiting = machine.input(now, Input::RequestSent { receive_cutoff: 0 });
        assert!(matches!(waiting, Effect::Receive(Some(_))));
        let completed = machine.input(now, inbound(model.state_command(), &current));
        assert!(matches!(
            completed,
            Effect::Complete(completed_id, Err(CommandError::ModeMismatch {requested: expected, observed}))
                if completed_id == id && expected == requested && observed == original.sound_mode_setting()
        ));
    }

    #[test]
    fn a_cancelled_send_marks_only_the_attempted_write_as_unconfirmed() {
        let attempted = CommandId(1);
        let queued = CommandId(2);
        for send_started in [false, true] {
            let mut machine = SessionMachine::new();
            machine.mode_write = Some(ModeWrite {
                id: attempted,
                requested: SoundModeSetting::NORMAL,
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
