#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::Duration};

    use bytes::Bytes;
    use openscl4_models::{ModelError, d1301};
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, CommandResult, ConnectionState, D1301Setting, DeviceState,
        ListeningMode, ModelDriver, PendingCommand, PostSleepAudio, SessionCommand, SessionHandle,
        SessionRunner,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};
    use openscl4_transport_mock::{VirtualA3062, VirtualD1301};

    use super::capture_replay::Replay;

    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SLEEP_CONTROL_COMMAND: [u8; 2] = [0x15, 0x85];
    const AUTO_SWITCH_COMMAND: [u8; 2] = [0x15, 0x8F];
    const LISTENING_MODE_COMMAND: [u8; 2] = [0x01, 0xA9];
    const DEFAULT_LISTENING_MODE_COMMAND: [u8; 2] = [0x10, 0x95];
    const NOISE_CANCELING_COMMAND: [u8; 2] = [0x06, 0x87];

    /// Captured D1301 sleep states with the serial masked and the checksum
    /// recomputed. See `core/data/d1301-sleep-reference.json`.
    const POST_SLEEP_PAUSE: &[u8] = include_bytes!("../../../data/d1301-post-sleep-pause.bin");
    const POST_SLEEP_PLAY_LOCAL: &[u8] =
        include_bytes!("../../../data/d1301-post-sleep-play-local.bin");

    /// Reference outbound frames from `core/data/d1301-sleep-reference.json`.
    /// The sleep bodies come from the legacy builders; the session compares the
    /// whole frame byte for byte, including the frame checksum.
    const SET_POST_SLEEP_PAUSE: &[u8] = b"\x08\xee\0\0\0\x15\x85\x0e\0\xff\xff\xff\x00\x9b";
    const SET_POST_SLEEP_PLAY_LOCAL: &[u8] = b"\x08\xee\0\0\0\x15\x85\x0e\0\xff\xff\xff\x01\x9c";
    const SET_AUTO_SWITCH_OFF: &[u8] = b"\x08\xee\0\0\0\x15\x8f\x0b\0\x00\xa5";
    const SET_AUTO_SWITCH_ON: &[u8] = b"\x08\xee\0\0\0\x15\x8f\x0b\0\x01\xa6";
    const SET_LISTENING_MODE_LOCAL: &[u8] = b"\x08\xee\0\0\0\x01\xa9\x0b\0\x01\xac";
    const SET_DEFAULT_LISTENING_MODE_LOCAL: &[u8] = b"\x08\xee\0\0\0\x10\x95\x0b\0\x01\xa7";
    const SET_NOISE_CANCELING_ON: &[u8] = b"\x08\xee\0\0\0\x06\x87\x0b\0\x01\x8f";

    #[tokio::test(start_paused = true)]
    async fn reads_the_captured_sleep_state_after_an_unavailable_identity_probe() {
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(state_frame(&state_body(POST_SLEEP_PAUSE))),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let state = ready(&handle).await;
        assert_eq!(state.model, ModelDriver::D1301);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(
            state.firmware_left.map(|firmware| firmware.to_string()),
            Some("01.91".to_owned())
        );
        assert_eq!(state.listening_mode, Some(ListeningMode::Bluetooth));
        assert_eq!(state.default_listening_mode, Some(ListeningMode::Bluetooth));
        assert_eq!(state.noise_canceling, Some(false));
        assert_eq!(
            state.sleep_audio.map(|sleep| (sleep.enabled, sleep.action)),
            Some((true, PostSleepAudio::Pause))
        );
        assert_eq!(state.sound_mode, None);
        handle.close().await.expect("close the virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_each_sleep_setting_from_a_fresh_state_response() {
        let initial = state_body(POST_SLEEP_PAUSE);
        let play_local = state_body(POST_SLEEP_PLAY_LOCAL);
        let mut exchanges = VecDeque::new();
        // The identity probe has no measured reply, so the session falls back
        // to the verified state read.
        exchanges.push_back((request(IDENTITY_COMMAND, &[]), None));
        exchanges.push_back((request(STATE_COMMAND, &[]), Some(state_frame(&initial))));

        let mut current = initial.clone();
        let mut step = |expected: &'static [u8], command: [u8; 2], next: Vec<u8>| {
            exchanges.push_back((request(STATE_COMMAND, &[]), Some(state_frame(&current))));
            exchanges.push_back((Bytes::from_static(expected), Some(inbound(command, &[]))));
            exchanges.push_back((request(STATE_COMMAND, &[]), Some(state_frame(&next))));
            current = next;
        };

        // The two bodies of command 15 85 keep the timer fields untouched.
        step(
            SET_POST_SLEEP_PLAY_LOCAL,
            SLEEP_CONTROL_COMMAND,
            play_local.clone(),
        );
        step(SET_POST_SLEEP_PAUSE, SLEEP_CONTROL_COMMAND, initial.clone());
        // The enable flag is separate from the retained action.
        step(
            SET_AUTO_SWITCH_OFF,
            AUTO_SWITCH_COMMAND,
            with_byte(&initial, d1301::AUTO_SWITCH_ONCE_ASLEEP, 0),
        );
        step(SET_AUTO_SWITCH_ON, AUTO_SWITCH_COMMAND, initial.clone());
        // The remaining confirmed sleep settings are single-byte writes.
        let local_listening = with_byte(&initial, d1301::LISTENING_MODE, 1);
        step(
            SET_LISTENING_MODE_LOCAL,
            LISTENING_MODE_COMMAND,
            local_listening.clone(),
        );
        let local_default = with_byte(&local_listening, d1301::DEFAULT_LISTENING_MODE, 1);
        step(
            SET_DEFAULT_LISTENING_MODE_LOCAL,
            DEFAULT_LISTENING_MODE_COMMAND,
            local_default.clone(),
        );
        let noise_canceling = with_byte(&local_default, d1301::NOISE_CANCELING, 1);
        step(
            SET_NOISE_CANCELING_ON,
            NOISE_CANCELING_COMMAND,
            noise_canceling,
        );
        drop(step);

        let handle = SessionRunner::start(Replay(exchanges), target());
        let state = ready(&handle).await;
        assert_eq!(
            state.sleep_audio.map(|sleep| sleep.action),
            Some(PostSleepAudio::Pause)
        );

        let CommandOutcome::Applied(play_local_state) = wait(
            handle
                .dispatch(SessionCommand::SetPostSleepAudio(PostSleepAudio::PlayLocal))
                .expect("queue the play-local action"),
        )
        .await
        .expect("confirm the play-local action");
        assert_eq!(
            play_local_state.sleep_audio.map(|sleep| sleep.action),
            Some(PostSleepAudio::PlayLocal)
        );

        let CommandOutcome::Applied(restored) = wait(
            handle
                .dispatch(SessionCommand::SetPostSleepAudio(PostSleepAudio::Pause))
                .expect("queue the pause action"),
        )
        .await
        .expect("confirm the pause action");
        assert_eq!(
            restored.sleep_audio, state.sleep_audio,
            "the captured baseline is restored"
        );

        let CommandOutcome::Applied(disabled) = wait(
            handle
                .dispatch(SessionCommand::SetAutoSwitchOnceAsleep(false))
                .expect("queue disabling the sleep switch"),
        )
        .await
        .expect("confirm the disabled switch");
        assert_eq!(disabled.sleep_audio.map(|sleep| sleep.enabled), Some(false));
        assert_eq!(
            disabled.sleep_audio.map(|sleep| sleep.action),
            Some(PostSleepAudio::Pause),
            "the action is retained while the switch is off"
        );

        let CommandOutcome::Applied(enabled) = wait(
            handle
                .dispatch(SessionCommand::SetAutoSwitchOnceAsleep(true))
                .expect("queue enabling the sleep switch"),
        )
        .await
        .expect("confirm the enabled switch");
        assert_eq!(enabled.sleep_audio.map(|sleep| sleep.enabled), Some(true));

        let CommandOutcome::Applied(local) = wait(
            handle
                .dispatch(SessionCommand::SetListeningMode(ListeningMode::Local))
                .expect("queue the local listening mode"),
        )
        .await
        .expect("confirm the local listening mode");
        assert_eq!(local.listening_mode, Some(ListeningMode::Local));

        let CommandOutcome::Applied(default_local) = wait(
            handle
                .dispatch(SessionCommand::SetDefaultListeningMode(
                    ListeningMode::Local,
                ))
                .expect("queue the local default mode"),
        )
        .await
        .expect("confirm the local default mode");
        assert_eq!(
            default_local.default_listening_mode,
            Some(ListeningMode::Local)
        );

        let CommandOutcome::Applied(noise_canceling_state) = wait(
            handle
                .dispatch(SessionCommand::SetNoiseCanceling(true))
                .expect("queue noise canceling"),
        )
        .await
        .expect("confirm noise canceling");
        assert_eq!(noise_canceling_state.noise_canceling, Some(true));

        let confirmed = ready(&handle).await;
        assert_eq!(confirmed, noise_canceling_state);
        handle.close().await.expect("close the virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn an_acknowledged_write_without_read_back_is_not_applied() {
        let initial = state_body(POST_SLEEP_PAUSE);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (request(STATE_COMMAND, &[]), Some(state_frame(&initial))),
            (request(STATE_COMMAND, &[]), Some(state_frame(&initial))),
            (
                Bytes::from_static(SET_POST_SLEEP_PLAY_LOCAL),
                Some(inbound(SLEEP_CONTROL_COMMAND, &[])),
            ),
            (request(STATE_COMMAND, &[]), Some(state_frame(&initial))),
            // The read-back retry receives no reply, so the command reaches its
            // deadline with the unchanged captured state.
            (request(STATE_COMMAND, &[]), None),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;

        let result = wait(
            handle
                .dispatch(SessionCommand::SetPostSleepAudio(PostSleepAudio::PlayLocal))
                .expect("queue the action"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::SleepSettingMismatch {
                requested: D1301Setting::PostSleepAudio(PostSleepAudio::PlayLocal),
                observed: Some(D1301Setting::PostSleepAudio(PostSleepAudio::Pause)),
            })
        );
        handle.close().await.expect("close the virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn the_virtual_sleep_a30_confirms_a_sleep_action_cycle() {
        let handle = SessionRunner::start(VirtualD1301, target());
        let initial = ready(&handle).await;
        assert_eq!(
            initial
                .sleep_audio
                .map(|sleep| (sleep.enabled, sleep.action)),
            Some((true, PostSleepAudio::Pause))
        );

        let CommandOutcome::Applied(changed) = wait(
            handle
                .dispatch(SessionCommand::SetPostSleepAudio(PostSleepAudio::PlayLocal))
                .expect("queue the play-local action"),
        )
        .await
        .expect("confirm the play-local action");
        assert_eq!(
            changed.sleep_audio.map(|sleep| sleep.action),
            Some(PostSleepAudio::PlayLocal)
        );

        let CommandOutcome::Applied(restored) = wait(
            handle
                .dispatch(SessionCommand::SetPostSleepAudio(PostSleepAudio::Pause))
                .expect("queue the pause action"),
        )
        .await
        .expect("confirm the pause action");
        assert_eq!(restored.sleep_audio, initial.sleep_audio);
        handle.close().await.expect("close the virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_sleep_settings_on_a_model_that_does_not_report_them() {
        let handle = SessionRunner::start(VirtualA3062, target());
        ready(&handle).await;
        let result = wait(
            handle
                .dispatch(SessionCommand::SetPostSleepAudio(PostSleepAudio::Pause))
                .expect("queue the rejected setting"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidSleepSettingWrite))
        );
        handle.close().await.expect("close the virtual link");
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

    fn inbound(command: [u8; 2], body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, command, body).expect("inbound frame")
    }

    fn state_frame(body: &[u8]) -> Bytes {
        inbound(STATE_COMMAND, body)
    }

    fn state_body(fixture: &[u8]) -> Vec<u8> {
        assert_eq!(fixture[5..7], STATE_COMMAND);
        fixture[9..fixture.len() - 1].to_vec()
    }

    fn with_byte(body: &[u8], offset: usize, value: u8) -> Vec<u8> {
        let mut updated = body.to_vec();
        updated[offset] = value;
        updated
    }
}
