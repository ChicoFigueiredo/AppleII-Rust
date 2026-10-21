//! Cell drawers of the table-driven renderers (AppleWin RGBMonitor.cpp): the idealized colour
//! video type and the RGB card (Apple Extended 80-Column/AppleColor, Video-7 SL7, Le Chat Mauve
//! EVE and Feline). Each draws one 14-dot-wide cell at the renderer's video address, two
//! framebuffer rows high.

use crate::config::RgbCard;
use crate::lookup::{lookup, SRCOFFS_DHIRES, SRCOFFS_HIRES, SRCOFFS_LORES};
use crate::renderer::{Renderer, MAT_H as MAT_H_PUB, MAT_Y as MAT_Y_PUB};
use crate::source::VideoSource;

const HIRES_COLUMN_SUBUNIT_SIZE: usize = 16;

impl Renderer {
    // ---- RGB card modes (RGBMonitor.cpp:1251). ----

    /// `RGB_Is160Mode`: unsupported by Feline, which falls back to the 140 mode (`RGB_Is140Mode`:
    /// the default branch of the DHGR drawer).
    pub(crate) fn rgb_160_mode(&self) -> bool {
        self.rgb.mode == 1 && self.cfg.rgb_card != RgbCard::LeChatMauveFeline
    }

    /// `RGB_IsMixMode`: the Extended 80-Column Text/AppleColor card's mode 3.
    pub(crate) fn rgb_mix_mode(&self) -> bool {
        self.rgb.mode == 2
    }

    /// `RGB_Is560Mode`: the card's mode 1.
    pub(crate) fn rgb_560_mode(&self) -> bool {
        self.rgb.mode == 3
    }

    // ---- Writing helpers. ----

    /// `CopySource` (RGBMonitor.cpp:556): a `w`-dot, two-row cell from the source image at
    /// `(sx, sy)`; `adj` shifts the source (160-colour images).
    fn copy_source(&mut self, w: usize, sx: usize, sy: usize, adj: usize) {
        let l = lookup();
        let half = self.half_scanlines();
        let a = self.addr;
        for row in 0..2isize {
            let dst = a + row * self.fb_w();
            if half && row == 1 {
                for i in 0..w as isize {
                    self.put(dst + i, crate::chroma::ALPHA);
                }
            } else {
                for i in 0..w {
                    let c = self.palette[l.at(sx + i + adj, sy) as usize];
                    self.put(dst + i as isize, c);
                }
            }
        }
    }

    /// Second line of a 14-dot RGB cell: black with 50% scan lines, else a copy of the first.
    fn second_line(&mut self) {
        let a = self.addr;
        let below = a + self.fb_w();
        for i in 0..14isize {
            let c = if self.half_scanlines() {
                crate::chroma::ALPHA
            } else {
                self.get(a + i)
            };
            self.put(below + i, c);
        }
    }

    // ---- Lo-res. ----

    /// `UpdateLoResCell` (:1026).
    pub(crate) fn lores_cell(&mut self, s: &dyn VideoSource, x: usize, y: usize, addr: u16) {
        let val = s.main(addr);
        let sy = if y & 4 == 0 {
            (val as usize & 0xF) << 4
        } else {
            val as usize & 0xF0
        };
        self.copy_source(14, SRCOFFS_LORES + ((x & 1) << 1), sy, 0);
    }

    /// `UpdateDLoResCell` (:1045): aux nibble left, main nibble right.
    pub(crate) fn dlores_cell(&mut self, s: &dyn VideoSource, x: usize, y: usize, addr: u16) {
        let mut aux = s.aux(addr);
        let main = s.main(addr);
        let rol = |n: u8| ((n << 1) & 0xF) | ((n >> 3) & 1);
        if !self.cfg.rgb_mac_lc_dlgr {
            // "Apple IIe Card for Macintosh LC" has a bug in its DLGR (GH#1258): no rotation.
            aux = (rol(aux >> 4) << 4) | rol(aux & 0xF);
        }
        let sx = SRCOFFS_LORES + ((x & 1) << 1);
        let (sa, sm) = if y & 4 == 0 {
            ((aux as usize & 0xF) << 4, (main as usize & 0xF) << 4)
        } else {
            (aux as usize & 0xF0, main as usize & 0xF0)
        };
        self.copy_source(7, sx, sa, 0);
        self.addr += 7;
        self.copy_source(7, sx, sm, 0);
        self.addr -= 7;
    }

