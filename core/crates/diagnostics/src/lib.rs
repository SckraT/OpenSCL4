//! Opt-in diagnostics for Soundcore sessions.

#![forbid(unsafe_code)]

mod archive;

pub use archive::{ExportError, export_zip};

use std::{collections::VecDeque, num::NonZeroUsize, time::Duration};

use bytes::Bytes;
use openscl4_models::{
    IDENTITY_COMMAND, ModelDriver, SOUND_MODE_COMMAND, STATE_COMMAND, SoundModeWrite,
};
use openscl4_protocol::{DecodeError, Direction, Frame, encode_frame};

const FRAME_OVERHEAD: usize = 10;

/// What was available to retain from a captured frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureOutcome {
    /// The frame has a reviewed layout and its identifiers have been cleared.
    Sanitized,
    /// The framing is valid but the payload layout has not been verified.
    MetadataOnly,
    /// Framing could not be completed.
    Incomplete,
    /// The decoder rejected the frame.
    Rejected(DecodeError),
}

/// A frame the decoder could not produce.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndecodedFrame {
    /// The available bytes ended before a complete frame.
    Incomplete,
    /// Framing validation failed.
    Rejected(DecodeError),
}

/// Diagnostic fields that can be retained even when the payload is unavailable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CaptureMetadata {
    /// Time relative to session start.
    pub elapsed: Duration,
    /// Direction at the transport.
    pub direction: Direction,
    /// Number of bytes observed on the wire.
    pub length: usize,
    /// Command from a successfully decoded frame.
    pub command: Option<[u8; 2]>,
    /// Result of capture processing.
    pub outcome: CaptureOutcome,
}

/// An exportable diagnostic record with an optional sanitized frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CaptureRecord {
    /// Safe details of the frame and its processing.
    pub metadata: CaptureMetadata,
    frame: Option<Bytes>,
}

impl CaptureRecord {
    /// Returns the sanitized wire bytes, with the checksum recomputed.
    pub fn frame_bytes(&self) -> Option<&[u8]> {
        self.frame.as_deref()
    }
}

/// An opt-in buffer of sanitized frames and diagnostic metadata.
#[derive(Debug)]
pub struct WireTap {
    enabled: bool,
    capacity: NonZeroUsize,
    records: VecDeque<CaptureRecord>,
}

impl WireTap {
    /// Creates a disabled buffer with a maximum number of retained records.
    pub fn new(capacity: NonZeroUsize) -> Self {
        Self {
            enabled: false,
            capacity,
            records: VecDeque::new(),
        }
    }

    /// Controls capture. Previously sanitized records remain available for export.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Records a decoded frame. State payloads require a confirmed model.
    pub fn record_frame(
        &mut self,
        elapsed: Duration,
        confirmed_model: Option<ModelDriver>,
        frame: &Frame,
    ) {
        if !self.enabled {
            return;
        }
        let sanitized = sanitized_frame(confirmed_model, frame);
        let outcome = if sanitized.is_some() {
            CaptureOutcome::Sanitized
        } else {
            CaptureOutcome::MetadataOnly
        };
        self.push(CaptureRecord {
            metadata: CaptureMetadata {
                elapsed,
                direction: frame.direction,
                length: FRAME_OVERHEAD + frame.body.len(),
                command: Some(frame.command),
                outcome,
            },
            frame: sanitized,
        });
    }

    /// Records safe metadata for an incomplete or rejected frame.
    pub fn record_undecoded(
        &mut self,
        elapsed: Duration,
        direction: Direction,
        length: usize,
        frame: UndecodedFrame,
    ) {
        if !self.enabled {
            return;
        }
        let outcome = match frame {
            UndecodedFrame::Incomplete => CaptureOutcome::Incomplete,
            UndecodedFrame::Rejected(error) => CaptureOutcome::Rejected(error),
        };
        self.push(CaptureRecord {
            metadata: CaptureMetadata {
                elapsed,
                direction,
                length,
                command: None,
                outcome,
            },
            frame: None,
        });
    }

