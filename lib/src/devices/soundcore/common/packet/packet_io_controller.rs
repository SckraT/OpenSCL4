use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, Weak},
    time::Duration,
};

use nom_language::error::VerboseError;
use tokio::{
    sync::{mpsc, watch},
    task::JoinHandle,
    time::{Instant, timeout_at},
};
use tracing::{Instrument, info_span, trace};

use crate::{
    api::{
        connection::{
            ConnectionSession, ConnectionStatus, RfcommConnection, SessionFailureReason,
            SessionStatus,
        },
        device,
    },
    devices::soundcore::common::packet::{self, ChecksumKind, Command, outbound::ToPacket},
};

use super::multi_queue::{MultiQueue, MultiQueueHandle};

pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
pub const WAIT_FOR_START_TIMEOUT: Duration = Duration::from_secs(3);
pub const MULTI_RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);
const ACK_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(500);
const MULTI_RESPONSE_PACKET_TIMEOUT: Duration = Duration::from_secs(2);
pub const SILENCE_TIMEOUT: Duration = Duration::from_secs(30);
const WATCHDOG_RETRY_DELAY: Duration = Duration::from_secs(1);

pub struct PacketIOController {
    checksum_kind: ChecksumKind,
    session: Arc<ConnectionSession>,
    packet_queues: Arc<MultiQueue<Command, packet::Inbound>>,
    handle: JoinHandle<()>,
    status_handle: JoinHandle<()>,
    watchdog_handle: Mutex<Option<JoinHandle<()>>>,
    last_inbound: Arc<Mutex<Instant>>,
}

impl Drop for PacketIOController {
    fn drop(&mut self) {
        self.handle.abort();
        self.status_handle.abort();
        if let Some(watchdog) = self
            .watchdog_handle
            .lock()
            .expect("watchdog handle mutex should not be poisoned")
            .take()
        {
            watchdog.abort();
        }
        self.session.shutdown();
        trace!("dropped PacketIOController");
    }
}

impl PacketIOController {
    /// In addition to the PacketIOController, also returns a channel that all packets received
    /// that weren't a result of send_with_response will be forwarded to.
    pub async fn new(
        connection: Arc<dyn RfcommConnection + Send + Sync>,
        checksum_kind: ChecksumKind,
    ) -> device::Result<(Self, mpsc::Receiver<packet::Inbound>)> {
        let session = Arc::new(ConnectionSession::from_connection(connection));
        Self::new_with_session(session, checksum_kind).await
    }

    pub async fn new_with_session(
        session: Arc<ConnectionSession>,
        checksum_kind: ChecksumKind,
    ) -> device::Result<(Self, mpsc::Receiver<packet::Inbound>)> {
        let packet_queues = Arc::new(MultiQueue::new());
        let incoming_receiver = session.read_channel();
        let last_inbound = Arc::new(Mutex::new(Instant::now()));

        let (handle, outgoing_receiver) = Self::spawn_packet_handler(
            checksum_kind,
            packet_queues.clone(),
            incoming_receiver,
            last_inbound.clone(),
        );
        let status_handle =
            Self::spawn_status_watcher(packet_queues.clone(), session.status_receiver());
        Ok((
            Self {
                checksum_kind,
                session,
                packet_queues,
                handle,
                status_handle,
                watchdog_handle: Mutex::new(None),
                last_inbound,
            },
            outgoing_receiver,
        ))
    }

