//! ESC/P commands as the parser recognises them; the LX-310 layout interprets them. `offset` on
//! each `Parsed` is the byte offset of the sequence's first byte in the job (R02 diagnostics).
//! Owned by the parser; add variants here, never remove one other code relies on.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    /// A printable byte (decoded to a glyph by the active character table).
    Char(u8),
    CarriageReturn,
    LineFeed,
    FormFeed,
    Backspace,
    HorizontalTab,
    VerticalTab,
    /// Any other recognised sequence, with its raw bytes; the parser module defines the typed
    /// variants it needs below this one.
    Unsupported {
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Parsed {
    pub offset: u64,
    pub command: Command,
}
