//! The printed result: Pages of ink dots in physical units (`crate::units`), plus an invisible text
//! layer for search/copy in the PDF. The preview and the PDF draw the same PageDocument.

use serde::{Deserialize, Serialize};

/// One pin impact. `x`, `y` are the dot centre in units (1/2160"), from the paper's top-left corner.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dot {
    pub x: u32,
    pub y: u32,
    /// Ribbon colour index (0 = black). Kept for colour profiles; the LX-310 uses 0 only.
    pub ink: u8,
}

/// Text that was printed at a position, for the PDF's invisible text layer (not used for drawing).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRun {
    /// Baseline-left of the first character, in units.
    pub x: u32,
    pub y: u32,
    /// Character advance in units (pitch), so a reader can place each character.
    pub advance: u32,
    /// Height of the character cell in units.
    pub height: u32,
    /// Unicode text as decoded through the active character table.
    pub text: String,
}

/// One sheet (or one form of continuous paper).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Page {
    /// Paper size in units.
    pub width: u32,
    pub height: u32,
    /// In print order. Repeated dots (emphasis, double strike) are kept: the ribbon darkens.
    pub dots: Vec<Dot>,
    pub text: Vec<TextRun>,
}

/// Something the engine could not print as the program meant (R02): never silent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Byte offset in the job's raw stream where the sequence starts.
    pub offset: u64,
    /// The bytes involved (truncated to a reasonable length by the producer).
    pub bytes: Vec<u8>,
    pub message: String,
}

/// The whole job as printed so far. `pages.last()` may be in progress.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageDocument {
    pub pages: Vec<Page>,
    /// True while the last page can still receive dots.
    pub last_in_progress: bool,
    pub diagnostics: Vec<Diagnostic>,
}
