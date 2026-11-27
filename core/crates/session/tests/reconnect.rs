#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{
        future::pending,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_session::{
        CommandError, ConnectionState, DispatchError, IoStage, ReconnectPolicy, SessionCommand,
        SessionError, SessionHandle, SessionRunner, SoundModeSetting,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::VirtualSoundcoreDevice;
    use tokio::time::{Instant, sleep};

    #[tokio::test(start_paused = true)]
    async fn reconnects_after_release_without_replaying_accepted_commands() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let handle = start(&trace, Retry::Available, ReconnectPolicy::Internal);
        let original = handle.confirmed_view().await.expect("initial state");
        let ConnectionState::Ready(state) = original.connection else {
            panic!("expected initial ready state");
        };
        let command = SessionCommand::SetSoundMode(SoundModeSetting::NORMAL);
        let active = handle.dispatch(command).expect("accept the first write");
        let queued = handle.dispatch(command).expect("accept the queued write");
        let mut view = handle.subscribe();
        let reconnecting = view
            .wait_for(|view| matches!(view.connection, ConnectionState::Reconnecting { .. }))
            .await
            .expect("publish recovery");
        assert_eq!(
            reconnecting.connection,
            ConnectionState::Reconnecting {
                last_confirmed: state,
                reason: LOST,
            }
        );
        drop(reconnecting);
        assert!(matches!(
            handle.dispatch(command),
            Err(DispatchError::NotReady)
        ));
        assert_eq!(active.wait().await, Err(CommandError::Disconnected));
        assert_eq!(queued.wait().await, Err(CommandError::Disconnected));
        let recovered = handle.confirmed_view().await.expect("recovered state");
        assert!(recovered.generation > original.generation);
        assert_eq!(recovered.connection, ConnectionState::Ready(state));
        assert_eq!(
            *trace.lock().expect("transport trace"),
            ["open", "write", "close", "drop reader", "open"]
        );
        handle.close().await.expect("close recovered link");
    }

    #[tokio::test(start_paused = true)]
    async fn external_policy_reports_loss_without_another_attempt() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let handle = start(&trace, Retry::Available, ReconnectPolicy::External);
        lose_link(&handle).await;
        assert_eq!(handle.confirmed_view().await, Err(LOST));
        assert_eq!(
            *trace.lock().expect("transport trace"),
            ["open", "write", "close", "drop reader"]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn retries_stop_thirty_seconds_after_loss() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let handle = start(&trace, Retry::Unavailable, ReconnectPolicy::Internal);
        let lost_at = Instant::now();
        lose_link(&handle).await;
        assert_eq!(handle.confirmed_view().await, Err(LOST));
        assert_eq!(lost_at.elapsed(), Duration::from_secs(30));
        let len = trace.lock().expect("transport trace").len();
        assert!(len > 4, "recovery must make a new connection attempt");
        sleep(Duration::from_secs(30)).await;
        assert_eq!(trace.lock().expect("transport trace").len(), len);
    }

    #[tokio::test(start_paused = true)]
    async fn explicit_close_cancels_an_in_progress_reopen() {
        let trace = Arc::new(Mutex::new(Vec::new()));
        let handle = start(&trace, Retry::Pending, ReconnectPolicy::Internal);
        lose_link(&handle).await;
        sleep(Duration::from_secs(1)).await;
        assert_eq!(trace.lock().expect("transport trace").last(), Some(&"open"));
        handle.close().await.expect("close pending reconnect");
        assert_eq!(handle.confirmed_view().await, Err(SessionError::Closed));
        sleep(Duration::from_secs(30)).await;
        assert_eq!(
            *trace.lock().expect("transport trace"),
            ["open", "write", "close", "drop reader", "open"]
        );
    }

    const LOST: SessionError = SessionError::Transport {
        stage: IoStage::Send,
        error: TransportError::Disconnected,
    };

    async fn lose_link(handle: &SessionHandle) {
        handle.confirmed_view().await.expect("initial state");
        assert_eq!(
            handle
                .dispatch(SessionCommand::SetSoundMode(SoundModeSetting::NORMAL))
                .expect("accept a write")
                .wait()
                .await,
            Err(CommandError::Disconnected),
        );
    }

    #[derive(Clone, Copy)]
    enum Retry {
        Available,
        Unavailable,
        Pending,
    }

    fn start(trace: &Trace, retry: Retry, policy: ReconnectPolicy) -> SessionHandle {
        SessionRunner::start_with_policy(
            Provider {
                trace: Arc::clone(trace),
                retry,
            },
            ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
                service: None,
            },
            policy,
        )
    }

    type Trace = Arc<Mutex<Vec<&'static str>>>;

    struct Provider {
        trace: Trace,
        retry: Retry,
    }

    #[async_trait]
    impl TransportProvider for Provider {
        async fn connect(
            &self,
            target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            let first = {
                let mut trace = self.trace.lock().expect("transport trace");
                let first = trace.is_empty();
                trace.push("open");
                first
            };
            if !first {
                match self.retry {
                    Retry::Available => {}
                    Retry::Unavailable => return Err(TransportError::Unavailable),
                    Retry::Pending => return pending().await,
                }
            }
            let link = VirtualSoundcoreDevice.connect(target).await?;
            let (tx, rx) = link.split();
            Ok(Box::new(Link(
                Tx {
                    inner: tx,
                    trace: Arc::clone(&self.trace),
                    first,
                },
                Rx {
                    inner: rx,
                    trace: Arc::clone(&self.trace),
                },
            )))
        }
    }

    struct Link(Tx, Rx);

    impl TransportLink for Link {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            (Box::new(self.0), Box::new(self.1))
        }
    }

    struct Tx {
        inner: Box<dyn LinkTx>,
        trace: Trace,
        first: bool,
    }

    #[async_trait]
    impl LinkTx for Tx {
        async fn send(&mut self, data: Bytes) -> Result<(), TransportError> {
            if data.get(5..7) == Some(&[0x06, 0x81]) {
                self.trace.lock().expect("transport trace").push("write");
                if self.first {
                    return Err(TransportError::Disconnected);
                }
            }
            self.inner.send(data).await
        }

        async fn close(&mut self) {
            self.inner.close().await;
            self.trace.lock().expect("transport trace").push("close");
        }
    }

    struct Rx {
        inner: Box<dyn LinkRx>,
        trace: Trace,
    }

    #[async_trait]
    impl LinkRx for Rx {
        async fn recv(&mut self) -> Result<Bytes, TransportError> {
            self.inner.recv().await
        }
    }

    impl Drop for Rx {
        fn drop(&mut self) {
            self.trace
                .lock()
                .expect("transport trace")
                .push("drop reader");
        }
    }
}
