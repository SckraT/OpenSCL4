use std::{
    future::Future,
    panic::Location,
    pin::Pin,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use macaddr::MacAddr6;
use tokio::{
    sync::{Notify, mpsc, watch},
    task::JoinHandle,
    time::{Instant, timeout, timeout_at},
};
use tracing::{debug, warn};

use super::{
    ConnectionStatus, Error, RfcommBackend, RfcommConnection, RfcommServiceSelectionStrategy,
};

pub const RECOVERY_WINDOW: Duration = Duration::from_secs(30);
pub const CONNECT_DEADLINE: Duration = Duration::from_secs(5);
pub const WRITE_DEADLINE: Duration = Duration::from_secs(2);

/// Backoff between connection attempts. The first attempt happens immediately, then these delays
/// produce attempts at 0, 1, 3, 7, 15 and 23 seconds after a loss.
const ATTEMPT_BACKOFFS: [Duration; 5] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(8),
];

#[derive(Clone, Copy, Debug)]
pub struct ConnectionSessionConfig {
    pub recovery_window: Duration,
    pub connect_deadline: Duration,
    pub write_deadline: Duration,
    /// Fractional jitter added to each backoff delay, for example `0.2` for up to +20%.
    pub jitter_ratio: f64,
}

impl Default for ConnectionSessionConfig {
    fn default() -> Self {
        Self {
            recovery_window: RECOVERY_WINDOW,
            connect_deadline: CONNECT_DEADLINE,
            write_deadline: WRITE_DEADLINE,
            jitter_ratio: 0.2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionStatus {
    Connecting,
    Connected,
    Recovering { attempt: u32, reason: String },
    Lost { reason: String },
    Closed,
}

impl SessionStatus {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    pub fn reason(&self) -> Option<&str> {
        match self {
            Self::Recovering { reason, .. } | Self::Lost { reason } => Some(reason),
            _ => None,
        }
    }
}

/// Runs after a reconnect has produced a new transport and before `Connected` is announced.
///
/// The device layer uses this to re-fetch its state. If it returns an error the transport is
/// discarded and recovery continues within the remaining budget.
pub type ReconnectHook =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<(), String>> + Send>> + Send + Sync>;

struct Transport {
    connection: Arc<dyn RfcommConnection + Send + Sync>,
    generation: u64,
}

struct SessionInner {
    backend: Option<Arc<dyn RfcommBackend + Send + Sync>>,
    mac_address: MacAddr6,
    service_selection_strategy: RfcommServiceSelectionStrategy,
    config: ConnectionSessionConfig,
    status_sender: watch::Sender<SessionStatus>,
    connection_status_sender: watch::Sender<ConnectionStatus>,
    transport: Mutex<Option<Transport>>,
    generation: AtomicU64,
    inbound_sender: mpsc::Sender<Vec<u8>>,
    inbound_receiver: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
    closed: AtomicBool,
    close_notify: Notify,
    self_ref: Weak<Self>,
    reconnect_hook: Mutex<Option<ReconnectHook>>,
    pump_handle: Mutex<Option<JoinHandle<()>>>,
    supervisor_handle: Mutex<Option<JoinHandle<()>>>,
    jitter_counter: AtomicU64,
}

impl SessionInner {
    fn status(&self) -> SessionStatus {
        self.status_sender.borrow().clone()
    }

    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    fn set_status(&self, status: SessionStatus) {
        let connection_status = match &status {
            SessionStatus::Connected => ConnectionStatus::Connected,
            _ => ConnectionStatus::Disconnected,
        };
        self.status_sender.send_if_modified(|current| {
            if *current == status {
                false
            } else {
                *current = status;
                true
            }
        });
        self.connection_status_sender.send_if_modified(|current| {
            if *current == connection_status {
                false
            } else {
                *current = connection_status;
                true
            }
        });
    }

    fn current_transport(&self) -> Option<(Arc<dyn RfcommConnection + Send + Sync>, u64)> {
        self.transport
            .lock()
            .expect("session transport mutex should not be poisoned")
            .as_ref()
            .map(|transport| (transport.connection.clone(), transport.generation))
    }

    fn install_transport(&self, connection: Arc<dyn RfcommConnection + Send + Sync>) -> u64 {
        self.abort_pump();
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        debug!(generation, "installed a new transport");
        {
            let mut transport = self
                .transport
                .lock()
                .expect("session transport mutex should not be poisoned");
            *transport = Some(Transport {
                connection: connection.clone(),
                generation,
            });
        }
        // A transport boundary must not leave a partially parsed packet from the old generation
        // in the packet parser.
        let _ = self.inbound_sender.try_send(Vec::new());
        let inner = self.clone_handle();
        let pump = tokio::spawn(async move {
            inner.run_pump(generation, connection).await;
        });
        *self
            .pump_handle
            .lock()
            .expect("session pump mutex should not be poisoned") = Some(pump);
        generation
    }

    fn drop_transport(&self) {
        self.abort_pump();
        self.generation.fetch_add(1, Ordering::SeqCst);
        let mut transport = self
            .transport
            .lock()
            .expect("session transport mutex should not be poisoned");
        *transport = None;
    }

    fn abort_pump(&self) {
        if let Some(pump) = self
            .pump_handle
            .lock()
            .expect("session pump mutex should not be poisoned")
            .take()
        {
            pump.abort();
        }
    }

    fn clone_handle(&self) -> Arc<Self> {
        self.self_ref
            .upgrade()
            .expect("session inner must outlive its background tasks")
    }

    async fn run_pump(&self, generation: u64, connection: Arc<dyn RfcommConnection + Send + Sync>) {
        let mut inbound = connection.read_channel();
        let mut status = connection.connection_status();
        if *status.borrow() == ConnectionStatus::Disconnected {
            self.on_transport_lost(generation, "connection status disconnected");
            return;
        }
        loop {
            tokio::select! {
                maybe_bytes = inbound.recv() => {
                    match maybe_bytes {
                        Some(bytes) => {
                            if self.generation.load(Ordering::SeqCst) != generation {
                                return;
                            }
                            if !bytes.is_empty()
                                && self.inbound_sender.send(bytes).await.is_err()
                            {
                                return;
                            }
                        }
                        None => {
                            self.on_transport_lost(generation, "read channel closed");
                            return;
                        }
                    }
                }
                result = status.changed() => {
                    if result.is_err() || *status.borrow() == ConnectionStatus::Disconnected {
                        self.on_transport_lost(generation, "connection status disconnected");
                        return;
                    }
                }
            }
        }
    }

    fn on_transport_lost(&self, generation: u64, reason: &str) {
        if self.generation.load(Ordering::SeqCst) != generation {
            return;
        }
        warn!(reason, "transport lost");
        self.report_unhealthy(reason);
    }

    fn report_unhealthy(&self, reason: impl Into<String>) {
        if self.is_closed() {
            return;
        }
        let reason = reason.into();
        let status = self.status();
        if matches!(status, SessionStatus::Lost { .. } | SessionStatus::Closed) {
            return;
        }
        self.drop_transport();
        let attempt = match status {
            SessionStatus::Recovering { attempt, .. } => attempt,
            _ => 0,
        };
        self.set_status(SessionStatus::Recovering { attempt, reason });
    }

    async fn connect_until(&self, deadline: Instant, initial: bool) -> Result<(), Error> {
        let mut attempt = 0_u32;
        let mut last_error: Option<Error> = None;
        let mut last_reason = "connection lost".to_owned();
        loop {
            if self.is_closed() {
                self.set_status(SessionStatus::Closed);
                return Err(connection_closed_error());
            }
            if Instant::now() >= deadline {
                let error = last_error.unwrap_or(Error::TimedOut {
                    action: "connection recovery",
                });
                self.lose(last_reason);
                return Err(error);
            }
            if !initial {
                self.set_status(SessionStatus::Recovering {
                    attempt,
                    reason: last_reason.clone(),
                });
            }
            match self.connect_once(deadline).await {
                Ok(generation) => {
                    let hook_result = self.run_reconnect_hook(deadline).await;
                    let still_current = self.generation.load(Ordering::SeqCst) == generation;
                    match hook_result {
                        Ok(()) if still_current && !self.is_closed() => {
                            self.set_status(SessionStatus::Connected);
                            return Ok(());
                        }
                        Ok(()) => {
                            self.drop_transport();
                            last_reason =
                                "transport was lost while verifying the connection".to_owned();
                            last_error = Some(Error::Other {
                                source: Box::new(std::io::Error::other(last_reason.clone())),
                                location: Location::caller(),
                            });
                        }
                        Err(message) => {
                            self.drop_transport();
                            last_reason = message;
                            last_error = Some(Error::Other {
                                source: Box::new(std::io::Error::other(last_reason.clone())),
                                location: Location::caller(),
                            });
                        }
                    }
                }
                Err(AttemptError::Terminal(error)) => {
                    self.lose(error_string(&error));
                    return Err(error);
                }
                Err(AttemptError::BudgetExhausted) => {
                    let error = last_error.unwrap_or(Error::TimedOut {
                        action: "connection recovery",
                    });
                    self.lose(last_reason);
                    return Err(error);
                }
                Err(AttemptError::Transient(error)) => {
                    last_reason = error_string(&error);
                    last_error = Some(error);
                }
            }

            let Some(delay) = ATTEMPT_BACKOFFS.get(attempt as usize).copied() else {
                if self.wait_until(deadline).await {
                    self.set_status(SessionStatus::Closed);
                    return Err(connection_closed_error());
                }
                let error = last_error.unwrap_or(Error::TimedOut {
                    action: "connection recovery",
                });
                self.lose(last_reason);
                return Err(error);
            };
            attempt += 1;
            let delay = self.jittered_delay(delay);
            let next_attempt_at = Instant::now() + delay;
            if !initial {
                self.set_status(SessionStatus::Recovering {
                    attempt,
                    reason: last_reason.clone(),
                });
            }
            if next_attempt_at >= deadline {
                if self.wait_until(deadline).await {
                    self.set_status(SessionStatus::Closed);
                    return Err(connection_closed_error());
                }
                let error = last_error.unwrap_or(Error::TimedOut {
                    action: "connection recovery",
                });
                self.lose(last_reason);
                return Err(error);
            }
            if self.wait_until(next_attempt_at).await {
                self.set_status(SessionStatus::Closed);
                return Err(connection_closed_error());
            }
        }
    }

    async fn connect_once(&self, deadline: Instant) -> Result<u64, AttemptError> {
        let Some(backend) = &self.backend else {
            return Err(AttemptError::Terminal(Error::Other {
                source: Box::new(std::io::Error::other(
                    "this connection does not support recovery",
                )),
                location: Location::caller(),
            }));
        };
        let connect = backend.connect(self.mac_address, self.service_selection_strategy);
        match timeout_at(deadline, timeout(self.config.connect_deadline, connect)).await {
            Err(_) => Err(AttemptError::BudgetExhausted),
            Ok(Err(_)) => Err(AttemptError::Transient(Error::TimedOut {
                action: "connect",
            })),
            Ok(Ok(Err(error))) => {
                if is_terminal(&error) {
                    Err(AttemptError::Terminal(error))
                } else {
                    Err(AttemptError::Transient(error))
                }
            }
            Ok(Ok(Ok(connection))) => Ok(self.install_transport(connection)),
        }
    }

    async fn run_reconnect_hook(&self, deadline: Instant) -> Result<(), String> {
        let Some(hook) = self
            .reconnect_hook
            .lock()
            .expect("session hook mutex should not be poisoned")
            .clone()
        else {
            return Ok(());
        };
        match timeout_at(deadline, hook()).await {
            Err(_) => Err("state refresh timed out".to_owned()),
            Ok(Ok(())) => Ok(()),
            Ok(Err(message)) => Err(message),
        }
    }

    fn jittered_delay(&self, delay: Duration) -> Duration {
        if self.config.jitter_ratio <= 0.0 {
            return delay;
        }
        let counter = self.jitter_counter.fetch_add(1, Ordering::SeqCst);
        // Deterministic pseudo-random fraction in [0, 1), avoiding a dependency on an RNG.
        let fraction =
            (counter.wrapping_mul(6364136223846793005) >> 33) as f64 / (1_u64 << 31) as f64;
        delay.mul_f64(1.0 + self.config.jitter_ratio * fraction)
    }

    /// Waits until `deadline`, returning true if the session was closed.
    async fn wait_until(&self, deadline: Instant) -> bool {
        if self.is_closed() {
            return true;
        }
        tokio::select! {
            () = tokio::time::sleep_until(deadline) => false,
            () = self.close_notify.notified() => true,
        }
    }

    fn lose(&self, reason: String) {
        self.drop_transport();
        self.set_status(SessionStatus::Lost { reason });
    }

    async fn supervise(self: Arc<Self>) {
        let mut status = self.status_sender.subscribe();
        loop {
            if self.is_closed() {
                self.set_status(SessionStatus::Closed);
                return;
            }
            let current = status.borrow().clone();
            match current {
                SessionStatus::Connected | SessionStatus::Connecting => {
                    if status.changed().await.is_err() {
                        return;
                    }
                }
                SessionStatus::Recovering { .. } => {
                    let deadline = Instant::now() + self.config.recovery_window;
                    if self.connect_until(deadline, false).await.is_err() {
                        return;
                    }
                }
                SessionStatus::Lost { .. } | SessionStatus::Closed => return,
            }
        }
    }

    fn shutdown(&self) {
        self.closed.store(true, Ordering::SeqCst);
        self.close_notify.notify_waiters();
        self.close_notify.notify_one();
        self.drop_transport();
        self.set_status(SessionStatus::Closed);
        if let Some(supervisor) = self
            .supervisor_handle
            .lock()
            .expect("session supervisor mutex should not be poisoned")
            .take()
        {
            supervisor.abort();
        }
    }
}

enum AttemptError {
    Transient(Error),
    Terminal(Error),
    BudgetExhausted,
}

/// Causes that retrying inside the recovery window cannot fix. Android's adapter collapses a
/// missing permission or an unbonded device into `DeviceNotFound`, so that is terminal too.
fn is_terminal(error: &Error) -> bool {
    matches!(
        error,
        Error::BluetoothAdapterUnavailable { .. }
            | Error::DeviceNotFound { .. }
            | Error::PermissionDenied { .. }
            | Error::Unpaired { .. }
            | Error::SecurityFailure { .. }
    )
}

fn error_string(error: &Error) -> String {
    error.to_string()
}

fn connection_closed_error() -> Error {
    Error::Other {
        source: Box::new(std::io::Error::other("connection closed")),
        location: Location::caller(),
    }
}

#[derive(Clone)]
pub struct ConnectionSession {
    inner: Arc<SessionInner>,
}

impl ConnectionSession {
    /// Creates a session and performs the initial connection within the recovery budget.
    pub async fn connect(
        backend: Arc<dyn RfcommBackend + Send + Sync>,
        mac_address: MacAddr6,
        service_selection_strategy: RfcommServiceSelectionStrategy,
        config: ConnectionSessionConfig,
    ) -> Result<Self, Error> {
        let (status_sender, _) = watch::channel(SessionStatus::Connecting);
        let (connection_status_sender, _) = watch::channel(ConnectionStatus::Disconnected);
        let (inbound_sender, inbound_receiver) = mpsc::channel(100);
        let inner = Arc::new_cyclic(|weak| SessionInner {
            backend: Some(backend),
            mac_address,
            service_selection_strategy,
            config,
            status_sender,
            connection_status_sender,
            transport: Mutex::new(None),
            generation: AtomicU64::new(0),
            inbound_sender,
            inbound_receiver: Mutex::new(Some(inbound_receiver)),
            closed: AtomicBool::new(false),
            close_notify: Notify::new(),
            self_ref: weak.clone(),
            reconnect_hook: Mutex::new(None),
            pump_handle: Mutex::new(None),
            supervisor_handle: Mutex::new(None),
            jitter_counter: AtomicU64::new(0),
        });
        let session = Self { inner };
        let deadline = Instant::now() + session.inner.config.recovery_window;
        session.inner.connect_until(deadline, true).await?;
        let supervisor = tokio::spawn(session.inner.clone().supervise());
        *session
            .inner
            .supervisor_handle
            .lock()
            .expect("session supervisor mutex should not be poisoned") = Some(supervisor);
        Ok(session)
    }

    /// Wraps an already connected transport. This is used by clients that do not own a backend,
    /// such as demo devices and protocol development tools; it cannot reconnect.
    pub fn from_connection(connection: Arc<dyn RfcommConnection + Send + Sync>) -> Self {
        let (status_sender, _) = watch::channel(SessionStatus::Connected);
        let (connection_status_sender, _) = watch::channel(ConnectionStatus::Connected);
        let (inbound_sender, inbound_receiver) = mpsc::channel(100);
        let inner = Arc::new_cyclic(|weak| SessionInner {
            backend: None,
            mac_address: MacAddr6::nil(),
            service_selection_strategy: RfcommServiceSelectionStrategy::Constant(uuid::Uuid::nil()),
            config: ConnectionSessionConfig::default(),
            status_sender,
            connection_status_sender,
            transport: Mutex::new(None),
            generation: AtomicU64::new(0),
            inbound_sender,
            inbound_receiver: Mutex::new(Some(inbound_receiver)),
            closed: AtomicBool::new(false),
            close_notify: Notify::new(),
            self_ref: weak.clone(),
            reconnect_hook: Mutex::new(None),
            pump_handle: Mutex::new(None),
            supervisor_handle: Mutex::new(None),
            jitter_counter: AtomicU64::new(0),
        });
        inner.install_transport(connection);
        let session = Self { inner };
        let supervisor = tokio::spawn(session.inner.clone().supervise());
        *session
            .inner
            .supervisor_handle
            .lock()
            .expect("session supervisor mutex should not be poisoned") = Some(supervisor);
        session
    }

    pub fn status(&self) -> SessionStatus {
        self.inner.status()
    }

    pub fn status_receiver(&self) -> watch::Receiver<SessionStatus> {
        self.inner.status_sender.subscribe()
    }

    pub fn connection_status(&self) -> watch::Receiver<ConnectionStatus> {
        self.inner.connection_status_sender.subscribe()
    }

    /// Takes the stable read channel that survives transport replacements.
    pub fn read_channel(&self) -> mpsc::Receiver<Vec<u8>> {
        self.inner
            .inbound_receiver
            .lock()
            .expect("session inbound mutex should not be poisoned")
            .take()
            .expect("ConnectionSession::read_channel should only be called once")
    }

    pub fn generation(&self) -> u64 {
        self.inner.generation.load(Ordering::SeqCst)
    }

    pub async fn write(&self, data: &[u8]) -> Result<(), Error> {
        self.write_if_current(data, self.generation()).await
    }

    /// Writes only if the session is still on the transport generation the caller queued for.
    /// This prevents a command queued before a reconnect from being replayed on the next socket.
    pub async fn write_if_current(&self, data: &[u8], generation: u64) -> Result<(), Error> {
        let Some((connection, current_generation)) = self.inner.current_transport() else {
            return Err(Error::WriteError {
                source: Some(Box::new(std::io::Error::other(
                    "no active transport for this session",
                ))),
                location: Location::caller(),
            });
        };
        if current_generation != generation {
            return Err(Error::WriteError {
                source: Some(Box::new(std::io::Error::other(
                    "transport generation changed before the command was written",
                ))),
                location: Location::caller(),
            });
        }
        match timeout(self.inner.config.write_deadline, connection.write(data)).await {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => {
                self.inner
                    .report_unhealthy(format!("write failed: {error}"));
                Err(error)
            }
            Err(_) => {
                self.inner.report_unhealthy("write timed out");
                Err(Error::TimedOut { action: "write" })
            }
        }
    }

    pub fn set_reconnect_hook(&self, hook: ReconnectHook) {
        *self
            .inner
            .reconnect_hook
            .lock()
            .expect("session hook mutex should not be poisoned") = Some(hook);
    }

    /// Marks the transport unhealthy, entering `Recovering` if this session can recover.
    pub fn report_unhealthy(&self, reason: impl Into<String>) {
        self.inner.report_unhealthy(reason);
    }

    pub async fn close(&self) {
        self.inner.shutdown();
    }

    /// Synchronous shutdown for drop paths, where awaiting is not possible.
    pub fn shutdown(&self) {
        self.inner.shutdown();
    }
}

impl Drop for ConnectionSession {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.inner.shutdown();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        panic::Location,
        sync::{
            Arc, Mutex,
            atomic::{AtomicBool, AtomicUsize, Ordering},
        },
        time::Duration,
    };

    use async_trait::async_trait;
    use macaddr::MacAddr6;
    use tokio::sync::{mpsc, watch};

    use super::*;
    use crate::api::connection::{ConnectionDescriptor, ConnectionStatus, RfcommBackend};

    struct ScriptedConnection {
        status: watch::Sender<ConnectionStatus>,
        inbound: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
        outbound: mpsc::Sender<Vec<u8>>,
        hang_writes: AtomicBool,
    }

    impl ScriptedConnection {
        fn new() -> Self {
            let (inbound_sender, inbound_receiver) = mpsc::channel(100);
            let (outbound, mut outbound_receiver) = mpsc::channel(100);
            tokio::spawn(async move { while outbound_receiver.recv().await.is_some() {} });
            // Keep the inbound sender alive for the lifetime of the connection by leaking one end
            // into the receiver task. Tests that need inbound bytes can use their own connection.
            std::mem::forget(inbound_sender);
            Self {
                status: watch::channel(ConnectionStatus::Connected).0,
                inbound: Mutex::new(Some(inbound_receiver)),
                outbound,
                hang_writes: AtomicBool::new(false),
            }
        }

        fn disconnect(&self) {
            self.status.send_replace(ConnectionStatus::Disconnected);
        }
    }

    #[async_trait]
    impl RfcommConnection for ScriptedConnection {
        async fn write(&self, data: &[u8]) -> Result<(), Error> {
            if self.hang_writes.load(Ordering::SeqCst) {
                std::future::pending::<()>().await;
            }
            let _ = self.outbound.send(data.to_vec()).await;
            Ok(())
        }

        fn read_channel(&self) -> mpsc::Receiver<Vec<u8>> {
            self.inbound
                .lock()
                .unwrap()
                .take()
                .expect("read_channel should only be called once")
        }

        fn connection_status(&self) -> watch::Receiver<ConnectionStatus> {
            self.status.subscribe()
        }
    }

    #[derive(Clone, Copy)]
    enum ScriptedFailure {
        TimedOut,
        AdapterUnavailable,
        DeviceNotFound,
        PermissionDenied,
        Unpaired,
    }

    struct ScriptedBackend {
        available: AtomicBool,
        failure: Mutex<ScriptedFailure>,
        attempts: AtomicUsize,
        attempt_times: Mutex<Vec<Instant>>,
        connections: Mutex<Vec<Arc<ScriptedConnection>>>,
    }

    impl ScriptedBackend {
        fn new(available: bool) -> Self {
            Self {
                available: AtomicBool::new(available),
                failure: Mutex::new(ScriptedFailure::TimedOut),
                attempts: AtomicUsize::new(0),
                attempt_times: Mutex::new(Vec::new()),
                connections: Mutex::new(Vec::new()),
            }
        }

        fn set_available(&self, available: bool) {
            self.available.store(available, Ordering::SeqCst);
        }

        fn set_failure(&self, failure: ScriptedFailure) {
            *self.failure.lock().unwrap() = failure;
        }

        fn attempt_times(&self) -> Vec<Instant> {
            self.attempt_times.lock().unwrap().clone()
        }

        fn latest_connection(&self) -> Arc<ScriptedConnection> {
            self.connections
                .lock()
                .unwrap()
                .last()
                .expect("a connection should have been created")
                .clone()
        }
    }

    #[async_trait]
    impl RfcommBackend for ScriptedBackend {
        async fn devices(&self) -> Result<HashSet<ConnectionDescriptor>, Error> {
            Ok(HashSet::new())
        }

        async fn connect(
            &self,
            _mac_address: MacAddr6,
            _service_selection_strategy: RfcommServiceSelectionStrategy,
        ) -> Result<Arc<dyn RfcommConnection + Send + Sync>, Error> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            self.attempt_times.lock().unwrap().push(Instant::now());
            if !self.available.load(Ordering::SeqCst) {
                let location = Location::caller();
                return Err(match *self.failure.lock().unwrap() {
                    ScriptedFailure::TimedOut => Error::TimedOut { action: "connect" },
                    ScriptedFailure::AdapterUnavailable => Error::BluetoothAdapterUnavailable {
                        source: None,
                        location,
                    },
                    ScriptedFailure::DeviceNotFound => Error::DeviceNotFound {
                        source: None,
                        location,
                    },
                    ScriptedFailure::PermissionDenied => Error::PermissionDenied {
                        source: None,
                        location,
                    },
                    ScriptedFailure::Unpaired => Error::Unpaired {
                        source: None,
                        location,
                    },
                });
            }
            let connection = Arc::new(ScriptedConnection::new());
            self.connections.lock().unwrap().push(connection.clone());
            Ok(connection)
        }
    }

    fn config() -> ConnectionSessionConfig {
        ConnectionSessionConfig {
            jitter_ratio: 0.0,
            ..Default::default()
        }
    }

    async fn connect_session(backend: Arc<ScriptedBackend>) -> ConnectionSession {
        ConnectionSession::connect(
            backend,
            MacAddr6::nil(),
            RfcommServiceSelectionStrategy::Constant(uuid::Uuid::nil()),
            config(),
        )
        .await
        .expect("initial connection should succeed")
    }

    async fn wait_for_status(
        session: &ConnectionSession,
        predicate: impl Fn(&SessionStatus) -> bool,
    ) -> SessionStatus {
        let mut status = session.status_receiver();
        loop {
            let current = status.borrow().clone();
            if predicate(&current) {
                return current;
            }
            status.changed().await.expect("session should stay alive");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_transport_drop_recovers_within_the_window_without_reaching_lost() {
        let backend = Arc::new(ScriptedBackend::new(true));
        let session = connect_session(backend.clone()).await;
        assert_eq!(SessionStatus::Connected, session.status());

        backend.set_available(false);
        backend.latest_connection().disconnect();
        wait_for_status(&session, |status| {
            matches!(status, SessionStatus::Recovering { .. })
        })
        .await;

        tokio::time::sleep(Duration::from_secs(10)).await;
        backend.set_available(true);

        let status = tokio::time::timeout(
            Duration::from_secs(20),
            wait_for_status(&session, |status| {
                matches!(
                    status,
                    SessionStatus::Connected | SessionStatus::Lost { .. }
                )
            }),
        )
        .await
        .expect("recovery should finish within the window");
        assert_eq!(SessionStatus::Connected, status);
        assert!(backend.attempts.load(Ordering::SeqCst) >= 5);
    }

    #[tokio::test(start_paused = true)]
    async fn an_unavailable_backend_exhausts_the_budget_at_thirty_seconds() {
        let backend = Arc::new(ScriptedBackend::new(true));
        let session = connect_session(backend.clone()).await;
        backend.set_available(false);

        backend.latest_connection().disconnect();
        wait_for_status(&session, |status| {
            matches!(status, SessionStatus::Recovering { .. })
        })
        .await;

        let lost = tokio::time::timeout(
            Duration::from_secs(35),
            wait_for_status(&session, |status| {
                matches!(status, SessionStatus::Lost { .. })
            }),
        )
        .await
        .expect("recovery budget should expire");
        assert_eq!(
            SessionStatus::Lost {
                reason: "connect timed out".to_owned()
            },
            lost
        );

        let times = backend.attempt_times();
        assert_eq!(7, times.len(), "initial attempt plus six recovery attempts");
        let base = times[0];
        let expected = [0_u64, 1, 3, 7, 15, 23];
        for (time, expected_seconds) in times[1..].iter().zip(expected) {
            let elapsed = time.duration_since(base).as_secs_f64();
            assert!(
                (elapsed - expected_seconds as f64).abs() < 0.1,
                "attempt at {elapsed}s, expected {expected_seconds}s"
            );
        }
        assert!(Instant::now().duration_since(base) >= Duration::from_secs(30));
        assert!(Instant::now().duration_since(base) < Duration::from_secs(31));
    }

    #[tokio::test(start_paused = true)]
    async fn a_terminal_error_does_not_retry() {
        let backend = Arc::new(ScriptedBackend::new(true));
        let session = connect_session(backend.clone()).await;
        backend.set_available(false);
        backend.set_failure(ScriptedFailure::AdapterUnavailable);

        backend.latest_connection().disconnect();
        let lost = tokio::time::timeout(
            Duration::from_secs(2),
            wait_for_status(&session, |status| {
                matches!(status, SessionStatus::Lost { .. })
            }),
        )
        .await
        .expect("terminal errors should be reported immediately");
        assert_eq!(
            SessionStatus::Lost {
                reason: "bluetooth adapter unavailable".to_owned()
            },
            lost
        );
        assert_eq!(2, backend.attempts.load(Ordering::SeqCst));
    }

    #[tokio::test(start_paused = true)]
    async fn not_found_permission_and_unpaired_errors_are_terminal() {
        for (failure, expected) in [
            (ScriptedFailure::DeviceNotFound, "device not found"),
            (
                ScriptedFailure::PermissionDenied,
                "bluetooth permission denied",
            ),
            (ScriptedFailure::Unpaired, "device is not paired"),
        ] {
            let backend = Arc::new(ScriptedBackend::new(true));
            let session = connect_session(backend.clone()).await;
            backend.set_available(false);
            backend.set_failure(failure);

            backend.latest_connection().disconnect();
            let lost = tokio::time::timeout(
                Duration::from_secs(2),
                wait_for_status(&session, |status| {
                    matches!(status, SessionStatus::Lost { .. })
                }),
            )
            .await
            .expect("terminal errors should be reported immediately");
            assert_eq!(
                SessionStatus::Lost {
                    reason: expected.to_owned()
                },
                lost
            );
            assert_eq!(2, backend.attempts.load(Ordering::SeqCst));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn an_explicit_close_during_recovery_stops_retrying() {
        let backend = Arc::new(ScriptedBackend::new(true));
        let session = connect_session(backend.clone()).await;
        backend.set_available(false);

        backend.latest_connection().disconnect();
        wait_for_status(&session, |status| {
            matches!(status, SessionStatus::Recovering { .. })
        })
        .await;
        session.close().await;
        assert_eq!(SessionStatus::Closed, session.status());
        let attempts = backend.attempts.load(Ordering::SeqCst);

        tokio::time::sleep(Duration::from_secs(5)).await;
        assert_eq!(attempts, backend.attempts.load(Ordering::SeqCst));
        assert_eq!(SessionStatus::Closed, session.status());
    }

    #[tokio::test(start_paused = true)]
    async fn the_reconnect_hook_runs_before_connected_is_announced() {
        let backend = Arc::new(ScriptedBackend::new(true));
        let session = connect_session(backend.clone()).await;
        let seen = Arc::new(Mutex::new(None));
        let seen_for_hook = seen.clone();
        let session_for_hook = session.clone();
        session.set_reconnect_hook(Arc::new(move || {
            let seen = seen_for_hook.clone();
            let session = session_for_hook.clone();
            Box::pin(async move {
                *seen.lock().unwrap() = Some(session.status());
                Ok(())
            })
        }));

        backend.set_available(false);
        backend.latest_connection().disconnect();
        wait_for_status(&session, |status| {
            matches!(status, SessionStatus::Recovering { .. })
        })
        .await;
        backend.set_available(true);
        let status = tokio::time::timeout(
            Duration::from_secs(5),
            wait_for_status(&session, |status| {
                matches!(
                    status,
                    SessionStatus::Connected | SessionStatus::Lost { .. }
                )
            }),
        )
        .await
        .expect("recovery should finish");
        assert_eq!(SessionStatus::Connected, status);
        assert!(matches!(
            seen.lock().unwrap().as_ref(),
            Some(SessionStatus::Recovering { .. })
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn a_failing_reconnect_hook_never_announces_connected() {
        let backend = Arc::new(ScriptedBackend::new(true));
        let session = connect_session(backend.clone()).await;
        session.set_reconnect_hook(Arc::new(|| {
            Box::pin(async { Err("state refresh failed".to_owned()) })
        }));

        backend.latest_connection().disconnect();
        let lost = tokio::time::timeout(
            Duration::from_secs(35),
            wait_for_status(&session, |status| {
                matches!(status, SessionStatus::Lost { .. })
            }),
        )
        .await
        .expect("recovery budget should expire");
        assert_eq!(
            SessionStatus::Lost {
                reason: "state refresh failed".to_owned()
            },
            lost
        );
        assert!(
            !session
                .connection_status()
                .borrow()
                .eq(&ConnectionStatus::Connected)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_command_from_an_old_generation_is_not_written_after_reconnect() {
        let backend = Arc::new(ScriptedBackend::new(true));
        let session = connect_session(backend.clone()).await;
        let old_generation = session.generation();

        backend.set_available(false);
        backend.latest_connection().disconnect();
        wait_for_status(&session, |status| {
            matches!(status, SessionStatus::Recovering { .. })
        })
        .await;
        backend.set_available(true);
        wait_for_status(&session, |status| {
            matches!(
                status,
                SessionStatus::Connected | SessionStatus::Lost { .. }
            )
        })
        .await;

        assert!(
            session
                .write_if_current(b"stale", old_generation)
                .await
                .is_err()
        );
        session
            .write_if_current(b"fresh", session.generation())
            .await
            .expect("a command from the current generation should be written");
    }

    #[tokio::test(start_paused = true)]
    async fn a_write_timeout_enters_recovering() {
        let backend = Arc::new(ScriptedBackend::new(true));
        let session = connect_session(backend.clone()).await;
        backend.set_available(false);
        backend
            .latest_connection()
            .hang_writes
            .store(true, Ordering::SeqCst);

        let started = Instant::now();
        let result = session.write(&[1, 2, 3]).await;
        assert!(matches!(result, Err(Error::TimedOut { action: "write" })));
        assert!(started.elapsed() >= Duration::from_secs(2));
        assert!(matches!(session.status(), SessionStatus::Recovering { .. }));
    }
}