    fn spawn_packet_handler(
        checksum_kind: ChecksumKind,
        packet_queues: Arc<MultiQueue<Command, packet::Inbound>>,
        mut incoming_receiver: mpsc::Receiver<Vec<u8>>,
        last_inbound: Arc<Mutex<Instant>>,
    ) -> (JoinHandle<()>, mpsc::Receiver<packet::Inbound>) {
        let (outgoing_sender, outgoing_receiver) = mpsc::channel(100);
        let handle = tokio::spawn(async move {
            let mut buffer = Vec::<u8>::new();

            'receive_packet:
            while let Some(mut bytes) = incoming_receiver.recv().await {
                if bytes.is_empty() {
                    // Transport generation boundary. Partially parsed data from the previous
                    // socket must not be combined with bytes from the new one.
                    buffer.clear();
                    continue;
                }
                buffer.extend_from_slice(&bytes);
                let mut start_index = 0;
                while start_index < buffer.len() {
                    let (remainder, packet) =
                        match packet::Inbound::take::<VerboseError<_>>(checksum_kind)(&buffer[start_index..]) {
                            Ok(parsed) => parsed,
                            Err(nom::Err::Incomplete(_)) => break,
                            Err(err) => {
                                tracing::warn!("failed to parse packet: {err:?}");
                                tracing::warn!("clearing buffer: {buffer:?}");
                                buffer.clear();
                                continue 'receive_packet;
                            }
                        };

                    tracing::debug!("received packet {packet:?}");
                    *last_inbound
                        .lock()
                        .expect("last inbound mutex should not be poisoned") = Instant::now();
                    let packet_length = buffer.len() - start_index - remainder.len();
                    start_index += packet_length;

                    if !packet_queues.pop(&packet.command, packet.clone()) {
                        match outgoing_sender.send(packet).await {
                            Ok(()) => (),
                            Err(err) => tracing::debug!(
                                "received packet that wasn't an ok, but the channel is closed, so it won't be forwarded: {err:?}"
                            ),
                        }
                    }
                }
                // Reuse bytes allocation for new buffer containing the remaining partial packet
                bytes.clear();
                bytes.extend_from_slice(&buffer[start_index..]);
                buffer = bytes;
            }
        }.instrument(info_span!("packet handler")));
        (handle, outgoing_receiver)
    }

    fn spawn_status_watcher(
        packet_queues: Arc<MultiQueue<Command, packet::Inbound>>,
        mut status: watch::Receiver<SessionStatus>,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                if status.changed().await.is_err() {
                    return;
                }
                let current = status.borrow().clone();
                if !current.is_connected() {
                    // Commands queued before the loss belong to the previous transport and must
                    // never be replayed on the next one.
                    packet_queues.cancel_all();
                }
                if matches!(current, SessionStatus::Lost { .. } | SessionStatus::Closed) {
                    return;
                }
            }
        })
    }

    /// Starts the silence watchdog. It is opt-in because protocol development and demo devices
    /// may legitimately receive no packets.
    pub fn start_watchdog(self: &Arc<Self>) {
        let mut watchdog_handle = self
            .watchdog_handle
            .lock()
            .expect("watchdog handle mutex should not be poisoned");
        if watchdog_handle.is_some() {
            return;
        }
        let weak = Arc::downgrade(self);
        *watchdog_handle = Some(tokio::spawn(async move {
            if let Err(message) = run_watchdog(weak).await {
                tracing::debug!("silence watchdog stopped: {message}");
            }
        }));
    }

    pub fn connection_status(&self) -> watch::Receiver<ConnectionStatus> {
        self.session.connection_status()
    }

    pub fn session_status(&self) -> watch::Receiver<SessionStatus> {
        self.session.status_receiver()
    }

    #[tracing::instrument(skip(self))]
    pub async fn send_with_response(
        &self,
        packet: &packet::Outbound,
    ) -> device::Result<packet::Inbound> {
        let queue_key = packet.command;
        let deadline = Instant::now() + COMMAND_TIMEOUT;
        let generation = self.session.generation();
        let pending = PendingCommand::new(self.packet_queues.clone(), queue_key);

        if pending.is_cancelled() {
            return Err(device::Error::Disconnected);
        }
        let start_timeout = (Instant::now() + WAIT_FOR_START_TIMEOUT).min(deadline);
        if timeout_at(start_timeout, pending.handle().wait_for_start())
            .await
            .is_err()
        {
            return Err(device::Error::ActionTimedOut {
                action: "waiting for queue turn",
            });
        }
        if pending.is_cancelled() {
            return Err(device::Error::Disconnected);
        }

        let packet_bytes = packet.bytes(self.checksum_kind);
        self.session
            .write_if_current(&packet_bytes, generation)
            .await?;

        for i in 1..=3 {
            if pending.is_cancelled() {
                return Err(device::Error::Disconnected);
            }
            let ack_timeout = (Instant::now() + ACK_ATTEMPT_TIMEOUT * i).min(deadline);
            if timeout_at(ack_timeout, pending.handle().wait_for_end())
                .await
                .is_ok()
            {
                if pending.is_cancelled() {
                    return Err(device::Error::Disconnected);
                }
                let handle = pending.into_handle();
                return Ok(handle.wait_for_value().await);
            }
            tracing::debug!("timed out waiting for packet ack ({i})");
            if Instant::now() >= deadline {
                break;
            }
        }

        Err(device::Error::ActionTimedOut {
            action: "resending packet until ack received",
        })
    }

    /// Sends a request packet, and then receives up to `limit` responses via `on_receive` until
    /// `on_receive` returns `false`. Returns `ActionTimedOut` if the whole exchange does not
    /// complete within five seconds; partially received data is discarded.
    #[tracing::instrument(level = "warn", skip(self, on_receive))]
    pub async fn send_with_multi_response(
        &self,
        packet: &packet::Outbound,
        mut on_receive: impl FnMut(packet::Inbound) -> bool,
        limit: usize,
    ) -> device::Result<()> {
        let queue_key = packet.command;
        let deadline = Instant::now() + MULTI_RESPONSE_TIMEOUT;
        let generation = self.session.generation();
        let mut pending = MultiPending::new(
            self.packet_queues.clone(),
            queue_key,
            self.packet_queues.add_n(queue_key, limit),
        );

        if pending.is_cancelled() {
            return Err(device::Error::Disconnected);
        }
        if let Some(first) = pending.front() {
            let start_timeout = (Instant::now() + WAIT_FOR_START_TIMEOUT).min(deadline);
            if timeout_at(start_timeout, first.wait_for_start())
                .await
                .is_err()
            {
                return Err(device::Error::ActionTimedOut {
                    action: "waiting for queue turn",
                });
            }
        }

        tracing::debug!("starting multi response");
        self.session
            .write_if_current(&packet.bytes(self.checksum_kind), generation)
            .await?;

        loop {
            if pending.is_cancelled() {
                return Err(device::Error::Disconnected);
            }
            let Some(handle) = pending.front() else {
                return Ok(());
            };
            let response_timeout = (Instant::now() + MULTI_RESPONSE_PACKET_TIMEOUT).min(deadline);
            if timeout_at(response_timeout, handle.wait_for_end())
                .await
                .is_err()
            {
                tracing::debug!("timed out waiting for multi response");
                // A command that returns nothing still has to terminate the exchange; silence is
                // a timeout, never an empty success. An empty list is a parsed packet whose
                // handler returns false.
                return Err(device::Error::ActionTimedOut {
                    action: "multi response",
                });
            }
            if pending.is_cancelled() {
                return Err(device::Error::Disconnected);
            }
            let handle = pending.pop_front().expect("front handle must exist");
            tracing::trace!("got multi response packet");
            let packet = handle.wait_for_value().await;
            if !on_receive(packet) {
                pending.cancel_all();
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(device::Error::ActionTimedOut {
                    action: "multi response",
                });
            }
        }
    }

    pub async fn send_without_response(&self, packet: &packet::Outbound) -> device::Result<()> {
        self.session
            .write(&packet.bytes(self.checksum_kind))
            .await?;
        Ok(())
    }
}

