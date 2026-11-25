use std::time::Duration;

use openscl4_models::{DeviceState, IDENTITY_COMMAND, ModelDriver};
use openscl4_protocol::{Direction, Frame};

use crate::{IoStage, SessionError};

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const SEND_TIMEOUT: Duration = Duration::from_secs(2);
const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);
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

pub enum Input {
    Opened,
    RequestSent,
    Frame(Frame),
    ReplyTimerElapsed,
    Close,
    Failed(SessionError),
    CleanupFailed(SessionError),
}

pub enum Effect {
    Request([u8; 2]),
    Receive(Option<Duration>),
    Stop,
}

/// The state transition logic has no runtime or transport access.
pub struct SessionMachine {
    view: SessionView,
    reply_deadline: Option<Duration>,
}

impl SessionMachine {
    pub fn new() -> Self {
        Self {
            view: SessionView {
                revision: 0,
                connection: ConnectionState::Connecting,
            },
            reply_deadline: None,
        }
    }

    pub fn view(&self) -> SessionView {
        self.view
    }

    pub fn input(&mut self, now: Duration, input: Input) -> Effect {
        // A frame and its receive timer can become ready in the same runtime poll.
        if matches!(input, Input::Frame(_) | Input::ReplyTimerElapsed)
            && self.reply_deadline.is_some_and(|deadline| now >= deadline)
        {
            return self.transition(
                ConnectionState::Failed(SessionError::Timeout(IoStage::Receive)),
                Effect::Stop,
            );
        }

        let next = match input {
            Input::Opened => Ok((
                ConnectionState::ProbingIdentity,
                Effect::Request(IDENTITY_COMMAND),
            )),
            Input::RequestSent => {
                self.reply_deadline = Some(now + REPLY_TIMEOUT);
                return Effect::Receive(self.reply_deadline);
            }
            Input::ReplyTimerElapsed => return Effect::Receive(self.reply_deadline),
            Input::Close => Ok((ConnectionState::Closed, Effect::Stop)),
            Input::CleanupFailed(_)
                if matches!(self.view.connection, ConnectionState::Failed(_)) =>
            {
                return Effect::Stop;
            }
            Input::Failed(error) | Input::CleanupFailed(error) => Err(error),
            Input::Frame(frame)
                if self.reply_deadline.is_none() || frame.direction != Direction::Inbound =>
            {
                return Effect::Receive(self.reply_deadline);
            }
            Input::Frame(frame) => match self.view.connection {
                ConnectionState::ProbingIdentity if frame.command == IDENTITY_COMMAND => {
                    ModelDriver::identify(&frame.body)
                        .map(|model| {
                            (
                                ConnectionState::SyncingState(model),
                                Effect::Request(model.state_command()),
                            )
                        })
                        .map_err(SessionError::from)
                }
                ConnectionState::SyncingState(model) if frame.command == model.state_command() => {
                    model
                        .decode_state(&frame.body)
                        .map(|state| (ConnectionState::Ready(state), Effect::Receive(None)))
                        .map_err(SessionError::from)
                }
                _ => return Effect::Receive(self.reply_deadline),
            },
        };
        let (connection, effect) =
            next.unwrap_or_else(|error| (ConnectionState::Failed(error), Effect::Stop));
        self.transition(connection, effect)
    }

    fn transition(&mut self, connection: ConnectionState, effect: Effect) -> Effect {
        self.reply_deadline = None;
        self.view.connection = connection;
        self.view.revision += 1;
        effect
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_deadline_from_send_success_across_unrelated_frames() {
        let mut machine = SessionMachine::new();
        let request = machine.input(Duration::ZERO, Input::Opened);
        assert!(matches!(request, Effect::Request(IDENTITY_COMMAND)));

        let sent = Duration::from_secs(1);
        let expected = sent + REPLY_TIMEOUT;
        let receive = machine.input(sent, Input::RequestSent);
        assert!(matches!(receive, Effect::Receive(Some(deadline)) if deadline == expected));

        let frame = Frame {
            direction: Direction::Inbound,
            command: [0, 0],
            body: Default::default(),
        };
        let receive = machine.input(sent + Duration::from_millis(500), Input::Frame(frame));
        assert!(matches!(receive, Effect::Receive(Some(deadline)) if deadline == expected));

        let stopped = machine.input(expected, Input::ReplyTimerElapsed);
        assert!(matches!(stopped, Effect::Stop));
        assert_eq!(
            machine.view().connection,
            ConnectionState::Failed(SessionError::Timeout(IoStage::Receive))
        );
    }
}
