//! Exercises the session with the measured A3062 equalizer write and read-back.

#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use bytes::Bytes;
    use openscl4_models::{IDENTITY_COMMAND, STATE_COMMAND};
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{
        CommandOutcome, ConnectionState, EqualizerSetting, QuickPresetValues, SessionCommand,
        SessionHandle, SessionRunner,
    };
    use openscl4_transport::{ConnectTarget, RedactedMac};

    use super::capture_replay::Replay;

    // The before and restored state responses in the measurement are byte-identical.
    const SIGNATURE_STATE: &[u8] = include_bytes!("../../../data/a3062-eq-before.bin");
    const CUSTOM_STATE: &[u8] = include_bytes!("../../../data/a3062-eq-custom.bin");
    const CUSTOM_WRITE: &[u8] = include_bytes!("../../../data/a3062-eq-write.bin");
    const SIGNATURE: EqualizerSetting = EqualizerSetting {
        preset_id: 0,
        volume_adjustments: [0, 0, 0, 0, 0, 0, 0, 0, 0, -120],
    };
    const CUSTOM: EqualizerSetting = EqualizerSetting {
        preset_id: 0xfefe,
        volume_adjustments: [30, -30, 60, 0, -60, 20, 0, -20, 0, -120],
    };

    #[tokio::test(start_paused = true)]
    async fn confirms_custom_and_restored_eq_from_captured_replies() {
        let state_request = encode_frame(Direction::Outbound, STATE_COMMAND, &[]).unwrap();
        let identity_request = encode_frame(Direction::Outbound, IDENTITY_COMMAND, &[]).unwrap();
        let replay = Replay(VecDeque::from([
            (identity_request, None),
            (state_request.clone(), Some(SIGNATURE_STATE)),
            (state_request.clone(), Some(SIGNATURE_STATE)),
            (Bytes::from_static(CUSTOM_WRITE), None),
            (state_request.clone(), Some(CUSTOM_STATE)),
            (state_request.clone(), Some(CUSTOM_STATE)),
            (signature_write(), None),
            (state_request, Some(SIGNATURE_STATE)),
        ]));
        let handle = SessionRunner::start(
            replay,
            ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
            },
        );
        let view = handle.confirmed_view().await.expect("measured baseline");
        let ConnectionState::Ready(initial) = view.connection else {
            panic!("expected ready A3062");
        };
        assert_eq!(initial.equalizer, Some(SIGNATURE));

        // No ACK is scripted. Confirmation requires the captured device response.
        assert_eq!(apply(&handle, CUSTOM).await, CUSTOM);
        assert_eq!(apply(&handle, SIGNATURE).await, SIGNATURE);
        assert_eq!(handle.subscribe().borrow().connection, view.connection);
        handle.close().await.expect("release the link");
    }

    async fn apply(handle: &SessionHandle, setting: EqualizerSetting) -> EqualizerSetting {
        let CommandOutcome::Applied(state) = handle
            .dispatch(SessionCommand::ApplyQuickPreset(QuickPresetValues {
                equalizer: Some(setting),
                ..QuickPresetValues::default()
            }))
            .expect("accept the preset")
            .wait()
            .await
            .expect("confirm the preset");
        state.equalizer.expect("curve in the captured response")
    }

    fn signature_write() -> Bytes {
        // The capture includes the restore response. For the restore request,
        // keep the measured command layout and replace its preset and both curves.
        let mut request = CUSTOM_WRITE.to_vec();
        let signature_bands = &SIGNATURE_STATE[34..44];
        request[9..11].copy_from_slice(&SIGNATURE.preset_id.to_le_bytes());
        request[13..23].copy_from_slice(signature_bands);
        request[51..61].copy_from_slice(signature_bands);
        let checksum_at = request.len() - 1;
        request[checksum_at] = request[..checksum_at]
            .iter()
            .copied()
            .fold(0, u8::wrapping_add);
        Bytes::from(request)
    }
}
