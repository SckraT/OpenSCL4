#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use bytes::Bytes;
    use openscl4_models::ModelError;
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, ConnectionState, DeviceState, EqualizerSetting, ModelDriver,
        PendingCommand, SessionCommand, SessionHandle, SessionRunner, SoundMode,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};

    use super::capture_replay::Replay;

    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];

    /// The A3933 and A3939 state bodies from `core/data/a3933-a3939-state-reference.json`.
    const A3933_BODY: &[u8] = &[
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x30, 0x2e, 0x30,
        0x30, 0x33, 0x39, 0x33, 0x33, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
        0x30, 0x30, 0xfe, 0xfe, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x01, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x07, 0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];
    const A3939_BODY: &[u8] = &[
        0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x2e, 0x30, 0x30, 0x30, 0x30, 0x2e, 0x30,
        0x30, 0x33, 0x39, 0x33, 0x39, 0x54, 0x45, 0x53, 0x54, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
        0x30, 0x30, 0xfe, 0xfe, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x00, 0x00, 0x00, 0x00, 0x00, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78,
        0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x78, 0x00, 0x00, 0x01, 0x00,
        0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x01, 0x00, 0x07, 0x00, 0x00, 0x01, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    // Legacy-generated frames from `core/data/a3933-a3939-state-reference.json`.
    const READ_STATE: &str = "08 EE 00 00 00 01 01 0A 00 02";
    const READ_IDENTITY: &str = "08 EE 00 00 00 01 05 0A 00 06";
    const WRITE_NOISE_CANCELING: &str = "08 EE 00 00 00 06 81 0E 00 00 00 01 00 8C";
    const RESTORE_NORMAL: &str = "08 EE 00 00 00 06 81 0E 00 02 00 01 00 8E";
    const WRITE_CUSTOM_EQUALIZER: &str = "08 EE 00 00 00 02 81 20 00 FE FE 3C B4 8F A0 8E B4 74 88 78 00 3C B4 8F A0 8E B4 74 88 78 00 3F";
    const WRITE_GAMING_ON: &str = "08 EE 00 00 00 01 87 0B 00 01 8A";
    const WRITE_GAMING_OFF: &str = "08 EE 00 00 00 01 87 0B 00 00 89";
    const GAMING_ACK: &str = "09 FF 00 00 01 01 87 0A 00 9B";
    const REFERENCE_CURVE: [i16; 10] = [-60, 60, 23, 40, 22, 60, -4, 16, 0, -120];

    #[tokio::test(start_paused = true)]
    async fn reads_the_reference_state_for_each_public_code() {
        for (body, model) in [
            (A3933_BODY, ModelDriver::A3933),
            (A3939_BODY, ModelDriver::A3939),
        ] {
            let replay = Replay(VecDeque::from([
                (golden(READ_IDENTITY), None),
                (golden(READ_STATE), Some(state_reply(body))),
            ]));
            let handle = SessionRunner::start(replay, target());
            let state = ready(&handle).await;
            assert_eq!(state.model, model);
            assert_eq!(state.sound_mode, Some(SoundMode::NoiseCanceling));
            assert_eq!(state.gaming_mode, Some(false));
            assert_eq!(state.battery.left, Some(0));
            assert_eq!(state.battery.right, Some(0));
            assert_eq!(state.battery.case, Some(0));
            assert_eq!(
                state.firmware_left.map(|version| version.to_string()),
                Some("00.00".to_owned())
            );
            assert_eq!(
                state.firmware_right.map(|version| version.to_string()),
                Some("00.00".to_owned())
            );
            assert_eq!(
                state.equalizer,
                Some(EqualizerSetting {
                    preset_id: 0xfefe,
                    volume_adjustments: [0; 10],
                })
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_reference_mode_cycle_from_preset_replies() {
        for initial in [A3933_BODY, A3939_BODY] {
            let initial_reply = state_reply(initial);
            let normal_reply = state_reply(&body_with_mode(initial, 0x02));
            let replay = Replay(VecDeque::from([
                (golden(READ_IDENTITY), None),
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
    async fn confirms_the_ten_band_curve_from_the_family_write_and_read_back() {
        for initial in [A3933_BODY, A3939_BODY] {
            let initial_reply = state_reply(initial);
            let requested = EqualizerSetting {
                preset_id: 0xfefe,
                volume_adjustments: REFERENCE_CURVE,
            };
            let changed_reply = state_reply(&body_with_curve(initial, requested));
            let replay = Replay(VecDeque::from([
                (golden(READ_IDENTITY), None),
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
    async fn confirms_and_restores_gaming_mode_from_independent_replies() {
        for initial in [A3933_BODY, A3939_BODY] {
            let initial_reply = state_reply(initial);
            let gaming_on_reply = state_reply(&body_with_gaming_mode(initial, true));
            let replay = Replay(VecDeque::from([
                (golden(READ_IDENTITY), None),
                (golden(READ_STATE), Some(initial_reply.clone())),
                (golden(READ_STATE), Some(initial_reply.clone())),
                (golden(WRITE_GAMING_ON), Some(golden(GAMING_ACK))),
                (golden(READ_STATE), Some(gaming_on_reply.clone())),
                (golden(READ_STATE), Some(gaming_on_reply)),
                (golden(WRITE_GAMING_OFF), Some(golden(GAMING_ACK))),
                (golden(READ_STATE), Some(initial_reply)),
            ]));
            let handle = SessionRunner::start(replay, target());
            let initial_state = ready(&handle).await;
            assert_eq!(initial_state.gaming_mode, Some(false));

            let CommandOutcome::Applied(enabled) = wait(
                handle
                    .dispatch(SessionCommand::SetGamingMode(true))
                    .expect("queue gaming on"),
            )
            .await
            .expect("confirm gaming on");
            assert_eq!(enabled.gaming_mode, Some(true));

            let CommandOutcome::Applied(disabled) = wait(
                handle
                    .dispatch(SessionCommand::SetGamingMode(false))
                    .expect("queue gaming off"),
            )
            .await
            .expect("confirm gaming off");
            assert_eq!(disabled.gaming_mode, Some(false));
            assert_eq!(ready(&handle).await, disabled);
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_gaming_mode_without_the_extra_fields_before_transmission() {
        let mut body = A3933_BODY.to_vec();
        body.truncate(122);
        let replay = Replay(VecDeque::from([
            (golden(READ_IDENTITY), None),
            (golden(READ_STATE), Some(state_reply(&body))),
            (golden(READ_STATE), Some(state_reply(&body))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let initial_state = ready(&handle).await;
        assert_eq!(initial_state.gaming_mode, None);

        let result = wait(
            handle
                .dispatch(SessionCommand::SetGamingMode(true))
                .expect("queue the typed rejection"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidGamingModeWrite))
        );
        assert_eq!(ready(&handle).await, initial_state);
        handle.close().await.expect("close virtual link");
    }

    async fn ready(handle: &SessionHandle) -> DeviceState {
        let view = tokio::time::timeout(std::time::Duration::from_secs(3), handle.confirmed_view())
            .await
            .expect("sync deadline")
            .expect("confirmed state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a confirmed state");
        };
        state
    }

    async fn wait(pending: PendingCommand) -> openscl4_session::CommandResult {
        tokio::time::timeout(std::time::Duration::from_secs(10), pending.wait())
            .await
            .expect("command deadline")
    }

    async fn wait_for_equalizer(
        handle: &SessionHandle,
        requested: EqualizerSetting,
    ) -> DeviceState {
        let mut view = handle.subscribe();
        let snapshot = tokio::time::timeout(
            std::time::Duration::from_secs(3),
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

    fn state_reply(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, STATE_COMMAND, body).expect("inbound state frame")
    }

    fn body_with_mode(initial: &[u8], mode: u8) -> Vec<u8> {
        let mut body = initial.to_vec();
        // Only the ambient sound-mode byte changes; the other controls are echoed.
        body[116] = mode;
        body
    }

    fn body_with_curve(initial: &[u8], curve: EqualizerSetting) -> Vec<u8> {
        let mut body = initial.to_vec();
        body[32..34].copy_from_slice(&curve.preset_id.to_le_bytes());
        let bands: Vec<u8> = curve
            .volume_adjustments
            .iter()
            .map(|gain| (gain + 120) as u8)
            .collect();
        body[34..44].copy_from_slice(&bands);
        body[44..54].copy_from_slice(&bands);
        body
    }

    fn body_with_gaming_mode(initial: &[u8], enabled: bool) -> Vec<u8> {
        let mut body = initial.to_vec();
        body[124] = u8::from(enabled);
        body
    }
}
