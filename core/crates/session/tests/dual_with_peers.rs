#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_models::{AutoPowerOff, AutoPowerOffSetting, EqualizerSetting, ModelError};
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState, DispatchError,
        ModelDriver, PendingCommand, SessionCommand, SessionHandle, SessionRunner,
        UnidentifiedReason,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualA3330, VirtualA3388, VirtualA3876, VirtualD1101};

    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];
    const GAMING_MODE_COMMAND: [u8; 2] = [0x01, 0x87];
    const LDAC_COMMAND: [u8; 2] = [0x01, 0xFF];
    const BUTTONS_ENABLED_COMMAND: [u8; 2] = [0x10, 0x94];

    const A3330_STATE: &[u8] = include_bytes!("../../../../core/data/a3330-paired-state.bin");
    const A3388_STATE: &[u8] = include_bytes!("../../../../core/data/a3388-paired-state.bin");
    const A3876_STATE: &[u8] = include_bytes!("../../../../core/data/a3876-paired-state.bin");
    const D1101_STATE: &[u8] = include_bytes!("../../../../core/data/d1101-paired-state.bin");

    /// The curve used by the legacy DRC example, extended with the invisible
    /// bands' defaults.
    const REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 120, 22, -120, -4, 16, 0, -120],
    };

    fn body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], STATE_COMMAND);
        &fixture[9..fixture.len() - 1]
    }

    #[tokio::test(start_paused = true)]
    async fn reads_each_paired_reference_state() {
        assert_a3330(VirtualA3330).await;
        assert_a3388(VirtualA3388).await;
        assert_a3876(VirtualA3876).await;
        assert_d1101(VirtualD1101).await;
    }

    #[tokio::test(start_paused = true)]
    async fn identifies_each_paired_model_from_the_state_alone() {
        for (fixture, model) in [
            (A3330_STATE, ModelDriver::A3330),
            (A3388_STATE, ModelDriver::A3388),
            (A3876_STATE, ModelDriver::A3876),
            (D1101_STATE, ModelDriver::D1101),
        ] {
            let (handle, frames) = start(NoIdentityState {
                body: body(fixture).to_vec(),
            });
            let state = ready(&handle).await;
            assert_eq!(state.model, model);
            handle.close().await.expect("close virtual link");
            assert_eq!(
                sent(&frames),
                vec![(IDENTITY_COMMAND, Vec::new()), (STATE_COMMAND, Vec::new())],
                "state-only discovery for {}",
                model.code()
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_paired_equalizer_writes() {
        for (provider, model, expected) in [
            (
                PairedProvider::A3330(VirtualA3330),
                ModelDriver::A3330,
                vec![
                    0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, 0x6d,
                    0x86, 0x70, 0x88, 0x7b, 0x66, 0x7e, 0x79, 0x78, 0x00,
                ],
            ),
            (
                PairedProvider::A3388(VirtualA3388),
                ModelDriver::A3388,
                vec![
                    0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, 0x3c,
                    0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00,
                ],
            ),
            (
                PairedProvider::A3876(VirtualA3876),
                ModelDriver::A3876,
                vec![
                    0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, 0x6d,
                    0x86, 0x70, 0x88, 0x7b, 0x66, 0x7e, 0x79, 0x78, 0x00,
                ],
            ),
            (
                PairedProvider::D1101(VirtualD1101),
                ModelDriver::D1101,
                vec![
                    0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, 0x3c,
                    0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00,
                ],
            ),
        ] {
            let (handle, frames) = start(provider);
            let initial = ready(&handle).await;
            assert_eq!(initial.model, model);
            handle
                .set_eq_live(REFERENCE_CURVE)
                .expect("accept live curve");
            let applied = wait_for_equalizer(&handle, REFERENCE_CURVE).await;
            assert_eq!(applied.equalizer, Some(REFERENCE_CURVE));
            handle.close().await.expect("close virtual link");

            let writes: Vec<_> = sent(&frames)
                .into_iter()
                .filter(|(command, _)| *command == model.equalizer_command())
                .collect();
            assert_eq!(writes.len(), 1, "{model:?} equalizer writes");
            assert_eq!(writes[0].1, expected, "{model:?} equalizer body");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_gaming_mode_and_auto_power_off_on_the_v20i() {
        let (handle, frames) = start(VirtualA3876);
        let initial = ready(&handle).await;
        assert_eq!(initial.gaming_mode, Some(false));
        assert_eq!(
            initial.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 2,
            })
        );

        let CommandOutcome::Applied(gaming) = wait(
            handle
                .dispatch(SessionCommand::SetGamingMode(true))
                .expect("queue gaming mode"),
        )
        .await
        .expect("confirm gaming mode");
        assert_eq!(gaming.gaming_mode, Some(true));

        let CommandOutcome::Applied(enabled) = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Enabled(3),
                ))
                .expect("queue auto power off enable"),
        )
        .await
        .expect("confirm auto power off enable");
        assert_eq!(
            enabled.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 3,
            })
        );

        let CommandOutcome::Applied(disabled) = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Disabled,
                ))
                .expect("queue auto power off disable"),
        )
        .await
        .expect("confirm auto power off disable");
        assert_eq!(
            disabled.auto_power_off,
            Some(AutoPowerOff {
                enabled: false,
                duration: 3,
            })
        );
        handle.close().await.expect("close virtual link");

        let writes = sent(&frames);
        assert!(
            writes.contains(&(GAMING_MODE_COMMAND, vec![1])),
            "gaming write {writes:?}"
        );
        assert!(
            writes.contains(&(AUTO_POWER_OFF_COMMAND, vec![1, 3])),
            "enable write {writes:?}"
        );
        assert!(
            writes.contains(&(AUTO_POWER_OFF_COMMAND, vec![0, 3])),
            "disable keeps the duration index {writes:?}"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_ldac_and_button_controls_on_the_c50i() {
        let (handle, frames) = start(VirtualD1101);
        let initial = ready(&handle).await;
        let paired = initial.paired.expect("paired readings");
        assert_eq!(paired.ldac, Some(false));
        assert_eq!(paired.buttons_enabled, Some(true));

        let outcome = wait(
            handle
                .dispatch(SessionCommand::SetLdac(true))
                .expect("queue LDAC"),
        )
        .await
        .expect("confirm LDAC");
        let CommandOutcome::Applied(ldac) = outcome;
        assert_eq!(ldac.paired.and_then(|paired| paired.ldac), Some(true));

        let outcome = wait(
            handle
                .dispatch(SessionCommand::SetButtonsEnabled(false))
                .expect("queue button controls"),
        )
        .await
        .expect("confirm button controls");
        let CommandOutcome::Applied(buttons) = outcome;
        assert_eq!(
            buttons.paired.and_then(|paired| paired.buttons_enabled),
            Some(false)
        );
        handle.close().await.expect("close virtual link");

        let writes = sent(&frames);
        assert!(writes.contains(&(LDAC_COMMAND, vec![1])), "LDAC write");
        assert!(
            writes.contains(&(BUTTONS_ENABLED_COMMAND, vec![1])),
            "inverted buttons write"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_button_controls_on_the_c30i_and_aeroclip() {
        for provider in [
            PairedProvider::A3330(VirtualA3330),
            PairedProvider::A3388(VirtualA3388),
        ] {
            let (handle, frames) = start(provider);
            ready(&handle).await;
            let outcome = wait(
                handle
                    .dispatch(SessionCommand::SetButtonsEnabled(false))
                    .expect("queue button controls"),
            )
            .await
            .expect("confirm button controls");
            let CommandOutcome::Applied(disabled) = outcome;
            assert_eq!(
                disabled.paired.and_then(|paired| paired.buttons_enabled),
                Some(false)
            );
            handle.close().await.expect("close virtual link");
            assert!(sent(&frames).contains(&(BUTTONS_ENABLED_COMMAND, vec![1])));
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_unsupported_paired_operations_before_transmission() {
        for (provider, model, commands) in [
            (
                PairedProvider::A3330(VirtualA3330),
                ModelDriver::A3330,
                vec![
                    (
                        SessionCommand::SetGamingMode(true),
                        ModelError::InvalidGamingModeWrite,
                    ),
                    (SessionCommand::SetLdac(true), ModelError::InvalidLdacWrite),
                    (
                        SessionCommand::SetAutoPowerOff(AutoPowerOffSetting::Disabled),
                        ModelError::InvalidAutoPowerOffWrite,
                    ),
                ],
            ),
            (
                PairedProvider::A3876(VirtualA3876),
                ModelDriver::A3876,
                vec![
                    (SessionCommand::SetLdac(true), ModelError::InvalidLdacWrite),
                    (
                        SessionCommand::SetButtonsEnabled(true),
                        ModelError::InvalidButtonsEnabledWrite,
                    ),
                ],
            ),
            (
                PairedProvider::D1101(VirtualD1101),
                ModelDriver::D1101,
                vec![(
                    SessionCommand::SetGamingMode(true),
                    ModelError::InvalidGamingModeWrite,
                )],
            ),
        ] {
            let (handle, frames) = start(provider);
            let initial = ready(&handle).await;
            assert_eq!(initial.model, model);
            for (command, expected) in commands {
                let result = wait(handle.dispatch(command).expect("queue rejection")).await;
                assert_eq!(result, Err(CommandError::Model(expected)), "{model:?}");
            }
            assert_eq!(ready(&handle).await, initial, "{model:?} state unchanged");
            handle.close().await.expect("close virtual link");
            assert_eq!(
                sent(&frames).len(),
                2,
                "{model:?} only sends the two startup reads"
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a_registered_model_with_an_incompatible_state_stays_read_only() {
        for (fixture, model) in [
            (A3330_STATE, ModelDriver::A3330),
            (A3876_STATE, ModelDriver::A3876),
        ] {
            let mut truncated = body(fixture).to_vec();
            truncated.pop();
            let (handle, _frames) = start(NoIdentityState { body: truncated });
            let view = tokio::time::timeout(Duration::from_secs(10), handle.confirmed_view())
                .await
                .expect("state deadline")
                .expect("read-only view");
            assert!(matches!(
                view.connection,
                ConnectionState::Unidentified(state)
                    if state.reason == UnidentifiedReason::UnknownIdentity
            ));
            assert_eq!(
                handle
                    .dispatch(SessionCommand::SetButtonsEnabled(false))
                    .err(),
                Some(DispatchError::ReadOnly),
                "{model:?}"
            );
            handle.close().await.expect("close read-only link");
        }
    }

    /// One of the four paired virtual devices, selected at runtime.
    enum PairedProvider {
        A3330(VirtualA3330),
        A3388(VirtualA3388),
        A3876(VirtualA3876),
        D1101(VirtualD1101),
    }

    #[async_trait]
    impl TransportProvider for PairedProvider {
        async fn connect(
            &self,
            target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            match self {
                Self::A3330(provider) => provider.connect(target).await,
                Self::A3388(provider) => provider.connect(target).await,
                Self::A3876(provider) => provider.connect(target).await,
                Self::D1101(provider) => provider.connect(target).await,
            }
        }
    }

    async fn assert_a3330<P: TransportProvider>(provider: P) {
        let (handle, frames) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3330);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(40));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("04.27".to_owned())
        );
        let paired = state.paired.expect("paired readings");
        assert!(paired.tws.connected);
        assert_eq!(paired.buttons_enabled, Some(true));
        assert_eq!(state.equalizer.map(|eq| eq.preset_id), Some(0xfefe));
        handle.close().await.expect("close virtual link");
        assert_eq!(
            sent(&frames),
            vec![(IDENTITY_COMMAND, Vec::new()), (STATE_COMMAND, Vec::new())]
        );
    }

    async fn assert_a3388<P: TransportProvider>(provider: P) {
        let (handle, _frames) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3388);
        assert_eq!(state.battery.left, Some(90));
        assert_eq!(state.battery.case, Some(80));
        let paired = state.paired.expect("paired readings");
        assert_eq!(paired.tws.host.code(), "Left");
        assert_eq!(paired.dual_connections_enabled, Some(true));
        handle.close().await.expect("close virtual link");
    }

    async fn assert_a3876<P: TransportProvider>(provider: P) {
        let (handle, _frames) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3876);
        assert_eq!(state.battery.left, Some(60));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, None);
        assert_eq!(state.gaming_mode, Some(false));
        let lights = state
            .paired
            .and_then(|paired| paired.colorful_lights)
            .expect("light settings");
        assert_eq!(lights.mode.code(), "Breathing");
        assert_eq!(lights.brightness, 4);
        assert_eq!(
            state.paired.and_then(|paired| paired.volume_balance),
            Some(0)
        );
        handle.close().await.expect("close virtual link");
    }

    async fn assert_d1101<P: TransportProvider>(provider: P) {
        let (handle, _frames) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::D1101);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(10));
        let paired = state.paired.expect("paired readings");
        assert_eq!(paired.ldac, Some(false));
        assert_eq!(paired.buttons_enabled, Some(true));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
        handle.close().await.expect("close virtual link");
    }

    async fn ready(handle: &SessionHandle) -> DeviceState {
        let view = tokio::time::timeout(Duration::from_secs(3), handle.confirmed_view())
            .await
            .expect("sync deadline")
            .expect("confirmed state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a confirmed state");
        };
        state
    }

    async fn wait(pending: PendingCommand) -> CommandResult {
        tokio::time::timeout(Duration::from_secs(10), pending.wait())
            .await
            .expect("command deadline")
    }

    async fn wait_for_equalizer(
        handle: &SessionHandle,
        requested: EqualizerSetting,
    ) -> DeviceState {
        let mut view = handle.subscribe();
        let snapshot = tokio::time::timeout(
            Duration::from_secs(3),
            view.wait_for(|view| {
                matches!(
                    view.connection,
                    ConnectionState::Ready(state) if state.equalizer == Some(requested)
                )
            }),
        )
        .await
        .expect("the confirmed curve deadline")
        .expect("confirmed curve");
        let ConnectionState::Ready(state) = snapshot.connection else {
            unreachable!("wait_for matched a ready state");
        };
        state
    }

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        }
    }

    /// The commands and bodies sent by the session, in order.
    fn sent(frames: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<([u8; 2], Vec<u8>)> {
        frames
            .lock()
            .expect("request trace")
            .iter()
            .map(|frame| {
                let command = frame[5..7].try_into().expect("command");
                let body = frame[9..frame.len() - 1].to_vec();
                (command, body)
            })
            .collect()
    }

    fn start<P: TransportProvider>(provider: P) -> (SessionHandle, Arc<Mutex<Vec<Vec<u8>>>>) {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = SessionRunner::start(
            Recording {
                inner: provider,
                frames: Arc::clone(&frames),
            },
            target(),
        );
        (handle, frames)
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

    /// A device that does not answer the identity probe and replays one state.
    struct NoIdentityState {
        body: Vec<u8>,
    }

    #[async_trait]
    impl TransportProvider for NoIdentityState {
        async fn connect(
            &self,
            _target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            Ok(Box::new(NoIdentityLink {
                decoder: openscl4_protocol::FrameDecoder::default(),
                body: Arc::new(self.body.clone()),
            }))
        }
    }

    struct NoIdentityLink {
        decoder: openscl4_protocol::FrameDecoder,
        body: Arc<Vec<u8>>,
    }

    impl TransportLink for NoIdentityLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            (
                Box::new(NoIdentityTx {
                    decoder: self.decoder,
                    body: self.body,
                    replies: tx,
                }),
                Box::new(NoIdentityRx(rx)),
            )
        }
    }

    struct NoIdentityTx {
        decoder: openscl4_protocol::FrameDecoder,
        body: Arc<Vec<u8>>,
        replies: tokio::sync::mpsc::UnboundedSender<Bytes>,
    }

    #[async_trait]
    impl LinkTx for NoIdentityTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.decoder.push(&request);
            while let Ok(Some(frame)) = self.decoder.next_frame() {
                if frame.command != STATE_COMMAND || !frame.body.is_empty() {
                    continue;
                }
                let reply = encode_frame(Direction::Inbound, STATE_COMMAND, &self.body)
                    .map_err(|_| TransportError::Closed)?;
                self.replies
                    .send(reply)
                    .map_err(|_| TransportError::Closed)?;
            }
            Ok(())
        }

        async fn close(&mut self) {}
    }

    struct NoIdentityRx(tokio::sync::mpsc::UnboundedReceiver<Bytes>);

    #[async_trait]
    impl LinkRx for NoIdentityRx {
        async fn recv(&mut self) -> Result<Bytes, TransportError> {
            self.0.recv().await.ok_or(TransportError::Closed)
        }
    }
}
