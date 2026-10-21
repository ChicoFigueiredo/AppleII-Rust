//! Debug Adapter Protocol (DAP) server for the headless Apple II machine (HU-1202).
//!
//! The adapter talks DAP over stdio (`Content-Length` framed JSON, written by hand on top of
//! `serde_json`) and owns its own [`Machine`](apple2_core::Machine); it never executes anything
//! outside the emulator. See `docs/manual/14-depuracao-dap-vscode.md`.

#![forbid(unsafe_code)]

pub mod base64;
pub mod protocol;
pub mod session;

pub use session::Session;
