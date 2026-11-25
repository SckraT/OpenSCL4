#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use bytes::{Bytes, BytesMut};
    use openscl4_protocol::{Direction, FrameDecoder, encode_frame};
    use openscl4_session::{
        CommandError, ConnectionState, ModelDriver, SessionCommand, SessionRunner, SoundModeSetting,
    };
    use openscl4_transport::{
        ConnectTarget, LinkRx, LinkTx, RedactedMac, TransportError, TransportLink,
        TransportProvider,
    };
    use tokio::sync::mpsc;

    const IDENTITY: &[u8] = b"\x09\xff\0\0\x01\x01\x05\x24\x0004.2904.29A3954DEMO0000000\xb8";
    const STATE: &[u8] = include_bytes!("../../../data/a3954-pilot-state.bin");
    const ACK: &[u8] = b"\x09\xff\0\0\x01\x06\x81\x0a\0\x9a";

    #[tokio::test(start_paused = true)]
    async fn a_state_buffered_before_read_back_cannot_confirm_the_write() {
        for split_at in [STATE.len(), STATE.len() / 2] {
            let handle = SessionRunner::start(
                StaleSnapshotDevice { split_at },
                ConnectTarget {
                    mac: RedactedMac::from_octets([0; 6]),
                },
            );
            let original = handle.confirmed_view().await.expect("baseline state");
            let ConnectionState::Ready(original) = original.connection else {
                panic!("expected a confirmed state");
            };

            let requested = SoundModeSetting::NORMAL;
            let result = handle
                .dispatch(SessionCommand::SetSoundMode(requested))
                .expect("accept the write")
                .wait()
                .await;
            assert_eq!(
                result,
                Err(CommandError::ModeMismatch {
                    requested,
                    observed: original.sound_mode_setting().expect("A3954 scale position"),
                }),
                "stale frame split at {split_at}"
            );
            handle.close().await.expect("release the link");
        }
    }

    struct StaleSnapshotDevice {
        split_at: usize,
    }

    struct Link(Tx, Rx);
    struct Rx(mpsc::UnboundedReceiver<Bytes>);

    struct Tx {
        replies: mpsc::UnboundedSender<Bytes>,
        stale: Bytes,
        split_at: usize,
        send_remainder: bool,
    }

    #[async_trait]
    impl TransportProvider for StaleSnapshotDevice {
        async fn connect(
            &self,
            _target: &ConnectTarget,
        ) -> Result<Box<dyn TransportLink>, TransportError> {
            let model = ModelDriver::A3954;
            let mut decoder = FrameDecoder::default();
            decoder.push(STATE);
            let mut body = decoder
                .next_frame()
                .expect("valid state fixture")
                .expect("complete state fixture")
                .body
                .to_vec();
            let original = model.decode_state(&body).expect("fixture state");
            original
                .sound_mode_write(SoundModeSetting::NORMAL)
                .expect("construct stale setting")
                .apply_to_state(&mut body)
                .expect("update stale state");
            let stale = encode_frame(Direction::Inbound, model.state_command(), &body)
                .expect("encode stale snapshot");
            let (tx, rx) = mpsc::unbounded_channel();
            Ok(Box::new(Link(
                Tx {
                    replies: tx,
                    stale,
                    split_at: self.split_at,
                    send_remainder: false,
                },
                Rx(rx),
            )))
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
            let mut reply = BytesMut::new();
            match &request[5..7] {
                [0x01, 0x05] => reply.extend_from_slice(IDENTITY),
                [0x06, 0x81] => {
                    reply.extend_from_slice(ACK);
                    reply.extend_from_slice(&self.stale[..self.split_at]);
                    self.send_remainder = true;
                }
                [0x01, 0x01] => {
                    if self.send_remainder {
                        reply.extend_from_slice(&self.stale[self.split_at..]);
                        self.send_remainder = false;
                    }
                    reply.extend_from_slice(STATE);
                }
                _ => panic!("unexpected request"),
            }
            self.replies
                .send(reply.freeze())
                .map_err(|_| TransportError::Closed)
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
