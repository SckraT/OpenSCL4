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
    use openscl4_models::{AutoPowerOff, AutoPowerOffSetting, EqualizerSetting, ModelError};
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, DeviceState, DispatchError,
        ModelDriver, PendingCommand, QuickPresetValues, SessionCommand, SessionHandle,
        SessionRunner, SoundMode,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualA3944, VirtualA3945, VirtualA3948, VirtualA3959};

    use super::capture_replay::Replay;

    // Wire opcodes from the legacy fixtures and packet builders.
    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const GAMING_MODE_COMMAND: [u8; 2] = [0x01, 0x87];
    const A3944_EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x83];
    const A3945_EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x81];
    const A3948_EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x83];
    const A3959_EQUALIZER_COMMAND: [u8; 2] = [0x02, 0x83];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];

    const A3944_FRAME: &[u8] = include_bytes!("../../../data/a3944-pilot-state.bin");
    const A3945_FRAME: &[u8] = include_bytes!("../../../data/a3945-pilot-state.bin");
    const A3948_FRAME: &[u8] = include_bytes!("../../../data/a3948-pilot-state.bin");
    const A3959_FRAME: &[u8] = include_bytes!("../../../data/a3959-pilot-state.bin");

    /// The A3944 legacy test's Acoustic-preset write body.
    const A3944_ACOUSTIC_WRITE: &[u8] = &[
        1, 0, 160, 130, 140, 140, 160, 160, 160, 140, 120, 0, 125, 118, 123, 120, 124, 122, 124,
        121, 120, 0,
    ];
    /// The A3945 legacy test's TrebleReducer two-channel write body.
    const A3945_TREBLE_WRITE: &[u8] = &[
        0x15, 0, 0x78, 0x78, 0x78, 0x64, 0x5a, 0x50, 0x50, 0x3c, 0x78, 0, 0x78, 0x78, 0x78, 0x64,
        0x5a, 0x50, 0x50, 0x3c, 0x78, 0,
    ];
    /// The A3959 legacy test's expectation for the requested curve.
    const A3959_LEGACY_EQ_WRITE: &[u8] = &[
        254, 254, 101, 120, 161, 171, 171, 152, 133, 85, 120, 120, 118, 119, 124, 123, 124, 121,
        123, 114, 120, 0,
    ];

    #[tokio::test(start_paused = true)]
    async fn reads_the_reference_state_of_each_virtual_model() {
        assert_a3944_reference(VirtualA3944).await;
        assert_a3945_reference(VirtualA3945).await;
        assert_a3948_reference(VirtualA3948).await;
        assert_a3959_reference(VirtualA3959).await;
    }

    #[tokio::test(start_paused = true)]
    async fn a3945_reads_its_case_battery_from_the_virtual_device() {
        let (handle, frames) = start(VirtualA3945);
        let state = ready(&handle).await;
        assert_eq!(state.battery.case, Some(0));
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

    #[tokio::test(start_paused = true)]
    async fn a3959_gaming_mode_follows_the_firmware_boundary() {
        // The captured 01.64 state exposes the flag and confirms the write.
        let (handle, frames) = start(VirtualA3959);
        let initial = ready(&handle).await;
        assert_eq!(initial.gaming_mode, Some(false));
        let CommandOutcome::Applied(enabled) = wait(
            handle
                .dispatch(SessionCommand::SetGamingMode(true))
                .expect("queue gaming mode"),
        )
        .await
        .expect("confirm gaming mode");
        assert_eq!(enabled.gaming_mode, Some(true));
        let CommandOutcome::Applied(disabled) = wait(
            handle
                .dispatch(SessionCommand::SetGamingMode(false))
                .expect("queue gaming off"),
        )
        .await
        .expect("confirm gaming off");
        assert_eq!(disabled.gaming_mode, Some(false));
        handle.close().await.expect("close virtual link");
        let requests: Vec<Bytes> = frames.lock().expect("request trace").clone();
        assert!(requests.contains(&request(GAMING_MODE_COMMAND, &[1])));
        assert!(requests.contains(&request(GAMING_MODE_COMMAND, &[0])));

        // The 01.59 state reports no flag, so the condition rejects the write
        // before transmission even though the capability is present.
        let (handle, frames) = start(VirtualA3959::with_old_firmware());
        let initial = ready(&handle).await;
        assert_eq!(initial.gaming_mode, None);
        assert_eq!(
            wait(
                handle
                    .dispatch(SessionCommand::SetGamingMode(true))
                    .expect("queue the typed rejection"),
            )
            .await,
            Err(CommandError::Model(ModelError::InvalidGamingModeWrite))
        );
        let requests: Vec<Bytes> = frames.lock().expect("request trace").clone();
        assert!(
            !requests
                .iter()
                .any(|frame| command(frame) == Some(GAMING_MODE_COMMAND)),
            "the unavailable flag must not be written"
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn a3944_rejects_gaming_and_ambient_writes_before_transmission() {
        let (handle, frames) = start(VirtualA3944);
        ready(&handle).await;
        assert_eq!(
            wait(
                handle
                    .dispatch(SessionCommand::SetGamingMode(true))
                    .expect("queue the typed rejection"),
            )
            .await,
            Err(CommandError::Model(ModelError::InvalidGamingModeWrite))
        );
        assert_eq!(
            wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                    .expect("queue the typed rejection"),
            )
            .await,
            Err(CommandError::Model(ModelError::InvalidSoundModeWrite))
        );
        let requests: Vec<Bytes> = frames.lock().expect("request trace").clone();
        assert!(
            !requests.iter().any(|frame| matches!(
                command(frame),
                Some(GAMING_MODE_COMMAND | SOUND_MODE_COMMAND)
            )),
            "rejected capabilities must not reach the device"
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3944_acoustic_curve_from_a_preset_reply() {
        let mut reply = body(A3944_FRAME);
        reply[32..34].copy_from_slice(&1_u16.to_le_bytes());
        reply[34..44].copy_from_slice(&[160, 130, 140, 140, 160, 160, 160, 140, 120, 0]);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3944_FRAME))),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3944_FRAME))),
            ),
            (request(A3944_EQUALIZER_COMMAND, A3944_ACOUSTIC_WRITE), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&reply))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let initial = ready(&handle).await;
        let acoustic = EqualizerSetting {
            preset_id: 1,
            volume_adjustments: [40, 10, 20, 20, 40, 40, 40, 20, 0, -120],
        };
        let applied = apply_equalizer(&handle, acoustic).await;
        assert_eq!(applied.equalizer, Some(acoustic));
        assert_ne!(initial.equalizer, Some(acoustic));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3945_two_channel_curve_from_preset_replies() {
        let mut reply = body(A3945_FRAME);
        reply[32..34].copy_from_slice(&0x0015_u16.to_le_bytes());
        reply[34..44].copy_from_slice(&[120, 120, 120, 100, 90, 80, 80, 60, 120, 0]);
        reply[44..54].copy_from_slice(&[120, 120, 120, 100, 90, 80, 80, 60, 120, 0]);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3945_FRAME))),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3945_FRAME))),
            ),
            (request(A3945_EQUALIZER_COMMAND, A3945_TREBLE_WRITE), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&reply))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let initial = ready(&handle).await;
        assert_eq!(initial.equalizer, None);
        let treble_reducer = EqualizerSetting {
            preset_id: 0x0015,
            volume_adjustments: [0, 0, 0, -20, -30, -40, -40, -60, 0, -120],
        };
        assert_eq!(
            apply_equalizer(&handle, treble_reducer).await.equalizer,
            Some(treble_reducer)
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3948_drc_curve_from_preset_replies() {
        let requested = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [0; 10],
        };
        let mut write = vec![0xfe, 0xfe];
        write.extend_from_slice(&[0x78; 10]);
        write.extend_from_slice(&[0x78; 9]);
        write.push(0);
        let mut reply = body(A3948_FRAME);
        reply[32..34].copy_from_slice(&0xfefe_u16.to_le_bytes());
        reply[34..44].copy_from_slice(&[0x78; 10]);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3948_FRAME))),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3948_FRAME))),
            ),
            (request(A3948_EQUALIZER_COMMAND, &write), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&reply))),
        ]));
        let handle = SessionRunner::start(replay, target());
        assert_eq!(
            ready(&handle).await.equalizer,
            Some(EqualizerSetting {
                preset_id: 2,
                volume_adjustments: [40, 30, 10, 0, 0, 0, 0, 0, 0, -120],
            })
        );
        assert_eq!(
            apply_equalizer(&handle, requested).await.equalizer,
            Some(requested)
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3959_drc_curve_from_preset_replies() {
        let requested = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [-19, 0, 41, 51, 51, 32, 13, -35, 0, 0],
        };
        let mut reply = body(A3959_FRAME);
        reply[32..34].copy_from_slice(&0xfefe_u16.to_le_bytes());
        reply[34..44].copy_from_slice(&[101, 120, 161, 171, 171, 152, 133, 85, 120, 120]);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3959_FRAME))),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3959_FRAME))),
            ),
            (
                request(A3959_EQUALIZER_COMMAND, A3959_LEGACY_EQ_WRITE),
                None,
            ),
            (request(STATE_COMMAND, &[]), Some(inbound(&reply))),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        assert_eq!(
            apply_equalizer(&handle, requested).await.equalizer,
            Some(requested)
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn refuses_the_two_channel_curve_while_tws_is_disconnected() {
        let mut disconnected = body(A3945_FRAME);
        disconnected[1] = 0;
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&disconnected))),
            (request(STATE_COMMAND, &[]), Some(inbound(&disconnected))),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    equalizer: Some(EqualizerSetting {
                        preset_id: 0,
                        volume_adjustments: [0; 10],
                    }),
                    ..QuickPresetValues::default()
                }))
                .expect("queue the equalizer change"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidEqualizerWrite))
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3959_ambient_mode_cycle_from_preset_replies() {
        let initial = body(A3959_FRAME);
        let mut normal = initial.clone();
        normal[64] = 0x02;
        normal[66] = 0x02;
        let anc_reply = inbound(&initial);
        let normal_reply = inbound(&normal);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(anc_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(anc_reply.clone())),
            (
                request(
                    SOUND_MODE_COMMAND,
                    &[0x02, 0x55, 0x02, 0x00, 0x01, 0xff, 0x01],
                ),
                None,
            ),
            (request(STATE_COMMAND, &[]), Some(normal_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(normal_reply.clone())),
            (
                request(
                    SOUND_MODE_COMMAND,
                    &[0x00, 0x55, 0x00, 0x00, 0x01, 0xff, 0x01],
                ),
                None,
            ),
            (request(STATE_COMMAND, &[]), Some(anc_reply)),
        ]));
        let handle = SessionRunner::start(replay, target());
        assert_eq!(
            ready(&handle).await.sound_mode,
            Some(SoundMode::NoiseCanceling)
        );
        let CommandOutcome::Applied(changed) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .expect("queue normal"),
        )
        .await
        .expect("confirm normal");
        assert_eq!(changed.sound_mode, Some(SoundMode::Normal));
        assert_eq!(
            changed.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
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
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3959_auto_power_off_values_from_preset_replies() {
        let initial = body(A3959_FRAME);
        let mut disabled = initial.clone();
        disabled[75] = 0;
        disabled[76] = 0;
        let mut enabled_three = initial.clone();
        enabled_three[75] = 1;
        enabled_three[76] = 3;
        let initial_reply = inbound(&initial);
        let disabled_reply = inbound(&disabled);
        let enabled_three_reply = inbound(&enabled_three);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(initial_reply)),
            (request(AUTO_POWER_OFF_COMMAND, &[0x00, 0x00]), None),
            (request(STATE_COMMAND, &[]), Some(disabled_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(disabled_reply)),
            (request(AUTO_POWER_OFF_COMMAND, &[0x01, 0x03]), None),
            (request(STATE_COMMAND, &[]), Some(enabled_three_reply)),
        ]));
        let handle = SessionRunner::start(replay, target());
        assert_eq!(
            ready(&handle).await.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
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
                duration: 0,
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
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn a3959_rejects_an_auto_power_off_duration_outside_the_family_table() {
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body(A3959_FRAME))),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        assert_eq!(
            wait(
                handle
                    .dispatch(SessionCommand::SetAutoPowerOff(
                        AutoPowerOffSetting::Enabled(4),
                    ))
                    .expect("queue the rejected duration"),
            )
            .await,
            Err(CommandError::Model(ModelError::InvalidAutoPowerOffWrite))
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3945_gaming_mode_cycle_from_preset_replies() {
        let mut enabled = body(A3945_FRAME);
        enabled[68] = 1;
        let initial_reply = inbound(&body(A3945_FRAME));
        let enabled_reply = inbound(&enabled);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
            (request(GAMING_MODE_COMMAND, &[0x01]), None),
            (request(STATE_COMMAND, &[]), Some(enabled_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(enabled_reply)),
            (request(GAMING_MODE_COMMAND, &[0x00]), None),
            (request(STATE_COMMAND, &[]), Some(initial_reply)),
        ]));
        let handle = SessionRunner::start(replay, target());
        assert_eq!(ready(&handle).await.gaming_mode, Some(false));
        let CommandOutcome::Applied(enabled) = wait(
            handle
                .dispatch(SessionCommand::SetGamingMode(true))
                .expect("queue gaming mode"),
        )
        .await
        .expect("confirm gaming mode");
        assert_eq!(enabled.gaming_mode, Some(true));
        let CommandOutcome::Applied(disabled) = wait(
            handle
                .dispatch(SessionCommand::SetGamingMode(false))
                .expect("queue gaming off"),
        )
        .await
        .expect("confirm gaming off");
        assert_eq!(disabled.gaming_mode, Some(false));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn reads_one_earbud_state_and_the_absent_percentages() {
        let mut one_earbud = body(A3948_FRAME);
        one_earbud[1] = 0;
        one_earbud[3] = u8::MAX;
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&one_earbud))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let state = ready(&handle).await;
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, None);
        assert_eq!(state.battery.case, None);
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn an_unidentified_state_stays_read_only() {
        let mut unknown = body(A3944_FRAME);
        unknown[16..32].copy_from_slice(b"0000TEST00000000");
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&unknown))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let view = tokio::time::timeout(Duration::from_secs(10), handle.confirmed_view())
            .await
            .expect("read-only view deadline")
            .expect("read-only view");
        assert!(matches!(view.connection, ConnectionState::Unidentified(_)));
        assert_eq!(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                .err(),
            Some(DispatchError::ReadOnly)
        );
        handle.close().await.expect("close virtual link");
    }

    async fn assert_a3944_reference<P: TransportProvider>(provider: P) {
        let (handle, frames) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3944);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, None);
        assert_eq!(firmware(&state), versions("02.27", "02.27"));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 0,
                volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
            })
        );
        assert_eq!(state.sound_mode, None);
        assert_eq!(state.gaming_mode, None);
        assert_eq!(trace(&frames), vec![[0x01, 0x05], [0x01, 0x01]]);
        handle.close().await.expect("close virtual link");
    }

    async fn assert_a3945_reference<P: TransportProvider>(provider: P) {
        let (handle, _) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3945);
        assert_eq!(state.battery.left, Some(0));
        assert_eq!(state.battery.right, Some(0));
        assert_eq!(state.battery.case, Some(0));
        assert_eq!(firmware(&state), versions("00.00", "00.00"));
        assert_eq!(state.equalizer, None);
        assert_eq!(state.gaming_mode, Some(false));
        handle.close().await.expect("close virtual link");
    }

    async fn assert_a3948_reference<P: TransportProvider>(provider: P) {
        let (handle, _) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3948);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(
            state.equalizer,
            Some(EqualizerSetting {
                preset_id: 2,
                volume_adjustments: [40, 30, 10, 0, 0, 0, 0, 0, 0, -120],
            })
        );
        handle.close().await.expect("close virtual link");
    }

    async fn assert_a3959_reference<P: TransportProvider>(provider: P) {
        let (handle, _) = start(provider);
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::A3959);
        assert_eq!(state.battery.left, Some(50));
        assert_eq!(state.battery.right, Some(60));
        assert_eq!(firmware(&state), versions("01.64", "01.64"));
        assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(state.gaming_mode, Some(false));
        assert_eq!(
            state.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 0,
            })
        );
        handle.close().await.expect("close virtual link");
    }

    async fn apply_equalizer(handle: &SessionHandle, setting: EqualizerSetting) -> DeviceState {
        let CommandOutcome::Applied(state) = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                    equalizer: Some(setting),
                    ..QuickPresetValues::default()
                }))
                .expect("queue the equalizer change"),
        )
        .await
        .expect("confirm the equalizer change");
        state
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

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        }
    }

    fn body(frame: &[u8]) -> Vec<u8> {
        assert_eq!(frame[5..7], [0x01, 0x01]);
        frame[9..frame.len() - 1].to_vec()
    }

    fn request(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    fn inbound(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, STATE_COMMAND, body).expect("inbound state frame")
    }

    fn firmware(state: &DeviceState) -> (Option<String>, Option<String>) {
        (
            state.firmware_left.map(|version| version.to_string()),
            state.firmware_right.map(|version| version.to_string()),
        )
    }

    fn versions(left: &str, right: &str) -> (Option<String>, Option<String>) {
        (Some(left.to_owned()), Some(right.to_owned()))
    }

    fn command(frame: &[u8]) -> Option<[u8; 2]> {
        frame.get(5..7)?.try_into().ok()
    }

    fn trace(frames: &Arc<Mutex<Vec<Bytes>>>) -> Vec<[u8; 2]> {
        frames
            .lock()
            .expect("request trace")
            .iter()
            .filter_map(|frame| command(frame))
            .collect()
    }

    fn start<P: TransportProvider>(provider: P) -> (SessionHandle, Arc<Mutex<Vec<Bytes>>>) {
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
        frames: Arc<Mutex<Vec<Bytes>>>,
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
        frames: Arc<Mutex<Vec<Bytes>>>,
    }

    #[async_trait]
    impl LinkTx for RecordingTx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            self.frames
                .lock()
                .expect("request trace")
                .push(request.clone());
            self.inner.send(request).await
        }

        async fn close(&mut self) {
            self.inner.close().await;
        }
    }
}