async fn run_watchdog(controller: Weak<PacketIOController>) -> Result<(), String> {
    let Some(controller) = controller.upgrade() else {
        return Ok(());
    };
    let session = controller.session.clone();
    let mut status = session.status_receiver();
    loop {
        let mut reconnected = false;
        while !session.status().is_connected() {
            if matches!(
                session.status(),
                SessionStatus::Lost { .. } | SessionStatus::Closed
            ) {
                return Ok(());
            }
            if status.changed().await.is_err() {
                return Ok(());
            }
            reconnected = true;
        }
        if reconnected {
            // Silence before the reconnect says nothing about the new transport; give it a full
            // watchdog interval before probing again.
            *controller
                .last_inbound
                .lock()
                .expect("last inbound mutex should not be poisoned") = Instant::now();
        }

        let idle = controller
            .last_inbound
            .lock()
            .expect("last inbound mutex should not be poisoned")
            .elapsed();
        if idle < SILENCE_TIMEOUT {
            tokio::time::sleep(SILENCE_TIMEOUT - idle).await;
            continue;
        }

        tracing::debug!("silence watchdog probing the device");
        let mut failures = 0;
        for attempt in 0..2 {
            if controller
                .send_with_response(&packet::outbound::RequestState.to_packet())
                .await
                .is_ok()
            {
                failures = 0;
                break;
            }
            failures += 1;
            if attempt == 0 {
                tokio::time::sleep(WATCHDOG_RETRY_DELAY).await;
            }
        }
        if failures >= 2 {
            session.report_unhealthy(SessionFailureReason::SilenceWatchdogFailed);
            // Keep the watchdog alive: if the session recovers, a second silence must be detected.
        }
    }
}

