#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::Duration,
    };

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_models::{
        AmbientSoundModeCycle, AutoPowerOffSetting, ButtonAction, EqualizerSetting,
        LimitHighVolume, ModelError, VolumeLimitRefreshRate,
    };
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState, ModelDriver,
        PendingCommand, SessionCommand, SessionHandle, SessionRunner, SoundMode,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualA3035, VirtualA3040};

    /// Reference wire opcodes from the legacy fixtures and packet builders.
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const EQUALIZER_COMMAND: [u8; 2] = [0x03, 0x87];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];
    const BUTTON_CONFIGURATION_COMMAND: [u8; 2] = [0x04, 0x81];
    const SOUND_MODE_CYCLE_COMMAND: [u8; 2] = [0x06, 0x82];
    const LIMIT_HIGH_VOLUME_COMMAND: [u8; 2] = [0x20, 0x82];
    const LIMIT_HIGH_VOLUME_REFRESH_RATE_COMMAND: [u8; 2] = [0x20, 0x81];
    const DUAL_CONNECTIONS_COMMAND: [u8; 2] = [0x0b, 0x84];
    const LDAC_COMMAND: [u8; 2] = [0x01, 0xff];
    const LDAC_STATE_COMMAND: [u8; 2] = [0x01, 0x7f];
    const AUTO_PLAY_PAUSE_COMMAND: [u8; 2] = [0x01, 0x81];
    const VOICE_PROMPT_COMMAND: [u8; 2] = [0x01, 0xae];
    const PROMPT_COMMAND: [u8; 2] = [0x01, 0xaf];

    const A3035_STATE: &[u8] = include_bytes!("../../../data/a3035-state-reference.bin");

    #[tokio::test(start_paused = true)]
    async fn reads_the_reference_states() {
        let (handle, frames) = start(VirtualA3035);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3035);
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.battery.device, Some(100));
        assert_eq!(
            state.firmware_device.map(|version| version.to_string()),
            Some("06.87".to_owned())
        );
        assert_eq!(state.equalizer.map(|curve| curve.preset_id), Some(0));
        assert_eq!(
            state.settings.dual_connections,
            Some(false),
            "the reference A3035 has no second connection"
        );
        handle.close().await.expect("close virtual link");
        // State-only discovery: an unavailable identity probe, then one state read.
        assert_eq!(commands(&frames), vec![[0x01, 0x05], [0x01, 0x01]]);

        let (handle, _) = start(VirtualA3040);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3040);
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.battery.device, Some(60));
        assert_eq!(
            state.firmware_device.map(|version| version.to_string()),
            Some("02.34".to_owned())
        );
        assert_eq!(state.settings.dual_connections, Some(true));
        assert_eq!(state.settings.ldac, Some(false));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3035_reference_mode_cycle() {
        let (handle, frames) = start(VirtualA3035);
        ready(&handle).await;

        let CommandOutcome::Applied(changed) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue normal"),
        )
        .await
        .expect("confirm normal");
        assert_eq!(changed.sound_mode, Some(SoundMode::Normal));
        let restored = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(
                    SoundMode::NoiseCanceling,
                ))
                .expect("queue noise canceling"),
        )
        .await
        .expect("confirm noise canceling");
        let CommandOutcome::Applied(restored) = restored;
        assert_eq!(restored.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(ready(&handle).await, restored);
        handle.close().await.expect("close virtual link");

        assert_sent(
            &frames,
            SOUND_MODE_COMMAND,
            &[0x02, 0x51, 0x02, 0x00, 0x01, 0x05],
        );
        assert_sent(
            &frames,
            SOUND_MODE_COMMAND,
            &[0x00, 0x51, 0x00, 0x00, 0x01, 0x05],
        );
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3040_reference_mode_cycle() {
        let (handle, frames) = start(VirtualA3040);
        ready(&handle).await;

        let CommandOutcome::Applied(changed) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue normal"),
        )
        .await
        .expect("confirm normal");
        assert_eq!(changed.sound_mode, Some(SoundMode::Normal));
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
        handle.close().await.expect("close virtual link");

        assert_sent(
            &frames,
            SOUND_MODE_COMMAND,
            &[0x02, 0x50, 0x00, 0x01, 0x00, 0x05],
        );
        assert_sent(
            &frames,
            SOUND_MODE_COMMAND,
            &[0x00, 0x50, 0x00, 0x01, 0x00, 0x05],
        );
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3035_reference_custom_equalizer() {
        let (handle, frames) = start(VirtualA3035);
        ready(&handle).await;

        let requested = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [-60, 0, 0, 0, 0, 0, 0, 60, 0, 0],
        };
        handle.set_eq_live(requested).expect("accept live curve");
        let applied = wait_for_equalizer(&handle, requested).await;
        assert_eq!(applied.equalizer, Some(requested));
        handle.close().await.expect("close virtual link");

        assert_sent(
            &frames,
            EQUALIZER_COMMAND,
            &[
                254, 254, 0, 0, 60, 120, 120, 120, 120, 120, 120, 180, 120, 120, 255, 255, 0, 255,
                255, 255, 255, 255, 255, 255, 255, 255, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255, 255,
                255, 255, 255, 255, 0, 112, 124, 119, 121, 119, 121, 116, 129, 120, 0, 0,
            ],
        );
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3040_reference_custom_equalizer() {
        let (handle, frames) = start(VirtualA3040);
        ready(&handle).await;

        let requested = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [60, 0, 0, 0, 0, 0, 0, -60, 0, 0],
        };
        handle.set_eq_live(requested).expect("accept live curve");
        let applied = wait_for_equalizer(&handle, requested).await;
        assert_eq!(applied.equalizer, Some(requested));
        handle.close().await.expect("close virtual link");

        let Some(frame) = sent_frame(&frames, EQUALIZER_COMMAND) else {
            panic!("the measured equalizer write was not sent");
        };
        assert_eq!(frame.len(), 93);
        assert_eq!(&frame[0..4], &[254, 254, 17, 0]);
        assert_eq!(
            &frame[72..92],
            &[
                128, 116, 121, 119, 121, 119, 124, 111, 120, 0, 128, 116, 121, 119, 121, 119, 124,
                111, 120, 0
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3035_additional_settings() {
        let (handle, frames) = start(VirtualA3035);
        ready(&handle).await;

        let applied = dispatch(&handle, SessionCommand::SetButtonDoublePress(None)).await;
        assert_eq!(applied.settings.button_double_press, None);
        assert_sent(&frames, BUTTON_CONFIGURATION_COMMAND, &[0, 0, 0x0f]);

        let applied = dispatch(
            &handle,
            SessionCommand::SetAmbientSoundModeCycle(AmbientSoundModeCycle {
                noise_canceling: false,
                transparency: true,
                normal: true,
            }),
        )
        .await;
        assert_eq!(
            applied.settings.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle {
                noise_canceling: false,
                transparency: true,
                normal: true,
            })
        );
        assert_sent(&frames, SOUND_MODE_CYCLE_COMMAND, &[0x06]);

        let applied = dispatch(
            &handle,
            SessionCommand::SetLimitHighVolume {
                enabled: true,
                db_limit: 85,
            },
        )
        .await;
        assert_eq!(
            applied.settings.limit_high_volume,
            Some(LimitHighVolume {
                enabled: true,
                db_limit: 85,
                refresh_rate: VolumeLimitRefreshRate::RealTime,
            })
        );
        assert_sent(&frames, LIMIT_HIGH_VOLUME_COMMAND, &[1, 85]);

        let applied = dispatch(
            &handle,
            SessionCommand::SetLimitHighVolumeRefreshRate(VolumeLimitRefreshRate::TenSeconds),
        )
        .await;
        assert_eq!(
            applied.settings.limit_high_volume,
            Some(LimitHighVolume {
                enabled: true,
                db_limit: 85,
                refresh_rate: VolumeLimitRefreshRate::TenSeconds,
            })
        );
        assert_sent(&frames, LIMIT_HIGH_VOLUME_REFRESH_RATE_COMMAND, &[1]);

        let applied = dispatch(&handle, SessionCommand::SetDualConnections(true)).await;
        assert_eq!(applied.settings.dual_connections, Some(true));
        assert_sent(&frames, DUAL_CONNECTIONS_COMMAND, &[1]);

        let applied = dispatch(&handle, SessionCommand::SetLdac(false)).await;
        assert_eq!(applied.settings.ldac, Some(false));
        assert_eq!(ready(&handle).await.settings.ldac, Some(false));
        assert_sent(&frames, LDAC_COMMAND, &[0]);
        assert_sent(&frames, LDAC_STATE_COMMAND, &[]);

        let applied = dispatch(&handle, SessionCommand::SetVoicePrompt(false)).await;
        assert_eq!(applied.settings.voice_prompt, Some(false));
        assert_sent(&frames, VOICE_PROMPT_COMMAND, &[0]);

        let applied = dispatch(&handle, SessionCommand::SetBatteryAlert(false)).await;
        assert_eq!(applied.settings.battery_alert, Some(false));
        assert_sent(&frames, PROMPT_COMMAND, &[0]);

        let applied = dispatch(&handle, SessionCommand::SetAutoPlayPause(false)).await;
        assert_eq!(applied.settings.auto_play_pause, Some(false));
        assert_sent(&frames, AUTO_PLAY_PAUSE_COMMAND, &[0]);

        let applied = dispatch(
            &handle,
            SessionCommand::SetAutoPowerOff(AutoPowerOffSetting::Disabled),
        )
        .await;
        assert_eq!(
            applied.auto_power_off.map(|value| value.enabled),
            Some(false)
        );
        assert_sent(&frames, AUTO_POWER_OFF_COMMAND, &[0, 2]);

        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3040_additional_settings() {
        let (handle, frames) = start(VirtualA3040);
        ready(&handle).await;

        let applied = dispatch(
            &handle,
            SessionCommand::SetButtonDoublePress(Some(ButtonAction::BassUp)),
        )
        .await;
        assert_eq!(
            applied.settings.button_double_press,
            Some(ButtonAction::BassUp)
        );
        assert_sent(&frames, BUTTON_CONFIGURATION_COMMAND, &[0, 0, 7]);

        let applied = dispatch(
            &handle,
            SessionCommand::SetAmbientSoundModeCycle(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: false,
                normal: true,
            }),
        )
        .await;
        assert_eq!(
            applied.settings.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle {
                noise_canceling: true,
                transparency: false,
                normal: true,
            })
        );
        assert_sent(&frames, SOUND_MODE_CYCLE_COMMAND, &[0x05]);

        let applied = dispatch(&handle, SessionCommand::SetDualConnections(false)).await;
        assert_eq!(applied.settings.dual_connections, Some(false));
        assert_sent(&frames, DUAL_CONNECTIONS_COMMAND, &[0]);

        let applied = dispatch(&handle, SessionCommand::SetLdac(true)).await;
        assert_eq!(applied.settings.ldac, Some(true));
        assert_sent(&frames, LDAC_COMMAND, &[1]);
        assert_sent(&frames, LDAC_STATE_COMMAND, &[]);

        let applied = dispatch(&handle, SessionCommand::SetVoicePrompt(false)).await;
        assert_eq!(applied.settings.voice_prompt, Some(false));
        assert_sent(&frames, VOICE_PROMPT_COMMAND, &[0]);

        let applied = dispatch(&handle, SessionCommand::SetLowBatteryPrompt(false)).await;
        assert_eq!(applied.settings.low_battery_prompt, Some(false));
        assert_sent(&frames, PROMPT_COMMAND, &[0]);

        let applied = dispatch(
            &handle,
            SessionCommand::SetLimitHighVolume {
                enabled: true,
                db_limit: 90,
            },
        )
        .await;
        assert_eq!(
            applied.settings.limit_high_volume,
            Some(LimitHighVolume {
                enabled: true,
                db_limit: 90,
                refresh_rate: VolumeLimitRefreshRate::RealTime,
            })
        );

        let applied = dispatch(
            &handle,
            SessionCommand::SetAutoPowerOff(AutoPowerOffSetting::Enabled(1)),
        )
        .await;
        assert_eq!(
            applied.auto_power_off.map(|value| value.enabled),
            Some(true)
        );
        assert_sent(&frames, AUTO_POWER_OFF_COMMAND, &[1, 1]);

        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_settings_the_model_does_not_report() {
        for (provider, command, expected) in [
            (
                start(VirtualA3035),
                SessionCommand::SetLowBatteryPrompt(true),
                CommandError::Model(ModelError::InvalidLowBatteryPromptWrite),
            ),
            (
                start(VirtualA3040),
                SessionCommand::SetAutoPlayPause(true),
                CommandError::Model(ModelError::InvalidAutoPlayPauseWrite),
            ),
            (
                start(VirtualA3040),
                SessionCommand::SetBatteryAlert(true),
                CommandError::Model(ModelError::InvalidBatteryAlertWrite),
            ),
        ] {
            let (handle, frames) = provider;
            ready(&handle).await;
            let before = frames.lock().expect("request trace").len();
            assert_eq!(
                wait(handle.dispatch(command).expect("queue rejection")).await,
                Err(expected)
            );
            assert_eq!(
                frames.lock().expect("request trace").len(),
                before,
                "a rejected setting must not reach the transport"
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn ldac_requires_the_dedicated_confirmation() {
        let initial = state_body(A3035_STATE);
        let (handle, _frames) = start(StaleLdac { initial });
        let state = ready(&handle).await;
        assert_eq!(state.settings.ldac, Some(true));

        let result = wait(
            handle
                .dispatch(SessionCommand::SetLdac(false))
                .expect("queue LDAC write"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::LdacMismatch {
                requested: false,
                observed: Some(true),
            })
        );
        let confirmed = ready(&handle).await;
        assert_eq!(confirmed.settings.ldac, Some(true));
        handle.close().await.expect("close virtual link");
    }

    async fn dispatch(handle: &SessionHandle, command: SessionCommand) -> DeviceState {
        match wait(handle.dispatch(command).expect("queue command"))
            .await
            .expect("confirm command")
        {
            CommandOutcome::Applied(state) => state,
        }
    }

    async fn ready(handle: &SessionHandle) -> DeviceState {
        let view = tokio::time::timeout(Duration::from_secs(5), handle.confirmed_view())
            .await
            .expect("sync deadline")
            .expect("confirmed state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a confirmed state");
        };
        state
    }

    async fn wait(pending: PendingCommand) -> CommandResult {
        tokio::time::timeout(Duration::from_secs(20), pending.wait())
            .await
            .expect("command deadline")
    }

    async fn wait_for_equalizer(
        handle: &SessionHandle,
        requested: EqualizerSetting,
    ) -> DeviceState {
        let mut view = handle.subscribe();
        let snapshot = tokio::time::timeout(
            Duration::from_secs(5),
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

    fn state_body(fixture: &[u8]) -> Vec<u8> {
        fixture[9..fixture.len() - 1].to_vec()
    }

    fn outbound(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    fn sent_frame(frames: &Arc<Mutex<Vec<Vec<u8>>>>, command: [u8; 2]) -> Option<Vec<u8>> {
        frames
            .lock()
            .expect("request trace")
            .iter()
            .find(|frame| frame.get(5..7) == Some(command.as_slice()))
            .map(|frame| frame[9..frame.len() - 1].to_vec())
    }

    fn assert_sent(frames: &Arc<Mutex<Vec<Vec<u8>>>>, command: [u8; 2], body: &[u8]) {
        let expected = outbound(command, body);
        assert!(
            frames
                .lock()
                .expect("request trace")
                .iter()
                .any(|frame| frame.as_slice() == expected.as_ref()),
            "the reference frame {command:?} {body:?} was not sent"
        );
    }

    fn commands(frames: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<[u8; 2]> {
        frames
            .lock()
            .expect("request trace")
            .iter()
            .filter_map(|frame| frame.get(5..7)?.try_into().ok())
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

    /// A virtual A3035 that keeps reporting the old LDAC flag to the dedicated
    /// request, so the write's confirmation must fail.
    struct StaleLdac {
        initial: Vec<u8>,
    }

    #[async_trait]
    impl TransportProvider for StaleLdac {
        async fn connect(
            &self,
            _target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            Ok(Box::new(StaticLink {
                decoder: openscl4_protocol::FrameDecoder::default(),
                state: Arc::new(Mutex::new(self.initial.clone())),
            }))
        }
    }

    struct StaticLink {
        decoder: openscl4_protocol::FrameDecoder,
        state: Arc<Mutex<Vec<u8>>>,
    }

    impl TransportLink for StaticLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            (
                Box::new(StaticTx {
                    decoder: self.decoder,
                    state: self.state,
                    replies: tx,
                }),
                Box::new(StaticRx(rx)),
            )
        }
    }

    struct StaticTx {
        decoder: openscl4_protocol::FrameDecoder,
        state: Arc<Mutex<Vec<u8>>>,
        replies: tokio::sync::mpsc::UnboundedSender<Bytes>,
    }

    #[async_trait]
    impl LinkTx for StaticTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.decoder.push(&request);
            while let Ok(Some(frame)) = self.decoder.next_frame() {
                let reply = match frame.command {
                    STATE_COMMAND if frame.body.is_empty() => {
                        let state = self.state.lock().expect("static state");
                        encode_frame(Direction::Inbound, frame.command, &state)
                    }
                    // The stale flag never changes in this state.
                    LDAC_STATE_COMMAND if frame.body.is_empty() => {
                        encode_frame(Direction::Inbound, frame.command, &[1])
                    }
                    SOUND_MODE_COMMAND
                    | EQUALIZER_COMMAND
                    | AUTO_POWER_OFF_COMMAND
                    | BUTTON_CONFIGURATION_COMMAND
                    | SOUND_MODE_CYCLE_COMMAND
                    | LIMIT_HIGH_VOLUME_COMMAND
                    | LIMIT_HIGH_VOLUME_REFRESH_RATE_COMMAND
                    | DUAL_CONNECTIONS_COMMAND => {
                        encode_frame(Direction::Inbound, frame.command, &[])
                    }
                    _ => continue,
                };
                let reply = reply.map_err(|_| TransportError::Closed)?;
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
