use std::{
    io::{self, Write},
    path::Path,
};

use openscl4_protocol::{DecodeError, Direction};
use serde::Serialize;
use tempfile::NamedTempFile;
use thiserror::Error;
use zip::{CompressionMethod, ZipWriter, result::ZipError, write::SimpleFileOptions};

use crate::{CaptureOutcome, CaptureRecord};

const SCHEMA_VERSION: u32 = 1;

/// A local diagnostic archive could not be written.
#[derive(Debug, Error)]
pub enum ExportError {
    /// Creating, syncing or replacing the file failed.
    #[error("diagnostic archive I/O failed")]
    Io(#[from] io::Error),
    /// The ZIP entries could not be written or finalized.
    #[error("could not write diagnostic ZIP")]
    Zip(#[from] ZipError),
    /// A diagnostic entry could not be serialized.
    #[error("could not serialize diagnostic metadata")]
    Json(#[from] serde_json::Error),
}

/// Writes a local ZIP of records already sanitized by [crate::WireTap].
///
/// The destination's parent directory must exist. An existing archive is replaced
/// only after the new ZIP has been finalized and synced. This function blocks on
/// filesystem I/O, so run it outside the transport loop.
pub fn export_zip(destination: &Path, records: &[CaptureRecord]) -> Result<(), ExportError> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = NamedTempFile::new_in(parent)?;
    let mut archive = ZipWriter::new(temporary.as_file_mut());
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);

    archive.start_file("manifest.json", options)?;
    serde_json::to_writer(
        &mut archive,
        &Manifest {
            schema_version: SCHEMA_VERSION,
            record_count: records.len(),
        },
    )?;

    archive.start_file("frames.jsonl", options)?;
    for record in records {
        serde_json::to_writer(&mut archive, &ArchiveRecord::from(record))?;
        archive.write_all(b"\n")?;
    }

    archive.finish()?.sync_all()?;
    temporary
        .persist(destination)
        .map_err(|error| error.error)?;
    Ok(())
}

#[derive(Serialize)]
struct Manifest {
    schema_version: u32,
    record_count: usize,
}

#[derive(Serialize)]
struct ArchiveRecord {
    elapsed_millis: u128,
    direction: &'static str,
    length: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<[u8; 2]>,
    #[serde(flatten)]
    outcome: ArchiveOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    frame_hex: Option<String>,
}

impl From<&CaptureRecord> for ArchiveRecord {
    fn from(record: &CaptureRecord) -> Self {
        let metadata = record.metadata;
        Self {
            elapsed_millis: metadata.elapsed.as_millis(),
            direction: match metadata.direction {
                Direction::Inbound => "inbound",
                Direction::Outbound => "outbound",
            },
            length: metadata.length,
            command: metadata.command,
            outcome: match metadata.outcome {
                CaptureOutcome::Sanitized => ArchiveOutcome::Sanitized,
                CaptureOutcome::MetadataOnly => ArchiveOutcome::MetadataOnly,
                CaptureOutcome::Incomplete => ArchiveOutcome::Incomplete,
                CaptureOutcome::Rejected(error) => ArchiveOutcome::Rejected {
                    error: match error {
                        DecodeError::InvalidPrefix => "invalid_prefix",
                        DecodeError::InvalidLength(_) => "invalid_length",
                        DecodeError::ChecksumMismatch => "checksum_mismatch",
                    },
                },
            },
            frame_hex: record.frame_bytes().map(hex::encode),
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum ArchiveOutcome {
    Sanitized,
    MetadataOnly,
    Incomplete,
    Rejected { error: &'static str },
}
