#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{future::pending, time::Duration};

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_session::{IoStage, SessionError, SessionEvent, SessionHandle, SessionRunner};
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use tokio::time::sleep;

    const IDENTITY: &[u8] = b"\x09\xff\0\0\x01\x01\x05\x24\x0004.2904.29A3954DEMO0000000\xb8";
    const STATE: &[u8] = include_bytes!("../../../data/a3954-pilot-state.bin");
    type Reading = Result<(Duration, &'static [u8]), TransportError>;

    #[tokio::test(start_paused = true)]
    async fn accepts_reply_before_deadline() {
        let handle = start(Ok((Duration::from_millis(1499), IDENTITY)), false);
        handle.confirmed_view().await.expect("confirm state");
        handle.close().await.expect("close the link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_ready_reply_at_deadline() {
        let handle = start(Ok((Duration::from_millis(1500), IDENTITY)), false);
        assert_eq!(
            handle.confirmed_view().await,
            Err(SessionError::Timeout(IoStage::Receive))
        );
    }

    #[tokio::test(start_paused = true)]
    async fn keeps_disconnect_as_the_primary_error_when_close_times_out() {
        let handle = start(Err(TransportError::Disconnected), true);
        let mut events = handle.events();
        let disconnected = SessionError::Transport {
            stage: IoStage::Receive,
            error: TransportError::Disconnected,
        };
        assert_eq!(handle.confirmed_view().await, Err(disconnected));
        assert_eq!(handle.close().await, Err(disconnected));
        let mut cleanup_failure = None;
        while let Ok(event) = events.recv().await {
            if let SessionEvent::CleanupFailed(error) = event {
                cleanup_failure = Some(error);
                break;
            }
        }
        assert_eq!(cleanup_failure, Some(SessionError::Timeout(IoStage::Close)));
    }

    #[tokio::test(start_paused = true)]
    async fn reports_close_timeout_as_primary_when_no_earlier_operation_failed() {
        let handle = start(Ok((Duration::ZERO, IDENTITY)), true);
        handle.confirmed_view().await.expect("confirm state");
        assert_eq!(
            handle.close().await,
            Err(SessionError::Timeout(IoStage::Close))
        );
    }

    fn start(identity: Reading, stall_close: bool) -> SessionHandle {
        SessionRunner::start(
            ScriptedProvider {
                identity,
                stall_close,
            },
            ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
            },
        )
    }

    struct ScriptedProvider {
        identity: Reading,
        stall_close: bool,
    }

    #[async_trait]
    impl TransportProvider for ScriptedProvider {
        async fn connect(
            &self,
            _target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            Ok(Box::new(ScriptedLink {
                tx: ScriptedTx {
                    stall_close: self.stall_close,
                },
                rx: ScriptedRx([self.identity, Ok((Duration::ZERO, STATE))].into_iter()),
            }))
        }
    }

    struct ScriptedLink {
        tx: ScriptedTx,
        rx: ScriptedRx,
    }

    impl TransportLink for ScriptedLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            (Box::new(self.tx), Box::new(self.rx))
        }
    }

    struct ScriptedTx {
        stall_close: bool,
    }

    #[async_trait]
    impl LinkTx for ScriptedTx {
        async fn send(&mut self, _data: Bytes) -> Result<(), TransportError> {
            Ok(())
        }

        async fn close(&mut self) {
            if self.stall_close {
                pending::<()>().await;
            }
        }
    }

    struct ScriptedRx(std::array::IntoIter<Reading, 2>);

    #[async_trait]
    impl LinkRx for ScriptedRx {
        async fn recv(&mut self) -> Result<Bytes, TransportError> {
            let Some(reading) = self.0.next() else {
                return pending().await;
            };
            let (delay, frame) = reading?;
            sleep(delay).await;
            Ok(Bytes::from_static(frame))
        }
    }
}