    // ---- Hi-res. ----

    /// `UpdateHiResCell` (:589): idealized single (and "double hires 40") hi-res.
    pub(crate) fn hires_cell(&mut self, s: &dyn VideoSource, x: usize, y: usize, addr: u16) {
        let mut b1 = if x > 0 {
            s.main(addr.wrapping_sub(1))
        } else {
            0
        };
        let mut b2 = s.main(addr);
        let mut b3 = if x < 39 {
            s.main(addr.wrapping_add(1))
        } else {
            0
        };
        if self.current_flags().dhires {
            b1 &= 0x7F;
            b2 &= 0x7F;
            b3 &= 0x7F;
        }
        let column = (((b1 & 0xE0) as usize) << 2) | (((b3 & 0x03) as usize) << 5);
        let sx = SRCOFFS_HIRES + column + (x & 1) * HIRES_COLUMN_SUBUNIT_SIZE;
        if self.cfg.vertical_blend {
            self.copy_mixed_source(x, y, sx, b2 as usize);
        } else {
            self.copy_source(14, sx, b2 as usize, 0);
        }
    }

    /// `MixColorsVertical` (:469): the six half-rows of colour index around matrix row `maty`.
    fn mix_colors_vertical(&self, matx: usize, maty: usize) -> [u8; 6] {
        let m = |y: usize| self.hgr_matrix[matx * MAT_H_PUB + y] & 0x0F;
        let (bot1, bot2);
        if self.is_mixed() && maty > 159 {
            if maty < 161 {
                bot1 = m(maty + 1);
                bot2 = 0;
            } else {
                bot1 = 0;
                bot2 = 0;
            }
        } else {
            bot1 = m(maty + 1);
            bot2 = m(maty + 2);
        }
        let mix = &lookup().colormix;
        let mut out = [0u8; 6];
        let v = mix[m(maty - 2) as usize][m(maty - 1) as usize][m(maty) as usize];
        (out[0], out[1]) = ((v >> 8) as u8, v as u8);
        let v = mix[m(maty - 1) as usize][m(maty) as usize][bot1 as usize];
        (out[2], out[3]) = ((v >> 8) as u8, v as u8);
        let v = mix[m(maty) as usize][bot1 as usize][bot2 as usize];
        (out[4], out[5]) = ((v >> 8) as u8, v as u8);
        out
    }

    /// `CopyMixedSource` (:510): GH#616 vertical colour blending of hi-res rows.
    fn copy_mixed_source(&mut self, x: usize, y: usize, sx: usize, sy: usize) {
        let l = lookup();
        let matx = x * 14;
        let maty = MAT_Y_PUB + y;
        for nb in (0..14).rev() {
            self.hgr_matrix[(matx + nb) * MAT_H_PUB + maty] = l.at(sx + nb, sy);
        }
        let half = self.half_scanlines();
        for nb in (0..14).rev() {
            let buf = self.mix_colors_vertical(matx + nb, maty);
            let mut dst = self.addr + nb as isize;
            for (k, &mixed) in buf[MAT_Y_PUB..=MAT_Y_PUB + 1].iter().enumerate() {
                let h = MAT_Y_PUB + k;
                let c = if half && h & 1 != 0 {
                    crate::chroma::ALPHA
                } else {
                    self.palette[mixed as usize]
                };
                self.put(dst, c);
                dst += self.fb_w();
            }
        }
    }

