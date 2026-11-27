use std::{
    collections::HashMap,
    future::Future,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use openscl4_protocol::{Direction, Frame, encode_frame};
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, TransportError, TransportLink, TransportProvider,
};
use tokio::{
    sync::{OwnedSemaphorePermit, Semaphore, broadcast, mpsc, oneshot, watch},
    task::JoinHandle,
    time::{Instant, sleep_until, timeout_at},
};

use crate::{
    CommandError, CommandId, CommandResult, ConnectionState, DeviceState, DiagnosticExportError,
    DispatchError, EqualizerSetting, IoStage, ReconnectPolicy, SessionCommand, SessionError,
    SessionEvent, SessionView,
    diagnostics::{Capture, CaptureDecoder},
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

enum ReceivedInput {
    Dispatch(Dispatch),
    Machine(Input),
}

#[derive(Clone, Copy)]
struct LiveEq {
    generation: u64,
    after_command: CommandId,
    setting: EqualizerSetting,
}

/// A shared handle for observing, controlling and closing one session.
pub struct SessionHandle {
    view: watch::Receiver<Arc<SessionView>>,
    events: broadcast::Receiver<SessionEvent>,
    close_requested: watch::Sender<bool>,
    commands: mpsc::Sender<Dispatch>,
    live_eq: watch::Sender<Option<LiveEq>>,
    slots: Arc<Semaphore>,
    last_command: Arc<Mutex<CommandId>>,
    capture: Capture,
    started: Instant,
}

impl Clone for SessionHandle {
    fn clone(&self) -> Self {
        Self {
            view: self.view.clone(),
            events: self.events.resubscribe(),
            close_requested: self.close_requested.clone(),
            commands: self.commands.clone(),
            live_eq: self.live_eq.clone(),
            slots: Arc::clone(&self.slots),
            last_command: Arc::clone(&self.last_command),
            capture: self.capture.clone(),
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

    /// Enables or pauses capture of sanitized frames for this session.
    pub fn set_frame_capture_enabled(&self, enabled: bool) {
        self.capture.set_enabled(enabled);
    }

    /// Writes a snapshot of the captured frames to a caller-selected local ZIP.
    pub async fn export_diagnostics(
        &self,
        destination: impl Into<PathBuf>,
    ) -> Result<(), DiagnosticExportError> {
        self.capture.export(destination.into()).await
    }

    /// Waits for the first confirmed device snapshot or read-only identity.
    pub async fn confirmed_view(&self) -> Result<Arc<SessionView>, SessionError> {
        let mut view = self.subscribe();
        let snapshot = view
            .wait_for(|view| {
                matches!(
                    view.connection,
                    ConnectionState::Ready(_) | ConnectionState::Unidentified(_)
                ) || view.connection.is_terminal()
            })
            .await
            .map_err(|_| SessionError::Closed)?;
        match snapshot.connection {
            ConnectionState::Ready(_) | ConnectionState::Unidentified(_) => {
                Ok(Arc::clone(&snapshot))
            }
            ConnectionState::Failed(error) => Err(error),
            _ => Err(SessionError::Closed),
        }
    }

    /// Reads the ready device and its generation in one snapshot.
    fn ready_device(&self) -> Result<(u64, DeviceState), DispatchError> {
        if *self.close_requested.borrow() {
            return Err(DispatchError::Closed);
        }
        let view = self.view.borrow();
        match view.connection {
            ConnectionState::Ready(device) => Ok((view.generation, device)),
            ConnectionState::Unidentified(_) => Err(DispatchError::ReadOnly),
            state if state.is_terminal() => Err(DispatchError::Closed),
            _ => Err(DispatchError::NotReady),
        }
    }

    /// Enqueues a discrete command and returns its independent completion handle.
    pub fn dispatch(&self, command: SessionCommand) -> Result<PendingCommand, DispatchError> {
        // Discrete admission and live publication share an order across cloned handles.
        let mut last_command = self.last_command.lock().expect("session submission order");
        let (generation, _) = self.ready_device()?;
        let slot = Arc::clone(&self.slots)
            .try_acquire_owned()
            .map_err(|_| DispatchError::QueueFull)?;
        let id = CommandId(last_command.0 + 1);
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
        *last_command = id;

        Ok(PendingCommand {
            id,
            outcome,
            _session: self.close_requested.clone(),
            _commands: self.commands.clone(),
        })
    }

    /// Publishes the latest live-equalizer curve for the session to write.
    ///
    /// Rapid values coalesce into one pending curve. The latest position is
    /// scheduled after the write interval and any earlier accepted presets.
    /// A live value has no completion handle; the confirmed state is its outcome.
    /// Model support and gain bounds are checked before publication.
    pub fn set_eq_live(&self, setting: EqualizerSetting) -> Result<(), DispatchError> {
        let last_command = self.last_command.lock().expect("session submission order");
        let (generation, device) = self.ready_device()?;
        if !device.model.capabilities().equalizer_write {
            return Err(DispatchError::UnsupportedEqualizer);
        }
        if !device.model.equalizer_is_writable(setting) {
            return Err(DispatchError::InvalidEqualizer);
        }
        self.live_eq.send_replace(Some(LiveEq {
            generation,
            after_command: *last_command,
            setting,
        }));
        Ok(())
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
    capture: Capture,
    view: watch::Sender<Arc<SessionView>>,
    events: broadcast::Sender<SessionEvent>,
    close_requested: watch::Receiver<bool>,
    commands: mpsc::Receiver<Dispatch>,
    live_eq: watch::Receiver<Option<LiveEq>>,
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
        let (live_eq, live_eq_updates) = watch::channel(None);
        let started = Instant::now();
        let capture = Capture::new(started);
        let runner = Self {
            machine,
            capture: capture.clone(),
            view,
            events,
            close_requested,
            commands: incoming,
            live_eq: live_eq_updates,
            replies: HashMap::new(),
        };
        tokio::spawn(runner.run(provider, target, started));

        SessionHandle {
            view: view_rx,
            events: events_rx,
            close_requested: close,
            commands,
            live_eq,
            slots: Arc::new(Semaphore::new(COMMAND_CAPACITY)),
            last_command: Arc::new(Mutex::new(CommandId(0))),
            capture,
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
                            let decoder =
                                CaptureDecoder::new(self.capture.clone(), self.view.subscribe());
                            active = Some(ActiveLink::new(link, self.events.clone(), decoder));
                            Input::Opened
                        }
                        Err(error) => terminal_input(error),
                    }
                }
                Effect::Release {
                    deadline,
                    interrupted_preset,
                } => {
                    last_disconnected = generation;
                    self.publish();
                    self.finish_disconnected_commands(interrupted_preset);
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
                    let frame = Frame {
                        direction: Direction::Outbound,
                        command,
                        body: body.into(),
                    };
                    let connection = self.machine.view().connection;
                    let sending = async {
                        let request = encode_frame(frame.direction, frame.command, &frame.body)?;
                        let receive_cutoff = link.receive_order.load(Ordering::SeqCst);
                        timeout_at(started + deadline, async {
                            send_started = true;
                            self.capture.record_frame(connection, &frame);
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
                    let received = tokio::select! {
                        biased;
                        _ = self.close_requested.wait_for(|requested| *requested) => ReceivedInput::Machine(Input::Close),
                        dispatch = self.commands.recv() => dispatch
                            .map_or(ReceivedInput::Machine(Input::Close), ReceivedInput::Dispatch),
                        live_eq = next_live_eq(&mut self.live_eq) => {
                            match live_eq {
                                Some(update) if update.generation == generation => {
                                    // A command can have arrived after its branch was polled.
                                    match self.commands.try_recv() {
                                        Ok(dispatch) => {
                                            self.live_eq.mark_changed();
                                            ReceivedInput::Dispatch(dispatch)
                                        }
                                        Err(_) => ReceivedInput::Machine(Input::EqLive {
                                            after_command: update.after_command,
                                            setting: update.setting,
                                        }),
                                    }
                                }
                                // A value stamped by a previous connection is not a
                                // new user action and is not replayed on the new one.
                                Some(_) | None => {
                                    effect = Effect::Receive(deadline);
                                    continue;
                                }
                            }
                        }
                        result = receiving => {
                            let input = result.unwrap_or_else(terminal_input);
                            if matches!(input, Input::TimerElapsed) {
                                self.commands.try_recv().map_or(
                                    ReceivedInput::Machine(input),
                                    ReceivedInput::Dispatch,
                                )
                            } else {
                                ReceivedInput::Machine(input)
                            }
                        }
                    };
                    match received {
                        ReceivedInput::Dispatch(dispatch) if dispatch.generation == generation => {
                            self.replies.insert(dispatch.command.id, dispatch.reply);
                            Input::Command(dispatch.command)
                        }
                        ReceivedInput::Dispatch(dispatch) => {
                            Self::complete(
                                &self.events,
                                dispatch.command.id,
                                dispatch.reply,
                                Err(CommandError::Disconnected),
                            );
                            effect = Effect::Receive(deadline);
                            continue;
                        }
                        ReceivedInput::Machine(input) => input,
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

    fn finish_disconnected_commands(
        &mut self,
        interrupted_preset: Option<(CommandId, CommandError)>,
    ) {
        for (id, reply) in std::mem::take(&mut self.replies) {
            let error = match interrupted_preset {
                Some((preset_id, error)) if id == preset_id => error,
                _ => CommandError::Disconnected,
            };
            Self::complete(&self.events, id, reply, Err(error));
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
    fn new(
        link: Box<dyn TransportLink>,
        events: broadcast::Sender<SessionEvent>,
        decoder: CaptureDecoder,
    ) -> Self {
        let (tx, rx) = link.split();
        let (frames, received) = mpsc::channel(16);
        let receive_order = Arc::new(AtomicU64::new(0));
        let reader = tokio::spawn(read_link(
            rx,
            frames,
            Arc::clone(&receive_order),
            events,
            decoder,
        ));
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
    mut decoder: CaptureDecoder,
) {
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

/// Waits for the next published live-equalizer value.
///
/// Once the handle is gone no further values can arrive, and this future
/// never completes, so the loop keeps waiting on frames and commands instead
/// of spinning on the closed channel.
async fn next_live_eq(live_eq: &mut watch::Receiver<Option<LiveEq>>) -> Option<LiveEq> {
    if live_eq.changed().await.is_err() {
        std::future::pending::<()>().await;
    }
    *live_eq.borrow_and_update()
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
