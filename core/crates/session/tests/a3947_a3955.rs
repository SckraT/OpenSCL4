//! End-to-end sessions for the measured Liberty 4 NC and P40i formats.
//!
//! Each test drives `SessionRunner` on a scripted transport. Every outbound
//! frame is compared byte for byte with the published reference, and every
//! write is confirmed by the independent device replies that follow it.

#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use bytes::Bytes;
    use openscl4_models::{
        A3947_GAMING_MODE_COMMAND, AutoPowerOff, EqualizerSetting, ImmersiveExperience, ModelError,
        SoundMode,
    };
    use openscl4_protocol::{Direction, FrameDecoder, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, ConnectionState, DeviceState, DispatchError, SessionCommand,
        SessionHandle, SessionRunner,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};

    use super::capture_replay::Replay;

    // Reference wire opcodes from the legacy fixtures and packet builders.
    const IDENTITY_COMMAND: [u8; 2] = [0x01, 0x05];
    const STATE_COMMAND: [u8; 2] = [0x01, 0x01];
    const SOUND_MODE_COMMAND: [u8; 2] = [0x06, 0x81];
    const EQUALIZER_COMMAND: [u8; 2] = [0x03, 0x87];
    const IMMERSIVE_EXPERIENCE_COMMAND: [u8; 2] = [0x12, 0x81];

    /// The masked `01 01` state responses from `core/data` and the measured
    /// equalizer write bodies from the legacy golden tests.
    const A3947_STATE: &[u8] = include_bytes!("../../../data/a3947-pilot-state.bin");
    const A3955_STATE: &[u8] = include_bytes!("../../../data/a3955-pilot-state.bin");
    const A3947_EQ_WRITE: &[u8] = include_bytes!("../../../data/a3947-eq-write.bin");
    const A3955_EQ_WRITE: &[u8] = include_bytes!("../../../data/a3955-eq-write.bin");

    const A3947_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [0, 1, 2, 3, 4, 5, 6, 7, 0, 0],
    };
    const A3955_CURVE: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [60, -60, 60, -60, 60, -60, 60, -60, 0, 0],
    };

    #[tokio::test(start_paused = true)]
    async fn confirms_liberty_sound_mode_gaming_and_equalizer_from_preset_replies() {
        let initial = fixture_body(A3947_STATE);
        let normal = with_byte(&initial, 126, 0x02);
        let gaming = with_byte(&initial, 143, 0x01);
        let custom = with_equalizer(&initial, A3947_EQ_WRITE, 37..39, 39..59);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            // Normal ambient mode, confirmed by a fresh state response.
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (
                request(
                    SOUND_MODE_COMMAND,
                    &[0x02, 0x50, 0x01, 0x00, 0x01, 0x01, 0x03],
                ),
                Some(ack(SOUND_MODE_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &normal)),
            ),
            // Restore noise canceling.
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &normal)),
            ),
            (
                request(
                    SOUND_MODE_COMMAND,
                    &[0x00, 0x50, 0x01, 0x00, 0x01, 0x01, 0x03],
                ),
                Some(ack(SOUND_MODE_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            // Gaming mode is the Liberty 4 NC specific flag.
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (
                request(A3947_GAMING_MODE_COMMAND, &[0x01]),
                Some(ack(A3947_GAMING_MODE_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &gaming)),
            ),
            // Equalizer write, confirmed by the derived read-back state.
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (
                request(EQUALIZER_COMMAND, A3947_EQ_WRITE),
                Some(ack(EQUALIZER_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &custom)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let baseline = ready(&handle).await;
        assert_eq!(baseline.model, openscl4_session::ModelDriver::A3947);
        assert_eq!(baseline.battery.left, Some(40));
        assert_eq!(baseline.battery.right, Some(80));
        assert_eq!(baseline.battery.case, Some(40));
        assert_eq!(
            baseline.firmware_left.map(|version| version.to_string()),
            Some("05.88".to_owned())
        );
        assert_eq!(baseline.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(baseline.gaming_mode, Some(false));
        assert_eq!(
            baseline.auto_power_off,
            Some(AutoPowerOff {
                enabled: true,
                duration: 1,
            })
        );
        assert_eq!(baseline.tws_connected, Some(true));

        let normal_state = applied(
            wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Normal))
                    .expect("queue normal"),
            )
            .await
            .expect("confirm normal"),
        );
        assert_eq!(normal_state.sound_mode, Some(SoundMode::Normal));

        let restored = applied(
            wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(
                        SoundMode::NoiseCanceling,
                    ))
                    .expect("queue restore"),
            )
            .await
            .expect("confirm restore"),
        );
        assert_eq!(restored.sound_mode, Some(SoundMode::NoiseCanceling));

        let gaming_state = applied(
            wait(
                handle
                    .dispatch(SessionCommand::SetGamingMode(true))
                    .expect("queue gaming mode"),
            )
            .await
            .expect("confirm gaming mode"),
        );
        assert_eq!(gaming_state.gaming_mode, Some(true));

        let eq_state = applied(
            wait(
                handle
                    .dispatch(SessionCommand::ApplyQuickPreset(
                        openscl4_session::QuickPresetValues {
                            equalizer: Some(A3947_CURVE),
                            ..openscl4_session::QuickPresetValues::default()
                        },
                    ))
                    .expect("queue the equalizer curve"),
            )
            .await
            .expect("confirm the equalizer curve"),
        );
        assert_eq!(eq_state.equalizer, Some(A3947_CURVE));
        assert_eq!(ready(&handle).await.equalizer, Some(A3947_CURVE));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_p40i_sound_mode_and_equalizer_from_preset_replies() {
        let initial = fixture_body(A3955_STATE);
        let transparency = with_byte(&initial, 119, 0x01);
        let custom = with_equalizer(&initial, A3955_EQ_WRITE, 38..40, 40..60);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            // Transparency, confirmed by a fresh state response.
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (
                request(
                    SOUND_MODE_COMMAND,
                    &[0x01, 0x51, 0x01, 0x02, 0x01, 0xff, 0x01],
                ),
                Some(ack(SOUND_MODE_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &transparency)),
            ),
            // Restore noise canceling.
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &transparency)),
            ),
            (
                request(
                    SOUND_MODE_COMMAND,
                    &[0x00, 0x51, 0x01, 0x02, 0x01, 0xff, 0x01],
                ),
                Some(ack(SOUND_MODE_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            // Equalizer write, confirmed by the derived read-back state.
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (
                request(EQUALIZER_COMMAND, A3955_EQ_WRITE),
                Some(ack(EQUALIZER_COMMAND)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &custom)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let baseline = ready(&handle).await;
        assert_eq!(baseline.model, openscl4_session::ModelDriver::A3955);
        assert_eq!(baseline.battery.left, Some(100));
        assert_eq!(baseline.battery.right, Some(100));
        assert_eq!(baseline.battery.case, Some(60));
        assert_eq!(
            baseline.firmware_left.map(|version| version.to_string()),
            Some("01.61".to_owned())
        );
        assert_eq!(baseline.sound_mode, Some(SoundMode::NoiseCanceling));
        assert_eq!(baseline.gaming_mode, None);
        assert_eq!(baseline.tws_connected, Some(true));

        let transparency_state = applied(
            wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(SoundMode::Transparency))
                    .expect("queue transparency"),
            )
            .await
            .expect("confirm transparency"),
        );
        assert_eq!(transparency_state.sound_mode, Some(SoundMode::Transparency));

        let restored = applied(
            wait(
                handle
                    .dispatch(SessionCommand::SetAmbientSoundMode(
                        SoundMode::NoiseCanceling,
                    ))
                    .expect("queue restore"),
            )
            .await
            .expect("confirm restore"),
        );
        assert_eq!(restored.sound_mode, Some(SoundMode::NoiseCanceling));

        let eq_state = applied(
            wait(
                handle
                    .dispatch(SessionCommand::ApplyQuickPreset(
                        openscl4_session::QuickPresetValues {
                            equalizer: Some(A3955_CURVE),
                            ..openscl4_session::QuickPresetValues::default()
                        },
                    ))
                    .expect("queue the equalizer curve"),
            )
            .await
            .expect("confirm the equalizer curve"),
        );
        assert_eq!(eq_state.equalizer, Some(A3955_CURVE));
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn accepts_the_p40i_immersive_experience_from_an_acknowledgement() {
        let initial = fixture_body(A3955_STATE);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (
                request(IMMERSIVE_EXPERIENCE_COMMAND, &[0x02]),
                Some(ack(IMMERSIVE_EXPERIENCE_COMMAND)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let baseline = ready(&handle).await;

        let outcome = wait(
            handle
                .dispatch(SessionCommand::SetImmersiveExperience(
                    ImmersiveExperience::MovieMode,
                ))
                .expect("queue movie mode"),
        )
        .await
        .expect("accept movie mode");
        let CommandOutcome::Accepted(state) = outcome else {
            panic!("a write-only action is accepted, not read back");
        };
        // The state does not report the experience; acceptance keeps the last
        // confirmed snapshot.
        assert_eq!(state, baseline);
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn unacknowledged_immersive_experience_is_unconfirmed() {
        let initial = fixture_body(A3955_STATE);
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &initial)),
            ),
            (request(IMMERSIVE_EXPERIENCE_COMMAND, &[0x02]), None),
        ]));
        let handle = SessionRunner::start(replay, target());
        ready(&handle).await;

        let result = wait(
            handle
                .dispatch(SessionCommand::SetImmersiveExperience(
                    ImmersiveExperience::MovieMode,
                ))
                .expect("queue movie mode"),
        )
        .await;
        assert!(
            matches!(result, Err(CommandError::Unconfirmed(_))),
            "{result:?}"
        );
        handle.close().await.expect("close virtual link");
    }

    #[tokio::test(start_paused = true)]
    async fn p40i_equalizer_write_is_rejected_while_earbuds_are_disconnected() {
        let mut disconnected = fixture_body(A3955_STATE);
        // The measured TWS status is the second state byte.
        disconnected[1] = 0;
        let replay = Replay(VecDeque::from([
            (request(IDENTITY_COMMAND, &[]), None),
            (
                request(STATE_COMMAND, &[]),
                Some(inbound(STATE_COMMAND, &disconnected)),
            ),
        ]));
        let handle = SessionRunner::start(replay, target());
        let baseline = ready(&handle).await;
        assert_eq!(baseline.tws_connected, Some(false));
        assert!(!baseline.equalizer_write_available());

        assert_eq!(
            handle.set_eq_live(A3955_CURVE),
            Err(DispatchError::UnavailableEqualizer)
        );
        let result = wait(
            handle
                .dispatch(SessionCommand::ApplyQuickPreset(
                    openscl4_session::QuickPresetValues {
                        equalizer: Some(A3955_CURVE),
                        ..openscl4_session::QuickPresetValues::default()
                    },
                ))
                .expect("queue the rejected curve"),
        )
        .await;
        assert_eq!(
            result,
            Err(CommandError::Model(ModelError::InvalidEqualizerWrite))
        );
        handle.close().await.expect("close virtual link");
    }

    fn applied(outcome: CommandOutcome) -> DeviceState {
        match outcome {
            CommandOutcome::Applied(state) => state,
            CommandOutcome::Accepted(_) => panic!("expected a read-back confirmation"),
        }
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

    async fn wait(
        pending: openscl4_session::PendingCommand,
    ) -> Result<CommandOutcome, CommandError> {
        tokio::time::timeout(std::time::Duration::from_secs(10), pending.wait())
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

    fn ack(command: [u8; 2]) -> Bytes {
        inbound(command, &[])
    }

    fn fixture_body(frame: &[u8]) -> Vec<u8> {
        let mut decoder = FrameDecoder::default();
        decoder.push(frame);
        let frame = decoder
            .next_frame()
            .expect("valid fixture frame")
            .expect("complete fixture frame");
        frame.body.to_vec()
    }

    fn with_byte(initial: &[u8], offset: usize, value: u8) -> Vec<u8> {
        let mut body = initial.to_vec();
        body[offset] = value;
        body
    }

    /// Applies the documented equalizer write effect to a confirmed state.
    fn with_equalizer(
        initial: &[u8],
        write: &[u8],
        preset: std::ops::Range<usize>,
        bands: std::ops::Range<usize>,
    ) -> Vec<u8> {
        let mut body = initial.to_vec();
        body[preset].copy_from_slice(&write[0..2]);
        body[bands].copy_from_slice(&write[4..24]);
        body
    }
}
