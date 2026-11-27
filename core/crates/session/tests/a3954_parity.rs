#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::time::Duration;

    use bytes::Bytes;
    use openscl4_models::{
        A3954_AUTO_POWER_OFF, A3954_CASE_FEATURES, A3954_CASE_LANGUAGE, A3954_EQUALIZER_BANDS,
        A3954_EQUALIZER_BANDS_SECOND, A3954_EQUALIZER_PRESET, A3954_SOUND_LEAK_COMPENSATION,
        A3954_SPATIAL_AUDIO, AutoPowerOff, AutoPowerOffSetting, CaseFeatures, CaseLanguage,
        EqualizerSetting, ModelDriver, SoundMode, SpatialAudio, SpatialAudioMode,
        SpatialAudioMusicMode,
    };
    use openscl4_protocol::{Direction, FrameDecoder, encode_frame};
    use openscl4_session::{
        CommandOutcome, ConnectionState, DeviceState, PendingCommand, SessionCommand,
        SessionHandle, SessionRunner,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};
    use openscl4_transport_mock::VirtualSoundcoreDevice;
    use tokio::time::timeout;

    use super::capture_replay::Replay;

    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const EQUALIZER_COMMAND: [u8; 2] = [0x03, 0x87];
    const CASE_FEATURES_COMMAND: [u8; 2] = [0x07, 0x87];
    const CASE_LANGUAGE_COMMAND: [u8; 2] = [0x07, 0x8A];
    const SPATIAL_AUDIO_COMMAND: [u8; 2] = [0x10, 0x81];
    const SOUND_LEAK_COMPENSATION_COMMAND: [u8; 2] = [0x10, 0x9A];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];

    const IDENTITY: &[u8] = b"04.2904.29A3954DEMO0000000";
    const REFERENCE_STATE: &[u8] = include_bytes!("../../../data/a3954-legacy-state.bin");

    /// The curve from the legacy driver's `set_eq_packet` example: eight visible
    /// bands plus the two fixed bands the shared curve keeps at zero.
    const REFERENCE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [60, 0, 0, 0, 0, 0, 0, -60, 0, 0],
    };

    /// The exact `03 87` body asserted by the legacy `set_eq_packet` test for
    /// `REFERENCE_CURVE` over the reference state.
    const REFERENCE_EQUALIZER_WRITE: [u8; 114] = [
        0xfe, 0xfe, 0x00, 0x00, 0xb4, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x3c, 0x78, 0x78, 0xb4,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x3c, 0x78, 0x78, 0x00, 0x00, 0x00, 0x91, 0x93, 0x8b,
        0x8d, 0x7a, 0x6f, 0x69, 0x64, 0x3c, 0x3c, 0x91, 0x93, 0x8b, 0x8d, 0x7a, 0x6f, 0x69, 0x64,
        0x3c, 0x3c, 0x00, 0x00, 0x00, 0x00, 0x01, 0x91, 0x93, 0x8b, 0x82, 0x7a, 0x85, 0x85, 0x72,
        0x3c, 0x3c, 0x91, 0x93, 0x8b, 0x82, 0x7a, 0x85, 0x85, 0x72, 0x3c, 0x3c, 0xb4, 0x00, 0x78,
        0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x3c, 0x00, 0x78, 0x00,
        0x78, 0x00, 0xb4, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78, 0x00, 0x78,
        0x00, 0x3c, 0x00, 0x78, 0x00, 0x78, 0x00, 0x00, 0x00,
    ];

    #[tokio::test(start_paused = true)]
    async fn reads_the_legacy_reference_state() {
        let handle = start_replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), Some(identity())),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&reference_body())),
            ),
        ]));
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3954);
        assert_eq!(state.tws_connected, Some(true));
        assert_eq!(state.battery.left, Some(99));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(100));
        assert_eq!(
            state.firmware_left.map(|version| version.to_string()),
            Some("03.29".to_owned())
        );
        assert_eq!(
            state.firmware_right.map(|version| version.to_string()),
            Some("03.29".to_owned())
        );
        assert_eq!(
            state.firmware_case.map(|version| version.to_string()),
            Some("02.58".to_owned())
        );
        assert_eq!(state.sound_mode, Some(SoundMode::Normal));
        assert_eq!(
            state.sound_mode_setting(),
            Some(openscl4_session::SoundModeSetting {
                mode: SoundMode::Normal,
                position: 6,
            })
        );
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [0; 10],
            })
        );
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
        assert_eq!(state.case_features, Some(CaseFeatures::default()));
        assert_eq!(
            state.spatial_audio,
            Some(SpatialAudio {
                enabled: false,
                mode: SpatialAudioMode::Music,
                music_mode: SpatialAudioMusicMode::Fixed,
            })
        );
        assert_eq!(state.sound_leak_compensation, Some(false));
        assert_eq!(state.case_language, Some(CaseLanguage::English));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_legacy_equalizer_write_from_a_fresh_state_reply() {
        let initial = reference_body();
        let mut applied = initial.clone();
        // The legacy state modifier writes the curve to both channels, disables
        // hear id, and disables spatial audio.
        applied[A3954_EQUALIZER_PRESET].copy_from_slice(&0xfefeu16.to_le_bytes());
        let bands = [0xb4, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x3c, 0x78, 0x78];
        applied[A3954_EQUALIZER_BANDS].copy_from_slice(&bands);
        applied[A3954_EQUALIZER_BANDS_SECOND].copy_from_slice(&bands);
        applied[67] = 0;
        applied[A3954_SPATIAL_AUDIO.start] = 0;

        let handle = start_replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), Some(identity())),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(EQUALIZER_COMMAND, &REFERENCE_EQUALIZER_WRITE), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&applied))),
        ]));
        assert_eq!(
            ready(&handle).await.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [0; 10],
            })
        );

        handle
            .set_eq_live(REFERENCE_CURVE)
            .expect("accept live curve");
        let confirmed = wait_for_equalizer(&handle, REFERENCE_CURVE).await;
        assert_eq!(confirmed.equalizer, Some(REFERENCE_CURVE));
        assert_eq!(
            confirmed.spatial_audio,
            Some(SpatialAudio {
                enabled: false,
                mode: SpatialAudioMode::Music,
                music_mode: SpatialAudioMusicMode::Fixed,
            })
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn applies_and_restores_the_reference_curve_on_the_virtual_device() {
        let handle = SessionRunner::start(VirtualSoundcoreDevice, target());
        let initial = ready(&handle).await;
        assert_eq!(
            initial.equalizer,
            Some(EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: [18, 8, 0, 0, 0, 0, 0, 0, 0, 0],
            })
        );
        handle
            .set_eq_live(REFERENCE_CURVE)
            .expect("accept live curve");
        let changed = wait_for_equalizer(&handle, REFERENCE_CURVE).await;
        assert_eq!(changed.equalizer, Some(REFERENCE_CURVE));

        let original = initial.equalizer.expect("initial curve");
        handle.set_eq_live(original).expect("accept restore curve");
        let restored = wait_for_equalizer(&handle, original).await;
        assert_eq!(restored.equalizer, Some(original));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_the_equalizer_write_when_one_earbud_is_disconnected() {
        let mut body = reference_body();
        body[1] = 0;
        let handle = start_replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), Some(identity())),
            (request(STATE_COMMAND, &[]), Some(inbound(&body))),
        ]));
        assert_eq!(ready(&handle).await.equalizer, None);
        assert_eq!(
            handle.set_eq_live(REFERENCE_CURVE),
            Err(openscl4_session::DispatchError::UnsupportedEqualizer)
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_case_feature_writes_from_reference_replies() {
        let initial = reference_body();
        let target = CaseFeatures {
            atmospheric: false,
            remote_camera: false,
            find_device: false,
            case_spatial_audio: true,
        };
        let mut enabled = initial.clone();
        enabled[A3954_CASE_FEATURES].copy_from_slice(&target.bytes());
        let handle = start_replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), Some(identity())),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (
                request(CASE_FEATURES_COMMAND, &[0x00, 0x00, 0x00, 0x01]),
                None,
            ),
            (request(STATE_COMMAND, &[]), Some(inbound(&enabled))),
        ]));
        ready(&handle).await;

        let CommandOutcome::Applied(state) = wait(
            handle
                .dispatch(SessionCommand::SetCaseFeatures(target))
                .expect("queue case features"),
        )
        .await
        .expect("confirm case features");
        assert_eq!(state.case_features, Some(target));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_spatial_audio_writes_from_reference_replies() {
        let initial = reference_body();
        let target = SpatialAudio {
            enabled: true,
            mode: SpatialAudioMode::Music,
            music_mode: SpatialAudioMusicMode::Fixed,
        };
        let mut enabled = initial.clone();
        enabled[A3954_SPATIAL_AUDIO].copy_from_slice(&target.bytes());
        let handle = start_replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), Some(identity())),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(SPATIAL_AUDIO_COMMAND, &[0x01, 0x01, 0x00]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&enabled))),
        ]));
        ready(&handle).await;

        let CommandOutcome::Applied(state) = wait(
            handle
                .dispatch(SessionCommand::SetSpatialAudio(target))
                .expect("queue spatial audio"),
        )
        .await
        .expect("confirm spatial audio");
        assert_eq!(state.spatial_audio, Some(target));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_sound_leak_compensation_writes_from_reference_replies() {
        let initial = reference_body();
        let mut enabled = initial.clone();
        enabled[A3954_SOUND_LEAK_COMPENSATION] = 1;
        let handle = start_replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), Some(identity())),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(SOUND_LEAK_COMPENSATION_COMMAND, &[0x01]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&enabled))),
        ]));
        ready(&handle).await;

        let CommandOutcome::Applied(state) = wait(
            handle
                .dispatch(SessionCommand::SetSoundLeakCompensation(true))
                .expect("queue sound leak compensation"),
        )
        .await
        .expect("confirm sound leak compensation");
        assert_eq!(state.sound_leak_compensation, Some(true));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_case_language_writes_from_reference_replies() {
        let initial = reference_body();
        let mut chinese = initial.clone();
        chinese[A3954_CASE_LANGUAGE] = 2;
        let handle = start_replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), Some(identity())),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(CASE_LANGUAGE_COMMAND, &[0x02]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&chinese))),
        ]));
        ready(&handle).await;

        let CommandOutcome::Applied(state) = wait(
            handle
                .dispatch(SessionCommand::SetCaseLanguage(CaseLanguage::Chinese))
                .expect("queue case language"),
        )
        .await
        .expect("confirm case language");
        assert_eq!(state.case_language, Some(CaseLanguage::Chinese));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_auto_power_off_writes_from_reference_replies() {
        let initial = reference_body();
        let mut disabled = initial.clone();
        disabled[A3954_AUTO_POWER_OFF].copy_from_slice(&[0, 3]);
        let mut enabled = initial.clone();
        enabled[A3954_AUTO_POWER_OFF].copy_from_slice(&[1, 3]);
        // Disabling keeps the device's previous duration index and the enable
        // write carries the requested half-hour table index.
        let mut disabled_from_three = disabled.clone();
        disabled_from_three[A3954_AUTO_POWER_OFF].copy_from_slice(&[0, 3]);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), Some(identity())),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(STATE_COMMAND, &[]), Some(inbound(&initial))),
            (request(AUTO_POWER_OFF_COMMAND, &[0x01, 0x03]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&enabled))),
            (request(STATE_COMMAND, &[]), Some(inbound(&enabled))),
            (request(AUTO_POWER_OFF_COMMAND, &[0x00, 0x03]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&disabled_from_three)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let state = ready(&handle).await;
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );

        let CommandOutcome::Applied(enabled_state) = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Enabled(3),
                ))
                .expect("queue enable"),
        )
        .await
        .expect("confirm enable");
        assert_eq!(
            enabled_state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 3,
            })
        );

        let CommandOutcome::Applied(disabled_state) = wait(
            handle
                .dispatch(SessionCommand::SetAutoPowerOff(
                    AutoPowerOffSetting::Disabled,
                ))
                .expect("queue disable"),
        )
        .await
        .expect("confirm disable");
        assert_eq!(
            disabled_state.auto_power_off,
            Some(AutoPowerOff {
                enabled: false,
                duration: 3,
            })
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn a_case_feature_write_that_the_device_ignores_is_not_confirmed() {
        // The device acknowledges and keeps returning the reference state: the
        // write must stay unconfirmed instead of reporting Applied.
        struct StaticReference;
        #[async_trait::async_trait]
        impl openscl4_transport::TransportProvider for StaticReference {
            async fn connect(
                &self,
                _target: &ConnectTarget,
            ) -> Result<
                Box<dyn openscl4_transport::TransportLink>,
                openscl4_transport::TransportError,
            > {
                Ok(Box::new(StaticLink {
                    decoder: FrameDecoder::default(),
                    body: reference_body(),
                }))
            }
        }
        struct StaticLink {
            decoder: FrameDecoder,
            body: Vec<u8>,
        }
        impl openscl4_transport::TransportLink for StaticLink {
            fn split(
                self: Box<Self>,
            ) -> (
                Box<dyn openscl4_transport::LinkTx>,
                Box<dyn openscl4_transport::LinkRx>,
            ) {
                let (replies, incoming) = tokio::sync::mpsc::unbounded_channel();
                (
                    Box::new(StaticTx {
                        decoder: self.decoder,
                        body: self.body,
                        replies,
                    }),
                    Box::new(StaticRx(incoming)),
                )
            }
        }
        struct StaticTx {
            decoder: FrameDecoder,
            body: Vec<u8>,
            replies: tokio::sync::mpsc::UnboundedSender<Bytes>,
        }
        #[async_trait::async_trait]
        impl openscl4_transport::LinkTx for StaticTx {
            async fn send(
                &mut self,
                request: Bytes,
            ) -> Result<(), openscl4_transport::TransportError> {
                self.decoder.push(&request);
                while let Ok(Some(frame)) = self.decoder.next_frame() {
                    let reply = match frame.command {
                        IDENTITY_COMMAND if frame.body.is_empty() => {
                            encode_frame(Direction::Inbound, IDENTITY_COMMAND, IDENTITY)
                        }
                        STATE_COMMAND if frame.body.is_empty() => {
                            encode_frame(Direction::Inbound, STATE_COMMAND, &self.body)
                        }
                        CASE_FEATURES_COMMAND => {
                            encode_frame(Direction::Inbound, CASE_FEATURES_COMMAND, &[])
                        }
                        _ => continue,
                    }
                    .map_err(|_| openscl4_transport::TransportError::Closed)?;
                    self.replies
                        .send(reply)
                        .map_err(|_| openscl4_transport::TransportError::Closed)?;
                }
                Ok(())
            }

            async fn close(&mut self) {}
        }
        struct StaticRx(tokio::sync::mpsc::UnboundedReceiver<Bytes>);
        #[async_trait::async_trait]
        impl openscl4_transport::LinkRx for StaticRx {
            async fn recv(&mut self) -> Result<Bytes, openscl4_transport::TransportError> {
                self.0
                    .recv()
                    .await
                    .ok_or(openscl4_transport::TransportError::Closed)
            }
        }

        let handle = SessionRunner::start(StaticReference, target());
        ready(&handle).await;
        let target_features = CaseFeatures {
            case_spatial_audio: true,
            ..CaseFeatures::default()
        };
        let result = wait(
            handle
                .dispatch(SessionCommand::SetCaseFeatures(target_features))
                .expect("queue case features"),
        )
        .await;
        assert_eq!(
            result,
            Err(openscl4_session::CommandError::CaseFeaturesMismatch {
                requested: target_features,
                observed: Some(CaseFeatures::default()),
            })
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_case_settings_on_a_model_that_does_not_report_them() {
        // The A3949 state carries none of the A3954 case fields.
        let state = include_bytes!("../../../data/a3949-pilot-state.bin");
        let mut decoder = FrameDecoder::default();
        decoder.push(state);
        let body = decoder.next_frame().expect("frame").expect("state").body;
        let handle = start_replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&body))),
        ]));
        assert_eq!(ready(&handle).await.model, ModelDriver::A3949);
        let result = wait(
            handle
                .dispatch(SessionCommand::SetCaseFeatures(CaseFeatures::default()))
                .expect("queue the typed rejection"),
        )
        .await;
        assert_eq!(
            result,
            Err(openscl4_session::CommandError::Model(
                openscl4_models::ModelError::InvalidCaseFeaturesWrite
            ))
        );
        handle.close().await.expect("close virtual link");
    }

    fn reference_body() -> Vec<u8> {
        let mut decoder = FrameDecoder::default();
        decoder.push(REFERENCE_STATE);
        decoder
            .next_frame()
            .expect("decoded reference frame")
            .expect("complete reference frame")
            .body
            .to_vec()
    }

    fn identity() -> Bytes {
        encode_frame(Direction::Inbound, IDENTITY_COMMAND, IDENTITY).expect("identity frame")
    }

    fn inbound(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, STATE_COMMAND, body).expect("inbound state frame")
    }

    fn request(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        }
    }

    fn start_replay(exchanges: VecDeque<(Bytes, Option<Bytes>)>) -> SessionHandle {
        SessionRunner::start(Replay(exchanges), target())
    }

    async fn ready(handle: &SessionHandle) -> DeviceState {
        let view = timeout(Duration::from_secs(3), handle.confirmed_view())
            .await
            .expect("sync deadline")
            .expect("confirmed state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a confirmed state");
        };
        state
    }

    async fn wait(pending: PendingCommand) -> openscl4_session::CommandResult {
        timeout(Duration::from_secs(10), pending.wait())
            .await
            .expect("command deadline")
    }

    async fn wait_for_equalizer(
        handle: &SessionHandle,
        requested: EqualizerSetting,
    ) -> DeviceState {
        let mut view = handle.subscribe();
        let snapshot = timeout(
            Duration::from_secs(3),
            view.wait_for(|view| {
                matches!(view.connection, ConnectionState::Ready(state) if state.equalizer == Some(requested))
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
}