struct PendingCommand {
    packet_queues: Arc<MultiQueue<Command, packet::Inbound>>,
    queue_key: Command,
    handle: Option<MultiQueueHandle<packet::Inbound>>,
}

impl PendingCommand {
    fn new(packet_queues: Arc<MultiQueue<Command, packet::Inbound>>, queue_key: Command) -> Self {
        let handle = packet_queues.add(queue_key);
        Self {
            packet_queues,
            queue_key,
            handle: Some(handle),
        }
    }

    fn handle(&self) -> &MultiQueueHandle<packet::Inbound> {
        self.handle.as_ref().expect("pending handle must exist")
    }

    fn is_cancelled(&self) -> bool {
        self.handle().is_cancelled()
    }

    fn into_handle(mut self) -> MultiQueueHandle<packet::Inbound> {
        self.handle.take().expect("pending handle must exist")
    }
}

impl Drop for PendingCommand {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            self.packet_queues.cancel(&self.queue_key, handle);
        }
    }
}

struct MultiPending {
    packet_queues: Arc<MultiQueue<Command, packet::Inbound>>,
    queue_key: Command,
    handles: VecDeque<MultiQueueHandle<packet::Inbound>>,
}

impl MultiPending {
    fn new(
        packet_queues: Arc<MultiQueue<Command, packet::Inbound>>,
        queue_key: Command,
        handles: VecDeque<MultiQueueHandle<packet::Inbound>>,
    ) -> Self {
        Self {
            packet_queues,
            queue_key,
            handles,
        }
    }

    fn front(&self) -> Option<&MultiQueueHandle<packet::Inbound>> {
        self.handles.front()
    }

    fn pop_front(&mut self) -> Option<MultiQueueHandle<packet::Inbound>> {
        self.handles.pop_front()
    }

    fn is_cancelled(&self) -> bool {
        self.handles.iter().any(MultiQueueHandle::is_cancelled)
    }

    fn cancel_all(&mut self) {
        for handle in self.handles.drain(..) {
            self.packet_queues.cancel(&self.queue_key, handle);
        }
    }
}

