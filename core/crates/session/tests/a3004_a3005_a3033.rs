#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_models::{AutoPowerOff, AutoPowerOffSetting, ModelError};
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState,
        EqualizerSetting, ModelDriver, PendingCommand, QuickPresetValues, SessionCommand,
        SessionHandle, SessionRunner, SoundMode,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualA3004, VirtualA3005, VirtualA3033};

    use super::capture_replay::Replay;

    // Reference wire opcodes from the legacy fixtures and packet builders.
    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const EQUALIZER_WITH_DRC_COMMAND: [u8; 2] = [0x02, 0x83];
    const EIGHT_BAND_EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x81];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];
    const DUAL_CONNECTIONS_COMMAND: [u8; 2] = [0x0b, 0x84];
    const WEARING_DETECTION_COMMAND: [u8; 2] = [0x01, 0x81];

    const A3004_STATE: &[u8] = include_bytes!("../../../data/a3004-pilot-state.bin");
    const A3005_STATE: &[u8] = include_bytes!("../../../data/a3005-pilot-state.bin");
    const A3033_STATE: &[u8] = include_bytes!("../../../data/a3033-pilot-state.bin");

    /// The legacy `set_equalizer_with_drc` curve with the family defaults in the
    /// last two bands, as published in
    /// `core/data/a3004-a3005-a3033-reference.json`.
    const DRC_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 120, 22, -120, -4, 16, 0, -120],
    };
    /// The ten-band write body from the legacy DRC builder: curve, then DRC.
    const DRC_WRITE_BODY: [u8; 22] = [
        0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xf0, 0x8e, 0x00, 0x74, 0x88, 0x78, 0x00, 0x6d, 0x86, 0x70,
        0x88, 0x7b, 0x66, 0x7e, 0x79, 0x78, 0x00,
    ];
    /// The legacy `set_equalizer` example curve and its eight-band body.
    const EIGHT_BAND_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0],
    };
    const EIGHT_BAND_WRITE_BODY: [u8; 10] =
        [0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88];

    #[tokio::test(start_paused = true)]
    async fn reads_the_reference_state_for_each_public_code() {
        let (handle, frames) = start(VirtualA3004);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3004);
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.battery.device, Some(80));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("00.26".to_owned())
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [0; 10],
            })
        );
        assert_eq!(state.auto_power_off, None);
        handle.close().await.expect("close virtual link");
        assert_eq!(
            commands(&frames),
            vec![[0x01, 0x05], [0x01, 0x01]],
            "state-only discovery sends one identity probe and one state read"
        );

        let (handle, _) = start(VirtualA3005);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3005);
        assert_eq!(state.sound_mode, None);
        assert_eq!(state.battery.device, Some(80));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("01.21".to_owned())
        );
        assert_eq!(state.dual_connections, Some(true));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
        handle.close().await.expect("close virtual link");

        let (handle, _) = start(VirtualA3033);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3033);
        assert_eq!(state.sound_mode, None);
        assert_eq!(state.battery.device, Some(80));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("01.21".to_owned())
        );
        assert_eq!(state.wearing_detection, Some(true));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3004_ambient_mode_cycle_from_preset_replies() {
        let initial = state_body(A3004_STATE).to_vec();
        let initial_reply = inbound(&initial);
        let normal = state_with(&initial, 35, &[2]);
        let normal_reply = inbound(&normal);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(initial_reply)),
            (request(SOUND_MODE_COMMAND, &[2, 0x3f, 0x02, 0xff]), None),
            (request(STATE_COMMAND, &[]), Some(normal_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(normal_reply)),
            (request(SOUND_MODE_COMMAND, &[0, 0x3f, 0x02, 0xff]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let initial_state = ready(&handle).await;
        assert_eq!(initial_state.sound_mode, Some(SoundMode::NoiseCanceling));

        let CommandOutcome::Applied(changed) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue normal"),
        )
        .await
        .expect("confirm normal");
        assert_eq!(changed.sound_mode, Some(SoundMode::Normal));
        assert_eq!(changed.equalizer, initial_state.equalizer);
        assert_eq!(
            changed.a3004_sound_mode_write(SoundMode::Normal),
            Ok([2, 0x3f, 0x02, 0xff])
        );

        let CommandOutcome::Applied(restored) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(
                    SoundMode::NoiseCanceling,
                ))
                .expect("queue noise canceling"),
        )
        .await
        .expect("confirm noise canceling");
        assert_eq!(restored.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(ready(&handle).await, restored);
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3004_ten_band_curve_from_preset_replies() {
        let initial = state_body(A3004_STATE).to_vec();
        let after = state_with_ten_band_curve(&initial);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(EQUALIZER_WITH_DRC_COMMAND, &DRC_WRITE_BODY), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&after))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let initial_state = ready(&handle).await;
        assert_eq!(
            initial_state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [0; 10],
            })
        );

        let CommandOutcome::Applied(applied) = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    equalizer: Some(DRC_CURVE),
                    ..Default::default()
                }))
                .expect("queue the curve"),
        )
        .await
        .expect("confirm the curve");
        assert_eq!(applied.equalizer, Some(DRC_CURVE));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_a3005_auto_power_off_and_dual_connections() {
        let initial = state_body(A3005_STATE).to_vec();
        let auto_off = state_with(&initial, 46, &[0, 1]);
        let auto_120 = state_with(&initial, 46, &[1, 3]);
        let dual_off = state_with(&auto_120, 41, &[0]);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(AUTO_POWER_OFF_COMMAND, &[0, 1]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&auto_off))),
            (request(STATE_COMMAND, &[]), Some(inbound(&auto_off))),
            (request(AUTO_POWER_OFF_COMMAND, &[1, 3]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&auto_120))),
            (request(STATE_COMMAND, &[]), Some(inbound(&auto_120))),
            (request(DUAL_CONNECTIONS_COMMAND, &[0]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&dual_off))),
            (request(STATE_COMMAND, &[]), Some(inbound(&dual_off))),
            (request(DUAL_CONNECTIONS_COMMAND, &[1]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let initial_state = ready(&handle).await;
        assert_eq!(initial_state.dual_connections, Some(true));
        assert_eq!(
            initial_state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );

        let CommandOutcome::Applied(disabled) = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Disabled,
                ))
                .expect("queue disable"),
        )
        .await
        .expect("confirm disable");
        assert_eq!(
            disabled.auto_power_off,
            Some(AutoPowerOff {
                enabled: false,
                duration: 1,
            })
        );

        let CommandOutcome::Applied(enabled) = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Enabled(3),
                ))
                .expect("queue enable"),
        )
        .await
        .expect("confirm enable");
        assert_eq!(
            enabled.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 3,
            })
        );

        let CommandOutcome::Applied(dual_disabled) = wait(
            handle
                .dispatch(SessionCommand::SetDualConnections(false))
                .expect("queue dual connections off"),
        )
        .await
        .expect("confirm dual connections off");
        assert_eq!(dual_disabled.dual_connections, Some(false));

        let CommandOutcome::Applied(dual_enabled) = wait(
            handle
                .dispatch(SessionCommand::SetDualConnections(true))
                .expect("queue dual connections on"),
        )
        .await
        .expect("confirm dual connections on");
        assert_eq!(dual_enabled.dual_connections, Some(true));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3005_ten_band_curve_from_preset_replies() {
        let initial = state_body(A3005_STATE).to_vec();
        let after = state_with_ten_band_curve(&initial);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(EQUALIZER_WITH_DRC_COMMAND, &DRC_WRITE_BODY), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&after))),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;

        let CommandOutcome::Applied(applied) = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    equalizer: Some(DRC_CURVE),
                    ..Default::default()
                }))
                .expect("queue the curve"),
        )
        .await
        .expect("confirm the curve");
        assert_eq!(applied.equalizer, Some(DRC_CURVE));
        assert_eq!(applied.dual_connections, Some(true));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_a3033_wearing_detection_and_the_eight_band_curve() {
        let initial = state_body(A3033_STATE).to_vec();
        let wearing_off = state_with(&initial, 33, &[0]);
        let eight_band = state_with_eight_band_curve(&initial);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(WEARING_DETECTION_COMMAND, &[0]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&wearing_off))),
            (request(STATE_COMMAND, &[]), Some(inbound(&wearing_off))),
            (request(WEARING_DETECTION_COMMAND, &[1]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (
                request(EIGHT_BAND_EQUALIZER_COMMAND, &EIGHT_BAND_WRITE_BODY),
                None,
            ),
            (request(STATE_COMMAND, &[]), Some(inbound(&eight_band))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let initial_state = ready(&handle).await;
        assert_eq!(initial_state.wearing_detection, Some(true));

        let CommandOutcome::Applied(disabled) = wait(
            handle
                .dispatch(SessionCommand::SetWearingDetection(false))
                .expect("queue wearing detection off"),
        )
        .await
        .expect("confirm wearing detection off");
        assert_eq!(disabled.wearing_detection, Some(false));

        let CommandOutcome::Applied(enabled) = wait(
            handle
                .dispatch(SessionCommand::SetWearingDetection(true))
                .expect("queue wearing detection on"),
        )
        .await
        .expect("confirm wearing detection on");
        assert_eq!(enabled.wearing_detection, Some(true));

        let CommandOutcome::Applied(applied) = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    equalizer: Some(EIGHT_BAND_CURVE),
                    ..Default::default()
                }))
                .expect("queue the curve"),
        )
        .await
        .expect("confirm the curve");
        assert_eq!(applied.equalizer, Some(EIGHT_BAND_CURVE));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_writes_that_the_model_does_not_support_before_transmission() {
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(state_body(A3005_STATE))),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        for (command, expected) in [
            (
                SessionCommand::SetAmbientSoundMode(SoundMode::Normal),
                ModelError::InvalidSoundModeWrite,
            ),
            (
                SessionCommand::SetWearingDetection(false),
                ModelError::InvalidWearingDetectionWrite,
            ),
        ] {
            assert_eq!(
                wait(handle.dispatch(command).expect("queue the typed rejection")).await,
                Err(CommandError::Model(expected))
            );
        }
        handle.close().await.expect("close virtual link");

        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(state_body(A3004_STATE))),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        for (command, expected) in [
            (
                SessionCommand::SetAutoPowerOff(AutoPowerOffSetting::Disabled),
                ModelError::InvalidAutoPowerOffWrite,
            ),
            (
                SessionCommand::SetDualConnections(false),
                ModelError::InvalidDualConnectionsWrite,
            ),
        ] {
            assert_eq!(
                wait(handle.dispatch(command).expect("queue the typed rejection")).await,
                Err(CommandError::Model(expected))
            );
        }
        handle.close().await.expect("close virtual link");

        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(state_body(A3033_STATE))),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        assert_eq!(
            wait(
                handle
                    .dispatch(SessionCommand::SetDualConnections(false))
                    .expect("queue the typed rejection")
            )
            .await,
            Err(CommandError::Model(ModelError::InvalidDualConnectionsWrite))
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn an_unchanged_flag_does_not_confirm_the_a3005_dual_connections_write() {
        let (handle, frames) = start(StaticState {
            body: state_body(A3005_STATE).to_vec(),
            ack_command: DUAL_CONNECTIONS_COMMAND,
        });
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetDualConnections(false))
                .expect("queue dual connections off"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::DualConnectionsMismatch {
                requested: false,
                observed: Some(true),
            })
        );
        assert_eq!(
            count(&frames, &request(DUAL_CONNECTIONS_COMMAND, &[0])),
            1,
            "the disable write was attempted once"
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn an_unchanged_flag_does_not_confirm_the_a3033_wearing_detection_write() {
        let (handle, frames) = start(StaticState {
            body: state_body(A3033_STATE).to_vec(),
            ack_command: WEARING_DETECTION_COMMAND,
        });
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetWearingDetection(false))
                .expect("queue wearing detection off"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::WearingDetectionMismatch {
                requested: false,
                observed: Some(true),
            })
        );
        assert_eq!(
            count(&frames, &request(WEARING_DETECTION_COMMAND, &[0])),
            1,
            "the disable write was attempted once"
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn a3004_has_no_wearing_detection_or_dual_connections_commands() {
        let (handle, _) = start(VirtualA3004);
        ready(&handle).await;
        for (command, expected) in [
            (
                SessionCommand::SetWearingDetection(false),
                ModelError::InvalidWearingDetectionWrite,
            ),
            (
                SessionCommand::SetDualConnections(false),
                ModelError::InvalidDualConnectionsWrite,
            ),
        ] {
            assert_eq!(
                wait(handle.dispatch(command).expect("queue the typed rejection")).await,
                Err(CommandError::Model(expected))
            );
        }
        handle.close().await.expect("close virtual link");
    }

    /// Mutation of the published reference body, used for the read-back replies.
    fn state_with(body: &[u8], offset: usize, data: &[u8]) -> Vec<u8> {
        let mut state = body.to_vec();
        state[offset..offset + data.len()].copy_from_slice(data);
        state
    }

    fn state_with_ten_band_curve(body: &[u8]) -> Vec<u8> {
        let mut state = body.to_vec();
        state[23..25].copy_from_slice(&DRC_CURVE.preset_id.to_le_bytes());
        state[25..35].copy_from_slice(&DRC_WRITE_BODY[2..12]);
        state
    }

    fn state_with_eight_band_curve(body: &[u8]) -> Vec<u8> {
        let mut state = body.to_vec();
        state[2..4].copy_from_slice(&EIGHT_BAND_CURVE.preset_id.to_le_bytes());
        state[4..12].copy_from_slice(&EIGHT_BAND_WRITE_BODY[2..10]);
        state
    }

    fn state_body(fixture: &[u8]) -> &[u8] {
        assert_eq!(fixture[5..7], STATE_COMMAND);
        &fixture[9..fixture.len() - 1]
    }

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        }
    }

    fn request(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    fn inbound(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, STATE_COMMAND, body).expect("inbound state frame")
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

    fn commands(frames: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<[u8; 2]> {
        frames
            .lock()
            .expect("request trace")
            .iter()
            .filter_map(|frame| frame.get(5..7)?.try_into().ok())
            .collect()
    }

    fn count(frames: &Arc<Mutex<Vec<Vec<u8>>>>, expected: &Bytes) -> usize {
        frames
            .lock()
            .expect("request trace")
            .iter()
            .filter(|frame| frame.as_slice() == expected.as_ref())
            .count()
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

    /// A responder that acknowledges one write command and keeps its initial state.
    struct StaticState {
        body: Vec<u8>,
        ack_command: [u8; 2],
    }

    #[async_trait]
    impl TransportProvider for StaticState {
        async fn connect(
            &self,
            _target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            Ok(Box::new(StaticLink {
                decoder: openscl4_protocol::FrameDecoder::default(),
                body: Arc::new(self.body.clone()),
                ack_command: self.ack_command,
            }))
        }
    }

    struct StaticLink {
        decoder: openscl4_protocol::FrameDecoder,
        body: Arc<Vec<u8>>,
        ack_command: [u8; 2],
    }

    impl TransportLink for StaticLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            (
                Box::new(StaticTx {
                    decoder: self.decoder,
                    body: self.body,
                    ack_command: self.ack_command,
                    replies: tx,
                }),
                Box::new(StaticRx(rx)),
            )
        }
    }

    struct StaticTx {
        decoder: openscl4_protocol::FrameDecoder,
        body: Arc<Vec<u8>>,
        ack_command: [u8; 2],
        replies: tokio::sync::mpsc::UnboundedSender<Bytes>,
    }

    #[async_trait]
    impl LinkTx for StaticTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.decoder.push(&request);
            while let Ok(Some(frame)) = self.decoder.next_frame() {
                let body = if frame.command == STATE_COMMAND && frame.body.is_empty() {
                    self.body.as_slice()
                } else if frame.command == self.ack_command {
                    &[]
                } else {
                    continue;
                };
                let reply = encode_frame(Direction::Inbound, frame.command, body)
                    .map_err(|_| TransportError::Closed)?;
                self.replies
                    .send(reply)
                    .map_err(|_| TransportError::Closed)?;
            }
            Ok(())
        }

        async fn close(&mut self) {}
    }

    struct StaticRx(tokio::sync::mpsc::UnboundedReceiver<Bytes>);

    #[async_trait]
    impl LinkRx for StaticRx {
        async fn recv(&mut self) -> Result<Bytes, TransportError> {
            self.0.recv().await.ok_or(TransportError::Closed)
        }
    }
}
