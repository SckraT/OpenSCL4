//! Replays the legacy A3949 equalizer, touch-tone, button and reset formats.
//!
//! The outbound frames are the legacy driver's documented command bodies
//! (`02 83` equalizer with DRC, `01 83` touch tone, `04 81` button action and
//! `04 82` reset). The inbound replies before and after each operation are the
//! measured P20i capture with the documented field patched, so the read-back
//! assertions do not depend on the codec under test.

#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use bytes::Bytes;
    use openscl4_models::{
        A3949_DISABLED_ACTION, A3949_EQUALIZER_BANDS, A3949_EQUALIZER_PRESET, A3949_TOUCH_TONE,
        A3949Button, EqualizerSetting, IDENTITY_COMMAND, STATE_COMMAND,
    };
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandError, CommandOutcome, ConnectionState, QuickPresetValues, SessionCommand,
        SessionRunner,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};

    use super::capture_replay::Replay;

    const PILOT_STATE: &[u8] = include_bytes!("../../../data/a3949-pilot-state.bin");

    /// `08 EE 00 00 00 02 83 20 00` followed by the 22-byte body of the legacy
    /// `r50i_set_eq_matches_soundcore_app` request and its additive checksum.
    const EQUALIZER_WRITE: &[u8] =
        b"\x08\xee\0\0\0\x02\x83\x20\0\x0e\0\x78\x96\x96\x8c\xa0\xaa\x96\xa0\x78\0\x77\x7b\x7a\x78\x7b\x7c\x78\x7d\x78\0\x19";
    const EQUALIZER_ACK: &[u8] = b"\x09\xff\0\0\x01\x02\x83\x0a\0\x98";
    /// `01 83` with the measured one-byte touch-tone body.
    const TOUCH_TONE_WRITE: &[u8] = b"\x08\xee\0\0\0\x01\x83\x0b\0\x01\x86";
    const TOUCH_TONE_ACK: &[u8] = b"\x09\xff\0\0\x01\x01\x83\x0a\0\x97";
    /// `04 81` with the measured button and action bytes: left, single press,
    /// action 2 (Previous Song).
    const BUTTON_WRITE: &[u8] = b"\x08\xee\0\0\0\x04\x81\x0d\0\0\x02\x02\x8c";
    const BUTTON_ACK: &[u8] = b"\x09\xff\0\0\x01\x04\x81\x0a\0\x98";
    /// `04 82` with the measured empty body.
    const RESET_BUTTONS_WRITE: &[u8] = b"\x08\xee\0\0\0\x04\x82\x0a\0\x86";
    const RESET_BUTTONS_ACK: &[u8] = b"\x09\xff\0\0\x01\x04\x82\x0a\0\x99";

    const BUTTON_LEFT_SINGLE: usize = 55;
    const PIANO: EqualizerSetting = EqualizerSetting {
        preset_id: 0x000e,
        volume_adjustments: [0, 30, 30, 20, 40, 50, 30, 40, 0, -120],
    };

    #[tokio::test(start_paused = true)]
    async fn confirms_the_legacy_equalizer_with_drc_write_by_read_back() {
        let request = state_request();
        let baseline = pilot_body();
        let changed = with_bands(&baseline, PIANO);
        let mut exchanges = VecDeque::from([
            (identity_request(), None),
            (request.clone(), Some(inbound(&baseline))),
        ]);
        exchanges.push_back((request.clone(), Some(inbound(&baseline))));
        exchanges.push_back((
            Bytes::from_static(EQUALIZER_WRITE),
            Some(Bytes::from_static(EQUALIZER_ACK)),
        ));
        exchanges.push_back((request.clone(), Some(inbound(&changed))));

        let handle = SessionRunner::start(Replay(exchanges), target());
        let initial = ready(&handle).await;
        let outcome = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                equalizer: Some(PIANO),
                ..QuickPresetValues::default()
            }))
            .expect("accept the equalizer command")
            .wait()
            .await
            .expect("confirm the equalizer write");
        let CommandOutcome::Applied(state) = outcome else {
            panic!("expected an applied equalizer outcome");
        };
        assert_eq!(state.equalizer, Some(PIANO));
        assert_eq!(state.model, initial.model);
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn confirms_the_legacy_touch_tone_write_by_read_back() {
        let request = state_request();
        let baseline = pilot_body();
        let mut changed = baseline.clone();
        changed[A3949_TOUCH_TONE] = 1;
        let mut exchanges = VecDeque::from([
            (identity_request(), None),
            (request.clone(), Some(inbound(&baseline))),
        ]);
        exchanges.push_back((request.clone(), Some(inbound(&baseline))));
        exchanges.push_back((
            Bytes::from_static(TOUCH_TONE_WRITE),
            Some(Bytes::from_static(TOUCH_TONE_ACK)),
        ));
        exchanges.push_back((request.clone(), Some(inbound(&changed))));

        let handle = SessionRunner::start(Replay(exchanges), target());
        assert_eq!(ready(&handle).await.touch_tone, Some(false));
        let outcome = handle
            .dispatch(SessionCommand::SetTouchTone(true))
            .expect("accept the touch-tone command")
            .wait()
            .await
            .expect("confirm the touch-tone write");
        let CommandOutcome::Applied(state) = outcome else {
            panic!("expected an applied touch-tone outcome");
        };
        assert_eq!(state.touch_tone, Some(true));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn writes_the_connected_button_action_and_confirms_it_by_read_back() {
        let request = state_request();
        let baseline = pilot_body();
        let mut changed = baseline.clone();
        changed[BUTTON_LEFT_SINGLE + 1] = 0xf2;
        let mut exchanges = VecDeque::from([
            (identity_request(), None),
            (request.clone(), Some(inbound(&baseline))),
        ]);
        exchanges.push_back((request.clone(), Some(inbound(&baseline))));
        exchanges.push_back((
            Bytes::from_static(BUTTON_WRITE),
            Some(Bytes::from_static(BUTTON_ACK)),
        ));
        exchanges.push_back((request.clone(), Some(inbound(&changed))));

        let handle = SessionRunner::start(Replay(exchanges), target());
        let initial = ready(&handle).await;
        assert_eq!(
            button_action(initial, A3949Button::LeftSinglePress),
            Some(A3949_DISABLED_ACTION)
        );
        let outcome = handle
            .dispatch(SessionCommand::SetButtonAction {
                button: A3949Button::LeftSinglePress,
                action: Some(2),
            })
            .expect("accept the button command")
            .wait()
            .await
            .expect("confirm the button write");
        let CommandOutcome::Applied(state) = outcome else {
            panic!("expected an applied button outcome");
        };
        assert_eq!(button_action(state, A3949Button::LeftSinglePress), Some(2));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn writes_the_disconnected_button_action_for_a_split_pair() {
        let request = state_request();
        let baseline = disconnected_body();
        let mut changed = baseline.clone();
        changed[BUTTON_LEFT_SINGLE + 1] = 0x3f;
        let mut exchanges = VecDeque::from([
            (identity_request(), None),
            (request.clone(), Some(inbound(&baseline))),
        ]);
        exchanges.push_back((request.clone(), Some(inbound(&baseline))));
        exchanges.push_back((
            Bytes::from_static(b"\x08\xee\0\0\0\x04\x81\x0d\0\0\x02\x03\x8d"),
            Some(Bytes::from_static(BUTTON_ACK)),
        ));
        exchanges.push_back((request.clone(), Some(inbound(&changed))));

        let handle = SessionRunner::start(Replay(exchanges), target());
        let initial = ready(&handle).await;
        assert_eq!(initial.tws.map(|tws| tws.connected), Some(false));
        let outcome = handle
            .dispatch(SessionCommand::SetButtonAction {
                button: A3949Button::LeftSinglePress,
                action: Some(3),
            })
            .expect("accept the button command")
            .wait()
            .await
            .expect("confirm the button write");
        let CommandOutcome::Applied(state) = outcome else {
            panic!("expected an applied button outcome");
        };
        let tws = state.tws.expect("measured TWS status");
        assert_eq!(
            state
                .buttons
                .expect("measured buttons")
                .status(A3949Button::LeftSinglePress)
                .action(tws),
            3
        );
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn rejects_the_equalizer_write_without_touching_a_split_pair() {
        let request = state_request();
        let baseline = disconnected_body();
        let exchanges = VecDeque::from([
            (identity_request(), None),
            (request, Some(inbound(&baseline))),
        ]);

        let handle = SessionRunner::start(Replay(exchanges), target());
        let initial = ready(&handle).await;
        let result = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                equalizer: Some(PIANO),
                ..QuickPresetValues::default()
            }))
            .expect("accept the equalizer command")
            .wait()
            .await;
        assert_eq!(result, Err(CommandError::Unavailable));
        let view = handle.confirmed_view().await.expect("confirmed view");
        assert_eq!(view.connection, ConnectionState::Ready(initial));
        handle.close().await.expect("release the link");
    }

    #[tokio::test(start_paused = true)]
    async fn accepts_the_reset_buttons_action_after_the_device_acknowledgement() {
        let request = state_request();
        let baseline = pilot_body();
        let mut exchanges = VecDeque::from([
            (identity_request(), None),
            (request.clone(), Some(inbound(&baseline))),
        ]);
        exchanges.push_back((request.clone(), Some(inbound(&baseline))));
        exchanges.push_back((
            Bytes::from_static(RESET_BUTTONS_WRITE),
            Some(Bytes::from_static(RESET_BUTTONS_ACK)),
        ));
        exchanges.push_back((request.clone(), Some(inbound(&baseline))));

        let handle = SessionRunner::start(Replay(exchanges), target());
        ready(&handle).await;
        let outcome = handle
            .dispatch(SessionCommand::ResetButtons)
            .expect("accept the reset command")
            .wait()
            .await
            .expect("confirm the reset action");
        let CommandOutcome::Accepted(state) = outcome else {
            panic!("expected an accepted reset outcome");
        };
        assert_eq!(state.model, openscl4_models::ModelDriver::A3949);
        handle.close().await.expect("release the link");
    }

    fn target() -> ConnectTarget {
        ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        }
    }

    async fn ready(handle: &openscl4_session::SessionHandle) -> openscl4_session::DeviceState {
        let view = handle.confirmed_view().await.expect("confirmed state");
        let ConnectionState::Ready(state) = view.connection else {
            panic!("expected a ready A3949");
        };
        state
    }

    fn button_action(state: openscl4_session::DeviceState, button: A3949Button) -> Option<u8> {
        let tws = state.tws?;
        Some(state.buttons?.status(button).action(tws))
    }

    fn identity_request() -> Bytes {
        encode_frame(Direction::Outbound, IDENTITY_COMMAND, &[]).expect("identity request")
    }

    fn state_request() -> Bytes {
        encode_frame(Direction::Outbound, STATE_COMMAND, &[]).expect("state request")
    }

    fn inbound(body: &[u8]) -> Bytes {
        encode_frame(Direction::Inbound, STATE_COMMAND, body).expect("state reply")
    }

    fn pilot_body() -> Vec<u8> {
        PILOT_STATE[9..PILOT_STATE.len() - 1].to_vec()
    }

    /// The measured body with the left earbud disconnected and only the
    /// disconnected action nibble set on the left single press.
    fn disconnected_body() -> Vec<u8> {
        let mut body = pilot_body();
        body[1] = 0;
        body[BUTTON_LEFT_SINGLE + 1] = 0x0f;
        body
    }

    fn with_bands(body: &[u8], setting: EqualizerSetting) -> Vec<u8> {
        let mut changed = body.to_vec();
        changed[A3949_EQUALIZER_PRESET].copy_from_slice(&setting.preset_id.to_le_bytes());
        for (band, gain) in setting.volume_adjustments.iter().enumerate() {
            changed[A3949_EQUALIZER_BANDS.start + band] = (gain + 120) as u8;
        }
        changed
    }
}
