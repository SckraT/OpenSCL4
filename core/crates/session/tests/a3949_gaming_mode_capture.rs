//! Replays the independently measured A3949 gaming-mode state cycle.

#![forbid(unsafe_code)]

#[cfg(test)]
#[path = "support/capture_replay.rs"]
mod capture_replay;

#[cfg(test)]
mod tests {
    use std::{collections::VecDeque, time::Duration};

    use bytes::Bytes;
    use openscl4_models::{IDENTITY_COMMAND, STATE_COMMAND};
    use openscl4_protocol::{Direction, encode_frame};
    use openscl4_session::{CommandOutcome, ConnectionState, SessionCommand, SessionRunner};
    use openscl4_transport::{ConnectTarget, RedactedMac};
    use tokio::time::Instant;

    use super::capture_replay::Replay;

    const OFF_STATE: &[u8] = include_bytes!("../../../data/a3949-gaming-off.bin");
    const ON_STATE: &[u8] = include_bytes!("../../../data/a3949-gaming-on.bin");
    const ENABLE: &[u8] = b"\x08\xee\0\0\0\x01\x87\x0b\0\x01\x8a";
    const DISABLE: &[u8] = b"\x08\xee\0\0\0\x01\x87\x0b\0\0\x89";
    const ACK: &[u8] = b"\x09\xff\0\0\x01\x01\x87\x0a\0\x9b";

    #[tokio::test(start_paused = true)]
    async fn confirms_and_restores_gaming_mode_from_measured_replies() {
        let state_request = encode_frame(Direction::Outbound, STATE_COMMAND, &[]).unwrap();
        let identity_request = encode_frame(Direction::Outbound, IDENTITY_COMMAND, &[]).unwrap();
        let mut exchanges = VecDeque::from([
            (identity_request, None),
            (state_request.clone(), Some(Bytes::from_static(OFF_STATE))),
        ]);
        // At the session's 100 ms poll interval, the changed value first appears
        // on the poll after the corresponding hardware delay (1022 or 1247 ms).
        let cycle = [
            (true, ENABLE, OFF_STATE, ON_STATE, 11, 1022),
            (false, DISABLE, ON_STATE, OFF_STATE, 13, 1247),
        ];
        for (_, request, before, after, stale_reads, _) in cycle {
            exchanges.push_back((state_request.clone(), Some(Bytes::from_static(before))));
            exchanges.push_back((Bytes::from_static(request), Some(Bytes::from_static(ACK))));
            exchanges.extend(std::iter::repeat_n(
                (state_request.clone(), Some(Bytes::from_static(before))),
                stale_reads,
            ));
            exchanges.push_back((state_request.clone(), Some(Bytes::from_static(after))));
        }

        let handle = SessionRunner::start(
            Replay(exchanges),
            ConnectTarget {
                mac: RedactedMac::from_octets([0; 6]),
            },
        );
        let initial = handle.confirmed_view().await.expect("captured baseline");
        let ConnectionState::Ready(state) = initial.connection else {
            panic!("expected ready A3949");
        };
        assert_eq!(state.gaming_mode, Some(false));

        for (requested, _, _, _, _, delay_ms) in cycle {
            let start = Instant::now();
            let CommandOutcome::Applied(state) = handle
                .dispatch(SessionCommand::SetGamingMode(requested))
                .expect("accept the gaming-mode command")
                .wait()
                .await
                .expect("confirm the gaming mode")
            else {
                panic!("expected a confirmed state")
            };
            assert!(start.elapsed() >= Duration::from_millis(delay_ms));
            assert_eq!(state.gaming_mode, Some(requested));
        }
        assert_eq!(handle.subscribe().borrow().connection, initial.connection);
        handle.close().await.expect("release the link");
    }
}
