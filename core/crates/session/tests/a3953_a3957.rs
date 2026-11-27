//! End-to-end Liberty 4 (A3953) and Liberty 5 (A3957) sessions on the virtual transport.
//!
//! Every expected outbound frame below is the documented legacy wire format built
//! from the masked pilot capture, so the replay transport compares the session's
//! actual request byte for byte. State before and after each operation is an
//! independent inbound frame; the after states apply only the documented effect
//! of the write at the offsets the models crate measured.

#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use bytes::Bytes;
    use openscl4_models::{IDENTITY_COMMAND, STATE_COMMAND};
    use openscl4_protocol::{Direction, FrameDecoder, encode_frame};
    use openscl4_session::{
        CommandOutcome, ConnectionState, EqualizerSetting, ModelDriver, SessionCommand,
        SessionRunner, SpatialAudioContentMode, SpatialAudioMode, SpatialAudioState,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};

    use super::capture_replay::Replay;

    /// The measured `01 01` body with the synthetic model-prefixed serial.
    const A3953_STATE: &[u8] = include_bytes!("../../../data/a3953-pilot-state.bin");
    /// The measured `01 01` body with the synthetic model-prefixed serial.
    const A3957_STATE: &[u8] = include_bytes!("../../../data/a3957-pilot-state.bin");

    const A3953_IDENTITY: &str =
        "09FF0000010105240030332E323330332E323333393533544553543030303030303030B3";
    const A3957_IDENTITY: &str =
        "09FF0000010105240030332E393030332E393033393537544553543030303030303030BF";
    const A3953_AMBIENT_NC: &str = "08EE00000006811000003000000101BF";
    const A3953_AMBIENT_NORMAL: &str = "08EE00000006811000023000000101C1";
    const A3953_SPATIAL_ON: &str = "08EE00000010810D0001010096";
    const A3953_PRESS_3: &str = "08EE00000004850B00038D";
    const A3957_GAMING_ON: &str = "08EE00000010850B000197";
    const A3957_GAMING_OFF: &str = "08EE00000010850B000096";
    const A3953_EQUALIZER: &str = "08EE00000003876700FEFE0000B43CB43CB43CB43C7878B43CB43CB43CB43C7878FFFF003C3C3C3C3C3C3C3C00003C3C3C3C3C3C3C3C000000000000003C3C3C3C3C3C3C3C00003C3C3C3C3C3C3C3C000085648D628E638C6A780085648D628E638C6A7800002F";
    const A3957_EQUALIZER: &str = "08EE00000003877C00FEFE0000B43CB43CB43CB43C7878B43CB43CB43CB43C7878000000918273778B938F960000918273778B938F9600000000000000918273778B938F960000918273778B938F960000B4003C00B4003C00B4003C00B4003C0078007800B4003C00B4003C00B4003C00B4003C00780078000000B8";

    fn hex(text: &str) -> Bytes {
        let digits: Vec<u8> = text
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .map(|byte| (byte as char).to_digit(16).expect("hex digit") as u8)
            .collect();
        Bytes::from(
            digits
                .chunks(2)
                .map(|pair| (pair[0] << 4) | pair[1])
                .collect::<Vec<_>>(),
        )
    }

    fn body(fixture: &[u8]) -> Vec<u8> {
        let mut decoder = FrameDecoder::default();
        decoder.push(fixture);
        let frame = decoder
            .next_frame()
            .expect("valid state fixture")
            .expect("complete state fixture");
        assert_eq!(frame.command, STATE_COMMAND);
        assert_eq!(frame.direction, Direction::Inbound);
        frame.body.to_vec()
    }

    fn state_request() -> Bytes {
        encode_frame(Direction::Outbound, STATE_COMMAND, &[]).expect("state request")
    }

    fn identity_request() -> Bytes {
        encode_frame(Direction::Outbound, IDENTITY_COMMAND, &[]).expect("identity request")
    }

    fn state_reply(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, STATE_COMMAND, body).expect("state reply")
    }

    fn acknowledgement(command: [u8; 2]) -> Bytes {
        encode_frame(Direction::Inbound, command, &[]).expect("acknowledgement")
    }

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        }
    }

    fn ready(state: &openscl4_session::SessionView) -> openscl4_session::DeviceState {
        let ConnectionState::Ready(state) = state.connection else {
            panic!("expected a confirmed device state");
        };
        state
    }

    #[tokio::test(start_paused = true)]
    async fn reads_the_measured_a3953_state() {
        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3953_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3953_STATE))),
            ])),
            target(),
        );
        let view = handle.confirmed_view().await.expect("captured A3953");
        let state = ready(&view);
        assert_eq!(state.model, ModelDriver::A3953);
        assert_eq!(state.battery.left, Some(100));
        assert_eq!(state.battery.right, Some(100));
        assert_eq!(state.battery.case, Some(100));
        assert_eq!(state.sound_mode, Some(openscl4_session::SoundMode::Normal));
        assert_eq!(
            state.spatial_audio,
            Some(SpatialAudioState {
                enabled: false,
                mode: SpatialAudioMode::Fixed,
                content: SpatialAudioContentMode::Music,
            })
        );
        assert_eq!(state.press_sensitivity, Some(0));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_and_restores_the_a3953_ambient_mode() {
        let pilot = body(A3953_STATE);
        let mut noise_canceling = pilot.clone();
        noise_canceling[121..127].copy_from_slice(&[0, 0x30, 0, 0, 1, 1]);
        let mut normal = noise_canceling.clone();
        normal[121..127].copy_from_slice(&[2, 0x30, 0, 0, 1, 1]);

        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3953_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3953_STATE))),
                (state_request(), Some(state_reply(&pilot))),
                (hex(A3953_AMBIENT_NC), Some(acknowledgement([0x06, 0x81]))),
                (state_request(), Some(state_reply(&noise_canceling))),
                (state_request(), Some(state_reply(&noise_canceling))),
                (
                    hex(A3953_AMBIENT_NORMAL),
                    Some(acknowledgement([0x06, 0x81])),
                ),
                (state_request(), Some(state_reply(&normal))),
            ])),
            target(),
        );
        assert_eq!(
            ready(&handle.confirmed_view().await.expect("pilot state")).sound_mode,
            Some(openscl4_session::SoundMode::Normal)
        );

        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::SetAmbientSoundMode(
                openscl4_session::SoundMode::NoiseCanceling,
            ))
            .expect("accept the ambient write")
            .wait()
            .await
            .expect("confirm the ambient write");
        assert_eq!(
            state.sound_mode,
            Some(openscl4_session::SoundMode::NoiseCanceling)
        );

        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::SetAmbientSoundMode(
                openscl4_session::SoundMode::Normal,
            ))
            .expect("accept the restore")
            .wait()
            .await
            .expect("confirm the restore");
        assert_eq!(state.sound_mode, Some(openscl4_session::SoundMode::Normal));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3953_spatial_audio_write() {
        let pilot = body(A3953_STATE);
        let mut enabled = pilot.clone();
        enabled[146..149].copy_from_slice(&[1, 1, 0]);
        let requested = SpatialAudioState {
            enabled: true,
            mode: SpatialAudioMode::Fixed,
            content: SpatialAudioContentMode::Music,
        };
        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3953_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3953_STATE))),
                (state_request(), Some(state_reply(&pilot))),
                (hex(A3953_SPATIAL_ON), Some(acknowledgement([0x10, 0x81]))),
                (state_request(), Some(state_reply(&enabled))),
            ])),
            target(),
        );
        handle.confirmed_view().await.expect("pilot state");
        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::SetSpatialAudio(requested))
            .expect("accept the spatial write")
            .wait()
            .await
            .expect("confirm the spatial write");
        assert_eq!(state.spatial_audio, Some(requested));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3953_press_sensitivity_write() {
        let pilot = body(A3953_STATE);
        let mut pressed = pilot.clone();
        pressed[155] = 3;
        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3953_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3953_STATE))),
                (state_request(), Some(state_reply(&pilot))),
                (hex(A3953_PRESS_3), Some(acknowledgement([0x04, 0x85]))),
                (state_request(), Some(state_reply(&pressed))),
            ])),
            target(),
        );
        handle.confirmed_view().await.expect("pilot state");
        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::SetPressSensitivity(3))
            .expect("accept the press-sensitivity write")
            .wait()
            .await
            .expect("confirm the press-sensitivity write");
        assert_eq!(state.press_sensitivity, Some(3));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3953_custom_equalizer_curve() {
        let pilot = body(A3953_STATE);
        let setting = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [60, -60, 60, -60, 60, -60, 60, -60, 0, 0],
        };
        let mut after = pilot.clone();
        after[32..34].copy_from_slice(&setting.preset_id.to_le_bytes());
        after[34..54].copy_from_slice(&hex(A3953_EQUALIZER)[13..33]);
        after[55] = 0;
        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3953_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3953_STATE))),
                (state_request(), Some(state_reply(&pilot))),
                (hex(A3953_EQUALIZER), Some(acknowledgement([0x03, 0x87]))),
                (state_request(), Some(state_reply(&after))),
            ])),
            target(),
        );
        handle.confirmed_view().await.expect("pilot state");
        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::ApplyQuickPreset(
                openscl4_session::QuickPresetValues {
                    equalizer: Some(setting),
                    ..Default::default()
                },
            ))
            .expect("accept the equalizer write")
            .wait()
            .await
            .expect("confirm the equalizer write");
        assert_eq!(state.equalizer, Some(setting));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn reads_the_measured_a3957_state() {
        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3957_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3957_STATE))),
            ])),
            target(),
        );
        let view = handle.confirmed_view().await.expect("captured A3957");
        let state = ready(&view);
        assert_eq!(state.model, ModelDriver::A3957);
        // The measured ten-level charging scale counts from one.
        assert_eq!(state.battery.left, Some(90));
        assert_eq!(state.battery.right, Some(90));
        assert_eq!(state.battery.case, Some(30));
        assert_eq!(
            state.sound_mode,
            Some(openscl4_session::SoundMode::NoiseCanceling)
        );
        assert_eq!(state.gaming_mode, Some(false));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_and_restores_the_a3957_gaming_mode() {
        let pilot = body(A3957_STATE);
        let mut enabled = pilot.clone();
        enabled[146] = 1;
        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3957_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3957_STATE))),
                (state_request(), Some(state_reply(&pilot))),
                (hex(A3957_GAMING_ON), Some(acknowledgement([0x10, 0x85]))),
                (state_request(), Some(state_reply(&enabled))),
                (state_request(), Some(state_reply(&enabled))),
                (hex(A3957_GAMING_OFF), Some(acknowledgement([0x10, 0x85]))),
                (state_request(), Some(state_reply(&pilot))),
            ])),
            target(),
        );
        ready(&handle.confirmed_view().await.expect("pilot state"));

        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::SetGamingMode(true))
            .expect("accept the gaming-mode write")
            .wait()
            .await
            .expect("confirm the gaming-mode write");
        assert_eq!(state.gaming_mode, Some(true));

        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::SetGamingMode(false))
            .expect("accept the restore")
            .wait()
            .await
            .expect("confirm the restore");
        assert_eq!(state.gaming_mode, Some(false));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_a3957_custom_equalizer_curve() {
        let pilot = body(A3957_STATE);
        let setting = EqualizerSetting {
            preset_id: 0xfefe,
            volume_adjustments: [60, -60, 60, -60, 60, -60, 60, -60, 0, 0],
        };
        let mut after = pilot.clone();
        after[38..40].copy_from_slice(&setting.preset_id.to_le_bytes());
        after[40..60].copy_from_slice(&hex(A3957_EQUALIZER)[13..33]);
        after[61] = 0;
        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3957_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3957_STATE))),
                (state_request(), Some(state_reply(&pilot))),
                (hex(A3957_EQUALIZER), Some(acknowledgement([0x03, 0x87]))),
                (state_request(), Some(state_reply(&after))),
            ])),
            target(),
        );
        ready(&handle.confirmed_view().await.expect("pilot state"));
        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::ApplyQuickPreset(
                openscl4_session::QuickPresetValues {
                    equalizer: Some(setting),
                    ..Default::default()
                },
            ))
            .expect("accept the equalizer write")
            .wait()
            .await
            .expect("confirm the equalizer write");
        assert_eq!(state.equalizer, Some(setting));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_liberty_writes_the_model_does_not_report() {
        let handle = SessionRunner::start(
            Replay(VecDeque::from([
                (identity_request(), Some(hex(A3957_IDENTITY))),
                (state_request(), Some(Bytes::from_static(A3957_STATE))),
            ])),
            target(),
        );
        handle.confirmed_view().await.expect("pilot state");
        let spatial = handle
            .dispatch(SessionCommand::SetSpatialAudio(SpatialAudioState {
                enabled: true,
                mode: SpatialAudioMode::Fixed,
                content: SpatialAudioContentMode::Music,
            }))
            .expect("the command reaches validation")
            .wait()
            .await;
        assert!(spatial.is_err(), "A3957 reports no spatial-audio controls");
        let press = handle
            .dispatch(SessionCommand::SetPressSensitivity(3))
            .expect("the command reaches validation")
            .wait()
            .await;
        assert!(press.is_err(), "A3957 reports no press sensitivity");
        handle.close().await.expect("release the link");
    }
}
