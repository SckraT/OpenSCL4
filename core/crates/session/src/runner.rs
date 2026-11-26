use std::{
    collections::HashMap,
    future::Future,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use openscl4_protocol::{Direction, Frame, FrameDecoder, encode_frame};
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, TransportError, TransportLink, TransportProvider,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, broadcast, mpsc, oneshot, watch},
    task::JoinHandle,
    time::{Instant, sleep_until, timeout_at},
};

use crate::{
    CommandError, CommandId, CommandResult, ConnectionState, DispatchError, IoStage,
    ReconnectPolicy, SessionCommand, SessionError, SessionEvent, SessionView,
    machine::{CLOSE_TIMEOUT, Effect, Input, QueuedCommand, SessionMachine},
};

const COMMAND_CAPACITY: usize = 8;

struct PendingReply {
    result: oneshot::Sender<CommandResult>,
    _slot: OwnedSemaphorePermit,
}

struct Dispatch {
    generation: u64,
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
        let generation = {
            let view = self.view.borrow();
            match view.connection {
                ConnectionState::Ready(_) => view.generation,
                state if state.is_terminal() => return Err(DispatchError::Closed),
                _ => return Err(DispatchError::NotReady),
            }
        };

        let slot = Arc::clone(&self.slots)
            .try_acquire_owned()
            .map_err(|_| DispatchError::QueueFull)?;
        let id = CommandId(self.next_id.fetch_add(1, Ordering::Relaxed));
        let (result, outcome) = oneshot::channel();
        self.commands
            .try_send(Dispatch {
                generation,
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
        Self::start_with_policy(provider, target, ReconnectPolicy::External)
    }

    /// Starts a session with an explicit policy for recovering a lost connection.
    pub fn start_with_policy(
        provider: impl TransportProvider,
        target: ConnectTarget,
        reconnect_policy: ReconnectPolicy,
    ) -> SessionHandle {
        let machine = SessionMachine::new(reconnect_policy);
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
        let mut active = None;
        let mut last_disconnected = 0;
        let mut effect = self.machine.open(started.elapsed());

        loop {
            let generation = self.machine.view().generation;
            let input = match effect {
                Effect::Open { deadline } => {
                    self.publish();
                    let opening = async {
                        timeout_at(started + deadline, provider.connect(&target))
                            .await
                            .map_err(|_| SessionError::Timeout(IoStage::Connect))?
                            .map_err(|error| transport_error(IoStage::Connect, error))
                    };
                    match until_close(&mut self.close_requested, opening).await {
                        Ok(link) => {
                            active = Some(ActiveLink::new(link, self.events.clone()));
                            Input::Opened
                        }
                        Err(error) => terminal_input(error),
                    }
                }
                Effect::Release { deadline } => {
                    last_disconnected = generation;
                    self.publish();
                    self.finish_disconnected_commands();
                    match active.take() {
                        Some(link) => self.release(link, started + deadline).await,
                        None => Input::Released,
                    }
                }
                Effect::RetryAt(deadline) => {
                    self.publish();
                    tokio::select! {
                        biased;
                        _ = self.close_requested.wait_for(|requested| *requested) => Input::Close,
                        () = sleep_until(started + deadline) => Input::RetryElapsed,
                    }
                }
                Effect::Request {
                    command,
                    body,
                    deadline,
                } => {
                    self.publish();
                    let link = active.as_mut().expect("requests use the active link");
                    let mut send_started = false;
                    let sending = async {
                        let request = encode_frame(Direction::Outbound, command, &body)?;
                        let receive_cutoff = link.receive_order.load(Ordering::SeqCst);
                        timeout_at(started + deadline, async {
                            send_started = true;
                            link.tx.send(request).await
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
                    let link = active.as_mut().expect("receives use the active link");
                    let receiving = receive_input(
                        &mut link.received,
                        deadline.map(|deadline| started + deadline),
                    );
                    tokio::select! {
                        biased;
                        _ = self.close_requested.wait_for(|requested| *requested) => Input::Close,
                        dispatch = self.commands.recv() => {
                            if let Some(dispatch) = dispatch {
                                if dispatch.generation != generation {
                                    Self::complete(&self.events, dispatch.command.id, dispatch.reply, Err(CommandError::Disconnected));
                                    effect = Effect::Receive(deadline);
                                    continue;
                                }
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
                        Self::complete(&self.events, id, reply, result);
                    }
                    effect = self.machine.next_effect(started.elapsed());
                    continue;
                }
                Effect::Stop => break,
            };
            effect = self
                .machine
                .input_from(generation, started.elapsed(), input)
                .expect("inputs in this loop belong to the current attempt");
        }

        if let Some(link) = active {
            let result = self.release(link, Instant::now() + CLOSE_TIMEOUT).await;
            if let Input::CleanupFailed(_) = result {
                self.machine.input(started.elapsed(), result);
            }
        }
        self.publish();
        self.commands.close();
        let generation = self.machine.view().generation;
        while let Ok(dispatch) = self.commands.try_recv() {
            Self::complete(
                &self.events,
                dispatch.command.id,
                dispatch.reply,
                Err(
                    if dispatch.generation == generation && generation > last_disconnected {
                        self.machine.terminal_command_error(dispatch.command.id)
                    } else {
                        CommandError::Disconnected
                    },
                ),
            );
        }
        for (id, reply) in std::mem::take(&mut self.replies) {
            Self::complete(
                &self.events,
                id,
                reply,
                Err(self.machine.terminal_command_error(id)),
            );
        }
    }

    async fn release(&self, link: ActiveLink, deadline: Instant) -> Input {
        if link.close(deadline).await.is_err() {
            let error = SessionError::Timeout(IoStage::Close);
            let _ = self.events.send(SessionEvent::CleanupFailed(error));
            Input::CleanupFailed(error)
        } else {
            Input::Released
        }
    }

    fn finish_disconnected_commands(&mut self) {
        for (id, reply) in std::mem::take(&mut self.replies) {
            Self::complete(&self.events, id, reply, Err(CommandError::Disconnected));
        }
        while let Ok(dispatch) = self.commands.try_recv() {
            Self::complete(
                &self.events,
                dispatch.command.id,
                dispatch.reply,
                Err(CommandError::Disconnected),
            );
        }
    }

    fn complete(
        events: &broadcast::Sender<SessionEvent>,
        id: CommandId,
        reply: PendingReply,
        result: CommandResult,
    ) {
        let PendingReply {
            result: sender,
            _slot: slot,
        } = reply;
        drop(slot);
        let _ = sender.send(result);
        let _ = events.send(SessionEvent::CommandCompleted { id, result });
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

struct ActiveLink {
    tx: Box<dyn LinkTx>,
    received: mpsc::Receiver<Result<ReceivedFrame, TransportError>>,
    receive_order: Arc<AtomicU64>,
    reader: JoinHandle<()>,
}

impl ActiveLink {
    fn new(link: Box<dyn TransportLink>, events: broadcast::Sender<SessionEvent>) -> Self {
        let (tx, rx) = link.split();
        let (frames, received) = mpsc::channel(16);
        let receive_order = Arc::new(AtomicU64::new(0));
        let reader = tokio::spawn(read_link(rx, frames, Arc::clone(&receive_order), events));
        Self {
            tx,
            received,
            receive_order,
            reader,
        }
    }

    async fn close(self, deadline: Instant) -> Result<(), tokio::time::error::Elapsed> {
        let Self {
            mut tx,
            received,
            reader,
            ..
        } = self;
        drop(received);
        let result = timeout_at(deadline, tx.close()).await;
        drop(tx);
        reader.abort();
        let _ = reader.await;
        result
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
