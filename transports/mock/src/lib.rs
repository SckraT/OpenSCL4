//! In-memory transport for testing byte exchange without Bluetooth.

#![forbid(unsafe_code)]

use async_trait::async_trait;
use bytes::Bytes;
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, TransportError, TransportLink, TransportProvider,
};
use tokio::sync::mpsc;

/// A bounded loopback that delivers each sent chunk to the receiver.
pub struct MockTransport;

#[async_trait]
impl TransportProvider for MockTransport {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let (tx, rx) = mpsc::channel(16);
        Ok(Box::new(LoopbackLink { tx, rx }))
    }
}

struct LoopbackLink {
    tx: mpsc::Sender<Bytes>,
    rx: mpsc::Receiver<Bytes>,
}

impl TransportLink for LoopbackLink {
    fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
        (
            Box::new(LoopbackTx(Some(self.tx))),
            Box::new(LoopbackRx(self.rx)),
        )
    }
}

struct LoopbackTx(Option<mpsc::Sender<Bytes>>);

#[async_trait]
impl LinkTx for LoopbackTx {
    async fn send(&mut self, data: Bytes) -> Result<(), TransportError> {
        let tx = self.0.as_ref().ok_or(TransportError::Closed)?;
        tx.send(data).await.map_err(|_| TransportError::Closed)
    }

    async fn close(&mut self) {
        self.0 = None;
    }
}

struct LoopbackRx(mpsc::Receiver<Bytes>);

#[async_trait]
impl LinkRx for LoopbackRx {
    async fn recv(&mut self) -> Result<Bytes, TransportError> {
        self.0.recv().await.ok_or(TransportError::Closed)
    }
}
