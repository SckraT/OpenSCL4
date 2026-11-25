use bytes::{Bytes, BytesMut};
use openscl4_models::{IDENTITY_COMMAND, ModelDriver, SOUND_MODE_COMMAND, SoundModeWrite};
use openscl4_protocol::{Direction, FrameDecoder, encode_frame};
use openscl4_transport::TransportProvider;

use crate::WithoutSoundModeAck;

const IDENTITY_REPLY: &[u8] = b"\x09\xff\0\0\x01\x01\x05\x24\x0004.2904.29A3954DEMO0000000\xb8";
const STATE_REPLY: &[u8; 175] = include_bytes!("../../../core/data/a3954-pilot-state.bin");
const SOUND_MODE_ACK: &[u8] = b"\x09\xff\0\0\x01\x06\x81\x0a\0\x9a";

/// A virtual Soundcore Liberty 4 Pro with per-connection sound-mode state.
pub struct VirtualSoundcoreDevice;

impl VirtualSoundcoreDevice {
    /// Applies writes and answers state requests while withholding the write ACK.
    pub fn without_sound_mode_ack() -> impl TransportProvider {
        WithoutSoundModeAck
    }
}

pub struct Responder {
    decoder: FrameDecoder,
    state: Vec<u8>,
    acknowledge_sound_mode: bool,
}

impl Responder {
    pub fn new(acknowledge_sound_mode: bool) -> Self {
        let mut decoder = FrameDecoder::default();
        decoder.push(STATE_REPLY);
        let state = decoder
            .next_frame()
            .expect("valid A3954 state fixture")
            .expect("complete A3954 state fixture");
        assert_eq!(state.command, ModelDriver::A3954.state_command());
        assert_eq!(state.direction, Direction::Inbound);
        Self {
            decoder,
            state: state.body.to_vec(),
            acknowledge_sound_mode,
        }
    }

    pub fn reply(&mut self, chunk: &[u8]) -> Option<Bytes> {
        self.decoder.push(chunk);
        let mut replies = BytesMut::new();
        loop {
            let frame = match self.decoder.next_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(_) => continue,
            };
            if frame.direction != Direction::Outbound {
                continue;
            }
            if frame.command == IDENTITY_COMMAND && frame.body.is_empty() {
                replies.extend_from_slice(IDENTITY_REPLY);
            } else if frame.command == ModelDriver::A3954.state_command() && frame.body.is_empty() {
                if let Ok(state) = encode_frame(Direction::Inbound, frame.command, &self.state) {
                    replies.extend_from_slice(&state);
                }
            } else if frame.command == SOUND_MODE_COMMAND {
                let applied = SoundModeWrite::decode(&frame.body)
                    .and_then(|write| write.apply_to_state(&mut self.state));
                if applied.is_ok() && self.acknowledge_sound_mode {
                    replies.extend_from_slice(SOUND_MODE_ACK);
                }
            }
        }
        (!replies.is_empty()).then(|| replies.freeze())
    }
}
