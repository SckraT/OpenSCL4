use std::collections::VecDeque;

use async_trait::async_trait;
use bytes::Bytes;
use openscl4_transport::{
    ConnectTarget, LinkRx, LinkTx, TransportError, TransportLink, TransportProvider,
};
use tokio::sync::mpsc;

pub type Exchange = (Bytes, Option<Bytes>);

pub struct Replay(pub VecDeque<Exchange>);
struct ReplayLink(ReplayTx, ReplayRx);
struct ReplayTx {
    exchanges: VecDeque<Exchange>,
    replies: mpsc::UnboundedSender<Bytes>,
}
struct ReplayRx(mpsc::UnboundedReceiver<Bytes>);

#[async_trait]
impl TransportProvider for Replay {
    async fn connect(
        &self,
        _target: &ConnectTarget,
    ) -> Result<Box<dyn TransportLink>, TransportError> {
        let (replies, incoming) = mpsc::unbounded_channel();
        Ok(Box::new(ReplayLink(
            ReplayTx {
                exchanges: self.0.clone(),
                replies,
            },
            ReplayRx(incoming),
        )))
    }
}

impl TransportLink for ReplayLink {
    fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
        (Box::new(self.0), Box::new(self.1))
    }
}

#[async_trait]
impl LinkTx for ReplayTx {
    async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
        let (expected, reply) = self.exchanges.pop_front().expect("next request");
        assert_eq!(request, expected);
        if let Some(reply) = reply {
            self.replies
                .send(reply)
                .map_err(|_| TransportError::Closed)?;
        }
        Ok(())
    }

    async fn close(&mut self) {
        assert!(self.exchanges.is_empty(), "every captured exchange ran");
    }
}

#[async_trait]
impl LinkRx for ReplayRx {
    async fn recv(&mut self) -> Result<Bytes, TransportError> {
        self.0.recv().await.ok_or(TransportError::Closed)
    }
}
