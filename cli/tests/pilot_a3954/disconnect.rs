use std::{
    collections::HashSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    },
};

use async_trait::async_trait;
use bytes::Bytes;
use macaddr::MacAddr6;
use openscl4_models::SOUND_MODE_COMMAND;
use openscl4_protocol::FrameDecoder;
use openscl4_session::{
    CommandError, ConnectionState, IoStage, ModelDriver, SessionCommand, SessionError,
    SessionRunner, SoundModeSetting,
};
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink, TransportProvider,
};
use openscl4_transport_mock::{
    MockDisconnect, SharedVirtualSoundcoreDevice, VirtualSoundcoreDevice,
};
use openscq30_lib::{
    ConnectionBackends, DeviceModel, OpenSCQ30Session,
    connection::{
        self, ConnectionDescriptor, ConnectionStatus, RfcommBackend, RfcommConnection,
        RfcommServiceSelectionStrategy,
    },
    settings::{Setting, SettingId},
    storage::PairedDevice,
};
use tokio::sync::{Mutex as AsyncMutex, mpsc, watch};

#[tokio::test]
async fn reconnects_explicitly_through_legacy_after_a_disconnected_write() {
    let device = TestDevice(Arc::new(Device {
        radio: VirtualSoundcoreDevice::with_shared_state(),
        open_links: AtomicUsize::new(0),
        writes: AtomicUsize::new(0),
        legacy_reads: AtomicUsize::new(0),
        pilot_closed: AtomicBool::new(false),
    }));
    let mac = [0; 6];
    let pilot = SessionRunner::start(
        device.clone(),
        ConnectTarget {
            mac: RedactedMac::from_octets(mac),
            service: None,
        },
    );
    let initial = pilot
        .confirmed_view()
        .await
        .expect("read the initial state");
    let ConnectionState::Ready(initial) = initial.connection else {
        panic!("expected a confirmed pilot state");
    };
    assert_eq!(
        initial.sound_mode_setting(),
        Some(SoundModeSetting::NOISE_CANCELING)
    );

    let changed = pilot
        .dispatch(SessionCommand::SetSoundMode(SoundModeSetting::NORMAL))
        .expect("queue the change");
    let queued = pilot
        .dispatch(SessionCommand::SetSoundMode(
            SoundModeSetting::NOISE_CANCELING,
        ))
        .expect("queue a subsequent command");
    assert_eq!(changed.wait().await, Err(CommandError::Disconnected));
    assert_eq!(queued.wait().await, Err(CommandError::Disconnected));
    assert_eq!(
        pilot.close().await,
        Err(SessionError::Transport {
            stage: IoStage::Receive,
            error: TransportError::Disconnected,
        }),
    );
    assert!(device.0.pilot_closed.load(Relaxed));
    assert_eq!(
        device.0.open_links.load(Relaxed),
        0,
        "both pilot halves exited"
    );
    assert_eq!(device.0.writes.load(Relaxed), 1);
    assert_eq!(device.0.legacy_reads.load(Relaxed), 0);

    let mac_address = MacAddr6::from(mac);
    let legacy = OpenSCQ30Session::new_with_in_memory_db()
        .await
        .expect("open isolated legacy settings");
    legacy
        .pair(PairedDevice {
            mac_address,
            model: DeviceModel::SoundcoreA3954,
            is_demo: false,
        })
        .await
        .expect("register the virtual device");
    let headphones = legacy
        .connect_with_backends(&device, mac_address)
        .await
        .expect("explicitly connect through the former A3954 driver");
    let Some(Setting::Select { value, .. }) = headphones.setting(&SettingId::AmbientSoundMode)
    else {
        panic!("the legacy driver should expose the freshly read mode");
    };
    assert_eq!(value, "Normal");
    assert_eq!(device.0.legacy_reads.load(Relaxed), 1);
    assert_eq!(
        device.0.writes.load(Relaxed),
        1,
        "the disconnected write is never replayed"
    );
}

#[derive(Clone)]
struct TestDevice(Arc<Device>);

struct Device {
    radio: SharedVirtualSoundcoreDevice,
    open_links: AtomicUsize,
    writes: AtomicUsize,
    legacy_reads: AtomicUsize,
    pilot_closed: AtomicBool,
}

// A mock socket remains open until the last owner of its halves exits.
struct OpenLink(Arc<Device>);

impl Drop for OpenLink {
    fn drop(&mut self) {
        self.0.open_links.fetch_sub(1, Relaxed);
    }
}

impl TestDevice {
    fn open_link(&self) -> (Box<dyn TransportLink>, MockDisconnect, Arc<OpenLink>) {
        assert_eq!(
            self.0.open_links.fetch_add(1, Relaxed),
            0,
            "RFCOMM sockets overlapped"
        );
        let (link, disconnect) = self.0.radio.connect_with_fault_injection();
        (link, disconnect, Arc::new(OpenLink(Arc::clone(&self.0))))
    }
}