    /// Iterates over retained records in capture order.
    pub fn records(&self) -> impl ExactSizeIterator<Item = &CaptureRecord> {
        self.records.iter()
    }

    fn push(&mut self, record: CaptureRecord) {
        if self.records.len() == self.capacity.get() {
            self.records.pop_front();
        }
        self.records.push_back(record);
    }
}

fn sanitized_frame(model: Option<ModelDriver>, frame: &Frame) -> Option<Bytes> {
    let body = &frame.body;
    let masks: &[(usize, usize)] = match (frame.direction, frame.command) {
        (Direction::Inbound, STATE_COMMAND) => state_identifiers(model?, body)?,
        (Direction::Inbound, IDENTITY_COMMAND)
            if ModelDriver::identify(body)
                .is_ok_and(|identified| model.is_none_or(|confirmed| confirmed == identified)) =>
        {
            &[(10, 26)]
        }
        (Direction::Outbound, [0x0b, 0x81..=0x83]) if body.len() == 6 => &[(0, 6)],
        (Direction::Outbound, STATE_COMMAND | IDENTITY_COMMAND | [0x0b, 0x01])
        | (Direction::Inbound, SOUND_MODE_COMMAND)
            if body.is_empty() =>
        {
            &[]
        }
        (Direction::Outbound, SOUND_MODE_COMMAND)
            if model == Some(ModelDriver::A3954) && SoundModeWrite::decode(body).is_ok() =>
        {
            &[]
        }
        _ => return None,
    };
    let mut sanitized = body.to_vec();
    for &(start, end) in masks {
        sanitized[start..end].fill(0);
    }
    encode_frame(frame.direction, frame.command, &sanitized).ok()
}

fn state_identifiers(model: ModelDriver, body: &[u8]) -> Option<&'static [(usize, usize)]> {
    // Use only the measured state layouts cited in core/data/README.md.
    // The dual-state layouts carry their serial at body offsets 16-31. A3944
    // documents a 73-byte prefix and ignores the legacy example's trailing 15
    // bytes; A3945 tolerates the same trailing bytes, while A3948 and A3959
    // document their full body length including padding.
    const DUAL_STATE_SERIAL: (usize, usize) = (16, 32);
    const A3944_DOCUMENTED_PREFIX: usize = 73;
    const A3945_DOCUMENTED_LENGTH: usize = 72;
    const A3948_DOCUMENTED_LENGTH: usize = 88;
    const A3959_DOCUMENTED_LENGTH: usize = 91;
    let fields: &[(usize, usize)] = match model {
        ModelDriver::A3062 if body.len() == 95 => &[(7, 23)],
        ModelDriver::A3944 if body.len() >= A3944_DOCUMENTED_PREFIX => &[DUAL_STATE_SERIAL],
        ModelDriver::A3945 if body.len() >= A3945_DOCUMENTED_LENGTH => &[DUAL_STATE_SERIAL],
        ModelDriver::A3948 if body.len() == A3948_DOCUMENTED_LENGTH => &[DUAL_STATE_SERIAL],
        ModelDriver::A3949 if body.len() == 88 => &[(16, 32)],
        ModelDriver::A3954 if body.len() == 165 && a3954_serial_is_recognized(&body[16..32]) => {
            &[(16, 32), (38, 44)]
        }
        ModelDriver::A3959 if body.len() == A3959_DOCUMENTED_LENGTH => &[DUAL_STATE_SERIAL],
        _ => return None,
    };
    // The decoder admits only states whose serial names this model, so an
    // A3944-sized body with another model's serial stays metadata-only.
    model.decode_state(body).ok()?;
    Some(fields)
}

fn a3954_serial_is_recognized(serial: &[u8]) -> bool {
    // The published capture uses ASCII zeroes in place of the device serial.
    serial == [b'0'; 16]
        || ((serial.starts_with(b"3954") || serial.starts_with(b"A3954"))
            && serial.iter().all(u8::is_ascii_alphanumeric))
}
