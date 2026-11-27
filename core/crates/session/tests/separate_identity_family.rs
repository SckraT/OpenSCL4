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
    use openscl4_models::{EqualizerSetting, ModelError};
    use openscl4_protocol::{Direction, FrameDecoder, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState, DispatchError,
        ModelDriver, PendingCommand, QuickPresetValues, SessionCommand, SessionHandle,
        SessionRunner, SoundMode, UnidentifiedReason, UnidentifiedState,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualA3909, VirtualA3926, VirtualA3930, VirtualA3951};

    use super::capture_replay::Replay;

    // Reference wire opcodes from the legacy drivers and packet builders.
    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const LDAC_STATE_COMMAND: [u8; 2] = [0x01, 0x7f];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const EQUALIZER_COMMAND: [u8; 2] = [0x03, 0x87];

    const A3909_IDENTITY: &[u8] = include_bytes!("../../../data/a3909-identity.bin");
    const A3909_STATE: &[u8] = include_bytes!("../../../data/a3909-state.bin");
    const A3926_IDENTITY: &[u8] = include_bytes!("../../../data/a3926-identity.bin");
    const A3926_STATE: &[u8] = include_bytes!("../../../data/a3926-state.bin");
    const A3930_IDENTITY: &[u8] = include_bytes!("../../../data/a3930-identity.bin");
    const A3930_STATE: &[u8] = include_bytes!("../../../data/a3930-state.bin");
    const A3930_LDAC: &[u8] = include_bytes!("../../../data/a3930-ldac.bin");
    const A3951_IDENTITY: &[u8] = include_bytes!("../../../data/a3951-identity.bin");
    const A3951_STATE: &[u8] = include_bytes!("../../../data/a3951-state.bin");
    const A3951_LDAC: &[u8] = include_bytes!("../../../data/a3951-ldac.bin");

    /// The legacy signature curve: preset 0 and eight `0x78` band bytes.
    const SIGNATURE_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0,
        volume_adjustments: [0; 10],
    };

    /// Legacy `03 87` body for the signature curve on each fixture state.
    const A3926_SIGNATURE_WRITE: &[u8] = &[
        0x00, 0x00, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x01, 0x02, 0x00, 0x00, 0x3c, 0x78, 0xb4, 0xf0, 0xfe, 0x6c,
        0x84, 0x79, 0x7a, 0x7b, 0x7c, 0x7d, 0x7e, 0x7f, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78,
    ];
    const A3930_SIGNATURE_WRITE: &[u8] = &[
        0x00, 0x00, 0x03, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x01, 0x02, 0x00, 0x46, 0x50, 0x5a, 0x64, 0x6e, 0x78, 0x82,
        0x8c, 0x96, 0xa0, 0xaa, 0xb4, 0xbe, 0xc8, 0xd2, 0xdc, 0x01, 0x02, 0x03, 0x04, 0x02, 0x14,
        0x28, 0x3c, 0x50, 0x64, 0x78, 0x8c, 0xa0, 0x5a, 0x64, 0x6e, 0x78, 0x82, 0x8c, 0x96, 0xa0,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78,
    ];
    const A3951_SIGNATURE_WRITE: &[u8] = &[
        0x00, 0x00, 0x02, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x02, 0x03, 0x00, 0x3c, 0x50, 0x64, 0x78, 0x8c, 0xa0, 0xb4,
        0xc8, 0x6e, 0x78, 0x82, 0x8c, 0x96, 0xa0, 0xaa, 0xb4, 0x01, 0x02, 0x03, 0x04, 0x01, 0x1e,
        0x32, 0x46, 0x5a, 0x6e, 0x82, 0x96, 0xaa, 0x50, 0x5a, 0x64, 0x6e, 0x78, 0x82, 0x8c, 0x96,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78,
    ];

    #[tokio::test(start_paused = true)]
    async fn reads_the_family_snapshots_from_its_separate_reads() {
        let cases = [
            (
                FamilyCase {
                    provider: FamilyProvider::A3909,
                    model: ModelDriver::A3909,
                    commands: vec![[0x01, 0x05], [0x01, 0x01]],
                },
                FamilyExpectation {
                    firmware_left: Some("01.28"),
                    firmware_right: Some("01.28"),
                    left: None,
                    right: Some(100),
                    equalizer: None,
                    sound_mode: None,
                    ldac: None,
                },
            ),
            (
                FamilyCase {
                    provider: FamilyProvider::A3926,
                    model: ModelDriver::A3926,
                    commands: vec![[0x01, 0x05], [0x01, 0x01]],
                },
                FamilyExpectation {
                    firmware_left: Some("01.60"),
                    firmware_right: Some("01.60"),
                    left: Some(60),
                    right: Some(80),
                    equalizer: Some(EqualizerSetting {
                        preset_id: 0xfefe,
                        volume_adjustments: [-60, -30, 0, 30, 60, -10, 10, 20, 0, 0],
                    }),
                    sound_mode: None,
                    ldac: None,
                },
            ),
            (
                FamilyCase {
                    provider: FamilyProvider::A3930,
                    model: ModelDriver::A3930,
                    commands: vec![[0x01, 0x05], [0x01, 0x7f], [0x01, 0x01]],
                },
                FamilyExpectation {
                    firmware_left: Some("01.70"),
                    firmware_right: Some("01.70"),
                    left: Some(80),
                    right: Some(100),
                    equalizer: Some(EqualizerSetting {
                        preset_id: 0xfefe,
                        volume_adjustments: [-120, -90, -60, -30, 0, 30, 60, 90, 0, 0],
                    }),
                    sound_mode: Some(SoundMode::Normal),
                    ldac: Some(false),
                },
            ),
            (
                FamilyCase {
                    provider: FamilyProvider::A3951,
                    model: ModelDriver::A3951,
                    commands: vec![[0x01, 0x05], [0x01, 0x7f], [0x01, 0x01]],
                },
                FamilyExpectation {
                    firmware_left: Some("01.80"),
                    firmware_right: Some("01.80"),
                    left: Some(100),
                    right: Some(40),
                    equalizer: Some(EqualizerSetting {
                        preset_id: 0xfefe,
                        volume_adjustments: [-100, -50, 0, 50, 100, -20, 20, 40, 0, 0],
                    }),
                    sound_mode: Some(SoundMode::Normal),
                    ldac: Some(true),
                },
            ),
        ];

        for (case, expectation) in cases {
            let (handle, frames) = start(case.provider);
            let state = ready(&handle).await;
            assert_eq!(state.model, case.model);
            assert_eq!(
                state.firmware_left.map(|version| version.to_string()),
                expectation.firmware_left.map(str::to_owned),
                "{:?}",
                case.model
            );
            assert_eq!(
                state.firmware_right.map(|version| version.to_string()),
                expectation.firmware_right.map(str::to_owned),
                "{:?}",
                case.model
            );
            assert_eq!(state.battery.left, expectation.left, "{:?}", case.model);
            assert_eq!(state.battery.right, expectation.right, "{:?}", case.model);
            assert_eq!(state.battery.device, None);
            assert_eq!(state.equalizer, expectation.equalizer, "{:?}", case.model);
            assert_eq!(state.sound_mode, expectation.sound_mode, "{:?}", case.model);
            assert_eq!(state.ldac, expectation.ldac, "{:?}", case.model);
            assert_eq!(state.gaming_mode, None);
            assert_eq!(state.auto_power_off, None);
            handle.close().await.expect("close virtual link");

            let commands: Vec<[u8; 2]> = frames
                .lock()
                .expect("request trace")
                .iter()
                .filter_map(|frame| command(frame))
                .collect();
            assert_eq!(commands, case.commands, "{:?}", case.model);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_ambient_mode_cycle_from_read_back_replies() {
        // A3930: Normal -> Transparency -> Normal.
        let initial = body(A3930_STATE, STATE_COMMAND);
        let transparent = {
            let mut body = initial.to_vec();
            body[77] = 1;
            Body::into_frame(body)
        };
        let replay = Replay(VecDeque::from([
            (
                request(IDENTITY_COMMAND, &[]),
                Some(inbound_frame(A3930_IDENTITY)),
            ),
            (
                request(LDAC_STATE_COMMAND, &[]),
                Some(inbound_frame(A3930_LDAC)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3930_STATE)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3930_STATE)),
            ),
            (
                request(SOUND_MODE_COMMAND, &[1, 0, 0, 0]),
                Some(inbound_ack(SOUND_MODE_COMMAND)),
            ),
            (request(STATE_COMMAND, &[]), Some(transparent.clone())),
            (request(STATE_COMMAND, &[]), Some(transparent)),
            (
                request(SOUND_MODE_COMMAND, &[2, 0, 0, 0]),
                Some(inbound_ack(SOUND_MODE_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3930_STATE)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let snapshot = ready(&handle).await;
        assert_eq!(snapshot.sound_mode, Some(SoundMode::Normal));
        assert_eq!(snapshot.ldac, Some(false));
        assert_eq!(
            snapshot.firmware_left.map(|f| f.to_string()),
            Some("01.70".into())
        );

        let CommandOutcome::Applied(changed) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Transparency))
                .expect("queue transparency"),
        )
        .await
        .expect("confirm transparency");
        assert_eq!(changed.sound_mode, Some(SoundMode::Transparency));
        assert_eq!(changed.firmware_left, snapshot.firmware_left);
        assert_eq!(changed.ldac, snapshot.ldac);

        let CommandOutcome::Applied(restored) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue normal"),
        )
        .await
        .expect("confirm normal");
        assert_eq!(restored.sound_mode, Some(SoundMode::Normal));
        assert_eq!(restored.equalizer, snapshot.equalizer);
        assert_eq!(ready(&handle).await, restored);
        handle.close().await.expect("close virtual link");

        // A3951: Normal -> NoiseCanceling -> Normal, keeping the transparency
        // and custom fields of the fresh state.
        let initial = body(A3951_STATE, STATE_COMMAND);
        let noise_canceling = {
            let mut body = initial.to_vec();
            body[77] = 0;
            Body::into_frame(body)
        };
        let replay = Replay(VecDeque::from([
            (
                request(IDENTITY_COMMAND, &[]),
                Some(inbound_frame(A3951_IDENTITY)),
            ),
            (
                request(LDAC_STATE_COMMAND, &[]),
                Some(inbound_frame(A3951_LDAC)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3951_STATE)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3951_STATE)),
            ),
            (
                request(SOUND_MODE_COMMAND, &[0, 0, 1, 0]),
                Some(inbound_ack(SOUND_MODE_COMMAND)),
            ),
            (request(STATE_COMMAND, &[]), Some(noise_canceling.clone())),
            (request(STATE_COMMAND, &[]), Some(noise_canceling)),
            (
                request(SOUND_MODE_COMMAND, &[2, 0, 1, 0]),
                Some(inbound_ack(SOUND_MODE_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3951_STATE)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let snapshot = ready(&handle).await;
        assert_eq!(snapshot.sound_mode, Some(SoundMode::Normal));
        assert_eq!(snapshot.ldac, Some(true));

        let CommandOutcome::Applied(changed) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(
                    SoundMode::NoiseCanceling,
                ))
                .expect("queue noise canceling"),
        )
        .await
        .expect("confirm noise canceling");
        assert_eq!(changed.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(changed.ldac, snapshot.ldac);

        let CommandOutcome::Applied(restored) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue normal"),
        )
        .await
        .expect("confirm normal");
        assert_eq!(restored.sound_mode, Some(SoundMode::Normal));
        assert_eq!(restored.equalizer, snapshot.equalizer);
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_family_equalizer_write_from_read_back_replies() {
        struct Case {
            model: ModelDriver,
            identity: &'static [u8],
            state: &'static [u8],
            ldac: Option<&'static [u8]>,
            write: &'static [u8],
        }
        let cases = [
            Case {
                model: ModelDriver::A3926,
                identity: A3926_IDENTITY,
                state: A3926_STATE,
                ldac: None,
                write: A3926_SIGNATURE_WRITE,
            },
            Case {
                model: ModelDriver::A3930,
                identity: A3930_IDENTITY,
                state: A3930_STATE,
                ldac: Some(A3930_LDAC),
                write: A3930_SIGNATURE_WRITE,
            },
            Case {
                model: ModelDriver::A3951,
                identity: A3951_IDENTITY,
                state: A3951_STATE,
                ldac: Some(A3951_LDAC),
                write: A3951_SIGNATURE_WRITE,
            },
        ];

        for case in cases {
            let initial_body = body(case.state, STATE_COMMAND);
            let signature_body = {
                let mut body = initial_body.to_vec();
                body[6..8].copy_from_slice(&[0, 0]);
                body[8..24].fill(0x78);
                body
            };
            let mut exchanges = VecDeque::from([
                (
                    request(IDENTITY_COMMAND, &[]),
                    Some(inbound_frame(case.identity)),
                ),
                (request(STATE_COMMAND, &[]), Some(inbound_frame(case.state))),
                (request(STATE_COMMAND, &[]), Some(inbound_frame(case.state))),
                (
                    request(EQUALIZER_COMMAND, case.write),
                    Some(inbound_ack(EQUALIZER_COMMAND)),
                ),
                (
                    request(STATE_COMMAND, &[]),
                    Some(Body::into_frame(signature_body)),
                ),
            ]);
            if let Some(ldac) = case.ldac {
                exchanges.insert(
                    1,
                    (request(LDAC_STATE_COMMAND, &[]), Some(inbound_frame(ldac))),
                );
            }
            let handle = SessionRunner::start(Replay(exchanges), target());
            let initial = ready(&handle).await;
            assert_eq!(initial.model, case.model);
            assert_ne!(initial.equalizer, Some(SIGNATURE_CURVE));

            let CommandOutcome::Applied(applied) = wait(
                handle
                    .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                        equalizer: Some(SIGNATURE_CURVE),
                        ..QuickPresetValues::default()
                    }))
                    .expect("queue the signature curve"),
            )
            .await
            .expect("confirm the signature curve");
            assert_eq!(applied.equalizer, Some(SIGNATURE_CURVE), "{:?}", case.model);
            assert_eq!(applied.firmware_left, initial.firmware_left);
            assert_eq!(applied.ldac, initial.ldac);
            assert_eq!(ready(&handle).await, applied);
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn an_ack_without_a_changed_state_does_not_confirm_the_equalizer_write() {
        let fixture = A3926_STATE;
        let (handle, frames) = start_static(fixture, A3926_IDENTITY, EQUALIZER_COMMAND);
        let initial = ready(&handle).await;
        assert_eq!(
            initial.model,
            ModelDriver::A3926,
            "the static responder serves the A3926 identity"
        );

        let result = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    equalizer: Some(SIGNATURE_CURVE),
                    ..QuickPresetValues::default()
                }))
                .expect("queue the signature curve"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::PresetMismatch {
                field: openscl4_session::PresetFieldMismatch::Equalizer {
                    requested: SIGNATURE_CURVE,
                    observed: initial.equalizer,
                },
                confirmed: initial,
            })
        );
        assert_eq!(
            count(&frames, &request(EQUALIZER_COMMAND, A3926_SIGNATURE_WRITE)),
            1,
            "the write reached the device once and was not confirmed"
        );
        assert_eq!(ready(&handle).await, initial);
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_unavailable_modes_and_equalizers_before_transmission() {
        // The A3930 offers no noise-canceling ambient mode.
        let replay = Replay(VecDeque::from([
            (
                request(IDENTITY_COMMAND, &[]),
                Some(inbound_frame(A3930_IDENTITY)),
            ),
            (
                request(LDAC_STATE_COMMAND, &[]),
                Some(inbound_frame(A3930_LDAC)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3930_STATE)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(
                    SoundMode::NoiseCanceling,
                ))
                .expect("queue the rejected mode"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidSoundModeWrite))
        );
        handle.close().await.expect("close virtual link");

        // The A3926 has no sound modes at all.
        let replay = Replay(VecDeque::from([
            (
                request(IDENTITY_COMMAND, &[]),
                Some(inbound_frame(A3926_IDENTITY)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3926_STATE)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue the rejected mode"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidSoundModeWrite))
        );
        handle.close().await.expect("close virtual link");

        // The A3909 state does not expose a curve, so no equalizer write is
        // accepted and no live curve can be published.
        let replay = Replay(VecDeque::from([
            (
                request(IDENTITY_COMMAND, &[]),
                Some(inbound_frame(A3909_IDENTITY)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3909_STATE)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        assert_eq!(
            handle.set_eq_live(SIGNATURE_CURVE),
            Err(DispatchError::UnsupportedEqualizer)
        );
        let result = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    equalizer: Some(SIGNATURE_CURVE),
                    ..QuickPresetValues::default()
                }))
                .expect("queue the rejected curve"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidEqualizerWrite))
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn an_unavailable_identity_leaves_the_family_read_only() {
        // The state reply names no model because the family has no serial in it.
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3930_STATE)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let view = tokio::time::timeout(Duration::from_secs(5), handle.confirmed_view())
            .await
            .expect("identity and state deadline")
            .expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: None,
                reason: UnidentifiedReason::UnknownIdentity,
            })
        );
        assert_eq!(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Transparency))
                .err(),
            Some(DispatchError::ReadOnly)
        );
        handle.close().await.expect("close the read-only link");
    }

    #[tokio::test(start_paused = true)]
    async fn a_state_outside_the_identified_model_layout_is_read_only() {
        // The identity names A3930, but the state is the 59-byte A3926 layout.
        let replay = Replay(VecDeque::from([
            (
                request(IDENTITY_COMMAND, &[]),
                Some(inbound_frame(A3930_IDENTITY)),
            ),
            (
                request(LDAC_STATE_COMMAND, &[]),
                Some(inbound_frame(A3930_LDAC)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound_frame(A3926_STATE)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let view = tokio::time::timeout(Duration::from_secs(5), handle.confirmed_view())
            .await
            .expect("identity and state deadline")
            .expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3930),
                reason: UnidentifiedReason::IncompatibleState,
            })
        );
        handle.close().await.expect("close the read-only link");
    }

    struct FamilyCase {
        provider: FamilyProvider,
        model: ModelDriver,
        commands: Vec<[u8; 2]>,
    }

    struct FamilyExpectation {
        firmware_left: Option<&'static str>,
        firmware_right: Option<&'static str>,
        left: Option<u8>,
        right: Option<u8>,
        equalizer: Option<EqualizerSetting>,
        sound_mode: Option<SoundMode>,
        ldac: Option<bool>,
    }

    #[derive(Clone, Copy)]
    enum FamilyProvider {
        A3909,
        A3926,
        A3930,
        A3951,
    }

    #[async_trait]
    impl TransportProvider for FamilyProvider {
        async fn connect(
            &self,
            target: &ConnectTarget,
        ) -> Result<Box<dyn openscl4_transport::TransportLink>, TransportError> {
            match self {
                Self::A3909 => VirtualA3909.connect(target).await,
                Self::A3926 => VirtualA3926.connect(target).await,
                Self::A3930 => VirtualA3930.connect(target).await,
                Self::A3951 => VirtualA3951.connect(target).await,
            }
        }
    }

    struct Body;

    impl Body {
        fn into_frame(body: Vec<u8>) -> Bytes {
            encode_frame(Direction::Inbound, STATE_COMMAND, &body).expect("inbound state frame")
        }
    }

    fn body(fixture: &[u8], command: [u8; 2]) -> Vec<u8> {
        let mut decoder = FrameDecoder::default();
        decoder.push(fixture);
        let frame = decoder
            .next_frame()
            .expect("valid fixture")
            .expect("complete fixture");
        assert_eq!(frame.command, command);
        assert_eq!(frame.direction, Direction::Inbound);
        frame.body.to_vec()
    }

    fn inbound_frame(fixture: &[u8]) -> Bytes {
        let mut decoder = FrameDecoder::default();
        decoder.push(fixture);
        let frame = decoder
            .next_frame()
            .expect("valid fixture")
            .expect("complete fixture");
        encode_frame(frame.direction, frame.command, &frame.body).expect("re-encoded fixture")
    }

    fn request(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    fn inbound_ack(command: [u8; 2]) -> Bytes {
        encode_frame(Direction::Inbound, command, &[]).expect("inbound acknowledgement")
    }

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
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
        tokio::time::timeout(Duration::from_secs(10), pending.wait())
            .await
            .expect("command deadline")
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

    /// Serves one identity, one state, and an empty acknowledgement for the
    /// given write command while recording every outbound frame.
    fn start_static(
        state: &'static [u8],
        identity: &'static [u8],
        acknowledged: [u8; 2],
    ) -> (SessionHandle, Arc<Mutex<Vec<Vec<u8>>>>) {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let handle = SessionRunner::start(
            Recording {
                inner: StaticDevice {
                    state,
                    identity,
                    acknowledged,
                },
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

    struct StaticDevice {
        state: &'static [u8],
        identity: &'static [u8],
        acknowledged: [u8; 2],
    }

    #[async_trait]
    impl TransportProvider for StaticDevice {
        async fn connect(
            &self,
            _target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            Ok(Box::new(StaticLink {
                decoder: FrameDecoder::default(),
                state: self.state,
                identity: self.identity,
                acknowledged: self.acknowledged,
            }))
        }
    }

    struct StaticLink {
        decoder: FrameDecoder,
        state: &'static [u8],
        identity: &'static [u8],
        acknowledged: [u8; 2],
    }

    impl TransportLink for StaticLink {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            (
                Box::new(StaticTx {
                    decoder: self.decoder,
                    state: self.state,
                    identity: self.identity,
                    acknowledged: self.acknowledged,
                    replies: tx,
                }),
                Box::new(StaticRx(rx)),
            )
        }
    }

    struct StaticTx {
        decoder: FrameDecoder,
        state: &'static [u8],
        identity: &'static [u8],
        acknowledged: [u8; 2],
        replies: tokio::sync::mpsc::UnboundedSender<Bytes>,
    }

    #[async_trait]
    impl LinkTx for StaticTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.decoder.push(&request);
            while let Ok(Some(frame)) = self.decoder.next_frame() {
                let reply = match frame.command {
                    IDENTITY_COMMAND if frame.body.is_empty() => Some(inbound_frame(self.identity)),
                    STATE_COMMAND if frame.body.is_empty() => Some(inbound_frame(self.state)),
                    command if command == self.acknowledged => Some(
                        encode_frame(Direction::Inbound, command, &[])
                            .map_err(|_| TransportError::Closed)?,
                    ),
                    _ => None,
                };
                if let Some(reply) = reply {
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