impl Drop for MultiPending {
    fn drop(&mut self) {
        self.cancel_all();
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use macaddr::MacAddr6;
    use tokio::sync::{mpsc, watch};
    use uuid::Uuid;

    use crate::{
        api::connection::{
            ConnectionDescriptor, ConnectionSession, ConnectionSessionConfig, ConnectionStatus,
            RfcommBackend, RfcommConnection, RfcommServiceSelectionStrategy, SessionStatus,
            test_stub::StubRfcommConnection,
        },
        devices::soundcore::common::packet::{
            self,
            outbound::{SetAmbientSoundModeCycle, SetSoundModes, ToPacket},
        },
    };

    use super::*;

    struct LossyConnection {
        status: watch::Sender<ConnectionStatus>,
        inbound: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
        outbound: mpsc::Sender<Vec<u8>>,
    }

    #[async_trait]
    impl RfcommConnection for LossyConnection {
        async fn write(&self, data: &[u8]) -> crate::api::connection::Result<()> {
            self.outbound.send(data.to_vec()).await.unwrap();
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

    struct SilentConnection {
        status: watch::Sender<ConnectionStatus>,
        inbound: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
        outbound: mpsc::Sender<Vec<u8>>,
    }

    #[async_trait]
    impl RfcommConnection for SilentConnection {
        async fn write(&self, data: &[u8]) -> crate::api::connection::Result<()> {
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

    #[derive(Default)]
    struct SilentBackend;

    #[async_trait]
    impl RfcommBackend for SilentBackend {
        async fn devices(&self) -> crate::api::connection::Result<HashSet<ConnectionDescriptor>> {
            Ok(HashSet::new())
        }

        async fn connect(
            &self,
            _mac_address: MacAddr6,
            _service_selection_strategy: RfcommServiceSelectionStrategy,
        ) -> crate::api::connection::Result<Arc<dyn RfcommConnection + Send + Sync>> {
            // Keep a reconnect observable instead of racing straight back to Connected.
            tokio::time::sleep(Duration::from_millis(100)).await;
            let (inbound, inbound_receiver) = mpsc::channel(10);
            std::mem::forget(inbound);
            let (outbound, mut outbound_receiver) = mpsc::channel(10);
            tokio::spawn(async move { while outbound_receiver.recv().await.is_some() {} });
            Ok(Arc::new(SilentConnection {
                status: watch::channel(ConnectionStatus::Connected).0,
                inbound: Mutex::new(Some(inbound_receiver)),
                outbound,
            }))
        }
    }

    #[tokio::test(start_paused = true)]
    async fn send_multiple() {
        let (connection, sender, _receiver) = StubRfcommConnection::new();
        let controller = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let handle1 = tokio::spawn({
            let controller = controller.clone();
            async move {
                controller
                    .send_with_response(&SetSoundModes::default().to_packet())
                    .await
                    .expect("should receive ack");
            }
        });
        let handle2 = tokio::spawn({
            let controller = controller.clone();
            async move {
                controller
                    .send_with_response(&SetSoundModes::default().to_packet())
                    .await
                    .expect("should receive ack");
            }
        });
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(!handle1.is_finished());
        assert!(!handle2.is_finished());

        sender
            .send(vec![
                0x09, 0xff, 0x00, 0x00, 0x01, 0x06, 0x81, 0x0a, 0x00, 0x9a,
            ])
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(handle1.is_finished());
        assert!(!handle2.is_finished());

        sender
            .send(vec![
                0x09, 0xff, 0x00, 0x00, 0x01, 0x06, 0x81, 0x0a, 0x00, 0x9a,
            ])
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(handle2.is_finished());
    }

    #[tokio::test(start_paused = true)]
    async fn out_of_order_responses() {
        let (connection, sender, _receiver) = StubRfcommConnection::new();
        let controller = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let set_cycle_packet = SetAmbientSoundModeCycle::default().to_packet();
        let set_cycle_ack = set_cycle_packet.ack();
        let handle1 = tokio::spawn({
            let controller = controller.clone();
            async move {
                controller
                    .send_with_response(&set_cycle_packet)
                    .await
                    .expect("should receive ack");
            }
        });
        let handle2 = tokio::spawn({
            let controller = controller.clone();
            async move {
                controller
                    .send_with_response(&SetSoundModes::default().to_packet())
                    .await
                    .expect("should receive ack");
            }
        });
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(!handle1.is_finished());
        assert!(!handle2.is_finished());

        sender
            .send(vec![
                0x09, 0xff, 0x00, 0x00, 0x01, 0x06, 0x81, 0x0a, 0x00, 0x9a,
            ])
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(!handle1.is_finished());
        assert!(handle2.is_finished());

        sender
            .send(set_cycle_ack.bytes_with_checksum())
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
        assert!(handle1.is_finished());
    }

    #[tokio::test(start_paused = true)]
    async fn fragmented_packet() {
        let (connection, sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let keepalive = sender.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            for byte in [0x09, 0xff, 0x00, 0x00, 0x01, 0x06, 0x81, 0x0a, 0x00, 0x9a] {
                sender.send(vec![byte]).await.unwrap();
            }
        });
        let _keepalive = keepalive;
        packet_io
            .send_with_response(&packet::outbound::SetSoundModes::default().to_packet())
            .await
            .expect("we should receive the fragmented ACK packet");
    }

    #[tokio::test(start_paused = true)]
    async fn merged_packets() {
        let (connection, sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let set_sound_modes: packet::Outbound =
            packet::outbound::SetSoundModes::default().to_packet();
        let set_sound_modes_ack = set_sound_modes.ack();
        let set_ambient_sound_mode_cycle: packet::Outbound =
            packet::outbound::SetAmbientSoundModeCycle::default().to_packet();
        let set_ambient_sound_mode_cycle_ack = set_ambient_sound_mode_cycle.ack();

        let keepalive = sender.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            let mut data = set_sound_modes_ack.bytes_with_checksum();
            data.extend_from_slice(&set_ambient_sound_mode_cycle_ack.bytes_with_checksum());
            data.extend_from_slice(&set_sound_modes_ack.bytes_with_checksum());
            sender.send(data).await.unwrap();
        });
        let _keepalive = keepalive;

        let (
            set_sound_modes_result_1,
            set_ambient_sound_mode_cycle_result,
            set_sound_modes_result_2,
        ) = tokio::join!(
            packet_io.send_with_response(&set_sound_modes),
            packet_io.send_with_response(&set_ambient_sound_mode_cycle),
            packet_io.send_with_response(&set_sound_modes),
        );
        set_sound_modes_result_1.expect("first set sound modes ack should be received");
        set_ambient_sound_mode_cycle_result
            .expect("set ambient sound mode cycle ack should be received");
        set_sound_modes_result_2.expect("second set sound modes ack should be received");
    }

    #[tokio::test(start_paused = true)]
    async fn garbage_data_recovery() {
        let (connection, sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let set_sound_modes: packet::Outbound =
            packet::outbound::SetSoundModes::default().to_packet();
        let set_sound_modes_ack = set_sound_modes.ack();

        let keepalive = sender.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            sender.send(vec![0; 100]).await.unwrap(); // garbage data
            // not enough time has passed to recover
            sender
                .send(set_sound_modes_ack.bytes_with_checksum())
                .await
                .unwrap();
        });
        let _keepalive = keepalive;

        packet_io
            .send_with_response(&set_sound_modes)
            .await
            .expect("we should recover from garbage data being sent and receive the ack");
    }

    #[tokio::test(start_paused = true)]
    async fn an_ack_timeout_fails_the_command_without_disconnecting() {
        let (connection, _sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let started = Instant::now();
        let result = packet_io
            .send_with_response(&SetSoundModes::default().to_packet())
            .await;
        assert!(matches!(result, Err(device::Error::ActionTimedOut { .. })));
        assert!(started.elapsed() >= Duration::from_secs(3));
        assert!(started.elapsed() <= Duration::from_secs(5));
        assert_eq!(
            SessionStatus::Connected,
            *packet_io.session_status().borrow()
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_timed_out_queue_head_does_not_poison_the_next_send() {
        let (connection, sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let first = tokio::spawn({
            let packet_io = packet_io.clone();
            async move {
                packet_io
                    .send_with_response(&SetSoundModes::default().to_packet())
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(1)).await;

        let second = packet_io
            .send_with_response(&SetSoundModes::default().to_packet())
            .await;
        assert!(matches!(second, Err(device::Error::ActionTimedOut { .. })));

        first.abort();
        let _ = first.await;

        let third = tokio::spawn({
            let packet_io = packet_io.clone();
            async move {
                packet_io
                    .send_with_response(&SetSoundModes::default().to_packet())
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(1)).await;
        sender
            .send(
                SetSoundModes::default()
                    .to_packet()
                    .ack()
                    .bytes_with_checksum(),
            )
            .await
            .unwrap();
        third
            .await
            .unwrap()
            .expect("the queue should work after a cancelled head");
    }

    #[tokio::test(start_paused = true)]
    async fn a_multi_response_timeout_is_an_error_and_the_queue_recovers() {
        let (connection, sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let request = packet::outbound::request_dual_connections_devices();
        let response_command = request.command;
        let responder = tokio::spawn({
            let sender = sender.clone();
            async move {
                tokio::time::sleep(Duration::from_millis(10)).await;
                sender
                    .send(
                        packet::Inbound::new(response_command, Vec::new())
                            .bytes(ChecksumKind::Suffix),
                    )
                    .await
                    .unwrap();
            }
        });
        let result = packet_io
            .send_with_multi_response(&request, |_| true, 3)
            .await;
        responder.await.unwrap();
        assert!(matches!(result, Err(device::Error::ActionTimedOut { .. })));

        let next = tokio::spawn({
            let packet_io = packet_io.clone();
            async move {
                packet_io
                    .send_with_response(&SetSoundModes::default().to_packet())
                    .await
            }
        });
        tokio::time::sleep(Duration::from_millis(1)).await;
        sender
            .send(
                SetSoundModes::default()
                    .to_packet()
                    .ack()
                    .bytes_with_checksum(),
            )
            .await
            .unwrap();
        next.await
            .unwrap()
            .expect("the queue should work after a multi response timeout");
    }

    #[tokio::test(start_paused = true)]
    async fn a_multi_response_with_no_packets_times_out_instead_of_succeeding() {
        let (connection, _sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let started = Instant::now();
        let request = packet::outbound::request_dual_connections_devices();
        let result = packet_io
            .send_with_multi_response(&request, |_| true, 3)
            .await;

        assert!(matches!(result, Err(device::Error::ActionTimedOut { .. })));
        assert!(started.elapsed() >= MULTI_RESPONSE_PACKET_TIMEOUT);
    }

    #[tokio::test(start_paused = true)]
    async fn the_watchdog_detects_a_second_silence_after_recovery() {
        let backend = Arc::new(SilentBackend);
        let session = Arc::new(
            ConnectionSession::connect(
                backend,
                MacAddr6::nil(),
                RfcommServiceSelectionStrategy::Constant(Uuid::nil()),
                ConnectionSessionConfig {
                    jitter_ratio: 0.0,
                    ..Default::default()
                },
            )
            .await
            .unwrap(),
        );
        let packet_io = Arc::new(
            PacketIOController::new_with_session(session.clone(), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );
        let observed = Arc::new(Mutex::new(Vec::<SessionStatus>::new()));
        let collected = observed.clone();
        let mut status = session.status_receiver();
        tokio::spawn(async move {
            loop {
                collected
                    .lock()
                    .unwrap()
                    .push(status.borrow_and_update().clone());
                if status.changed().await.is_err() {
                    break;
                }
            }
        });
        packet_io.start_watchdog();

        let deadline = Instant::now() + Duration::from_secs(120);
        loop {
            let recoveries = observed
                .lock()
                .unwrap()
                .iter()
                .filter(|status| matches!(status, SessionStatus::Recovering { .. }))
                .count();
            if recoveries >= 2 {
                break;
            }
            assert!(Instant::now() < deadline, "{:?}", observed.lock().unwrap());
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    #[tokio::test(start_paused = true)]
    async fn transport_loss_cancels_a_pending_command() {
        let (inbound_sender, inbound_receiver) = mpsc::channel(10);
        let (outbound, mut outbound_receiver) = mpsc::channel(10);
        let status = watch::channel(ConnectionStatus::Connected).0;
        let connection = Arc::new(LossyConnection {
            status: status.clone(),
            inbound: Mutex::new(Some(inbound_receiver)),
            outbound,
        });
        let session = Arc::new(ConnectionSession::from_connection(connection));
        let packet_io = Arc::new(
            PacketIOController::new_with_session(session, ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );

        let send = tokio::spawn({
            let packet_io = packet_io.clone();
            async move {
                packet_io
                    .send_with_response(&SetSoundModes::default().to_packet())
                    .await
            }
        });
        outbound_receiver
            .recv()
            .await
            .expect("command should be written before the loss");
        status.send_replace(ConnectionStatus::Disconnected);

        let result = tokio::time::timeout(Duration::from_secs(1), send)
            .await
            .expect("a lost transport must not leave the command hanging")
            .unwrap();
        assert!(
            matches!(
                result,
                Err(device::Error::Disconnected) | Err(device::Error::ConnectionError { .. })
            ),
            "{result:?}"
        );
        drop(inbound_sender);
    }

    #[tokio::test(start_paused = true)]
    async fn the_silence_watchdog_probes_and_enters_recovering() {
        let (connection, _sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );
        packet_io.start_watchdog();

        let started = Instant::now();
        let mut status = packet_io.session_status();
        let mut changed = false;
        while !changed {
            if !status.borrow().is_connected() {
                changed = true;
                continue;
            }
            status.changed().await.unwrap();
        }
        assert!(started.elapsed() >= SILENCE_TIMEOUT);
        assert!(!status.borrow().is_connected());
    }

    #[tokio::test(start_paused = true)]
    async fn packets_reset_the_silence_watchdog() {
        let (connection, sender, _receiver) = StubRfcommConnection::new();
        let packet_io = Arc::new(
            PacketIOController::new(Arc::new(connection), ChecksumKind::Suffix)
                .await
                .unwrap()
                .0,
        );
        packet_io.start_watchdog();

        tokio::time::sleep(Duration::from_secs(20)).await;
        sender
            .send(vec![
                0x09, 0xff, 0x00, 0x00, 0x01, 0x06, 0x81, 0x0a, 0x00, 0x9a,
            ])
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_secs(20)).await;

        assert_eq!(
            SessionStatus::Connected,
            *packet_io.session_status().borrow()
        );
    }
}
