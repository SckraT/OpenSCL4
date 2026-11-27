#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::Duration};

    use bytes::Bytes;
    use openscl4_models::{AutoPowerOff, AutoPowerOffSetting, ModelError};
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, ConnectionState, DeviceState, DispatchError,
        EqualizerSetting, ModelDriver, PendingCommand, SessionCommand, SessionHandle,
        SessionRunner, SoundMode, UnidentifiedReason, UnidentifiedState,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};

    use super::capture_replay::Replay;

    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];

    /// The A3931 faker state body from `core/data/a3931-a3935-state-reference.json`.
    const STATE_BODY: &[u8] = &[
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const A3931_IDENTITY: &[u8] = &[
        0x30, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x30, 0x2e, 0x30, 0x30, 0x33, 0x39, 0x33, 0x31, 0x54,
        0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ];
    const A3935_IDENTITY: &[u8] = &[
        0x30, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x30, 0x2e, 0x30, 0x30, 0x33, 0x39, 0x33, 0x35, 0x54,
        0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
    ];

    // Legacy-generated frames from `core/data/a3931-a3935-state-reference.json`.
    const READ_STATE: &str = "08 EE 00 00 00 01 01 0A 00 02";
    const READ_IDENTITY: &str = "08 EE 00 00 00 01 05 0A 00 06";
    const WRITE_NOISE_CANCELING: &str = "08 EE 00 00 00 06 81 0E 00 00 00 01 00 8C";
    const RESTORE_NORMAL: &str = "08 EE 00 00 00 06 81 0E 00 02 00 01 00 8E";
    const WRITE_CUSTOM_EQUALIZER: &str = "08 EE 00 00 00 02 83 2C 00 FE FE 3C B4 8F A0 8E B4 74 88 3C B4 8F A0 8E B4 74 88 6D 85 74 7E 74 82 72 7C 6D 85 74 7E 74 82 72 7C ED";
    const WRITE_AUTO_POWER_OFF_DISABLED: &str = "08 EE 00 00 00 01 86 0C 00 00 01 8A";
    const WRITE_AUTO_POWER_OFF_ENABLED_3: &str = "08 EE 00 00 00 01 86 0C 00 01 03 8D";
    const REFERENCE_CURVE: [i16; 10] = [-60, 60, 23, 40, 22, 60, -4, 16, 0, 0];

    #[tokio::test(start_paused = true)]
    async fn reads_the_reference_state_for_each_public_code() {
        for (identity, model) in [
            (A3931_IDENTITY, ModelDriver::A3931),
            (A3935_IDENTITY, ModelDriver::A3935),
        ] {
            let replay = Replay(VecDeque::from([
                (golden(READ_IDENTITY), Some(identity_reply(identity))),
                (golden(READ_STATE), Some(state_reply(STATE_BODY))),
            ]));
            let handle = SessionRunner::start(replay, target());
            let state = ready(&handle).await;
            assert_eq!(state.model, model);
            assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
            assert_eq!(state.battery.left, Some(0));
            assert_eq!(state.battery.right, Some(0));
            assert_eq!(state.gaming_mode, None);
            assert_eq!(
                state.auto_power_off,
                Some(AutoPowerOff {
                    enabled: false,
                    duration: 0,
                })
            );
            assert_eq!(
                state.equalizer,
                Some(EqualizerSetting {
                    preset_id: 0,
                    volume_adjustments: [0; 10],
                })
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_reference_mode_cycle_from_preset_replies() {
        for identity in [A3931_IDENTITY, A3935_IDENTITY] {
            let initial_reply = state_reply(STATE_BODY);
            let normal_reply = state_reply(&body_with_mode(STATE_BODY, 0x02));
            let replay = Replay(VecDeque::from([
                (golden(READ_IDENTITY), Some(identity_reply(identity))),
                (golden(READ_STATE), Some(initial_reply.clone())),
                (golden(READ_STATE), Some(initial_reply.clone())),
                (golden(RESTORE_NORMAL), None),
                (golden(READ_STATE), Some(normal_reply.clone())),
                (golden(READ_STATE), Some(normal_reply)),
                (golden(WRITE_NOISE_CANCELING), None),
                (golden(READ_STATE), Some(initial_reply)),
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
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_family_curve_from_the_drc_write_and_read_back() {
        for identity in [A3931_IDENTITY, A3935_IDENTITY] {
            let initial_reply = state_reply(STATE_BODY);
            let requested = EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: REFERENCE_CURVE,
            };
            let changed_reply = state_reply(&body_with_curve(STATE_BODY, requested));
            let replay = Replay(VecDeque::from([
                (golden(READ_IDENTITY), Some(identity_reply(identity))),
                (golden(READ_STATE), Some(initial_reply.clone())),
                (golden(READ_STATE), Some(initial_reply)),
                (golden(WRITE_CUSTOM_EQUALIZER), None),
                (golden(READ_STATE), Some(changed_reply)),
            ]));
            let handle = SessionRunner::start(replay, target());
            ready(&handle).await;

            handle.set_eq_live(requested).expect("accept live curve");
            let applied = wait_for_equalizer(&handle, requested).await;
            assert_eq!(applied.equalizer, Some(requested));
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_enable_and_disable_from_preset_replies() {
        for identity in [A3931_IDENTITY, A3935_IDENTITY] {
            let enabled_reply = state_reply(&body_with_auto_power_off(STATE_BODY, true, 1));
            let disabled_reply = state_reply(&body_with_auto_power_off(STATE_BODY, false, 1));
            let enabled_three_reply = state_reply(&body_with_auto_power_off(STATE_BODY, true, 3));
            let replay = Replay(VecDeque::from([
                (golden(READ_IDENTITY), Some(identity_reply(identity))),
                (golden(READ_STATE), Some(enabled_reply.clone())),
                (golden(READ_STATE), Some(enabled_reply)),
                (golden(WRITE_AUTO_POWER_OFF_DISABLED), None),
                (golden(READ_STATE), Some(disabled_reply.clone())),
                (golden(READ_STATE), Some(disabled_reply)),
                (golden(WRITE_AUTO_POWER_OFF_ENABLED_3), None),
                (golden(READ_STATE), Some(enabled_three_reply)),
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
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_a_duration_outside_the_family_table_before_transmission() {
        let replay = Replay(VecDeque::from([
            (golden(READ_IDENTITY), Some(identity_reply(A3931_IDENTITY))),
            (
                golden(READ_STATE),
                Some(state_reply(&body_with_auto_power_off(STATE_BODY, true, 1))),
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
    async fn rejects_an_unrecognized_auto_power_off_state_before_transmission() {
        let mut body = STATE_BODY.to_vec();
        body[42] = 2;
        let replay = Replay(VecDeque::from([
            (golden(READ_IDENTITY), Some(identity_reply(A3931_IDENTITY))),
            (golden(READ_STATE), Some(state_reply(&body))),
            (golden(READ_STATE), Some(state_reply(&body))),
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

    #[tokio::test(start_paused = true)]
    async fn a_malformed_sound_mode_state_is_read_only_at_start() {
        let mut body = STATE_BODY.to_vec();
        body[36] = 3;
        let replay = Replay(VecDeque::from([
            (golden(READ_IDENTITY), Some(identity_reply(A3931_IDENTITY))),
            (golden(READ_STATE), Some(state_reply(&body))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let view = tokio::time::timeout(Duration::from_secs(10), handle.confirmed_view())
            .await
            .expect("identity and state deadline")
            .expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3931),
                reason: UnidentifiedReason::IncompatibleState,
            })
        );
        assert_eq!(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal,))
                .err(),
            Some(DispatchError::ReadOnly)
        );
        handle.close().await.expect("close the read-only link");
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

    async fn wait(pending: PendingCommand) -> openscl4_session::CommandResult {
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

    fn golden(frame: &str) -> Bytes {
        let bytes: Vec<u8> = frame
            .split_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16).expect("golden frame byte"))
            .collect();
        Bytes::from(bytes)
    }

    fn identity_reply(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, IDENTITY_COMMAND, body).expect("inbound identity frame")
    }

    fn state_reply(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, STATE_COMMAND, body).expect("inbound state frame")
    }

    fn body_with_mode(initial: &[u8], mode: u8) -> Vec<u8> {
        let mut body = initial.to_vec();
        // Only the ambient sound-mode byte changes; the other controls are echoed.
        body[36] = mode;
        body
    }

    fn body_with_curve(initial: &[u8], curve: EqualizerSetting) -> Vec<u8> {
        let mut body = initial.to_vec();
        body[6..8].copy_from_slice(&curve.preset_id.to_le_bytes());
        let bands: Vec<u8> = curve.volume_adjustments[..8]
            .iter()
            .map(|gain| (gain + 120) as u8)
            .collect();
        body[8..16].copy_from_slice(&bands);
        body[16..24].copy_from_slice(&bands);
        body
    }

    fn body_with_auto_power_off(initial: &[u8], enabled: bool, duration: u8) -> Vec<u8> {
        let mut body = initial.to_vec();
        body[42..44].copy_from_slice(&[u8::from(enabled), duration]);
        body
    }
}
