//! A printer profile's engine: bytes in (as many calls as the card produces, any split), Pages out.
//! The LX-310 engine (`lx310`) implements it; the spool drives any engine through this trait.

use crate::document::PageDocument;
use crate::paper::RenderOptions;

pub trait PrinterEngine: Send {
    /// Starts a fresh job with frozen options (paper size and margins come from `options.paper`).
    fn begin(&mut self, options: RenderOptions);
    /// Bytes of the current job in order; `offset` = bytes already fed (for diagnostics). Must give
    /// the same document for any split of the same stream (T-010) and never panic (T-027).
    fn feed(&mut self, offset: u64, data: &[u8]);
    /// The document so far; the last page may be in progress.
    fn document(&self) -> &PageDocument;
    /// Number of dots/pages changed since the last call, so the preview can redraw only when needed.
    fn take_revision(&mut self) -> u64;
    /// Ends the job: flushes pending state, marks the last page finished.
    fn finish(&mut self);
}
