use std::{collections::VecDeque, time::Duration};

use openscl4_models::{
    DeviceState, IDENTITY_COMMAND, ModelDriver, SOUND_MODE_COMMAND, SoundModeSetting,
};
use openscl4_protocol::{Direction, Frame};

use crate::{
    CommandError, CommandId, CommandOutcome, CommandResult, IoStage, SessionCommand, SessionError,
};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const SEND_TIMEOUT: Duration = Duration::from_secs(2);
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
    RequestSent,
    Frame(Frame),
    TimerElapsed,
    Command(QueuedCommand),
    Close,
    Failed(SessionError),
    CleanupFailed(SessionError),
}

pub enum Effect {
    Request([u8; 2], Vec<u8>),
    Receive(Option<Duration>),
    Complete(CommandId, CommandResult),
    Stop,
}

impl Effect {
    fn read(command: [u8; 2]) -> Self {
        Self::Request(command, Vec::new())
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteStage {
    Ack,
    ReadBack,
}

#[derive(Clone, Copy)]
struct ModeWrite {
    id: CommandId,
    requested: SoundModeSetting,
    model: ModelDriver,
    stage: WriteStage,
}

/// The state transition logic has no runtime or transport access.
pub struct SessionMachine {
    view: SessionView,
    reply_deadline: Option<Duration>,
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
                Effect::read(IDENTITY_COMMAND)
            }
            Input::RequestSent => {
                self.reply_deadline = Some(now + REPLY_TIMEOUT);
                self.next_effect(now)
            }
            Input::TimerElapsed => self.on_timer(now),
            Input::Command(command) => {
                self.queue.push_back(command);
                self.next_effect(now)
            }
            Input::Frame(frame) => self.on_frame(now, frame),
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
            return match device.sound_mode_write(requested) {
                Ok(write) => {
                    self.mode_write = Some(ModeWrite {
                        id: command.id,
                        requested,
                        model: device.model,
                        stage: WriteStage::Ack,
                    });
                    Effect::Request(SOUND_MODE_COMMAND, write.bytes().to_vec())
                }
                Err(error) => Effect::Complete(command.id, Err(error.into())),
            };
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
                return self.read_back(write);
            }
            return self.fail(SessionError::Timeout(IoStage::Receive));
        }
        self.next_effect(now)
    }

    fn on_frame(&mut self, now: Duration, frame: Frame) -> Effect {
        // A frame and its receive timer can become ready in the same runtime poll.
        if self.reply_deadline.is_some_and(|deadline| now >= deadline) {
            return self.on_timer(now);
        }
        if self.reply_deadline.is_none() || frame.direction != Direction::Inbound {
            return self.next_effect(now);
        }

        if let Some(write) = self.mode_write {
            return match write.stage {
                WriteStage::Ack if frame.command == SOUND_MODE_COMMAND && frame.body.is_empty() => {
                    self.read_back(write)
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
                        Effect::read(model.state_command())
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

    fn read_back(&mut self, write: ModeWrite) -> Effect {
        self.reply_deadline = None;
        self.mode_write = Some(ModeWrite {
            stage: WriteStage::ReadBack,
            ..write
        });
        Effect::read(write.model.state_command())
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
        assert!(matches!(request, Effect::Request(IDENTITY_COMMAND, body) if body.is_empty()));

        let sent = Duration::from_secs(1);
        let expected = sent + REPLY_TIMEOUT;
        let receive = machine.input(sent, Input::RequestSent);
        assert!(matches!(receive, Effect::Receive(Some(deadline)) if deadline == expected));

        let receive = machine.input(
            sent + Duration::from_millis(500),
            Input::Frame(inbound([0, 0], &[])),
        );
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
        let state = &STATE[9..STATE.len() - 1];
        let original = model.decode_state(state).expect("decode baseline fixture");
        let mut machine = SessionMachine::new();
        machine.input(now, Input::Opened);
        machine.input(now, Input::RequestSent);
        machine.input(
            now,
            Input::Frame(inbound(IDENTITY_COMMAND, b"04.2904.29A3954DEMO0000000")),
        );
        machine.input(now, Input::RequestSent);
        machine.input(now, Input::Frame(inbound(model.state_command(), state)));

        let id = CommandId(1);
        let requested = SoundModeSetting::NORMAL;
        let write = machine.input(
            now,
            Input::Command(QueuedCommand {
                id,
                command: SessionCommand::SetSoundMode(requested),
                submitted_at: now,
            }),
        );
        assert!(
            matches!(write, Effect::Request(SOUND_MODE_COMMAND, body) if body == [2, 6, 0, 1, 255])
        );
        machine.input(now, Input::RequestSent);

        let read_back = machine.input(now, Input::Frame(inbound(SOUND_MODE_COMMAND, &[])));
        assert!(
            matches!(read_back, Effect::Request(command, body) if command == model.state_command() && body.is_empty())
        );
        assert_eq!(machine.view().connection, ConnectionState::Ready(original));

        let waiting = machine.input(now, Input::RequestSent);
        assert!(matches!(waiting, Effect::Receive(Some(_))));
        let completed = machine.input(now, Input::Frame(inbound(model.state_command(), state)));
        assert!(matches!(
            completed,
            Effect::Complete(completed_id, Err(CommandError::ModeMismatch {requested: expected, observed}))
                if completed_id == id && expected == requested && observed == original.sound_mode_setting()
        ));
    }

    fn inbound(command: [u8; 2], body: &[u8]) -> Frame {
        Frame {
            direction: Direction::Inbound,
            command,
            body: bytes::Bytes::copy_from_slice(body),
        }
    }
}