    /// `UpdateHiResRGBCell` (:652): RGB-card hi-res, one pixel at a time.
    pub(crate) fn hires_rgb_cell(&mut self, s: &dyn VideoSource, x: usize, addr: u16) {
        let xoffset = x & 1;
        let addr = addr.wrapping_sub(xoffset as u16);
        let b1 = if x < 2 {
            0
        } else {
            s.main(addr.wrapping_sub(1))
        };
        let b2 = s.main(addr);
        let b3 = s.main(addr.wrapping_add(1));
        let b4 = if x >= 38 {
            0
        } else {
            s.main(addr.wrapping_add(2))
        };
        let mut dword: u32 = (b1 & 0x7F) as u32
            | ((b2 & 0x7F) as u32) << 7
            | ((b3 & 0x7F) as u32) << 14
            | ((b4 & 0x7F) as u32) << 21;
        // Extraction of the 14 colour pixels.
        let mut colors = [0u32; 14];
        let mut tmp = dword >> 7;
        let mut offset = b2 & 0x80 != 0;
        for (i, c) in colors.iter_mut().enumerate() {
            if i == 7 {
                offset = b3 & 0x80 != 0;
            }
            let color = (tmp & 3) as usize;
            // AppleWin's palette is in a strange order, hence two cases.
            *c = if offset {
                self.palette[1 + color]
            } else {
                self.palette[6 - color]
            };
            if i % 2 == 1 {
                tmp >>= 2;
            }
        }
        let bw = [self.palette[0], self.palette[1]];
        let (mask, chck1, chck2) = (0x01C0u32, 0x0140u32, 0x0080u32);
        let mut start = 0;
        if xoffset != 0 {
            dword >>= 7;
            start = 7;
        }
        // A pixel is colour if it makes a 101 or 010 pattern with its neighbours, else B&W.
        let mut dst = self.addr;
        for c in &colors[start..start + 7] {
            let px = if (dword & mask) == chck1 || (dword & mask) == chck2 {
                *c
            } else {
                bw[(dword & chck2 != 0) as usize]
            };
            self.put(dst, px);
            self.put(dst + 1, px);
            dst += 2;
            dword >>= 1;
        }
        self.second_line();
    }

    /// `UpdateHiResDuochromeCell` (:1105): foreground/background colours from aux memory.
    pub(crate) fn hires_duochrome_cell(&mut self, s: &dyn VideoSource, addr: u16) {
        let bits = s.main(addr);
        let val = s.aux(addr);
        self.duochrome_cell(2, 14, bits, val >> 4, val & 0xF);
    }

    // ---- Double hi-res. ----

    /// `UpdateDHiResCell` (:618): idealized DHGR, aux half then main half.
    pub(crate) fn dhires_cell(
        &mut self,
        s: &dyn VideoSource,
        x: usize,
        addr: u16,
        update_aux: bool,
        update_main: bool,
    ) {
        let xpixel = x * 14;
        let b1 = if x > 0 {
            s.main(addr.wrapping_sub(1))
        } else {
            0
        };
        let b2 = s.aux(addr);
        let b3 = s.main(addr);
        let b4 = if x < 39 {
            s.aux(addr.wrapping_add(1))
        } else {
            0
        };
        let dword: u32 = (b1 & 0x70) as u32
            | ((b2 & 0x7F) as u32) << 7
            | ((b3 & 0x7F) as u32) << 14
            | ((b4 & 0x07) as u32) << 21;
        let draw = |r: &mut Renderer, pixel: usize| {
            let color = (xpixel + pixel) & 3;
            let value = dword >> (4 + pixel - color);
            r.copy_source(
                7,
                SRCOFFS_DHIRES + 10 * ((value >> 8) & 0xFF) as usize + color,
                (value & 0xFF) as usize,
                0,
            );
        };
        let mut off = 0;
        if update_aux {
            draw(self, 0);
            off = 7;
        }
        if update_main {
            self.addr += off;
            draw(self, 7);
            self.addr -= off;
        }
    }

