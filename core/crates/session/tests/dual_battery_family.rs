#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::Duration};

    use bytes::Bytes;
    use openscl4_models::{
        A3936_AUTO_POWER_OFF, A3936_SOUND_MODE_FIELDS, A3952_AUTO_POWER_OFF,
        A3952_SOUND_MODE_FIELDS, A3968_AUTO_POWER_OFF, A3968_SOUND_MODE_FIELDS,
        AutoPowerOffSetting, ModelError,
    };
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        AmbientSoundModeCycle, CommandError, CommandOutcome, CommandResult, ConnectionState,
        DeviceState, DispatchError, ModelDriver, PendingCommand, SessionCommand, SessionHandle,
        SessionRunner, SoundMode, SoundModeSetting, UnidentifiedReason, UnidentifiedState,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};

    use super::capture_replay::Replay;

    // Reference wire opcodes from the legacy fixtures and packet builders.
    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const AUTO_POWER_OFF_COMMAND: [u8; 2] = [0x01, 0x86];
    const GAMING_MODE_COMMAND: [u8; 2] = [0x01, 0x87];

    const A3936_FRAME: &[u8] = include_bytes!("../../../data/a3936-pilot-state.bin");
    const A3952_FRAME: &[u8] = include_bytes!("../../../data/a3952-pilot-state.bin");
    const A3968_FRAME: &[u8] = include_bytes!("../../../data/a3968-pilot-state.bin");

    fn body(frame: &[u8]) -> &[u8] {
        assert_eq!(frame[5..7], STATE_COMMAND);
        &frame[9..frame.len() - 1]
    }

    #[derive(Clone, Copy)]
    struct Model {
        driver: ModelDriver,
        frame: &'static [u8],
        sound_modes: usize,
        auto_power_off: usize,
    }

    const MODELS: [Model; 3] = [
        Model {
            driver: ModelDriver::A3936,
            frame: A3936_FRAME,
            sound_modes: A3936_SOUND_MODE_FIELDS.start,
            auto_power_off: A3936_AUTO_POWER_OFF.start,
        },
        Model {
            driver: ModelDriver::A3952,
            frame: A3952_FRAME,
            sound_modes: A3952_SOUND_MODE_FIELDS.start,
            auto_power_off: A3952_AUTO_POWER_OFF.start,
        },
        Model {
            driver: ModelDriver::A3968,
            frame: A3968_FRAME,
            sound_modes: A3968_SOUND_MODE_FIELDS.start,
            auto_power_off: A3968_AUTO_POWER_OFF.start,
        },
    ];

    fn wire(mode: SoundMode) -> u8 {
        match mode {
            SoundMode::NoiseCanceling => 0x00,
            SoundMode::Transparency => 0x01,
            SoundMode::Normal => 0x02,
            SoundMode::AirplaneMode => 0x03,
        }
    }

    fn state(model: Model) -> DeviceState {
        model
            .driver
            .decode_confirmed_state(body(model.frame))
            .expect("confirmed state")
    }

    #[tokio::test(start_paused = true)]
    async fn reads_the_recorded_state_of_each_dual_battery_model() {
        for model in MODELS {
            let initial = inbound(body(model.frame));
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(initial)),
            ]));
            let handle = SessionRunner::start(replay, target());
            let state = ready(&handle).await;
            assert_eq!(state.model, model.driver);
            assert!(state.battery.left.is_some(), "left battery");
            assert!(state.battery.right.is_some(), "right battery");
            assert_eq!(
                state.firmware_left.map(|version| version.to_string()),
                state.firmware_right.map(|version| version.to_string())
            );
            assert!(state.tws_status.is_some());
            assert!(state.ambient_sound_mode_cycle.is_some());
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_an_ambient_mode_change_from_an_independent_state_reply() {
        for model in MODELS {
            let initial_state = state(model);
            let original = initial_state.sound_mode.expect("reported mode");
            let target_mode = match original {
                SoundMode::Normal => SoundMode::NoiseCanceling,
                _ => SoundMode::Normal,
            };
            let initial_reply = inbound(body(model.frame));
            let changed_reply = inbound(&body_with_mode(model, target_mode));
            let restored_reply = inbound(body(model.frame));
            let expected_write = initial_state
                .dual_sound_mode_write(target_mode)
                .expect("valid write");
            let expected_restore = initial_state
                .dual_sound_mode_write(original)
                .expect("valid write");
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(initial_reply)),
                (
                    request(SOUND_MODE_COMMAND, &expected_write),
                    Some(inbound_command(SOUND_MODE_COMMAND, &[])),
                ),
                (request(STATE_COMMAND, &[]), Some(changed_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(changed_reply)),
                (
                    request(SOUND_MODE_COMMAND, &expected_restore),
                    Some(inbound_command(SOUND_MODE_COMMAND, &[])),
                ),
                (request(STATE_COMMAND, &[]), Some(restored_reply)),
            ]));
            let handle = SessionRunner::start(replay, target());
            assert_eq!(ready(&handle).await, initial_state);

            let CommandOutcome::Applied(changed) = wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(target_mode))
                    .expect("queue the ambient change"),
            )
            .await
            .expect("confirm the ambient change");
            assert_eq!(changed.sound_mode, Some(target_mode));
            assert_eq!(changed.battery, initial_state.battery);

            let CommandOutcome::Applied(restored) = wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(original))
                    .expect("queue the restore"),
            )
            .await
            .expect("confirm the restore");
            assert_eq!(restored.sound_mode, Some(original));
            assert_eq!(ready(&handle).await, restored);
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn keeps_the_sport_x20_sound_mode_tail_on_every_write() {
        let model = MODELS[2];
        let initial_reply = inbound(body(model.frame));
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(initial_reply)),
            (
                request(SOUND_MODE_COMMAND, &[0x01, 0x32, 0x01, 0x00, 0x00, 0xff]),
                Some(inbound_command(SOUND_MODE_COMMAND, &[])),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(&body_with_mode(model, SoundMode::Transparency))),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;
        let CommandOutcome::Applied(state) = wait(
            handle
                .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Transparency))
                .expect("queue transparency"),
        )
        .await
        .expect("confirm transparency");
        assert_eq!(state.sound_mode, Some(SoundMode::Transparency));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_space_a40_gaming_mode_flag_from_a_state_reply() {
        let model = MODELS[0];
        let initial_reply = inbound(body(model.frame));
        let enabled_reply = inbound(&body_with_gaming(model, true));
        let disabled_reply = inbound(body(model.frame));
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(initial_reply)),
            (
                request(GAMING_MODE_COMMAND, &[0x01]),
                Some(inbound_command(GAMING_MODE_COMMAND, &[])),
            ),
            (request(STATE_COMMAND, &[]), Some(enabled_reply.clone())),
            (request(STATE_COMMAND, &[]), Some(enabled_reply)),
            (
                request(GAMING_MODE_COMMAND, &[0x00]),
                Some(inbound_command(GAMING_MODE_COMMAND, &[])),
            ),
            (request(STATE_COMMAND, &[]), Some(disabled_reply)),
        ]));
        let handle = SessionRunner::start(replay, target());
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
                .expect("queue the restore"),
        )
        .await
        .expect("confirm the restore");
        assert_eq!(disabled.gaming_mode, Some(false));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_auto_power_off_pair_from_a_state_reply() {
        for model in MODELS {
            let initial = state(model);
            let reported = initial.auto_power_off.expect("reported auto power off");
            let initial_reply = inbound(body(model.frame));
            let disabled_reply = inbound(&body_with_auto_power_off(model, 0x00, reported.duration));
            let enabled_reply = inbound(&body_with_auto_power_off(model, 0x01, 0x03));
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(initial_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(initial_reply)),
                (
                    request(AUTO_POWER_OFF_COMMAND, &[0x00, reported.duration]),
                    Some(inbound_command(AUTO_POWER_OFF_COMMAND, &[])),
                ),
                (request(STATE_COMMAND, &[]), Some(disabled_reply.clone())),
                (request(STATE_COMMAND, &[]), Some(disabled_reply)),
                (
                    request(AUTO_POWER_OFF_COMMAND, &[0x01, 0x03]),
                    Some(inbound_command(AUTO_POWER_OFF_COMMAND, &[])),
                ),
                (request(STATE_COMMAND, &[]), Some(enabled_reply)),
            ]));
            let handle = SessionRunner::start(replay, target());
            assert_eq!(ready(&handle).await, initial);

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
                Some(openscl4_session::AutoPowerOff {
                    enabled: false,
                    duration: reported.duration,
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
                Some(openscl4_session::AutoPowerOff {
                    enabled: true,
                    duration: 3,
                })
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_a_positioned_sound_mode_write_before_transmission() {
        for model in MODELS {
            let initial = inbound(body(model.frame));
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(initial)),
            ]));
            let handle = SessionRunner::start(replay, target());
            ready(&handle).await;
            let result = wait(
                handle
                    .dispatch(SessionCommand::SetSoundMode(SoundModeSetting::NORMAL))
                    .expect("queue the typed rejection"),
            )
            .await;
            assert_eq!(
                result,
                Err(CommandError::Model(ModelError::InvalidSoundModeWrite))
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_an_equalizer_preset_before_transmission() {
        for model in MODELS {
            let initial = inbound(body(model.frame));
            let replay = Replay(VecDeque::from([
                (request(IDENTITY_COMMAND, &[]), None),
                (request(STATE_COMMAND, &[]), Some(initial)),
            ]));
            let handle = SessionRunner::start(replay, target());
            let state = ready(&handle).await;
            let preset = openscl4_session::QuickPresetValues {
                equalizer: state.equalizer,
                ..Default::default()
            };
            let result = wait(
                handle
                    .dispatch(SessionCommand::ApplyQuickPreset(preset))
                    .expect("queue the typed rejection"),
            )
            .await;
            assert_eq!(
                result,
                Err(CommandError::Model(ModelError::InvalidEqualizerWrite))
            );
            assert_eq!(
                handle.set_eq_live(openscl4_session::EqualizerSetting {
                    preset_id: 0,
                    volume_adjustments: [0; 10],
                }),
                Err(DispatchError::UnsupportedEqualizer)
            );
            handle.close().await.expect("close virtual link");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn an_unrecognized_state_is_read_only() {
        let mut malformed = body(A3936_FRAME).to_vec();
        malformed[A3936_SOUND_MODE_FIELDS.start] = 0x03;
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(&malformed))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let view = tokio::time::timeout(Duration::from_secs(10), handle.confirmed_view())
            .await
            .expect("identity and state deadline")
            .expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3936),
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
    async fn a_truncated_liberty_4_state_is_not_confirmed() {
        let truncated = &body(A3952_FRAME)[..141];
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(truncated))),
        ]));
        let handle = SessionRunner::start(replay, target());
        let view = tokio::time::timeout(Duration::from_secs(10), handle.confirmed_view())
            .await
            .expect("identity and state deadline")
            .expect("read-only view");
        assert_eq!(
            view.connection,
            ConnectionState::Unidentified(UnidentifiedState {
                model: Some(ModelDriver::A3952),
                reason: UnidentifiedReason::IncompatibleState,
            })
        );
        handle.close().await.expect("close the read-only link");
    }

    #[tokio::test(start_paused = true)]
    async fn reads_the_short_liberty_4_layout_the_legacy_writer_produces() {
        let short = &body(A3952_FRAME)[..140];
        let state = ModelDriver::A3952
            .decode_confirmed_state(short)
            .expect("confirmed short layout");
        assert_eq!(
            state.ambient_sound_mode_cycle,
            Some(AmbientSoundModeCycle::Shared(
                openscl4_session::SoundModeCycle {
                    noise_canceling: true,
                    transparency: true,
                    normal: false,
                }
            ))
        );
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(inbound(short))),
        ]));
        let handle = SessionRunner::start(replay, target());
        assert_eq!(ready(&handle).await, state);
        handle.close().await.expect("close virtual link");
    }

    fn body_with_mode(model: Model, mode: SoundMode) -> Vec<u8> {
        let mut changed = body(model.frame).to_vec();
        changed[model.sound_modes] = wire(mode);
        changed
    }

    fn body_with_gaming(model: Model, enabled: bool) -> Vec<u8> {
        let mut changed = body(model.frame).to_vec();
        changed[openscl4_models::A3936_GAMING_MODE] = u8::from(enabled);
        changed
    }

    fn body_with_auto_power_off(model: Model, enabled: u8, duration: u8) -> Vec<u8> {
        let mut changed = body(model.frame).to_vec();
        changed[model.auto_power_off] = enabled;
        changed[model.auto_power_off + 1] = duration;
        changed
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

    fn request(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Outbound, command, body).expect("outbound frame")
    }

    fn inbound(body: &[u8]) -> Bytes {
        inbound_command(STATE_COMMAND, body)
    }

    fn inbound_command(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, command, body).expect("inbound frame")
    }
}
