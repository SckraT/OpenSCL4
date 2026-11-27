//! Framing codec for Soundcore packets with an optional trailing checksum.

#![forbid(unsafe_code)]

use bytes::{BufMut, Bytes, BytesMut};

const OUTBOUND_PREFIX: [u8; 5] = [0x08, 0xee, 0x00, 0x00, 0x00];
const INBOUND_PREFIX: [u8; 5] = [0x09, 0xff, 0x00, 0x00, 0x01];

/// The common Soundcore frame header: five prefix bytes, the command and the
/// little-endian length.
pub const HEADER_LEN: usize = 9;

/// Whether a model's frames end with the additive checksum used by the
/// Soundcore framing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ChecksumKind {
    /// The length field and the trailing byte include an additive checksum.
    #[default]
    Suffix,
    /// The frame ends after its body; the A3116 speaker uses this framing.
    None,
}

impl ChecksumKind {
    /// Number of trailing checksum bytes the framing appends.
    pub const fn checksum_len(self) -> usize {
        match self {
            Self::Suffix => 1,
            Self::None => 0,
        }
    }
}

/// The direction indicated by the frame prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// From the host to the device.
    Outbound,
    /// From the device to the host.
    Inbound,
}

/// A validated Soundcore frame.
pub struct Frame {
    /// The direction indicated by the header.
    pub direction: Direction,
    /// The command group and opcode, in wire order.
    pub command: [u8; 2],
    /// The payload between the header and checksum.
    pub body: Bytes,
}

/// An invalid frame header or checksum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DecodeError {
    /// The prefix does not identify a Soundcore frame.
    #[error("unrecognized frame prefix")]
    InvalidPrefix,
    /// The length field cannot include a full header and checksum.
    #[error("invalid frame length: {0}")]
    InvalidLength(usize),
    /// The checksum did not match the preceding bytes.
    #[error("frame checksum mismatch")]
    ChecksumMismatch,
}

/// A frame that does not fit the wire length field.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("frame body does not fit the wire length field")]
pub struct EncodeError;

/// Encodes a complete checksummed frame.
pub fn encode_frame(
    direction: Direction,
    command: [u8; 2],
    body: &[u8],
) -> Result<Bytes, EncodeError> {
    encode_frame_with(ChecksumKind::Suffix, direction, command, body)
}

/// Encodes a complete frame with the selected checksum framing.
pub fn encode_frame_with(
    checksum: ChecksumKind,
    direction: Direction,
    command: [u8; 2],
    body: &[u8],
) -> Result<Bytes, EncodeError> {
    let frame_len = HEADER_LEN + body.len() + checksum.checksum_len();
    let length = u16::try_from(frame_len).map_err(|_| EncodeError)?;
    let prefix = match direction {
        Direction::Outbound => OUTBOUND_PREFIX,
        Direction::Inbound => INBOUND_PREFIX,
    };
    let mut bytes = BytesMut::with_capacity(frame_len);
    bytes.extend_from_slice(&prefix);
    bytes.extend_from_slice(&command);
    bytes.put_u16_le(length);
    bytes.extend_from_slice(body);
    if checksum == ChecksumKind::Suffix {
        bytes.put_u8(checksum_of(&bytes));
    }
    Ok(bytes.freeze())
}

fn checksum_of(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

/// Buffers byte chunks from a single connection.
pub struct FrameDecoder {
    buffer: BytesMut,
    checksum: ChecksumKind,
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new(ChecksumKind::Suffix)
    }
}

impl FrameDecoder {
    /// Creates a decoder for one model's framing.
    pub fn new(checksum: ChecksumKind) -> Self {
        Self {
            buffer: BytesMut::new(),
            checksum,
        }
    }

    /// The framing this decoder expects.
    pub const fn checksum_kind(&self) -> ChecksumKind {
        self.checksum
    }

