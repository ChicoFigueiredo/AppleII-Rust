//! Debug helper: the 40-column text page as strings (for tests, the debugger and phase 3).
//!
//! Limits: only 40-column text page 1 (`$0400-$07FF`, main memory) is decoded. 80-column text
//! (main/aux interleave), page 2 and the mixed-mode split are not handled yet. Character
//! attributes are dropped: inverse and flashing characters decode like normal ones.

use crate::machine::Machine;
use crate::Model;

/// Text rows and columns of the 40-column screen.
pub const ROWS: usize = 24;
pub const COLS: usize = 40;

/// Address of text row `row` (0-23), column 0, on page 1: the screen memory is interleaved
/// in three groups of eight rows (`$80` apart, groups `$28` apart).
pub const fn row_address(row: usize) -> u16 {
    0x400 + ((row % 8) * 0x80 + (row / 8) * 0x28) as u16
}

/// Decodes one screen byte to ASCII. Bits 7-6 select inverse (`00`), flash (`01`) or normal;
/// the low 6 bits pick the glyph of the Apple ][ character set (`@A-Z[\]^_` then ` !"#...?`).
/// `lowercase`: the character set has lowercase at `$E0-$FF` (`` `a-z{|}~ ``): //e family and
/// Pravets. `alt_charset` (//e ALTCHARSET): `$60-$7F` is inverse lowercase too and `$40-$5F` is
/// MouseText, shown as `?` since it has no ASCII form.
pub fn decode_char(code: u8, lowercase: bool, alt_charset: bool) -> char {
    match code {
        0x40..=0x5F if alt_charset => return '?',
        0x60..=0x7F if alt_charset => return char::from(code),
        0xE0..=0xFF if lowercase => return char::from(code & 0x7F),
        _ => {}
    }
    let glyph = code & 0x3F;
    char::from(if glyph < 0x20 { glyph + 0x40 } else { glyph })
}

/// The 24 text rows of page 1, 40 characters each.
pub fn text_screen(m: &Machine) -> Vec<String> {
    let model = m.config().model;
    let lowercase = !model.is_apple2()
        || matches!(
            model,
            Model::Pravets82 | Model::Pravets8M | Model::Pravets8A
        );
    let alt = m.board().video.altcharset && !model.is_apple2();
    (0..ROWS)
        .map(|r| {
            let base = row_address(r);
            (0..COLS as u16)
                .map(|c| decode_char(m.peek_main(base + c), lowercase, alt))
                .collect()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_interleaved() {
        assert_eq!(row_address(0), 0x400);
        assert_eq!(row_address(1), 0x480);
        assert_eq!(row_address(8), 0x428);
        assert_eq!(row_address(16), 0x450);
        assert_eq!(row_address(23), 0x7D0);
    }

    #[test]
    fn decodes_attributes_and_charsets() {
        assert_eq!(decode_char(0xC1, false, false), 'A'); // normal
        assert_eq!(decode_char(0x01, false, false), 'A'); // inverse
        assert_eq!(decode_char(0x41, false, false), 'A'); // flash
        assert_eq!(decode_char(0xDD, false, false), ']');
        assert_eq!(decode_char(0xA0, false, false), ' ');
        assert_eq!(decode_char(0xAA, false, false), '*');
        assert_eq!(decode_char(0xE1, true, false), 'a'); // lowercase set
        assert_eq!(decode_char(0xE1, false, false), '!');
        assert_eq!(decode_char(0x61, true, true), 'a'); // inverse lowercase, ALTCHARSET
        assert_eq!(decode_char(0x61, true, false), '!');
    }
}
