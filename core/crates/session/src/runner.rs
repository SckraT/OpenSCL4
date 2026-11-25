use std::{future::Future, sync::Arc};

use openscl4_protocol::{Direction, FrameDecoder, encode_frame};
use openscl4_transport::{ConnectTarget, LinkRx, TransportError, TransportProvider};
use tokio::{
    sync::{broadcast, watch},
    time::{Instant, timeout, timeout_at},
};

use crate::{
    ConnectionState, IoStage, SessionError, SessionEvent, SessionView,
    machine::{CLOSE_TIMEOUT, CONNECT_TIMEOUT, Effect, Input, SEND_TIMEOUT, SessionMachine},
};

/// A shared handle for observing and closing one session.
pub struct SessionHandle {
    view: watch::Receiver<Arc<SessionView>>,
    events: broadcast::Receiver<SessionEvent>,
    close_requested: watch::Sender<bool>,
}

impl Clone for SessionHandle {
    fn clone(&self) -> Self {
        Self {
            view: self.view.clone(),
            events: self.events.resubscribe(),
            close_requested: self.close_requested.clone(),
        }
    }
}

impl SessionHandle {
    /// Subscribes to the latest session snapshot.
    pub fn subscribe(&self) -> watch::Receiver<Arc<SessionView>> {
        self.view.clone()
    }

    /// Starts receiving new session notifications.
    pub fn events(&self) -> broadcast::Receiver<SessionEvent> {
        self.events.resubscribe()
    }

    /// Waits for the first confirmed device snapshot.
    pub async fn confirmed_view(&self) -> Result<Arc<SessionView>, SessionError> {
        let mut view = self.subscribe();
        let snapshot = view
            .wait_for(|view| {
                matches!(view.connection, ConnectionState::Ready(_))
                    || view.connection.is_terminal()
            })
            .await
            .map_err(|_| SessionError::Closed)?;
        match snapshot.connection {
            ConnectionState::Ready(_) => Ok(Arc::clone(&snapshot)),
            ConnectionState::Failed(error) => Err(error),
            _ => Err(SessionError::Closed),
        }
    }

    /// Closes the session and waits until the link is released.
    pub async fn close(&self) -> Result<(), SessionError> {
        self.close_requested.send_replace(true);
        let mut view = self.subscribe();
        let snapshot = view
            .wait_for(|view| view.connection.is_terminal())
            .await
            .map_err(|_| SessionError::Closed)?;
        match snapshot.connection {
            ConnectionState::Closed => Ok(()),
            ConnectionState::Failed(error) => Err(error),
            _ => Err(SessionError::Closed),
        }
    }
}

/// Executes the state machine's transport operations on a Tokio runtime.
pub struct SessionRunner {
    machine: SessionMachine,
    view: watch::Sender<Arc<SessionView>>,
    events: broadcast::Sender<SessionEvent>,
    close_requested: watch::Receiver<bool>,
}

impl SessionRunner {
    /// Starts one read-only connection in the current Tokio runtime.
    pub fn start(provider: impl TransportProvider, target: ConnectTarget) -> SessionHandle {
        let machine = SessionMachine::new();
        let (view, view_rx) = watch::channel(Arc::new(machine.view()));
        let (events, events_rx) = broadcast::channel(16);
        let (close, close_requested) = watch::channel(false);
        let runner = Self {
            machine,
            view,
            events,
            close_requested,
        };
        tokio::spawn(runner.run(provider, target));

        SessionHandle {
            view: view_rx,
            events: events_rx,
            close_requested: close,
        }
    }

    async fn run(mut self, provider: impl TransportProvider, target: ConnectTarget) {
        let started = Instant::now();
        let opening = async {
            timeout(CONNECT_TIMEOUT, provider.connect(&target))
                .await
                .map_err(|_| SessionError::Timeout(IoStage::Connect))?
                .map_err(|error| transport_error(IoStage::Connect, error))
        };
        let link = match until_close(&mut self.close_requested, opening).await {
            Ok(link) => link,
            Err(error) => {
                self.machine.input(started.elapsed(), terminal_input(error));
                self.publish();
                return;
            }
        };
        let (mut tx, mut rx) = link.split();
        let mut decoder = FrameDecoder::default();
        let mut effect = self.machine.input(started.elapsed(), Input::Opened);

        loop {
            let input = match effect {
                Effect::Request(command) => {
                    self.publish();
                    let sending = async {
                        let request = encode_frame(Direction::Outbound, command, &[])?;
                        timeout(SEND_TIMEOUT, tx.send(request))
                            .await
                            .map_err(|_| SessionError::Timeout(IoStage::Send))?
                            .map_err(|error| transport_error(IoStage::Send, error))
                    };
                    until_close(&mut self.close_requested, sending)
                        .await
                        .map_or_else(terminal_input, |()| Input::RequestSent)
                }
                Effect::Receive(deadline) => {
                    self.publish();
                    let receiving = receive_input(
                        rx.as_mut(),
                        &mut decoder,
                        &self.events,
                        deadline.map(|deadline| started + deadline),
                    );
                    until_close(&mut self.close_requested, receiving)
                        .await
                        .unwrap_or_else(terminal_input)
                }
                Effect::Stop => break,
            };
            effect = self.machine.input(started.elapsed(), input);
        }

        let close_timed_out = timeout(CLOSE_TIMEOUT, tx.close()).await.is_err();
        drop((tx, rx));
        if close_timed_out {
            let error = SessionError::Timeout(IoStage::Close);
            self.machine
                .input(started.elapsed(), Input::CleanupFailed(error));
            let _ = self.events.send(SessionEvent::CleanupFailed(error));
        }
        self.publish();
    }

    fn publish(&self) {
        let next = self.machine.view();
        if self.view.borrow().revision == next.revision {
            return;
        }
        let view = Arc::new(next);
        self.view.send_replace(Arc::clone(&view));
        let _ = self.events.send(SessionEvent::ViewChanged(view));
    }
}

async fn until_close<T>(
    close: &mut watch::Receiver<bool>,
    operation: impl Future<Output = Result<T, SessionError>>,
) -> Result<T, SessionError> {
    tokio::select! {
        biased;
        _ = close.wait_for(|requested| *requested) => Err(SessionError::Closed),
        result = operation => result,
    }
}

async fn receive_input(
    rx: &mut dyn LinkRx,
    decoder: &mut FrameDecoder,
    events: &broadcast::Sender<SessionEvent>,
    deadline: Option<Instant>,
) -> Result<Input, SessionError> {
    let receive = async {
        loop {
            match decoder.next_frame() {
                Ok(Some(frame)) => return Ok(Input::Frame(frame)),
                Ok(None) => {
                    let chunk = rx
                        .recv()
                        .await
                        .map_err(|error| transport_error(IoStage::Receive, error))?;
                    decoder.push(&chunk);
                }
                Err(error) => {
                    let _ = events.send(SessionEvent::FrameRejected(error));
                }
            }
        }
    };
    if let Some(deadline) = deadline {
        match timeout_at(deadline, receive).await {
            Ok(result) => result,
            Err(_) => Ok(Input::ReplyTimerElapsed),
        }
    } else {
        receive.await
    }
}

fn transport_error(stage: IoStage, error: TransportError) -> SessionError {
    SessionError::Transport { stage, error }
}

fn terminal_input(error: SessionError) -> Input {
    if error == SessionError::Closed {
        Input::Close
    } else {
        Input::Failed(error)
    }
}