    /// Appends bytes received from the transport.
    pub fn push(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    /// Whether unread bytes remain, including an incomplete frame.
    pub fn has_pending_bytes(&self) -> bool {
        !self.buffer.is_empty()
    }

    /// Number of buffered bytes, including an incomplete frame.
    pub fn pending_bytes(&self) -> usize {
        self.buffer.len()
    }

    /// Takes one frame, or returns `None` while waiting for more bytes.
    ///
    /// An invalid header clears the buffer. A checksum error discards the
    /// candidate frame and leaves any trailing bytes available.
    pub fn next_frame(&mut self) -> Result<Option<Frame>, DecodeError> {
        if self.buffer.len() < HEADER_LEN {
            return Ok(None);
        }

        let direction = if self.buffer.starts_with(&INBOUND_PREFIX) {
            Direction::Inbound
        } else if self.buffer.starts_with(&OUTBOUND_PREFIX) {
            Direction::Outbound
        } else {
            self.buffer.clear();
            return Err(DecodeError::InvalidPrefix);
        };

        let length = usize::from(u16::from_le_bytes([self.buffer[7], self.buffer[8]]));
        if length < HEADER_LEN + self.checksum.checksum_len() {
            self.buffer.clear();
            return Err(DecodeError::InvalidLength(length));
        }
        if self.buffer.len() < length {
            return Ok(None);
        }

        let bytes = self.buffer.split_to(length).freeze();
        let body_end = length - self.checksum.checksum_len();
        if self.checksum == ChecksumKind::Suffix
            && bytes[body_end] != checksum_of(&bytes[..body_end])
        {
            return Err(DecodeError::ChecksumMismatch);
        }

        Ok(Some(Frame {
            direction,
            command: [bytes[5], bytes[6]],
            body: bytes.slice(HEADER_LEN..body_end),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksummed_frame_round_trips() {
        let encoded =
            encode_frame(Direction::Inbound, [0x01, 0x01], &[1, 2, 3]).expect("checksummed frame");
        assert_eq!(encoded.len(), HEADER_LEN + 3 + 1);
        let mut decoder = FrameDecoder::default();
        decoder.push(&encoded);
        let frame = decoder
            .next_frame()
            .expect("valid checksum")
            .expect("complete frame");
        assert_eq!(frame.direction, Direction::Inbound);
        assert_eq!(frame.command, [0x01, 0x01]);
        assert_eq!(frame.body.as_ref(), [1, 2, 3]);
    }

    #[test]
    fn unchecksummed_frame_pipeline_does_not_consume_a_checksum_byte() {
        let encoded = encode_frame_with(ChecksumKind::None, Direction::Inbound, [0x01, 0x01], &[7])
            .expect("unchecksummed frame");
        assert_eq!(encoded.len(), HEADER_LEN + 1);
        assert_eq!(&encoded[7..9], &[0x0a, 0x00]);

        let mut decoder = FrameDecoder::new(ChecksumKind::None);
        decoder.push(&encoded);
        let frame = decoder
            .next_frame()
            .expect("no checksum to verify")
            .expect("complete frame");
        assert_eq!(frame.command, [0x01, 0x01]);
        assert_eq!(frame.body.as_ref(), [7]);
        assert!(!decoder.has_pending_bytes());
    }

    #[test]
    fn unchecksummed_decoder_rejects_a_header_only_length() {
        // The checksummed framing needs at least one trailing checksum byte.
        let mut decoder = FrameDecoder::default();
        decoder.push(&[0x08, 0xee, 0x00, 0x00, 0x00, 0x01, 0x05, 0x09, 0x00]);
        assert!(matches!(
            decoder.next_frame(),
            Err(DecodeError::InvalidLength(HEADER_LEN))
        ));
    }

    #[test]
    fn checksummed_decoder_waits_for_the_checksum_byte() {
        let encoded = encode_frame(Direction::Inbound, [0x01, 0x01], &[]).expect("empty frame");
        let mut decoder = FrameDecoder::default();
        decoder.push(&encoded[..encoded.len() - 1]);
        assert!(decoder.next_frame().expect("incomplete frame").is_none());
        decoder.push(&encoded[encoded.len() - 1..]);
        assert!(decoder.next_frame().expect("complete frame").is_some());
    }
}
