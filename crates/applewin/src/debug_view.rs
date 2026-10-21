//! Draws the debugger's 80x48 text grid into a 560x384 [`Frame`] with the //e video ROM glyphs
//! (7x8 pixels per cell, so the grid fills exactly the Apple screen area).

use crate::video::{Frame, SCREEN_H, SCREEN_W};
use apple2_debugger::Grid;

const CELL_W: usize = 7;
const CELL_H: usize = 8;

/// Glyph rows for an ASCII character (bit 0 = leftmost pixel, 1 = lit).
fn glyph(ch: char) -> [u8; 8] {
    let c = if (' '..='~').contains(&ch) {
        ch as u8
    } else {
        b'?'
    };
    // The ROM's first 256 characters are the alternate set: normal text at $80-$FF, and the
    // pixels are stored inverted.
    let idx = usize::from(0x80 | c);
    let rom = apple2_roms::APPLE2E_ENHANCED_VIDEO;
    std::array::from_fn(|r| !rom[idx * 8 + r] & 0x7F)
}

pub fn render_grid(grid: &Grid, blink_on: bool, frame: &mut Frame) {
    frame.resize(SCREEN_W, SCREEN_H);
    let w = SCREEN_W as usize;
    for gy in 0..grid.h.min(48) {
        for gx in 0..grid.w.min(80) {
            let cell = grid.cell(gx, gy);
            let rows = glyph(cell.ch);
            let cursor = blink_on && grid.cursor == Some((gx, gy));
            for (py, bits) in rows.iter().enumerate() {
                for px in 0..CELL_W {
                    let lit = (bits >> px & 1 != 0) != cursor;
                    let (r, g, b) = if lit { cell.fg } else { cell.bg };
                    let o = ((gy * CELL_H + py) * w + gx * CELL_W + px) * 4;
                    frame.rgba[o..o + 4].copy_from_slice(&[r, g, b, 255]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_debugger::GridCell;

    #[test]
    fn draws_foreground_and_background_pixels() {
        let mut g = Grid::new((1, 2, 3), (200, 210, 220));
        g.cells[0] = GridCell {
            ch: 'A',
            fg: (200, 210, 220),
            bg: (1, 2, 3),
        };
        let mut f = Frame::new(0, 0);
        render_grid(&g, false, &mut f);
        assert_eq!((f.width, f.height), (560, 384));
        let px: Vec<[u8; 3]> = (0..7 * 8)
            .map(|i| {
                let o = ((i / 7) * 560 + i % 7) * 4;
                [f.rgba[o], f.rgba[o + 1], f.rgba[o + 2]]
            })
            .collect();
        assert!(px.contains(&[200, 210, 220]) && px.contains(&[1, 2, 3]));
        // a blank cell is all background
        let o = (10 * 7) * 4;
        assert_eq!(&f.rgba[o..o + 3], &[1, 2, 3]);
    }

    #[test]
    fn cursor_inverts_its_cell() {
        let mut g = Grid::new((0, 0, 0), (255, 255, 255));
        g.cursor = Some((0, 0));
        let mut f = Frame::new(0, 0);
        render_grid(&g, true, &mut f);
        assert_eq!(&f.rgba[..3], &[255, 255, 255]);
    }
}
