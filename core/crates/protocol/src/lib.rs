//! Framing codec for Soundcore packets with a trailing checksum.

#![forbid(unsafe_code)]

use bytes::{BufMut, Bytes, BytesMut};

const OUTBOUND_PREFIX: [u8; 5] = [0x08, 0xee, 0x00, 0x00, 0x00];
const INBOUND_PREFIX: [u8; 5] = [0x09, 0xff, 0x00, 0x00, 0x01];
const HEADER_LEN: usize = 9;
const MIN_FRAME_LEN: usize = HEADER_LEN + 1;

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
    let length = u16::try_from(MIN_FRAME_LEN + body.len()).map_err(|_| EncodeError)?;
    let prefix = match direction {
        Direction::Outbound => OUTBOUND_PREFIX,
        Direction::Inbound => INBOUND_PREFIX,
    };
    let mut bytes = BytesMut::with_capacity(usize::from(length));
    bytes.extend_from_slice(&prefix);
    bytes.extend_from_slice(&command);
    bytes.put_u16_le(length);
    bytes.extend_from_slice(body);
    bytes.put_u8(checksum(&bytes));
    Ok(bytes.freeze())
}

fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, byte| sum.wrapping_add(*byte))
}

/// Buffers byte chunks from a single connection.
#[derive(Default)]
pub struct FrameDecoder {
    buffer: BytesMut,
}

impl FrameDecoder {
    /// Appends bytes received from the transport.
    pub fn push(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
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
        if length < MIN_FRAME_LEN {
            self.buffer.clear();
            return Err(DecodeError::InvalidLength(length));
        }
        if self.buffer.len() < length {
            return Ok(None);
        }

        let bytes = self.buffer.split_to(length).freeze();
        let checksum_index = length - 1;
        if bytes[checksum_index] != checksum(&bytes[..checksum_index]) {
            return Err(DecodeError::ChecksumMismatch);
        }

        Ok(Some(Frame {
            direction,
            command: [bytes[5], bytes[6]],
            body: bytes.slice(HEADER_LEN..checksum_index),
        }))
    }
}
