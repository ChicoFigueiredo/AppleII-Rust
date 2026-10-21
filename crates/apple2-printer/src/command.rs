//! ESC/P commands as the parser recognises them; the LX-310 layout interprets them. `offset` on
//! each `Parsed` is the byte offset of the sequence's first byte in the job (R02 diagnostics).
//! Owned by the parser; add variants here, never remove one other code relies on.
//!
//! The variants carry the parameters exactly as the command defines them (the unit is named in
//! each variant); converting to physical units is the layout's job (`lx310`). Sources: ESC/P
//! Reference Manual, "9-pin ESC/P" sections (https://files.support.epson.com/pdf/general/escp2ref.pdf)
//! and the LX-310 command list (https://support2.epson.net/manuals/english/sidm/lx_310/bps0131-01/html/apspe_3.htm).

use serde::{Deserialize, Serialize};

/// Fixed character pitch (ESC P / ESC M / ESC g); condensed is a separate flag.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Pitch {
    Cpi10,
    Cpi12,
    Cpi15,
}

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
    /// ESC @.
    Reset,
    /// Line spacing in 1/216" (ESC 0 = 27, ESC 1 = 21, ESC 2 = 36, ESC 3 n = n, ESC A n = 3n).
    LineSpacing216(u8),
    /// ESC J n: advance n/216".
    AdvanceVertical216(u8),
    /// ESC C n: page length in lines of the current line spacing.
    PageLengthLines(u8),
    /// ESC C 0 n: page length in inches.
    PageLengthInches(u8),
    /// ESC N n: bottom margin in lines.
    BottomMargin(u8),
    /// ESC O.
    CancelBottomMargin,
    /// ESC l n: left margin in columns of the current pitch.
    LeftMargin(u8),
    /// ESC Q n: right margin in columns of the current pitch.
    RightMargin(u8),
    /// ESC $ nL nH: absolute horizontal position, value in 1/60".
    AbsoluteHorizontal(u16),
    /// ESC \ nL nH: relative horizontal move, value in 1/120" (signed, two's complement on the wire).
    RelativeHorizontal(i16),
    /// ESC D ... NUL: tab stops in columns (ascending).
    HorizontalTabs(Vec<u8>),
    /// ESC B ... NUL: vertical tab stops in lines (ascending).
    VerticalTabs(Vec<u8>),
    /// ESC P / ESC M / ESC g.
    Pitch(Pitch),
    /// SI (true) / DC2 (false).
    Condensed(bool),
    /// SO (true) / DC4 (false): double width until the end of the line.
    DoubleWidthLine(bool),
    /// ESC W.
    DoubleWidth(bool),
    /// ESC E (true) / ESC F (false).
    Bold(bool),
    /// ESC G (true) / ESC H (false).
    DoubleStrike(bool),
    /// ESC - n.
    Underline(bool),
    /// ESC 4 (true) / ESC 5 (false).
    Italic(bool),
    /// ESC ! n: the combined-mode byte, bits as in the manual.
    MasterSelect(u8),
    /// ESC t n: selects table slot 0 or 1.
    CharTable(u8),
    /// ESC ( t 3 0 d1 d2 d3: assigns registered table (d2, d3) to slot d1.
    AssignCharTable {
        slot: u8,
        id: u8,
        variant: u8,
    },
    /// ESC R n: international character set 0..=12.
    International(u8),
    /// ESC x: true = NLQ.
    Nlq(bool),
    /// ESC k n: typeface (0 Roman, 1 Sans serif).
    Typeface(u8),
    /// ESC SP n: extra intercharacter space of n/120".
    ExtraSpace(u8),
    /// ESC U: unidirectional printing (no effect on dot positions).
    Unidirectional(bool),
    /// ESC 6 (true) / ESC 7 (false): codes 128..=159 print as characters / are control codes.
    UpperControlCodes(bool),
    /// ESC K/L/Y/Z (density 0..=3), ESC * m (density m), ESC ^ m (`nine_pin`, two bytes per column).
    /// `data` holds exactly `columns` bytes (`2 * columns` for `nine_pin`), fewer only for a stream
    /// that ended inside the payload (a diagnostic accompanies that).
    Graphics {
        density: u8,
        nine_pin: bool,
        columns: u16,
        data: Vec<u8>,
    },
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
