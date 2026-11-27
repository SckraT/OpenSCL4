//! Session scenarios for the D1202 family and its recorded connection metadata.

#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_models::{
        AutoPowerOff, AutoPowerOffSetting, D1202_EQUALIZER_COMMAND, D1202_SERVICE_ID,
        D1202C_SERVICE_ID, EqualizerSetting, ModelDriver, SOUND_MODE_COMMAND,
        SPATIAL_AUDIO_COMMAND,
    };
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, ConnectionState, DeviceState, DispatchError,
        QuickPresetValues, SessionCommand, SessionHandle, SessionRunner, SoundMode, SpatialAudio,
        SpatialAudioMode, UnidentifiedReason, UnidentifiedState,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualD1202, VirtualD1202C};
    use tokio::sync::Mutex;

    const D1202_STATE: &[u8] = include_bytes!("../../../data/d1202-pilot-state.bin");
    const D1202C_STATE: &[u8] = include_bytes!("../../../data/d1202c-pilot-state.bin");
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];

    /// The legacy builder example curve of the `set_preset_eq` test.
    const LEGACY_CURVE: [i16; 10] = [40, 10, 20, 20, 40, 40, 40, 20, 0, -120];

    #[tokio::test(start_paused = true)]
    async fn reads_each_public_code_from_the_recorded_connection_metadata() {
        for (provider, service, fixture, expected) in [
            (
                Provider::D1202(VirtualD1202),
                D1202_SERVICE_ID,
                D1202_STATE,
                ModelDriver::D1202,
            ),
            (
                Provider::D1202C(VirtualD1202C),
                D1202C_SERVICE_ID,
                D1202C_STATE,
                ModelDriver::D1202C,
            ),
        ] {
            let (handle, frames) = start(provider, Some(service)).await;
            let state = ready(&handle).await;
            assert_eq!(state.model, expected);
            assert_eq!(
                state.battery.left,
                Some(100),
                "{} left battery",
                expected.code()
            );
            assert_eq!(state.battery.case, case_percent(fixture));
            assert_eq!(
                state.firmware_left.map(|firmware| firmware.to_string()),
                Some(firmware(fixture).to_owned())
            );
            let commands: Vec<[u8; 2]> = frames
                .lock()
                .await
                .iter()
                .filter_map(|frame| {
                    let command: [u8; 2] = frame.get(5..7)?.try_into().ok()?;
                    Some(command)
                })
                .collect();
            assert_eq!(
                commands,
                vec![[0x01, 0x05], STATE_COMMAND],
                "state-only discovery for {}",
                expected.code()
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn uses_the_canonical_code_without_connection_metadata() {
        for (provider, fixture) in [
            (Provider::D1202(VirtualD1202), D1202_STATE),
            (Provider::D1202C(VirtualD1202C), D1202C_STATE),
        ] {
            let (handle, _) = start(provider, None).await;
            let state = ready(&handle).await;
            assert_eq!(state.model, ModelDriver::D1202);
            // Both codes report the same state serial; only the recorded
            // connection metadata selects the public code.
            assert_eq!(
                &body(fixture)[16..32],
                &body(D1202C_STATE)[16..32],
                "the measured family serials overlap"
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_ambient_mode_write_for_both_codes() {
        for (provider, service, fixture) in family_sessions() {
            let (handle, frames) = start(provider, Some(service)).await;
            let initial = ready(&handle).await;
            let original = initial.sound_mode.expect("ambient mode");
            let target = if original == SoundMode::NoiseCanceling {
                SoundMode::Normal
            } else {
                SoundMode::NoiseCanceling
            };
            let expected = sound_mode_body(fixture, target);
            let CommandOutcome::Applied(changed) = wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(target))
                    .expect("queue the ambient mode"),
            )
            .await
            .expect("confirm the ambient mode");
            assert_eq!(changed.sound_mode, Some(target));
            assert_eq!(changed.equalizer, initial.equalizer);
            assert_eq!(
                count(&frames, &request(SOUND_MODE_COMMAND, &expected)).await,
                1,
                "the outgoing body matches the legacy eight-byte format"
            );

            let restored_body = sound_mode_body(fixture, original);
            let CommandOutcome::Applied(restored) = wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(original))
                    .expect("queue the restore"),
            )
            .await
            .expect("confirm the restore");
            assert_eq!(restored.sound_mode, Some(original));
            assert_eq!(restored.equalizer, initial.equalizer);
            assert_eq!(
                count(&frames, &request(SOUND_MODE_COMMAND, &restored_body)).await,
                1
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_equalizer_write_for_both_codes() {
        for (provider, service, fixture) in family_sessions() {
            let (handle, frames) = start(provider, Some(service)).await;
            ready(&handle).await;
            let requested = EqualizerSetting {
                preset_id: 1,
                volume_adjustments: LEGACY_CURVE,
            };
            let CommandOutcome::Applied(applied) = wait(
                handle
                    .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                        sound_mode: None,
                        ambient_sound_mode: None,
                        equalizer: Some(requested),
                    }))
                    .expect("queue the curve"),
            )
            .await
            .expect("confirm the curve");
            assert_eq!(applied.equalizer, Some(requested));

            let expected = legacy_equalizer_body(requested, body(fixture));
            assert_eq!(expected.len(), 114);
            assert_eq!(
                count(&frames, &request(D1202_EQUALIZER_COMMAND, &expected)).await,
                1,
                "the outgoing body follows the legacy equalizer builder"
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_auto_power_off_from_a_fresh_read_back() {
        for (provider, service, _) in family_sessions() {
            let (handle, frames) = start(provider, Some(service)).await;
            let initial = ready(&handle).await;
            let current = initial.auto_power_off.expect("reported auto power off");

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
                    duration: current.duration,
                })
            );
            assert_eq!(
                count(
                    &frames,
                    &request(AUTO_POWER_OFF_COMMAND, &[0, current.duration])
                )
                .await,
                1
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
            assert_eq!(
                count(&frames, &request(AUTO_POWER_OFF_COMMAND, &[1, 3])).await,
                1
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_spatial_audio_enable_and_disable() {
        for (provider, service, fixture) in family_sessions() {
            let (handle, frames) = start(provider, Some(service)).await;
            let initial = ready(&handle).await;
            assert_eq!(
                initial.spatial_audio,
                Some(SpatialAudio {
                    enabled: false,
                    mode: SpatialAudioMode::Music,
                })
            );

            let CommandOutcome::Applied(enabled) = wait(
                handle
                    .dispatch(SessionCommand::SetSpatialAudio(true))
                    .expect("queue enable"),
            )
            .await
            .expect("confirm enable");
            assert_eq!(
                enabled.spatial_audio,
                Some(SpatialAudio {
                    enabled: true,
                    mode: SpatialAudioMode::Music,
                })
            );
            assert_eq!(
                count(&frames, &request(SPATIAL_AUDIO_COMMAND, &[1, 0, 0])).await,
                1
            );

            // Disabling follows the legacy state modifier: the current
            // equalizer configuration is written with the spatial setting cleared.
            let current_equalizer = initial.equalizer.expect("reported curve");
            let expected = legacy_equalizer_body(current_equalizer, body(fixture));
            let CommandOutcome::Applied(disabled) = wait(
                handle
                    .dispatch(SessionCommand::SetSpatialAudio(false))
                    .expect("queue disable"),
            )
            .await
            .expect("confirm disable");
            assert_eq!(
                disabled.spatial_audio,
                Some(SpatialAudio {
                    enabled: false,
                    mode: SpatialAudioMode::Music,
                })
            );
            assert_eq!(
                count(&frames, &request(D1202_EQUALIZER_COMMAND, &expected)).await,
                1,
                "disable uses the legacy equalizer command"
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_every_write_when_the_family_state_is_incompatible() {
        let mut body = body(D1202_STATE).to_vec();
        // An airplane-mode byte cannot belong to this family's state.
        body[119] = 3;
        let (handle, frames) = start_static(StaticState { body }, Some(D1202_SERVICE_ID)).await;
        let view = tokio::time::timeout(Duration::from_secs(3), handle.confirmed_view())
            .await
            .expect("identity and state deadline")
            .expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::D1202),
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
        assert_eq!(
            handle.dispatch(SessionCommand::SetSpatialAudio(true)).err(),
            Some(DispatchError::ReadOnly)
        );
        assert!(
            frames
                .lock()
                .await
                .iter()
                .all(|frame| frame.get(5..7) == Some(&STATE_COMMAND)
                    || frame.get(5..7) == Some(&[0x01, 0x05]))
        );
        handle.close().await.expect("close the read-only link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_spatial_audio_on_a_model_without_the_capability() {
        let handle = SessionRunner::start(openscl4_transport_mock::VirtualA3028, target(None));
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3028);
        let result = wait(
            handle
                .dispatch(SessionCommand::SetSpatialAudio(true))
                .expect("queue the typed rejection"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(
                openscl4_models::ModelError::InvalidSpatialAudioWrite
            ))
        );
        handle.close().await.expect("close virtual link");
    }

    enum Provider {
        D1202(VirtualD1202),
        D1202C(VirtualD1202C),
    }

    #[async_trait]
    impl TransportProvider for Provider {
        async fn connect(
            &self,
            target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            match self {
                Self::D1202(provider) => provider.connect(target).await,
                Self::D1202C(provider) => provider.connect(target).await,
            }
        }
    }

    fn family_sessions() -> [(Provider, [u8; 16], &'static [u8]); 2] {
        [
            (Provider::D1202(VirtualD1202), D1202_SERVICE_ID, D1202_STATE),
            (
                Provider::D1202C(VirtualD1202C),
                D1202C_SERVICE_ID,
                D1202C_STATE,
            ),
        ]
    }

    fn body(fixture: &[u8]) -> &[u8] {
        &fixture[9..fixture.len() - 1]
    }

    fn case_percent(fixture: &[u8]) -> Option<u8> {
        Some((((u16::from(body(fixture)[37]) + 1) * 100) / 10) as u8)
    }

    fn firmware(fixture: &[u8]) -> &'static str {
        match body(fixture)[7] {
            b'1' => "01.83",
            _ => "04.89",
        }
    }

    fn sound_mode_body(fixture: &[u8], mode: SoundMode) -> [u8; 8] {
        let mut expected = [0; 8];
        expected[0] = match mode {
            SoundMode::NoiseCanceling => 0,
            SoundMode::Transparency => 1,
            SoundMode::Normal => 2,
            SoundMode::AirplaneMode => 3,
        };
        expected[1..].copy_from_slice(&body(fixture)[120..127]);
        expected
    }

    /// Builds the expected `03 87` body from the legacy builder's layout.
    fn legacy_equalizer_body(setting: EqualizerSetting, state: &[u8]) -> Vec<u8> {
        let bands = setting.volume_adjustments.map(|gain| (gain + 120) as u8);
        let mut body = Vec::with_capacity(114);
        body.extend_from_slice(&setting.preset_id.to_le_bytes());
        body.extend_from_slice(&state[107..109]);
        body.extend_from_slice(&bands);
        body.extend_from_slice(&bands);
        body.extend_from_slice(&[0, 0, 0]);
        body.extend_from_slice(&state[62..82]);
        body.extend_from_slice(&state[82..86]);
        body.push(state[86]);
        body.extend_from_slice(&state[87..107]);
        for _ in 0..2 {
            for band in bands {
                body.extend_from_slice(&[band, 0]);
            }
        }
        body.extend_from_slice(&[0, 0]);
        body
    }

    async fn start(provider: Provider, service: Option<[u8; 16]>) -> (SessionHandle, Frames) {
        let frames = Frames::default();
        let handle = SessionRunner::start(
            Recording {
                inner: provider,
                frames: frames.clone(),
            },
            target(service),
        );
        (handle, frames)
    }

    async fn start_static(
        provider: StaticState,
        service: Option<[u8; 16]>,
    ) -> (SessionHandle, Frames) {
        let frames = Frames::default();
        let handle = SessionRunner::start(
            Recording {
                inner: provider,
                frames: frames.clone(),
            },
            target(service),
        );
        (handle, frames)
    }

    fn target(service: Option<[u8; 16]>) -> ConnectTarget {
        match service {
            Some(service) => {
                ConnectTarget::from_mac(RedactedMac::from_octets([0; 6])).with_service(service)
            }
            None => ConnectTarget::from_mac(RedactedMac::from_octets([0; 6])),
        }
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

    async fn wait(
        pending: openscl4_session::PendingCommand,
    ) -> Result<CommandOutcome, CommandError> {
        tokio::time::timeout(Duration::from_secs(10), pending.wait())
            .await
            .expect("command deadline")
    }

    fn request(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    async fn count(frames: &Frames, expected: &Bytes) -> usize {
        frames
            .lock()
            .await
            .iter()
            .filter(|frame| frame.as_slice() == expected.as_ref())
            .count()
    }

    type Frames = Arc<Mutex<Vec<Vec<u8>>>>;

    struct Recording<P> {
        inner: P,
        frames: Frames,
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
                    frames: self.frames.clone(),
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
        frames: Frames,
    }

    #[async_trait]
    impl LinkTx for RecordingTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.frames.lock().await.push(request.to_vec());
            self.inner.send(request).await
        }

        async fn close(&mut self) {
            self.inner.close().await;
        }
    }

    /// Answers the state read with a fixed body and never acknowledges writes.
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
                body: self.body.clone(),
            }))
        }
    }

    struct StaticLink {
        decoder: openscl4_protocol::FrameDecoder,
        body: Vec<u8>,
    }

    impl TransportLink for StaticLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            let (replies, receiver) = tokio::sync::mpsc::unbounded_channel();
            (
                Box::new(StaticTx {
                    decoder: self.decoder,
                    body: self.body,
                    replies,
                }),
                Box::new(StaticRx(receiver)),
            )
        }
    }

    struct StaticTx {
        decoder: openscl4_protocol::FrameDecoder,
        body: Vec<u8>,
        replies: tokio::sync::mpsc::UnboundedSender<Bytes>,
    }

    #[async_trait]
    impl LinkTx for StaticTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.decoder.push(&request);
            while let Ok(Some(frame)) = self.decoder.next_frame() {
                if frame.direction != Direction::Outbound {
                    continue;
                }
                if frame.command == STATE_COMMAND && frame.body.is_empty() {
                    let reply = encode_frame(Direction::Inbound, STATE_COMMAND, &self.body)
                        .map_err(|_| TransportError::Io)?;
                    self.replies
                        .send(reply)
                        .map_err(|_| TransportError::Closed)?;
                }
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
