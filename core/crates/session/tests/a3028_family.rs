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
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState, DispatchError,
        EqualizerSetting, ModelDriver, PendingCommand, SessionCommand, SessionHandle,
        SessionRunner, SoundMode, UnidentifiedReason, UnidentifiedState,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualA3028, VirtualA3029, VirtualA3062};

    use super::capture_replay::Replay;

    // Reference wire opcodes from the legacy fixtures and packet builders.
    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const A3028_EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x81];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];

    /// The published state bodies from `core/data/a3028-a3029-mode-reference.json`.
    /// The reference replaces the legacy capture's full serial at body offsets
    /// 44–59 with these synthetic model-specific values and recomputes the
    /// checksum. Neither body carries the optional extra fields.
    const A3028_INITIAL: &[u8] = &[
        0x05, 0x00, 0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x30, 0x32, 0x2e, 0x33, 0x30, 0x33,
        0x30, 0x32, 0x38, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ];
    const A3029_INITIAL: &[u8] = &[
        0x05, 0x00, 0xfe, 0xfe, 0x3c, 0xb4, 0x8f, 0xa0, 0x8e, 0xb4, 0x74, 0x88, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01, 0x00, 0x30, 0x32, 0x2e, 0x33, 0x30, 0x33,
        0x30, 0x32, 0x39, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ];
    const REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0],
    };

    #[tokio::test(start_paused = true)]
    async fn reads_the_reference_state_for_each_public_code() {
        assert_reference_state(VirtualA3028, ModelDriver::A3028).await;
        assert_reference_state(VirtualA3029, ModelDriver::A3029).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_malformed_sound_mode_state_is_read_only_at_start() {
        let mut body = A3028_INITIAL.to_vec();
        body[36] = 0xff;
        let (handle, _) = start(StaticState { body });
        let view = tokio::time::timeout(Duration::from_secs(10), handle.confirmed_view())
            .await
            .expect("identity and state deadline")
            .expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3028),
                reason: UnidentifiedReason::IncompatibleState,
            })
        );
        assert_eq!(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(
                    SoundMode::NoiseCanceling,
                ))
                .err(),
            Some(DispatchError::ReadOnly)
        );
        handle.close().await.expect("close the read-only link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_reference_mode_cycle_from_preset_replies() {
        for initial in [A3028_INITIAL, A3029_INITIAL] {
            let initial_reply = inbound(initial);
            let anc_reply = inbound(&body_with_mode(initial, 0x00));
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
                (request(SOUND_MODE_COMMAND, &[0x00, 0x00, 0x01, 0x00]), None),
                (request(STATE_COMMAND, &[]), Some(anc_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(anc_reply.clone())),
                (request(SOUND_MODE_COMMAND, &[0x02, 0x00, 0x01, 0x00]), None),
                (request(STATE_COMMAND, &[]), Some(initial_reply)),
            ]));
            let handle = SessionRunner::start(replay, target());
            let initial_state = ready(&handle).await;
            assert_eq!(initial_state.sound_mode, Some(SoundMode::Normal));

            let CommandOutcome::Applied(changed) = wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(
                        SoundMode::NoiseCanceling,
                    ))
                    .expect("queue noise canceling"),
            )
            .await
            .expect("confirm noise canceling") else {
                panic!("expected an applied outcome");
            };
            assert_eq!(changed.sound_mode, Some(SoundMode::NoiseCanceling));
            assert_eq!(changed.equalizer, initial_state.equalizer);

            let CommandOutcome::Applied(restored) = wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                    .expect("queue normal"),
            )
            .await
            .expect("confirm normal") else {
                panic!("expected an applied outcome");
            };
            assert_eq!(restored.sound_mode, Some(SoundMode::Normal));
            assert_eq!(restored.equalizer, initial_state.equalizer);
            assert_eq!(ready(&handle).await, restored);
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_family_eight_band_curve_from_preset_replies() {
        for initial in [A3028_INITIAL, A3029_INITIAL] {
            let initial_reply = inbound(initial);
            let signature_reply = inbound(&body_with_signature(initial));
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(initial_reply)),
                (
                    request(
                        A3028_EQUALIZER_COMMAND,
                        &[0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78],
                    ),
                    None,
                ),
                (request(STATE_COMMAND, &[]), Some(signature_reply)),
            ]));
            let handle = SessionRunner::start(replay, target());
            assert_eq!(ready(&handle).await.equalizer, Some(REFERENCE_CURVE));

            let requested = EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0; 10],
            };
            handle.set_eq_live(requested).expect("accept live curve");
            let applied = wait_for_equalizer(&handle, requested).await;
            assert_eq!(applied.equalizer, Some(requested));
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_enable_and_disable_from_preset_extra_field_replies() {
        for initial in [A3028_INITIAL, A3029_INITIAL] {
            let enabled_reply = inbound(&body_with_extras(initial, true, 1));
            let disabled_reply = inbound(&body_with_extras(initial, false, 1));
            let enabled_three_reply = inbound(&body_with_extras(initial, true, 3));
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(enabled_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(enabled_reply.clone())),
                (request(AUTO_POWER_OFF_COMMAND, &[0x00, 0x01]), None),
                (request(STATE_COMMAND, &[]), Some(disabled_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(disabled_reply)),
                (request(AUTO_POWER_OFF_COMMAND, &[0x01, 0x03]), None),
                (request(STATE_COMMAND, &[]), Some(enabled_three_reply)),
            ]));
            let handle = SessionRunner::start(replay, target());
            let initial_state = ready(&handle).await;
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
            .expect("confirm disable") else {
                panic!("expected an applied outcome");
            };
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
            .expect("confirm enable") else {
                panic!("expected an applied outcome");
            };
            assert_eq!(
                enabled.auto_power_off,
                Some(AutoPowerOff {
                    enabled: true,
                    duration: 3,
                })
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_a_duration_outside_the_family_table_before_transmission() {
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body_with_extras(A3028_INITIAL, true, 1))),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;

        let result = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Enabled(4),
                ))
                .expect("queue the rejected duration"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidAutoPowerOffWrite))
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_auto_power_off_before_transmission_when_unavailable() {
        for initial in [A3028_INITIAL, A3029_INITIAL] {
            let initial_reply = inbound(initial);
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(initial_reply)),
            ]));
            let handle = SessionRunner::start(replay, target());
            let initial_state = ready(&handle).await;
            assert_eq!(initial_state.auto_power_off, None);

            let result = wait(
                handle
                    .dispatch(SessionCommand::SetAutoPowerOff(
                        AutoPowerOffSetting::Disabled,
                    ))
                    .expect("queue the typed rejection"),
            )
            .await;
            assert_eq!(
                result,
                Err(CommandError::Model(ModelError::InvalidAutoPowerOffWrite))
            );
            assert_eq!(ready(&handle).await, initial_state);
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn an_ack_and_an_unchanged_state_do_not_confirm_the_disabled_write() {
        let (handle, frames) = start(StaticState {
            body: body_with_extras(A3028_INITIAL, true, 1),
        });
        let initial = ready(&handle).await;
        assert_eq!(
            initial.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );

        let result = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Disabled,
                ))
                .expect("queue disable"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::AutoPowerOffMismatch {
                requested: AutoPowerOffSetting::Disabled,
                observed: Some(AutoPowerOff {
                    enabled: true,
                    duration: 1,
                }),
            })
        );
        let write = request(AUTO_POWER_OFF_COMMAND, &[0x00, 0x01]);
        assert_eq!(
            count(&frames, &write),
            1,
            "the disabled body keeps the previous index"
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn unrecognized_auto_power_off_fields_are_not_confirmed() {
        for (offset, value) in [(63, 2), (64, 4)] {
            let mut body = body_with_extras(A3028_INITIAL, true, 1);
            body[offset] = value;
            let (handle, frames) = start(StaticState { body });
            let initial = ready(&handle).await;
            assert_eq!(
                initial.auto_power_off, None,
                "unrecognized byte {value} at {offset}"
            );

            let result = wait(
                handle
                    .dispatch(SessionCommand::SetAutoPowerOff(
                        AutoPowerOffSetting::Disabled,
                    ))
                    .expect("queue disable"),
            )
            .await;
            assert_eq!(
                result,
                Err(CommandError::Model(ModelError::InvalidAutoPowerOffWrite))
            );
            assert_eq!(
                count(&frames, &request(AUTO_POWER_OFF_COMMAND, &[0x00, 0x01])),
                0
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn a3062_keeps_auto_power_off_unsupported() {
        let (handle, frames) = start(VirtualA3062);
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Enabled(0),
                ))
                .expect("queue the typed rejection"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidAutoPowerOffWrite))
        );
        assert_eq!(
            count(&frames, &request(AUTO_POWER_OFF_COMMAND, &[0x01, 0x00])),
            0
        );
        handle.close().await.expect("close virtual link");
    }

    async fn assert_reference_state<P: TransportProvider>(provider: P, model: ModelDriver) {
        let (handle, frames) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, model);
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(state.battery.device, Some(100));
        assert_eq!(
            state.firmware_device.map(|firmware| firmware.to_string()),
            Some("02.30".to_owned())
        );
        assert_eq!(state.equalizer, Some(REFERENCE_CURVE));
        assert_eq!(state.auto_power_off, None);
        handle.close().await.expect("close virtual link");

        // State-only discovery: the unavailable identity probe is followed by
        // one verified state read and no writes.
        let commands: Vec<[u8; 2]> = frames
            .lock()
            .expect("request trace")
            .iter()
            .filter_map(|frame| command(frame))
            .collect();
        assert_eq!(commands, vec![[0x01, 0x05], [0x01, 0x01]]);
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
            view.wait_for(
                |view| matches!(view.connection, ConnectionState::Ready(state) if state.equalizer == Some(requested)),
            ),
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

    fn request(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    fn inbound(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, STATE_COMMAND, body).expect("inbound state frame")
    }

    fn body_with_mode(initial: &[u8], mode: u8) -> Vec<u8> {
        let mut body = initial.to_vec();
        // Reference `noiseCanceling`: only the ambient mode byte changes.
        body[35] = mode;
        body
    }

    fn body_with_signature(initial: &[u8]) -> Vec<u8> {
        let mut body = initial.to_vec();
        // The legacy signature write: preset 0 and eight 0x78 band bytes.
        body[2..4].copy_from_slice(&0u16.to_le_bytes());
        body[4..12].fill(0x78);
        body
    }

    fn body_with_extras(initial: &[u8], enabled: bool, duration: u8) -> Vec<u8> {
        let mut body = initial.to_vec();
        // unknown1, touch control, dual connections, auto power off (flag,
        // duration index), ambient tone and battery alert tone.
        body.extend_from_slice(&[0x00, 0x00, 0x00, u8::from(enabled), duration, 0x01, 0x01]);
        body
    }

    fn command(frame: &[u8]) -> Option<[u8; 2]> {
        frame.get(5..7)?.try_into().ok()
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

    /// A responder that acknowledges auto power off and keeps returning its initial state.
    struct StaticState {
        body: Vec<u8>,
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
            }))
        }
    }

    struct StaticLink {
        decoder: openscl4_protocol::FrameDecoder,
        body: Arc<Vec<u8>>,
    }

    impl TransportLink for StaticLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            (
                Box::new(StaticTx {
                    decoder: self.decoder,
                    body: self.body,
                    replies: tx,
                }),
                Box::new(StaticRx(rx)),
            )
        }
    }

    struct StaticTx {
        decoder: openscl4_protocol::FrameDecoder,
        body: Arc<Vec<u8>>,
        replies: tokio::sync::mpsc::UnboundedSender<Bytes>,
    }

    #[async_trait]
    impl LinkTx for StaticTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.decoder.push(&request);
            while let Ok(Some(frame)) = self.decoder.next_frame() {
                let body = match frame.command {
                    STATE_COMMAND if frame.body.is_empty() => self.body.as_slice(),
                    AUTO_POWER_OFF_COMMAND => &[],
                    _ => continue,
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
