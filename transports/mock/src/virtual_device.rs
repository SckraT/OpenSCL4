use bytes::{Bytes, BytesMut};
use openscl4_models::{IDENTITY_COMMAND, ModelDriver};
use openscl4_protocol::{Direction, FrameDecoder};

const IDENTITY_REPLY: &[u8] = b"\x09\xff\0\0\x01\x01\x05\x24\x0004.2904.29A3954DEMO0000000\xb8";
const STATE_REPLY: &[u8] = include_bytes!("../../../core/data/a3954-pilot-state.bin");

/// A virtual Soundcore Liberty 4 Pro that responds to the pilot's reads.
pub struct VirtualSoundcoreDevice;

#[derive(Default)]
pub struct Responder {
    decoder: FrameDecoder,
}

impl Responder {
    pub fn reply(&mut self, chunk: &[u8]) -> Option<Bytes> {
        self.decoder.push(chunk);
        let mut replies = BytesMut::new();
        loop {
            let frame = match self.decoder.next_frame() {
                Ok(Some(frame)) => frame,
                Ok(None) => break,
                Err(_) => continue,
            };
            if frame.direction != Direction::Outbound || !frame.body.is_empty() {
                continue;
            }
            if frame.command == IDENTITY_COMMAND {
                replies.extend_from_slice(IDENTITY_REPLY);
            } else if frame.command == ModelDriver::A3954.state_command() {
                replies.extend_from_slice(STATE_REPLY);
            }
        }
        (!replies.is_empty()).then(|| replies.freeze())
    }
}
