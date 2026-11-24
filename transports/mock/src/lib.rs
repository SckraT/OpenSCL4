//! In-memory transport for testing byte exchange without Bluetooth.

#![forbid(unsafe_code)]

use async_trait::async_trait;
use bytes::Bytes;
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, TransportError, TransportLink, TransportProvider,
};
use tokio::sync::{mpsc, watch};

/// A bounded loopback that delivers each sent chunk to the receiver.
pub struct MockTransport;

impl MockTransport {
    /// Opens a loopback link with a handle for simulating loss of this connection.
    pub fn connect_with_fault_injection(&self) -> (Box<dyn TransportLink>, MockDisconnect) {
        let (tx, rx) = mpsc::channel(16);
        let (trigger, disconnected) = watch::channel(false);
        (
            Box::new(LoopbackLink {
                tx,
                rx,
                disconnected,
            }),
            MockDisconnect(trigger),
        )
    }
}

#[async_trait]
impl TransportProvider for MockTransport {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let (link, _) = self.connect_with_fault_injection();
        Ok(link)
    }
}

/// Simulates unexpected loss of a single mock connection.
///
/// Dropping this handle leaves the connection open.
pub struct MockDisconnect(watch::Sender<bool>);

impl MockDisconnect {
    /// Disconnects the link and releases pending reads and sends.
    pub fn disconnect(&self) {
        self.0.send_replace(true);
    }
}

struct LoopbackLink {
    tx: mpsc::Sender<Bytes>,
    rx: mpsc::Receiver<Bytes>,
    disconnected: watch::Receiver<bool>,
}

impl TransportLink for LoopbackLink {
    fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
        (
            Box::new(LoopbackTx {
                tx: Some(self.tx),
                disconnected: self.disconnected.clone(),
            }),
            Box::new(LoopbackRx {
                rx: self.rx,
                disconnected: self.disconnected,
            }),
        )
    }
}

struct LoopbackTx {
    tx: Option<mpsc::Sender<Bytes>>,
    disconnected: watch::Receiver<bool>,
}

#[async_trait]
impl LinkTx for LoopbackTx {
    async fn send(&mut self, data: Bytes) -> Result<(), TransportError> {
        let tx = self.tx.as_ref().ok_or(TransportError::Closed)?;
        tokio::select! {
            biased;
            Ok(_) = self.disconnected.wait_for(|disconnected| *disconnected) => {
                Err(TransportError::Disconnected)
            }
            result = tx.send(data) => result.map_err(|_| TransportError::Closed),
        }
    }

    async fn close(&mut self) {
        self.tx = None;
    }
}

struct LoopbackRx {
    rx: mpsc::Receiver<Bytes>,
    disconnected: watch::Receiver<bool>,
}

#[async_trait]
impl LinkRx for LoopbackRx {
    async fn recv(&mut self) -> Result<Bytes, TransportError> {
        tokio::select! {
            biased;
            Ok(_) = self.disconnected.wait_for(|disconnected| *disconnected) => {
                Err(TransportError::Disconnected)
            }
            data = self.rx.recv() => data.ok_or(TransportError::Closed),
        }
    }
}
