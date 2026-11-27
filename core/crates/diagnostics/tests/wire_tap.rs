#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{num::NonZeroUsize, time::Duration};

    use openscl4_diagnostics::{CaptureOutcome, UndecodedFrame, WireTap};
    use openscl4_models::ModelDriver;
    use openscl4_protocol::{DecodeError, Direction, Frame, FrameDecoder, encode_frame};

    fn decode(bytes: &[u8]) -> Frame {
        let mut decoder = FrameDecoder::default();
        decoder.push(bytes);
        decoder.next_frame().unwrap().unwrap()
    }

    fn make_frame(direction: Direction, command: [u8; 2], body: &[u8]) -> Frame {
        decode(&encode_frame(direction, command, body).unwrap())
    }

    #[test]
    fn captures_only_while_enabled() {
        let mut tap = WireTap::new(NonZeroUsize::new(2).unwrap());
        let state = decode(include_bytes!("../../../data/a3954-pilot-state.bin"));
        let model = Some(ModelDriver::A3954);

        tap.record_frame(Duration::ZERO, model, &state);
        assert_eq!(tap.records().len(), 0);

        tap.set_enabled(true);
        tap.record_frame(Duration::from_millis(1), model, &state);
        tap.set_enabled(false);
        tap.record_frame(Duration::from_millis(2), model, &state);

        assert_eq!(tap.records().len(), 1);
        assert_eq!(
            tap.records().next().unwrap().metadata.elapsed,
            Duration::from_millis(1)
        );
    }

    #[test]
    fn clears_device_and_case_serials_before_preserving_a_valid_frame() {
        let mut tap = WireTap::new(NonZeroUsize::new(1).unwrap());
        let fixture = decode(include_bytes!("../../../data/a3954-pilot-state.bin"));
        let mut body = fixture.body.to_vec();
        body[16..32].copy_from_slice(b"3954SERIALABC123");
        body[38..44].copy_from_slice(b"CASE12");
        let frame = make_frame(Direction::Inbound, [0x01, 0x01], &body);

        tap.set_enabled(true);
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3954), &frame);

        let record = tap.records().next().unwrap();
        assert_eq!(record.metadata.outcome, CaptureOutcome::Sanitized);
        assert_eq!(record.metadata.length, 175);
        let sanitized = decode(record.frame_bytes().unwrap());
        body[16..32].fill(0);
        body[38..44].fill(0);
        assert_eq!(sanitized.body.as_ref(), body);
        let debug = format!("{tap:?}");
        assert!(!debug.contains("3954SERIALABC123"));
        assert!(!debug.contains("CASE12"));
    }

    #[test]
    fn clears_state_serials_for_the_other_reviewed_models() {
        let mut tap = WireTap::new(NonZeroUsize::new(2).unwrap());
        tap.set_enabled(true);
        for (model, fixture, start) in [
            (
                ModelDriver::A3062,
                include_bytes!("../../../data/a3062-pilot-state.bin").as_slice(),
                7,
            ),
            (
                ModelDriver::A3949,
                include_bytes!("../../../data/a3949-pilot-state.bin").as_slice(),
                16,
            ),
        ] {
            let frame = decode(fixture);
            tap.record_frame(Duration::ZERO, Some(model), &frame);
            let record = tap.records().last().unwrap();
            let sanitized = decode(record.frame_bytes().unwrap());
            let mut expected = frame.body.to_vec();
            expected[start..start + 16].fill(0);
            assert_eq!(record.metadata.outcome, CaptureOutcome::Sanitized);
            assert_eq!(sanitized.body.as_ref(), expected);
        }
    }

    #[test]
    fn clears_state_serials_for_the_dual_state_family() {
        let mut tap = WireTap::new(NonZeroUsize::new(4).unwrap());
        tap.set_enabled(true);
        for (model, fixture, serial) in [
            (
                ModelDriver::A3944,
                include_bytes!("../../../data/a3944-pilot-state.bin").as_slice(),
                "3944TEST00000000",
            ),
            (
                ModelDriver::A3945,
                include_bytes!("../../../data/a3945-pilot-state.bin").as_slice(),
                "3945TEST00000000",
            ),
            (
                ModelDriver::A3948,
                include_bytes!("../../../data/a3948-pilot-state.bin").as_slice(),
                "3948TEST00000000",
            ),
            (
                ModelDriver::A3959,
                include_bytes!("../../../data/a3959-pilot-state.bin").as_slice(),
                "3959TEST00000000",
            ),
        ] {
            let frame = decode(fixture);
            tap.record_frame(Duration::ZERO, Some(model), &frame);
            let record = tap.records().last().unwrap();
            assert_eq!(record.metadata.outcome, CaptureOutcome::Sanitized);
            assert_eq!(record.metadata.length, fixture.len());
            let sanitized = decode(record.frame_bytes().unwrap());
            let mut expected = frame.body.to_vec();
            expected[16..32].fill(0);
            assert_eq!(sanitized.body.as_ref(), expected);
            let debug = format!("{tap:?}");
            assert!(!debug.contains(serial), "{} serial leaked", model.code());
        }
    }

    #[test]
    fn accepts_the_a3944_documented_prefix_and_rejects_foreign_states() {
        let mut tap = WireTap::new(NonZeroUsize::new(4).unwrap());
        tap.set_enabled(true);
        let fixture = decode(include_bytes!("../../../data/a3944-pilot-state.bin"));
        let prefix = make_frame(Direction::Inbound, [0x01, 0x01], &fixture.body[..73]);
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3944), &prefix);
        assert_eq!(
            tap.records().last().unwrap().metadata.outcome,
            CaptureOutcome::Sanitized
        );

        // One byte short of the documented prefix stays metadata-only.
        let short = make_frame(Direction::Inbound, [0x01, 0x01], &fixture.body[..72]);
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3944), &short);
        assert_eq!(
            tap.records().last().unwrap().metadata.outcome,
            CaptureOutcome::MetadataOnly
        );

        // A body whose serial names another model stays metadata-only.
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3945), &fixture);
        assert_eq!(
            tap.records().last().unwrap().metadata.outcome,
            CaptureOutcome::MetadataOnly
        );

        // A confirmed state recorded under an unrelated model stays metadata-only.
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3954), &fixture);
        assert_eq!(
            tap.records().last().unwrap().metadata.outcome,
            CaptureOutcome::MetadataOnly
        );
    }

    #[test]
    fn clears_identity_serial_and_outbound_mac_addresses() {
        let mut tap = WireTap::new(NonZeroUsize::new(4).unwrap());
        tap.set_enabled(true);

        let identity = make_frame(
            Direction::Inbound,
            [0x01, 0x05],
            b"04.2904.293954SERIALABC12302.58",
        );
        tap.record_frame(Duration::ZERO, None, &identity);
        for command in [0x81, 0x82, 0x83] {
            let mac = make_frame(Direction::Outbound, [0x0b, command], b"MAC123");
            tap.record_frame(Duration::ZERO, None, &mac);
        }

        let mut records = tap.records();
        let identity = decode(records.next().unwrap().frame_bytes().unwrap());
        assert_eq!(&identity.body[..10], b"04.2904.29");
        assert_eq!(&identity.body[10..26], [0; 16]);
        assert_eq!(&identity.body[26..], b"02.58");
        for record in records {
            assert_eq!(decode(record.frame_bytes().unwrap()).body.as_ref(), [0; 6]);
        }
        let debug = format!("{tap:?}");
        assert!(!debug.contains("3954SERIALABC123"));
        assert!(!debug.contains("MAC123"));
    }

    #[test]
    fn retains_metadata_for_unconfirmed_unknown_and_incomplete_layouts() {
        let mut tap = WireTap::new(NonZeroUsize::new(8).unwrap());
        tap.set_enabled(true);
        let state = decode(include_bytes!("../../../data/a3954-pilot-state.bin"));
        tap.record_frame(Duration::ZERO, None, &state);
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3949), &state);

        let mut body = state.body.to_vec();
        body[16..32].copy_from_slice(b"UNKNOWNMODEL1234");
        let unverified = make_frame(Direction::Inbound, [0x01, 0x01], &body);
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3954), &unverified);

        let truncated = make_frame(Direction::Inbound, [0x01, 0x01], &state.body[..37]);
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3954), &truncated);
        let unexpected = make_frame(Direction::Outbound, [0x0b, 0x81], b"MAC123EXTRA");
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3954), &unexpected);
        let unknown = make_frame(Direction::Inbound, [0x0b, 0x01], b"MAC123ALSO");
        tap.record_frame(Duration::ZERO, Some(ModelDriver::A3954), &unknown);

        tap.record_undecoded(
            Duration::ZERO,
            Direction::Inbound,
            42,
            UndecodedFrame::Incomplete,
        );
        tap.record_undecoded(
            Duration::ZERO,
            Direction::Inbound,
            43,
            UndecodedFrame::Rejected(DecodeError::ChecksumMismatch),
        );

        let mut records = tap.records();
        for record in records.by_ref().take(6) {
            assert_eq!(record.metadata.outcome, CaptureOutcome::MetadataOnly);
            assert!(record.metadata.command.is_some());
            assert!(record.frame_bytes().is_none());
        }
        let incomplete = records.next().unwrap();
        assert_eq!(incomplete.metadata.outcome, CaptureOutcome::Incomplete);
        assert_eq!(incomplete.metadata.length, 42);
        assert!(incomplete.metadata.command.is_none());
        assert!(incomplete.frame_bytes().is_none());
        let rejected = records.next().unwrap();
        assert_eq!(
            rejected.metadata.outcome,
            CaptureOutcome::Rejected(DecodeError::ChecksumMismatch)
        );
        assert_eq!(rejected.metadata.length, 43);
        assert!(rejected.metadata.command.is_none());
        assert!(rejected.frame_bytes().is_none());
    }

    #[test]
    fn evicts_the_oldest_record_when_full() {
        let mut tap = WireTap::new(NonZeroUsize::new(2).unwrap());
        tap.set_enabled(true);
        let request = make_frame(Direction::Outbound, [0x01, 0x01], &[]);
        for millis in 1..=3 {
            tap.record_frame(Duration::from_millis(millis), None, &request);
        }
        let elapsed: Vec<_> = tap
            .records()
            .map(|record| record.metadata.elapsed)
            .collect();
        assert_eq!(
            elapsed,
            [Duration::from_millis(2), Duration::from_millis(3)]
        );
    }
}
