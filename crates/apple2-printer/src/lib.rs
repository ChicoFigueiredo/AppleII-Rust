//! Virtual printers (Impressora virtual, ADR 0008): turns the bytes a program prints through the
//! printer card into Pages, exports them to PDF and keeps them in the Arquivo de impressões.
//!
//! Pure domain crate: no winit/egui/wgpu. The pipeline is
//! `CaptureEvent` (card) -> `escp::Parser` -> `Command` -> `lx310::Printer` (layout) -> `PageDocument`
//! -> preview (frontend) / `pdf` / `archive`.
//!
//! Contract owners (one owner per file; others read only):
//! - `units`, `document`, `paper`, `capture`, `command`: shared contract (supervisor).
//! - `escp`, `lx310`: ESC/P parser and LX-310 layout.
//! - `font`: dot-matrix glyphs and character tables.
//! - `pdf`: PDF export.
//! - `archive`, `spool`: Arquivo de impressões and job service.

pub mod capture;
pub mod command;
pub mod document;
pub mod engine;
pub mod font;
pub mod paper;
#[cfg(feature = "pdf")]
pub mod pdf;
pub mod units;
