use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use openscl4_protocol::{Direction, Frame, FrameDecoder, encode_frame};
use openscl4_transport::{ConnectTarget, LinkRx, TransportError, TransportProvider};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, broadcast, mpsc, oneshot, watch},
    time::{Instant, timeout, timeout_at},
};

use crate::{
    CommandId, CommandResult, ConnectionState, DispatchError, IoStage, SessionCommand,
    SessionError, SessionEvent, SessionView,
    machine::{CLOSE_TIMEOUT, CONNECT_TIMEOUT, Effect, Input, QueuedCommand, SessionMachine},
};

const COMMAND_CAPACITY: usize = 8;

struct PendingReply {
    result: oneshot::Sender<CommandResult>,
    _slot: OwnedSemaphorePermit,
}

struct Dispatch {
    command: QueuedCommand,
    reply: PendingReply,
}

/// A shared handle for observing, controlling and closing one session.
pub struct SessionHandle {
    view: watch::Receiver<Arc<SessionView>>,
    events: broadcast::Receiver<SessionEvent>,
    close_requested: watch::Sender<bool>,
    commands: mpsc::Sender<Dispatch>,
    slots: Arc<Semaphore>,
    next_id: Arc<AtomicU64>,
    started: Instant,
}

impl Clone for SessionHandle {
    fn clone(&self) -> Self {
        Self {
            view: self.view.clone(),
            events: self.events.resubscribe(),
            close_requested: self.close_requested.clone(),
            commands: self.commands.clone(),
            slots: Arc::clone(&self.slots),
            next_id: Arc::clone(&self.next_id),
            started: self.started,
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

    /// Enqueues a discrete command and returns its independent completion handle.
    pub fn dispatch(&self, command: SessionCommand) -> Result<PendingCommand, DispatchError> {
        if *self.close_requested.borrow() {
            return Err(DispatchError::Closed);
        }
        match self.view.borrow().connection {
            ConnectionState::Ready(_) => {}
            state if state.is_terminal() => return Err(DispatchError::Closed),
            _ => return Err(DispatchError::NotReady),
        }

        let slot = Arc::clone(&self.slots)
            .try_acquire_owned()
            .map_err(|_| DispatchError::QueueFull)?;
        let id = CommandId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let (result, outcome) = oneshot::channel();
        self.commands
            .try_send(Dispatch {
                command: QueuedCommand {
                    id,
                    command,
                    submitted_at: self.started.elapsed(),
                },
                reply: PendingReply {
                    result,
                    _slot: slot,
                },
            })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => DispatchError::QueueFull,
                mpsc::error::TrySendError::Closed(_) => DispatchError::Closed,
            })?;

        Ok(PendingCommand {
            id,
            outcome,
            _session: self.close_requested.clone(),
            _commands: self.commands.clone(),
        })
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

/// An accepted command and its completion result.
pub struct PendingCommand {
    id: CommandId,
    outcome: oneshot::Receiver<CommandResult>,
    _session: watch::Sender<bool>,
    _commands: mpsc::Sender<Dispatch>,
}

impl PendingCommand {
    /// Identifies this command in session events.
    pub const fn id(&self) -> CommandId {
        self.id
    }

    /// Waits independently of any session event subscriber.
    pub async fn wait(self) -> CommandResult {
        self.outcome
            .await
            .unwrap_or_else(|_| Err(SessionError::Closed.into()))
    }
}

/// Executes the state machine's transport operations on a Tokio runtime.
pub struct SessionRunner {
    machine: SessionMachine,
    view: watch::Sender<Arc<SessionView>>,
    events: broadcast::Sender<SessionEvent>,
    close_requested: watch::Receiver<bool>,
    commands: mpsc::Receiver<Dispatch>,
    replies: HashMap<CommandId, PendingReply>,
}

impl SessionRunner {
    /// Starts one connection in the current Tokio runtime.
    pub fn start(provider: impl TransportProvider, target: ConnectTarget) -> SessionHandle {
        let machine = SessionMachine::new();
        let (view, view_rx) = watch::channel(Arc::new(machine.view()));
        let (events, events_rx) = broadcast::channel(16);
        let (close, close_requested) = watch::channel(false);
        let (commands, incoming) = mpsc::channel(COMMAND_CAPACITY);
        let started = Instant::now();
        let runner = Self {
            machine,
            view,
            events,
            close_requested,
            commands: incoming,
            replies: HashMap::new(),
        };
        tokio::spawn(runner.run(provider, target, started));

        SessionHandle {
            view: view_rx,
            events: events_rx,
            close_requested: close,
            commands,
            slots: Arc::new(Semaphore::new(COMMAND_CAPACITY)),
            next_id: Arc::new(AtomicU64::new(1)),
            started,
        }
    }

    async fn run(
        mut self,
        provider: impl TransportProvider,
        target: ConnectTarget,
        started: Instant,
    ) {
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
        let (mut tx, rx) = link.split();
        let (frames, mut received) = mpsc::channel(16);
        let receive_order = Arc::new(AtomicU64::new(0));
        let reader = tokio::spawn(read_link(
            rx,
            frames,
            Arc::clone(&receive_order),
            self.events.clone(),
        ));
        let mut effect = self.machine.input(started.elapsed(), Input::Opened);

        loop {
            let input = match effect {
                Effect::Request {
                    command,
                    body,
                    deadline,
                } => {
                    self.publish();
                    let mut send_started = false;
                    let sending = async {
                        let request = encode_frame(Direction::Outbound, command, &body)?;
                        let receive_cutoff = receive_order.load(Ordering::SeqCst);
                        timeout_at(started + deadline, async {
                            send_started = true;
                            tx.send(request).await
                        })
                        .await
                        .map_err(|_| SessionError::Timeout(IoStage::Send))?
                        .map_err(|error| transport_error(IoStage::Send, error))?;
                        Ok(receive_cutoff)
                    };
                    match until_close(&mut self.close_requested, sending).await {
                        Ok(receive_cutoff) => Input::RequestSent { receive_cutoff },
                        Err(error) => Input::RequestFailed {
                            error,
                            send_started,
                        },
                    }
                }
                Effect::Receive(deadline) => {
                    self.publish();
                    let receiving =
                        receive_input(&mut received, deadline.map(|deadline| started + deadline));
                    tokio::select! {
                        biased;
                        _ = self.close_requested.wait_for(|requested| *requested) => Input::Close,
                        dispatch = self.commands.recv() => {
                            if let Some(dispatch) = dispatch {
                                self.replies.insert(dispatch.command.id, dispatch.reply);
                                Input::Command(dispatch.command)
                            } else {
                                Input::Close
                            }
                        }
                        result = receiving => result.unwrap_or_else(terminal_input),
                    }
                }
                Effect::Complete(id, result) => {
                    self.publish();
                    if let Some(reply) = self.replies.remove(&id) {
                        self.complete(id, reply, result);
                    }
                    effect = self.machine.next_effect(started.elapsed());
                    continue;
                }
                Effect::Stop => break,
            };
            effect = self.machine.input(started.elapsed(), input);
        }

        drop(received);
        let close_timed_out = timeout(CLOSE_TIMEOUT, tx.close()).await.is_err();
        drop(tx);
        reader.abort();
        let _ = reader.await;
        if close_timed_out {
            let error = SessionError::Timeout(IoStage::Close);
            self.machine
                .input(started.elapsed(), Input::CleanupFailed(error));
            let _ = self.events.send(SessionEvent::CleanupFailed(error));
        }
        self.publish();
        self.commands.close();
        while let Ok(dispatch) = self.commands.try_recv() {
            self.replies.insert(dispatch.command.id, dispatch.reply);
        }
        for (id, reply) in std::mem::take(&mut self.replies) {
            self.complete(id, reply, Err(self.machine.terminal_command_error(id)));
        }
    }

    fn complete(&self, id: CommandId, reply: PendingReply, result: CommandResult) {
        let PendingReply {
            result: sender,
            _slot: slot,
        } = reply;
        drop(slot);
        let _ = sender.send(result);
        let _ = self
            .events
            .send(SessionEvent::CommandCompleted { id, result });
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

struct ReceivedFrame {
    frame: Frame,
    first_read: u64,
}

async fn read_link(
    mut rx: Box<dyn LinkRx>,
    frames: mpsc::Sender<Result<ReceivedFrame, TransportError>>,
    receive_order: Arc<AtomicU64>,
    events: broadcast::Sender<SessionEvent>,
) {
    let mut decoder = FrameDecoder::default();
    let mut partial_start = None;
    loop {
        let chunk = match rx.recv().await {
            Ok(chunk) => chunk,
            Err(error) => {
                let _ = frames.send(Err(error)).await;
                return;
            }
        };
        let current_read = receive_order.fetch_add(1, Ordering::SeqCst) + 1;
        let mut first_read = partial_start.take().unwrap_or(current_read);
        decoder.push(&chunk);
        loop {
            match decoder.next_frame() {
                Ok(Some(frame)) => {
                    if frames
                        .send(Ok(ReceivedFrame { frame, first_read }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
                Ok(None) => {
                    // A frame that spans reads keeps the order of its first bytes.
                    partial_start = decoder.has_pending_bytes().then_some(first_read);
                    break;
                }
                Err(error) => {
                    let _ = events.send(SessionEvent::FrameRejected(error));
                }
            }
            first_read = current_read;
        }
    }
}

async fn receive_input(
    frames: &mut mpsc::Receiver<Result<ReceivedFrame, TransportError>>,
    deadline: Option<Instant>,
) -> Result<Input, SessionError> {
    let receive = async {
        let ReceivedFrame { frame, first_read } = frames
            .recv()
            .await
            .unwrap_or(Err(TransportError::Closed))
            .map_err(|error| transport_error(IoStage::Receive, error))?;
        Ok(Input::Frame { frame, first_read })
    };
    if let Some(deadline) = deadline {
        match timeout_at(deadline, receive).await {
            Ok(result) => result,
            Err(_) => Ok(Input::TimerElapsed),
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
