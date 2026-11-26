use std::{
    num::NonZeroUsize,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use openscl4_diagnostics::{ExportError, UndecodedFrame, WireTap, export_zip};
use openscl4_models::{STATE_COMMAND, registry};
use openscl4_protocol::{DecodeError, Direction, Frame, FrameDecoder};
use tokio::{sync::watch, time::Instant};

use crate::{ConnectionState, SessionView};

/// A local diagnostic export could not be produced.
#[derive(Debug, thiserror::Error)]
pub enum DiagnosticExportError {
    /// The archive could not be written.
    #[error(transparent)]
    Archive(#[from] ExportError),
    /// The background export task did not finish.
    #[error("diagnostic export task failed")]
    Task(#[from] tokio::task::JoinError),
}

#[derive(Clone)]
pub struct Capture {
    tap: Arc<Mutex<WireTap>>,
    started: Instant,
}

impl Capture {
    pub(super) fn new(started: Instant) -> Self {
        let capacity = NonZeroUsize::new(256).expect("nonzero diagnostic capacity");
        Self {
            tap: Arc::new(Mutex::new(WireTap::new(capacity))),
            started,
        }
    }

    pub(super) fn set_enabled(&self, enabled: bool) {
        self.tap
            .lock()
            .expect("diagnostic capture lock")
            .set_enabled(enabled);
    }

    pub(super) fn record_frame(&self, state: ConnectionState, frame: &Frame) {
        let model = match state {
            ConnectionState::Ready(device)
            | ConnectionState::Reconnecting {
                last_confirmed: device,
                ..
            } => Some(device.model),
            ConnectionState::SyncingState(model) => Some(model),
            ConnectionState::ProbingState(_)
                if frame.direction == Direction::Inbound && frame.command == STATE_COMMAND =>
            {
                registry::identify_state(&frame.body).ok()
            }
            _ => None,
        };
        self.tap
            .lock()
            .expect("diagnostic capture lock")
            .record_frame(self.started.elapsed(), model, frame);
    }

    fn record_undecoded(&self, length: usize, outcome: UndecodedFrame) {
        self.tap
            .lock()
            .expect("diagnostic capture lock")
            .record_undecoded(self.started.elapsed(), Direction::Inbound, length, outcome);
    }

    pub(super) async fn export(&self, path: PathBuf) -> Result<(), DiagnosticExportError> {
        let records = self
            .tap
            .lock()
            .expect("diagnostic capture lock")
            .records()
            .cloned()
            .collect::<Vec<_>>();
        tokio::task::spawn_blocking(move || export_zip(&path, &records)).await??;
        Ok(())
    }
}

pub struct CaptureDecoder {
    capture: Capture,
    view: watch::Receiver<Arc<SessionView>>,
    decoder: FrameDecoder,
}

impl CaptureDecoder {
    pub(super) fn new(capture: Capture, view: watch::Receiver<Arc<SessionView>>) -> Self {
        Self {
            capture,
            view,
            decoder: FrameDecoder::default(),
        }
    }

    pub(super) fn push(&mut self, bytes: &[u8]) {
        self.decoder.push(bytes);
    }

    pub(super) fn has_pending_bytes(&self) -> bool {
        self.decoder.has_pending_bytes()
    }

    pub(super) fn next_frame(&mut self) -> Result<Option<Frame>, DecodeError> {
        let pending = self.decoder.pending_bytes();
        let result = self.decoder.next_frame();
        match &result {
            Ok(Some(frame)) => self
                .capture
                .record_frame(self.view.borrow().connection, frame),
            Err(error) => self.capture.record_undecoded(
                pending - self.decoder.pending_bytes(),
                UndecodedFrame::Rejected(*error),
            ),
            Ok(None) => {}
        }
        result
    }
}

impl Drop for CaptureDecoder {
    fn drop(&mut self) {
        let pending = self.decoder.pending_bytes();
        if pending > 0 {
            self.capture
                .record_undecoded(pending, UndecodedFrame::Incomplete);
        }
    }
}