    /// `UpdateDHiRes160Cell` (:963): the 160x192 RGB card mode, squashed to 560 dots. Returns
    /// the width drawn.
    pub(crate) fn dhires160_cell(&mut self, s: &dyn VideoSource, x: usize, addr: u16) -> isize {
        let xpixel = x * 16;
        let b1 = if x > 0 {
            s.main(addr.wrapping_sub(1))
        } else {
            0
        };
        let b2 = s.aux(addr);
        let b3 = s.main(addr);
        let b4 = if x < 39 {
            s.aux(addr.wrapping_add(1))
        } else {
            0
        };
        let mut dword: u32 =
            (b1 & 0xF8) as u32 | (b2 as u32) << 8 | (b3 as u32) << 16 | ((b4 & 0x1F) as u32) << 24;
        dword <<= 2;
        for (pixel, advance) in [(0usize, 7isize), (8, 0)] {
            let color = (xpixel + pixel) & 3;
            let value = dword >> (4 + pixel - color);
            self.copy_source(
                7,
                SRCOFFS_DHIRES + 10 * ((value >> 8) & 0xFF) as usize + color,
                (value & 0xFF) as usize,
                1,
            );
            self.addr += advance;
        }
        self.addr -= 7;
        14
    }

    /// `UpdateDHiResCellRGB` (:749): RGB-card DHGR (140-pixel colour, 560 B&W and the mixed mode).
    pub(crate) fn dhires_cell_rgb(
        &mut self,
        s: &dyn VideoSource,
        x: usize,
        addr: u16,
        mix_mode: bool,
        invert_bit7: bool,
    ) {
        let xoffset = x & 1;
        let addr = addr.wrapping_sub(xoffset as u16);
        let (mut b1, mut b2, mut b3, mut b4) = (
            s.aux(addr),
            s.main(addr),
            s.aux(addr.wrapping_add(1)),
            s.main(addr.wrapping_add(1)),
        );
        let mut dword: u32 = (b1 & 0x7F) as u32
            | ((b2 & 0x7F) as u32) << 7
            | ((b3 & 0x7F) as u32) << 14
            | ((b4 & 0x7F) as u32) << 21;
        // Seven colour pixels of 4 bits; DHGR colours are rotated one bit.
        let mut colors = [0u32; 7];
        let mut tmp = dword;
        for c in colors.iter_mut() {
            let bits = tmp & 0xF;
            let color = (((bits & 7) << 1) | ((bits & 8) >> 3)) as usize;
            *c = self.palette[12 + color];
            tmp >>= 4;
        }
        let bw = [self.palette[12], self.palette[12 + 15]];
        if invert_bit7 {
            (b1, b2, b3, b4) = (!b1, !b2, !b3, !b4);
        }
        // Colour mode is a real 140x192 RGB mode with no fringe (patent US4631692); B&W is a real
        // 560x192 mode; in mixed mode bit 7 of each byte selects colour or B&W for its 7 bits. A
        // colour pixel may be cut short by a B&W byte, and a B&W cell crossing into a colour byte
        // repeats its last bit until the next colour pixel (tested on a Le Chat Mauve IIc adapter).
        let mut out = [0u32; 14];
        let mut n = 0;
        let mut put = |c: u32| {
            out[n] = c;
            n += 1;
        };
        let st = &mut self.rgb;
        let bw_cell =
            |dword: &mut u32, put: &mut dyn FnMut(u32), st: &mut crate::renderer::RgbState| {
                for _ in 0..7 {
                    st.dhgr_last_bit = *dword & 1;
                    put(bw[st.dhgr_last_bit as usize]);
                    *dword >>= 1;
                }
                st.dhgr_last_cell_is_color = false;
            };
        let color_or_bw = |b: u8| (b & 0x80 != 0) || !mix_mode;
        if xoffset == 0 {
            if color_or_bw(b1) {
                for _ in 0..4 {
                    put(colors[0]);
                }
                for _ in 0..3 {
                    put(colors[1]);
                }
                dword >>= 7;
                st.dhgr_last_cell_is_color = true;
            } else {
                bw_cell(&mut dword, &mut put, st);
            }
            if color_or_bw(b2) {
                put(if st.dhgr_last_cell_is_color {
                    colors[1]
                } else {
                    bw[st.dhgr_last_bit as usize]
                });
                for _ in 0..4 {
                    put(colors[2]);
                }
                for _ in 0..2 {
                    put(colors[3]);
                }
                st.dhgr_last_cell_is_color = true;
            } else {
                bw_cell(&mut dword, &mut put, st);
            }
        } else {
            dword >>= 14;
            if color_or_bw(b3) {
                for _ in 0..2 {
                    put(if st.dhgr_last_cell_is_color {
                        colors[3]
                    } else {
                        bw[st.dhgr_last_bit as usize]
                    });
                }
                for _ in 0..4 {
                    put(colors[4]);
                }
                put(colors[5]);
                dword >>= 7;
                st.dhgr_last_cell_is_color = true;
            } else {
                bw_cell(&mut dword, &mut put, st);
            }
            if color_or_bw(b4) {
                for _ in 0..3 {
                    put(if st.dhgr_last_cell_is_color {
                        colors[5]
                    } else {
                        bw[st.dhgr_last_bit as usize]
                    });
                }
                for _ in 0..4 {
                    put(colors[6]);
                }
                st.dhgr_last_cell_is_color = true;
            } else {
                bw_cell(&mut dword, &mut put, st);
            }
        }
        let a = self.addr;
        for (i, c) in out.iter().enumerate() {
            self.put(a + i as isize, *c);
        }
        self.second_line();
    }

