//! Video output seam: [`FrameSource`] turns the machine state into an RGBA framebuffer.
//!
//! `apple2-video` plugs in by implementing [`FrameSource`]; until then [`TextPlaceholder`]
//! draws the text screen (40 and 80 columns) with the embedded video ROM, so BASIC is visible.

use apple2_core::text_screen::{row_address, COLS, ROWS};
use apple2_core::{Machine, Model};

/// Native framebuffer size of the placeholder (AppleWin's 560x384 for the Apple screen).
pub const SCREEN_W: u32 = 560;
pub const SCREEN_H: u32 = 384;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    /// `width * height * 4` bytes, RGBA8, row-major.
    pub rgba: Vec<u8>,
}

impl Frame {
    pub fn new(width: u32, height: u32) -> Self {
        Frame { width, height, rgba: vec![0; (width * height * 4) as usize] }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if (self.width, self.height) != (width, height) {
            *self = Frame::new(width, height);
        }
    }
}

/// Renders the emulated screen. Runs on the emulation thread (it reads the [`Machine`]).
pub trait FrameSource {
    /// Draws the current screen into `frame` (resizing it if the source changes resolution).
    fn render(&mut self, machine: &Machine, frame: &mut Frame);
    /// F9 / Shift+F9: next/previous video type (colour, mono, ...).
    fn cycle_video_mode(&mut self, _delta: i32) {}
    /// Ctrl+Shift+F9.
    fn toggle_scanlines(&mut self) {}
    /// F10 on the //e: video ROM rocker switch.
    fn toggle_rocker_switch(&mut self) {}
    fn name(&self) -> &'static str {
        "frame source"
    }
}

/// Text-only stand-in for `apple2-video`.
pub struct TextPlaceholder {
    charset: &'static [u8],
    is_e: bool,
    /// Green phosphor, like a monochrome monitor.
    fg: [u8; 3],
}

impl TextPlaceholder {
    pub fn new(model: Model) -> Self {
        let is_e = !model.is_apple2();
        let charset = match model {
            Model::Apple2 | Model::Apple2Plus => apple2_roms::APPLE2_VIDEO,
            Model::Apple2JPlus => apple2_roms::APPLE2_JPLUS_VIDEO,
            Model::Base64A => apple2_roms::BASE64A_GERMAN_VIDEO,
            _ => apple2_roms::APPLE2E_ENHANCED_VIDEO,
        };
        TextPlaceholder { charset, is_e, fg: [0xE8, 0xE8, 0xE8] }
    }

    /// 7 pixel columns (bit 0 = leftmost); true = lit. `flash_on`: phase of FLASH characters.
    fn glyph(&self, code: u8, alt: bool, flash_on: bool) -> [u8; 8] {
        let mut rows = [0u8; 8];
        if self.is_e {
            // The ROM stores inverted pixels (1 = dark). First 256 characters = alternate set
            // (inverse, MouseText, inverse lowercase, normal).
            let idx = if alt || code >= 0x80 || code < 0x40 {
                usize::from(code)
            } else {
                // FLASH ($40-$7F): the inverse glyph while the flash phase is "inverse"
                let base = usize::from(code - 0x40);
                if flash_on {
                    base
                } else {
                    base + 0x80
                }
            };
            for (r, row) in rows.iter_mut().enumerate() {
                *row = !self.charset[idx * 8 + r] & 0x7F;
            }
        } else {
            // ][ ROM: 64 positive glyphs (bit set = lit) at $40-$7F; inverse/flash invert them.
            let glyph = usize::from(code & 0x3F) + 0x40;
            let inverse = code < 0x40 || (code < 0x80 && flash_on);
            for (r, row) in rows.iter_mut().enumerate() {
                let b = self.charset[glyph * 8 + r] & 0x7F;
                *row = if inverse { !b & 0x7F } else { b };
            }
        }
        rows
    }
}

impl FrameSource for TextPlaceholder {
    fn render(&mut self, m: &Machine, frame: &mut Frame) {
        frame.resize(SCREEN_W, SCREEN_H);
        let v = m.board().video;
        let flash_on = (m.frames() / 16) % 2 == 0;
        let col80 = v.col80 && self.is_e;
        let alt = v.altcharset && self.is_e;
        let w = SCREEN_W as usize;
        for px in frame.rgba.chunks_exact_mut(4) {
            px.copy_from_slice(&[0, 0, 0, 255]);
        }
        let cols = if col80 { 80 } else { COLS };
        let cell_w = if col80 { 7 } else { 14 };
        for row in 0..ROWS {
            let base = row_address(row);
            for col in 0..cols {
                let code = if col80 {
                    // even columns come from AUX memory, odd from main
                    let a = base + (col / 2) as u16;
                    if col % 2 == 0 {
                        m.peek_aux(a)
                    } else {
                        m.peek_main(a)
                    }
                } else {
                    m.peek_main(base + col as u16)
                };
                let glyph = self.glyph(code, alt, flash_on);
                for (gy, bits) in glyph.iter().enumerate() {
                    for gx in 0..7usize {
                        if bits >> gx & 1 == 0 {
                            continue;
                        }
                        let (x0, nx) = if col80 { (col * 7 + gx, 1) } else { (col * cell_w + gx * 2, 2) };
                        for dy in 0..2 {
                            let y = row * 16 + gy * 2 + dy;
                            for dx in 0..nx {
                                let o = (y * w + x0 + dx) * 4;
                                frame.rgba[o..o + 3].copy_from_slice(&self.fg);
                            }
                        }
                    }
                }
            }
        }
    }

    fn name(&self) -> &'static str {
        "text placeholder"
    }
}