struct PilotLink {
    link: Box<dyn TransportLink>,
    disconnect: MockDisconnect,
    lifetime: Arc<OpenLink>,
}

struct PilotTx {
    inner: Box<dyn LinkTx>,
    disconnect: MockDisconnect,
    lifetime: Arc<OpenLink>,
}

struct PilotRx {
    inner: Box<dyn LinkRx>,
    _lifetime: Arc<OpenLink>,
}

#[async_trait]
impl TransportProvider for TestDevice {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let (link, disconnect, lifetime) = self.open_link();
        Ok(Box::new(PilotLink {
            link,
            disconnect,
            lifetime,
        }))
    }
}

impl TransportLink for PilotLink {
    fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
        let (tx, rx) = self.link.split();
        (
            Box::new(PilotTx {
                inner: tx,
                disconnect: self.disconnect,
                lifetime: Arc::clone(&self.lifetime),
            }),
            Box::new(PilotRx {
                inner: rx,
                _lifetime: self.lifetime,
            }),
        )
    }
}

#[async_trait]
impl LinkTx for PilotTx {
    async fn send(&mut self, data: Bytes) -> Result<(), TransportError> {
        let is_write = request_command(&data) == SOUND_MODE_COMMAND;
        if is_write {
            self.lifetime.0.writes.fetch_add(1, Relaxed);
        }
        self.inner.send(data).await?;
        if is_write {
            // The mock has applied the write and omitted the ACK at this point.
            self.disconnect.disconnect();
        }
        Ok(())
    }

    async fn close(&mut self) {
        self.inner.close().await;
        self.lifetime.0.pilot_closed.store(true, Relaxed);
    }
}

#[async_trait]
impl LinkRx for PilotRx {
    async fn recv(&mut self) -> Result<Bytes, TransportError> {
        self.inner.recv().await
    }
}

impl ConnectionBackends for TestDevice {
    type Rfcomm = Self;

    async fn rfcomm(&self) -> connection::Result<Self::Rfcomm> {
        Ok(self.clone())
    }
}

#[async_trait]
impl RfcommBackend for TestDevice {
    async fn devices(&self) -> connection::Result<HashSet<ConnectionDescriptor>> {
        Ok(HashSet::new())
    }

    async fn connect(
        &self,
        _mac_address: MacAddr6,
        _service: RfcommServiceSelectionStrategy,
    ) -> connection::Result<Arc<dyn RfcommConnection + Send + Sync>> {
        let (link, _disconnect, lifetime) = self.open_link();
        let (responses, reader) = mpsc::channel(16);
        let (status, _) = watch::channel(ConnectionStatus::Connected);
        Ok(Arc::new(LegacyLink {
            link: AsyncMutex::new(link.split()),
            responses,
            reader: Mutex::new(Some(reader)),
            status,
            lifetime,
        }))
    }
}

struct LegacyLink {
    link: AsyncMutex<(Box<dyn LinkTx>, Box<dyn LinkRx>)>,
    responses: mpsc::Sender<Vec<u8>>,
    reader: Mutex<Option<mpsc::Receiver<Vec<u8>>>>,
    status: watch::Sender<ConnectionStatus>,
    lifetime: Arc<OpenLink>,
}

#[async_trait]
impl RfcommConnection for LegacyLink {
    async fn write(&self, data: &[u8]) -> connection::Result<()> {
        let command = request_command(data);
        if command == SOUND_MODE_COMMAND {
            self.lifetime.0.writes.fetch_add(1, Relaxed);
        }
        let (tx, rx) = &mut *self.link.lock().await;
        tx.send(Bytes::copy_from_slice(data))
            .await
            .expect("legacy mock send");
        if command == SOUND_MODE_COMMAND {
            return Ok(());
        }
        if command == ModelDriver::A3954.state_command() {
            self.lifetime.0.legacy_reads.fetch_add(1, Relaxed);
        }
        let response = rx.recv().await.expect("legacy mock response");
        self.responses
            .send(response.to_vec())
            .await
            .expect("forward the legacy response");
        Ok(())
    }

    fn read_channel(&self) -> mpsc::Receiver<Vec<u8>> {
        self.reader
            .lock()
            .expect("legacy read lock")
            .take()
            .expect("one legacy reader")
    }

    fn connection_status(&self) -> watch::Receiver<ConnectionStatus> {
        self.status.subscribe()
    }
}

fn request_command(data: &[u8]) -> [u8; 2] {
    let mut decoder = FrameDecoder::default();
    decoder.push(data);
    decoder
        .next_frame()
        .expect("valid outbound frame")
        .expect("complete outbound frame")
        .command
}
