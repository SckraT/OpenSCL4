#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bytes::Bytes;
    use openscl4_protocol::{Direction, FrameDecoder};
    use openscl4_transport::{ConnectTarget, RedactedMac, TransportProvider};
    use openscl4_transport_mock::MockTransport;
    use tokio::time::timeout;

    #[tokio::test]
    async fn decodes_one_frame_received_over_mock_transport() {
        let transport: &dyn TransportProvider = &MockTransport;
        let target = ConnectTarget {
            mac: RedactedMac::from_octets([0; 6]),
        };
        let link = transport.connect(&target).await.expect("open mock link");
        let (mut tx, mut rx) = link.split();
        let synthetic = Bytes::from_static(&[
            0x09, 0xff, 0x00, 0x00, 0x01, 0x01, 0x03, 0x0b, 0x00, 0x2a, 0x42,
        ]);

        tx.send(synthetic).await.expect("send synthetic frame");
        let received = timeout(Duration::from_secs(5), rx.recv())
            .await
            .expect("receive before deadline")
            .expect("read mock link");

        let mut decoder = FrameDecoder::default();
        decoder.push(&received);
        let frame = decoder
            .next_frame()
            .expect("decode valid bytes")
            .expect("receive a complete frame");
        assert_eq!(frame.direction, Direction::Inbound);
        assert_eq!(frame.command, [0x01, 0x03]);
        assert_eq!(frame.body.as_ref(), &[0x2a]);
        assert!(
            decoder
                .next_frame()
                .expect("check remaining bytes")
                .is_none()
        );
        tx.close().await;
    }
}
