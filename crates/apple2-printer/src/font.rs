//! Dot-matrix glyphs (fonte própria, aproximação: ADR 0008) and character tables. Owned by the
//! font module; the API below is the contract the LX-310 layout uses.

use serde::{Deserialize, Serialize};

/// Print quality. Draft: 9 rows 1/72" apart, columns 1/120" apart. NLQ: 18 rows 1/144" apart
/// (two passes), columns 1/240" apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    #[default]
    Draft,
    Nlq,
}

/// Character tables the LX-310 offers that this project supports (ESC t / ESC R / ESC ( t).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharTable {
    /// Italic table / ASCII with an international variant from ESC R (`country` 0..=12).
    #[default]
    Pc437,
    Pc850,
    Pc860,
    Brascii,
    Abicomp,
    /// ESC t 0: italics in the upper half.
    Italic,
}

/// One character's dots. `columns[i]` is a bitmask of rows (bit 0 = top row) at column `i`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub quality: Quality,
    pub columns: Vec<u32>,
    /// Row (from the top) of the baseline, so descenders can go below it.
    pub baseline_row: u8,
}

/// The glyph for `code` in `table` with ESC R `country` (0 = USA), or `None` if the table has no
/// glyph there (the layout reports a diagnostic and prints nothing, never `?`).
pub fn glyph(_table: CharTable, _country: u8, _code: u8, _quality: Quality) -> Option<Glyph> {
    None
}

/// Unicode for the text layer (search/copy in the PDF); `None` for control or undefined codes.
pub fn to_unicode(_table: CharTable, _country: u8, _code: u8) -> Option<char> {
    None
}
