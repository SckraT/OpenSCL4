#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_session::{
        ConnectionState, DispatchError, EqualizerSetting, ModelDriver, QuickPresetValues,
        SessionCommand, SessionHandle, SessionRunner, SoundMode, SoundModeSetting,
        UnidentifiedReason, UnidentifiedState,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualIncompatibleA3954, VirtualUnknownIdentity};

    /// The verified reads sent before a read-only view is published.
    const IDENTITY_READ: [[u8; 2]; 1] = [[0x01, 0x05]];
    const IDENTITY_AND_STATE_READS: [[u8; 2]; 2] = [[0x01, 0x05], [0x01, 0x01]];

    #[tokio::test(start_paused = true)]
    async fn unknown_identity_exposes_safe_metadata_and_rejects_every_write() {
        let (handle, commands) = start(VirtualUnknownIdentity);
        let view = handle.confirmed_view().await.expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: None,
                reason: UnidentifiedReason::UnknownIdentity,
            })
        );
        let debug = format!("{view:?}");
        assert!(!debug.contains("UNKNOWN"), "raw identity serial leaked");
        assert!(!debug.contains("9999"), "raw identity serial leaked");

        assert_eq!(
            handle
                .dispatch(SessionCommand::SetSoundMode(SoundModeSetting::NORMAL))
                .err(),
            Some(DispatchError::ReadOnly)
        );
        assert_eq!(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .err(),
            Some(DispatchError::ReadOnly)
        );
        assert_eq!(
            handle.dispatch(SessionCommand::SetGamingMode(true)).err(),
            Some(DispatchError::ReadOnly)
        );
        assert_eq!(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    equalizer: Some(curve()),
                    ..QuickPresetValues::default()
                }))
                .err(),
            Some(DispatchError::ReadOnly)
        );
        assert_eq!(handle.set_eq_live(curve()), Err(DispatchError::ReadOnly));

        handle.close().await.expect("close the read-only link");
        assert_eq!(
            *commands.lock().expect("request trace"),
            IDENTITY_READ,
            "an explicit unknown identity must not probe state, and writes must not reach the transport"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn incompatible_known_state_exposes_its_model_and_rejects_writes() {
        let (handle, commands) = start(VirtualIncompatibleA3954);
        let view = handle.confirmed_view().await.expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3954),
                reason: UnidentifiedReason::IncompatibleState,
            })
        );
        assert_eq!(
            handle
                .dispatch(SessionCommand::SetSoundMode(SoundModeSetting::NORMAL))
                .err(),
            Some(DispatchError::ReadOnly)
        );
        assert_eq!(handle.set_eq_live(curve()), Err(DispatchError::ReadOnly));

        handle.close().await.expect("close the read-only link");
        assert_eq!(
            *commands.lock().expect("request trace"),
            IDENTITY_AND_STATE_READS
        );
    }

    fn curve() -> EqualizerSetting {
        EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [0; 10],
        }
    }

    fn start<P>(provider: P) -> (SessionHandle, Arc<Mutex<Vec<[u8; 2]>>>)
    where
        P: TransportProvider,
    {
        let commands = Arc::new(Mutex::new(Vec::new()));
        let handle = SessionRunner::start(
            Recording {
                inner: provider,
                commands: Arc::clone(&commands),
            },
            ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
            },
        );
        (handle, commands)
    }

    struct Recording<P> {
        inner: P,
        commands: Arc<Mutex<Vec<[u8; 2]>>>,
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
                    commands: Arc::clone(&self.commands),
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
        commands: Arc<Mutex<Vec<[u8; 2]>>>,
    }

    #[async_trait]
    impl LinkTx for RecordingTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            if let Some(command) = request.get(5..7) {
                self.commands
                    .lock()
                    .expect("request trace")
                    .push([command[0], command[1]]);
            }
            self.inner.send(request).await
        }

        async fn close(&mut self) {
            self.inner.close().await;
        }
    }
}
