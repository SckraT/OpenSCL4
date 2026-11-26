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

    fn start() -> SessionHandle {
        SessionRunner::start(
            Device,
            ConnectTarget {
                mac: RedactedMac::from_octets(*MAC),
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
