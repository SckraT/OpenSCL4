#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::{fs, io::Read, num::NonZeroUsize, time::Duration};

    use openscl4_diagnostics::{ExportError, UndecodedFrame, WireTap, export_zip};
    use openscl4_models::ModelDriver;
    use openscl4_protocol::{DecodeError, Direction, Frame, FrameDecoder};
    use serde_json::{Value, json};
    use zip::ZipArchive;

    #[test]
    fn archive_contains_only_sanitized_records_and_fixed_entry_names() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("diagnostics.zip");
        let mut tap = WireTap::new(NonZeroUsize::new(5).unwrap());
        tap.set_enabled(true);

        let mut decoder = FrameDecoder::default();
        decoder.push(include_bytes!("../../../data/a3954-pilot-state.bin"));
        let state = decoder.next_frame().unwrap().unwrap();
        let mut body = state.body.to_vec();
        body[16..32].copy_from_slice(b"3954SERIALABC123");
        body[38..44].copy_from_slice(b"CASE12");
        let state = Frame {
            body: body.into(),
            ..state
        };
        tap.record_frame(Duration::from_millis(2), Some(ModelDriver::A3954), &state);
        let mac = Frame {
            direction: Direction::Outbound,
            command: [0x0b, 0x81],
            body: bytes::Bytes::from_static(b"MAC123"),
        };
        tap.record_frame(Duration::from_millis(3), None, &mac);
        let unknown = Frame {
            direction: Direction::Inbound,
            command: [0x0b, 0x01],
            body: bytes::Bytes::from_static(b"MAC456"),
        };
        tap.record_frame(Duration::from_millis(4), None, &unknown);
        tap.record_undecoded(
            Duration::from_millis(5),
            Direction::Inbound,
            42,
            UndecodedFrame::Incomplete,
        );
        tap.record_undecoded(
            Duration::from_millis(6),
            Direction::Inbound,
            43,
            UndecodedFrame::Rejected(DecodeError::ChecksumMismatch),
        );

        let records: Vec<_> = tap.records().cloned().collect();
        fs::write(&path, b"previous archive contents").unwrap();
        export_zip(&path, &records).unwrap();
        let bytes = fs::read(path).unwrap();
        for marker in [
            b"3954SERIALABC123".as_slice(),
            b"CASE12",
            b"MAC123",
            b"MAC456",
        ] {
            assert!(!bytes.windows(marker.len()).any(|window| window == marker));
            let hex_marker = hex::encode(marker);
            assert!(
                !bytes
                    .windows(hex_marker.len())
                    .any(|window| window.eq_ignore_ascii_case(hex_marker.as_bytes()))
            );
        }

        let mut archive = ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        assert_eq!(archive.len(), 2);
        assert_eq!(
            archive.by_index(0).unwrap().name().unwrap(),
            "manifest.json"
        );
        assert_eq!(archive.by_index(1).unwrap().name().unwrap(), "frames.jsonl");
        let manifest: Value =
            serde_json::from_reader(archive.by_name("manifest.json").unwrap()).unwrap();
        assert_eq!(manifest, json!({"schema_version": 1, "record_count": 5}));
        let mut lines = String::new();
        archive
            .by_name("frames.jsonl")
            .unwrap()
            .read_to_string(&mut lines)
            .unwrap();
        let records: Vec<Value> = lines
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(records.len(), 5);

        let saved_state = hex::decode(records[0]["frame_hex"].as_str().unwrap()).unwrap();
        decoder.push(&saved_state);
        let state = decoder.next_frame().unwrap().unwrap();
        assert_eq!(&state.body[16..32], &[0; 16]);
        assert_eq!(&state.body[38..44], &[0; 6]);
        let saved_mac = hex::decode(records[1]["frame_hex"].as_str().unwrap()).unwrap();
        decoder.push(&saved_mac);
        assert_eq!(
            decoder.next_frame().unwrap().unwrap().body.as_ref(),
            &[0; 6]
        );
        assert_eq!(
            records[2],
            json!({"elapsed_millis": 4, "direction": "inbound", "length": 16, "command": [11, 1], "outcome": "metadata_only"})
        );
        assert_eq!(
            records[3],
            json!({"elapsed_millis": 5, "direction": "inbound", "length": 42, "outcome": "incomplete"})
        );
        assert_eq!(
            records[4],
            json!({"elapsed_millis": 6, "direction": "inbound", "length": 43, "outcome": "rejected", "error": "checksum_mismatch"})
        );
    }

    #[test]
    fn failed_publish_preserves_the_destination_and_removes_the_temporary_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("existing");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), b"preserved").unwrap();

        assert!(matches!(export_zip(&path, &[]), Err(ExportError::Io(_))));
        assert_eq!(fs::read(path.join("keep")).unwrap(), b"preserved");
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
