#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bytes::Bytes;
use openscl4_protocol::{Direction, encode_frame};
use openscl4_session::{SessionHandle, SessionRunner};
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink, TransportProvider,
};

/// Runs a session against a provider while recording every outbound frame.
#[derive(Clone)]
pub struct Recorder {
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl Recorder {
    pub fn new() -> Self {
        Self {
            frames: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn start<P: TransportProvider>(&self, provider: P) -> SessionHandle {
        SessionRunner::start(
            Recording {
                inner: provider,
                frames: Arc::clone(&self.frames),
            },
            target(),
        )
    }

    pub fn frames(&self) -> Vec<Vec<u8>> {
        self.frames.lock().expect("request trace").clone()
    }

    /// Every outbound command pair in send order.
    pub fn commands(&self) -> Vec<[u8; 2]> {
        self.frames
            .lock()
            .expect("request trace")
            .iter()
            .filter_map(|frame| command(frame))
            .collect()
    }

    /// The bodies sent for one command pair, in send order.
    pub fn bodies(&self, command: [u8; 2]) -> Vec<Vec<u8>> {
        self.frames
            .lock()
            .expect("request trace")
            .iter()
            .filter(|frame| frame.get(5..7) == Some(command.as_slice()))
            .map(|frame| frame[9..frame.len() - 1].to_vec())
            .collect()
    }

    /// The number of frames byte-equal to the expected encoded frame.
    pub fn count(&self, expected: &Bytes) -> usize {
        self.frames
            .lock()
            .expect("request trace")
            .iter()
            .filter(|frame| frame.as_slice() == expected.as_ref())
            .count()
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

pub fn target() -> ConnectTarget {
    ConnectTarget {
        mac: RedactedMac::from_octets([0; 6]),
    }
}

pub fn request(command: [u8; 2], body: &[u8]) -> Bytes {
    encode_frame(Direction::Outbound, command, body).expect("outbound frame")
}

pub fn inbound(command: [u8; 2], body: &[u8]) -> Bytes {
    encode_frame(Direction::Inbound, command, body).expect("inbound frame")
}

fn command(frame: &[u8]) -> Option<[u8; 2]> {
    frame.get(5..7)?.try_into().ok()
}

struct Recording<P> {
    inner: P,
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
}

#[async_trait]
impl<P: TransportProvider> TransportProvider for Recording<P> {
    async fn connect(
        &self,
        target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let (tx, rx) = self.inner.connect(target).await?.split();
        Ok(Box::new(RecordingLink {
            tx: RecordingTx {
                inner: tx,
                frames: Arc::clone(&self.frames),
            },
            rx,
        }))
    }
}

struct RecordingLink {
    tx: RecordingTx,
    rx: Box<dyn LinkRx>,
}

impl TransportLink for RecordingLink {
    fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
        (Box::new(self.tx), self.rx)
    }
}

struct RecordingTx {
    inner: Box<dyn LinkTx>,
    frames: Arc<Mutex<Vec<Vec<u8>>>>,
}

#[async_trait]
impl LinkTx for RecordingTx {
    async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
        self.frames
            .lock()
            .expect("request trace")
            .push(request.to_vec());
        self.inner.send(request).await
    }

    async fn close(&mut self) {
        self.inner.close().await;
    }
}
