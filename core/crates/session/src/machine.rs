use std::time::Duration;

use openscl4_models::{DeviceState, IDENTITY_COMMAND, ModelDriver};
use openscl4_protocol::{Direction, Frame};

use crate::SessionError;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const SEND_TIMEOUT: Duration = Duration::from_secs(2);
pub const REPLY_TIMEOUT: Duration = Duration::from_millis(1500);
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

pub enum Input<'a> {
    Opened,
    Frame(&'a Frame),
    Close,
    Failed(SessionError),
}

pub enum Effect {
    Request([u8; 2]),
    Wait,
    Ignore,
    Stop,
}

/// The state transition logic has no runtime or transport access.
pub struct SessionMachine {
    view: SessionView,
}

impl SessionMachine {
    pub fn new() -> Self {
        Self {
            view: SessionView {
                revision: 0,
                connection: ConnectionState::Connecting,
            },
        }
    }

    pub fn view(&self) -> SessionView {
        self.view
    }

    pub fn input(&mut self, input: Input<'_>) -> Effect {
        let next = match input {
            Input::Opened => Ok((
                ConnectionState::ProbingIdentity,
                Effect::Request(IDENTITY_COMMAND),
            )),
            Input::Close => Ok((ConnectionState::Closed, Effect::Stop)),
            Input::Failed(error) => Err(error),
            Input::Frame(frame) if frame.direction != Direction::Inbound => {
                return Effect::Ignore;
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
                        .map(|state| (ConnectionState::Ready(state), Effect::Wait))
                        .map_err(SessionError::from)
                }
                _ => return Effect::Ignore,
            },
        };
        let (connection, effect) =
            next.unwrap_or_else(|error| (ConnectionState::Failed(error), Effect::Stop));
        self.view.connection = connection;
        self.view.revision += 1;
        effect
    }
}
