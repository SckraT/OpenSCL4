#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::fs;

    use async_trait::async_trait;
    use bytes::Bytes;
    use openscl4_protocol::{Direction, FrameDecoder, encode_frame};
    use openscl4_session::{SessionHandle, SessionRunner};
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use openscl4_transport_mock::{VirtualIncompatibleA3954, VirtualUnknownIdentity};
    use tokio::sync::mpsc;

    const SERIAL: &[u8; 16] = b"3954PRIVATE00000";
    const CASE: &[u8; 6] = b"CASE42";
    const MAC: &[u8; 6] = b"MAC456";

    #[tokio::test]
    async fn exports_only_sanitized_frames_and_metadata_from_an_enabled_capture() {
        let directory = tempfile::tempdir().expect("local export directory");
        let path = directory.path().join("capture.zip");
        let idle = start();
        idle.confirmed_view().await.expect("state without capture");
        idle.export_diagnostics(&path)
            .await
            .expect("export empty capture");
        let empty = fs::read(&path).expect("empty archive");
        idle.close().await.expect("close first virtual link");

        let handle = start();
        let mut events = handle.events();
        handle.set_frame_capture_enabled(true);
        let view = handle.confirmed_view().await.expect("confirmed state");
        handle.set_frame_capture_enabled(false);
        handle.close().await.expect("close captured virtual link");
        handle
            .export_diagnostics(&path)
            .await
            .expect("export capture");
        let archive = fs::read(&path).expect("read generated archive");
        assert!(archive.len() > empty.len());
        let text = String::from_utf8_lossy(&archive).to_ascii_lowercase();
        assert!(text.contains("frames.jsonl"));
        assert!(text.contains("metadata_only"));
        assert!(text.contains("checksum_mismatch"));
        assert!(text.contains("sanitized"));

        let mut log = format!("{view:?}");
        while let Ok(event) = events.try_recv() {
            use std::fmt::Write;
            writeln!(log, "{event:?}").expect("format structured event");
        }
        for marker in [SERIAL.as_slice(), CASE, MAC] {
            let raw = std::str::from_utf8(marker).expect("ASCII marker");
            let hex = marker
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            assert!(!text.contains(&raw.to_ascii_lowercase()));
            assert!(!text.contains(&hex));
            assert!(!log.contains(raw));
        }
    }

    #[tokio::test]
    async fn unknown_identity_keeps_safe_metadata_only_in_the_archive_and_view() {
        let directory = tempfile::tempdir().expect("local export directory");
        let path = directory.path().join("unknown.zip");
        let handle = SessionRunner::start(
            VirtualUnknownIdentity,
            ConnectTarget {
                mac: RedactedMac::from_octets(*MAC),
                service: None,
            },
        );
        let mut events = handle.events();
        handle.set_frame_capture_enabled(true);
        let view = handle.confirmed_view().await.expect("read-only view");
        handle.set_frame_capture_enabled(false);
        handle.close().await.expect("close the read-only link");
        handle
            .export_diagnostics(&path)
            .await
            .expect("export unknown-identity capture");

        let archive = fs::read(&path).expect("read generated archive");
        let text = String::from_utf8_lossy(&archive);
        let records = capture_records(&archive);

        let identity = inbound_record(&records, [0x01, 0x05]);
        assert!(identity.contains("\"outcome\":\"metadata_only\""));
        assert!(
            !identity.contains("\"frame_hex\""),
            "unknown identity payload was serialized"
        );
        assert!(
            !records
                .iter()
                .any(|record| record.contains("\"direction\":\"inbound\"")
                    && record.contains("\"command\":[1,1]")),
            "an explicit unknown identity must not read state"
        );
        assert_marker_absent(&text, b"9999UNKNOWN00000");
        assert_marker_absent(&text, MAC);

        let mut log = format!("{view:?}");
        while let Ok(event) = events.try_recv() {
            use std::fmt::Write;
            writeln!(log, "{event:?}").expect("format structured event");
        }
        let log = log.to_ascii_lowercase();
        assert!(!log.contains("unknown00000"));
        assert!(!log.contains("9999"));
    }

    #[tokio::test]
    async fn a_rejected_untrusted_state_layout_is_metadata_only_without_its_payload() {
        let directory = tempfile::tempdir().expect("local export directory");
        let path = directory.path().join("untrusted-layout.zip");
        let handle = SessionRunner::start(
            VirtualIncompatibleA3954,
            ConnectTarget {
                mac: RedactedMac::from_octets(*MAC),
                service: None,
            },
        );
        let mut events = handle.events();
        handle.set_frame_capture_enabled(true);
        let view = handle.confirmed_view().await.expect("read-only view");
        handle.set_frame_capture_enabled(false);
        handle.close().await.expect("close the read-only link");
        handle
            .export_diagnostics(&path)
            .await
            .expect("export untrusted-layout capture");

        let archive = fs::read(&path).expect("read generated archive");
        let text = String::from_utf8_lossy(&archive);
        let records = capture_records(&archive);

        let identity = inbound_record(&records, [0x01, 0x05]);
        assert!(identity.contains("\"outcome\":\"sanitized\""));
        let state = inbound_record(&records, [0x01, 0x01]);
        assert!(state.contains("\"outcome\":\"metadata_only\""));
        assert!(
            !state.contains("\"frame_hex\""),
            "a rejected startup state was serialized"
        );
        assert_marker_absent(&text, b"UNTRUSTED-STATE");
        assert_marker_absent(&text, b"A3954DEMO0000000");
        assert_marker_absent(&text, MAC);

        let mut log = format!("{view:?}");
        while let Ok(event) = events.try_recv() {
            use std::fmt::Write;
            writeln!(log, "{event:?}").expect("format structured event");
        }
        assert!(!log.to_ascii_lowercase().contains("untrusted-state"));
    }

    /// Extracts compact JSONL records from the stored ZIP entry.
    fn capture_records(archive: &[u8]) -> Vec<String> {
        let text = String::from_utf8_lossy(archive);
        let mut records = Vec::new();
        let mut rest = text.as_ref();
        while let Some(start) = rest.find("{\"elapsed_millis\":") {
            let after = &rest[start..];
            let Some(end) = after.find('}') else {
                break;
            };
            records.push(after[..=end].to_owned());
            rest = &after[end + 1..];
        }
        records
    }

    fn inbound_record(records: &[String], command: [u8; 2]) -> &str {
        let command = format!("\"command\":[{},{}]", command[0], command[1]);
        records
            .iter()
            .find(|record| {
                record.contains("\"direction\":\"inbound\"") && record.contains(&command)
            })
            .map(String::as_str)
            .expect("captured inbound record")
    }

    fn assert_marker_absent(text: &str, marker: &[u8]) {
        let text = text.to_ascii_lowercase();
        let raw = std::str::from_utf8(marker)
            .expect("ASCII marker")
            .to_ascii_lowercase();
        let hex = marker
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert!(!text.contains(&raw), "raw marker leaked: {raw}");
        assert!(!text.contains(&hex), "hex marker leaked: {hex}");
    }

    fn start() -> SessionHandle {
        SessionRunner::start(
            Device,
            ConnectTarget {
                mac: RedactedMac::from_octets(*MAC),
                service: None,
            },
        )
    }

    struct Device;
    struct Link(Tx, Rx);
    struct Tx(mpsc::UnboundedSender<Bytes>);
    struct Rx(mpsc::UnboundedReceiver<Bytes>);

    #[async_trait]
    impl TransportProvider for Device {
        async fn connect(
            &self,
            _: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            let (tx, rx) = mpsc::unbounded_channel();
            Ok(Box::new(Link(Tx(tx), Rx(rx))))
        }
    }

    impl TransportLink for Link {
        fn split(self: Box<Self>) -> (Box<dyn LinkTx>, Box<dyn LinkRx>) {
            (Box::new(self.0), Box::new(self.1))
        }
    }

    #[async_trait]
    impl LinkTx for Tx {
        async fn send(&mut self, request: Bytes) -> Result<(), TransportError> {
            let answer = match request[5..7] {
                [0x01, 0x05] => {
                    let mut corrupt = encode_frame(Direction::Inbound, [0x7f, 0x74], MAC)
                        .expect("framed invalid data")
                        .to_vec();
                    *corrupt.last_mut().expect("checksum byte") ^= 1;
                    self.0
                        .send(Bytes::from(corrupt))
                        .map_err(|_| TransportError::Closed)?;
                    self.0
                        .send(
                            encode_frame(Direction::Inbound, [0x7f, 0x75], MAC)
                                .expect("unknown response"),
                        )
                        .map_err(|_| TransportError::Closed)?;
                    let body = [b"04.2904.29".as_slice(), SERIAL].concat();
                    encode_frame(Direction::Inbound, [0x01, 0x05], &body)
                        .expect("identity response")
                }
                [0x01, 0x01] => {
                    let fixture = include_bytes!("../../../data/a3954-pilot-state.bin");
                    let mut decoder = FrameDecoder::default();
                    decoder.push(fixture);
                    let mut body = decoder.next_frame().unwrap().unwrap().body.to_vec();
                    body[16..32].copy_from_slice(SERIAL);
                    body[38..44].copy_from_slice(CASE);
                    encode_frame(Direction::Inbound, [0x01, 0x01], &body).expect("state response")
                }
                _ => panic!("unexpected diagnostic test request"),
            };
            self.0.send(answer).map_err(|_| TransportError::Closed)
        }

        async fn close(&mut self) {}
    }

    #[async_trait]
    impl LinkRx for Rx {
        async fn recv(&mut self) -> Result<Bytes, TransportError> {
            self.0.recv().await.ok_or(TransportError::Closed)
        }
    }
}