    // ---- Text on RGB cards. ----

    /// `UpdateText40ColorCell` (:1070).
    pub(crate) fn text40_color_cell(
        &mut self,
        s: &dyn VideoSource,
        addr: u16,
        bits: u8,
        character: u8,
    ) {
        let (mut fg, mut bg) = (self.cfg.rgb_text_fg, self.cfg.rgb_text_bg);
        if self.rgb.text_fb {
            let val = s.aux(addr); // RGB cards with F/B text use their own AUX memory
            fg = val >> 4;
            bg = val & 0xF;
        } else if self.cfg.rgb_card == RgbCard::Video7Sl7 && character < 0x80 {
            // In regular 40COL mode the SL7 renders inverse characters as B&W.
            fg = 15;
            bg = 0;
        }
        self.duochrome_cell(2, 14, bits, fg, bg);
    }

    /// `UpdateText80ColorCell` (:1090).
    pub(crate) fn text80_color_cell(&mut self, bits: u8, character: u8) {
        let (mut fg, mut bg) = (self.cfg.rgb_text_fg, self.cfg.rgb_text_bg);
        if self.cfg.rgb_card == RgbCard::Video7Sl7 && character < 0x80 {
            // In all 80COL modes the SL7 renders inverse characters as B&W.
            fg = 15;
            bg = 0;
        }
        self.duochrome_cell(2, 7, bits, fg, bg);
    }

    /// `UpdateDuochromeCell` (:1120): 7 bits as a foreground/background pattern, `h` rows of `w`
    /// dots (`w` = 14 doubles each bit).
    fn duochrome_cell(&mut self, h: usize, w: usize, bits: u8, fg: u8, bg: u8) {
        let colors = [
            self.palette[12 + (bg & 15) as usize],
            self.palette[12 + (fg & 15) as usize],
        ];
        let half = self.half_scanlines();
        let doubled = w == 14;
        let base = self.addr;
        for row in 0..h {
            let dst = base + row as isize * self.fb_w();
            // The C++ counts `h` down, so with 50% scan lines the last row is black.
            if half && row == h - 1 {
                for i in 0..w as isize {
                    self.put(dst + i, crate::chroma::ALPHA);
                }
                continue;
            }
            let mut b = bits;
            let mut i = 0;
            while i < w {
                let c = colors[(b & 1) as usize];
                b >>= 1;
                self.put(dst + i as isize, c);
                if doubled {
                    self.put(dst + i as isize + 1, c);
                    i += 2;
                } else {
                    i += 1;
                }
            }
        }
    }
}
