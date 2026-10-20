//! VidHD (Blue Shift): SHR video for any Apple II (AppleWin VidHD.cpp, partial support there too).
//!
//! The card is detected through three magic bytes in its slot ROM and mirrors the //e memory
//! switches (80STORE, AUXREAD, AUXWRITE, PAGE2) plus the IIgs video registers NEWVIDEO
//! (`$C029`), SCREENCOLOR (`$C022`), BORDERCOLOR (`$C034`) and SHADOW (`$C035`). Those writes
//! are seen through [`Card::io_snoop`]. What a renderer needs is exposed here:
//! [`VidHd::is_shr`], the registers, and [`iigs_color_to_rgb`] / [`shr_cell`] (the SHR pixel
//! decode of `VidHDCard::UpdateSHRCell`). SHR scanlines live in aux memory `$2000-$9FFF`.
//!
//! On the ][/][+ the card turns on the core's dual write (`memVidHD`: CPU RAM writes are also
//! stored in aux RAM while [`VidHd::is_write_aux`]) through `CardCtx::set_dual_write_aux`; the
//! //e with an empty aux slot is not covered. Not modelled: the 80x45 text modes (see the
//! video crate) and border colour rendering.

use crate::state::{Dec, Enc};
use apple2_core::memory::mf;
use apple2_core::{Card, CardCtx};
use std::any::Any;

const MIRRORED: u32 = mf::STORE80 | mf::AUXREAD | mf::AUXWRITE | mf::PAGE2;

/// The registers of [`VidHd::snapshot`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VidHdSnapshot {
    pub mem_mode: u32,
    pub screen_color: u8,
    pub new_video: u8,
    pub border_color: u8,
    pub shadow: u8,
}

#[derive(Default)]
pub struct VidHd {
    /// Mirror of 80STORE/AUXREAD/AUXWRITE/PAGE2 (`m_memMode`, only meaningful on the ][ family).
    mem_mode: u32,
    screen_color: u8,
    new_video: u8,
    border_color: u8,
    shadow: u8,
}

impl VidHd {
    pub fn new() -> Self {
        Self::default()
    }

    /// SHR enabled and linearised (`IsSHR`, VidHD.h:30): NEWVIDEO b7 and b6.
    pub fn is_shr(&self) -> bool {
        self.new_video & 0xC0 == 0xC0
    }

    pub fn is_dhgr_black_and_white(&self) -> bool {
        self.new_video & 0x20 != 0
    }

    /// `IsWriteAux` (VidHD.cpp:84, `MemIsWriteAux`): AUXWRITE, or 80STORE with PAGE2.
    pub fn is_write_aux(&self) -> bool {
        self.mem_mode & mf::AUXWRITE != 0 || (self.mem_mode & mf::STORE80 != 0 && self.mem_mode & mf::PAGE2 != 0)
    }

    /// The registers `VidHDCard::SaveSnapshot` writes (VidHD.cpp:199), unmasked.
    pub fn snapshot(&self) -> VidHdSnapshot {
        VidHdSnapshot {
            mem_mode: self.mem_mode,
            screen_color: self.screen_color,
            new_video: self.new_video,
            border_color: self.border_color,
            shadow: self.shadow,
        }
    }

    /// `VidHDCard::LoadSnapshot` (VidHD.cpp:224).
    pub fn apply_snapshot(&mut self, s: &VidHdSnapshot) {
        (self.mem_mode, self.screen_color, self.new_video, self.border_color, self.shadow) =
            (s.mem_mode & MIRRORED, s.screen_color, s.new_video, s.border_color, s.shadow);
    }

    pub fn mem_mode(&self) -> u32 {
        self.mem_mode
    }
    pub fn new_video(&self) -> u8 {
        self.new_video
    }
    pub fn screen_color(&self) -> u8 {
        self.screen_color
    }
    pub fn border_color(&self) -> u8 {
        self.border_color & 0x0F
    }
    pub fn shadow(&self) -> u8 {
        self.shadow
    }
}

impl Card for VidHd {
    fn name(&self) -> &'static str {
        "VidHD"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    /// `VidHDCard::IORead` (:56): the Cn00 ROM is just the detection bytes.
    fn slot_rom_read(&mut self, offset: u8, ctx: &mut CardCtx) -> u8 {
        match offset {
            0 => 0x24,
            1 => 0xEA,
            2 => 0x4C,
            _ => ctx.floating_bus,
        }
    }

    fn wants_io_snoop(&self) -> bool {
        true
    }

    /// `VidHDCard::VideoIOWrite` (:69). The mirrored switches react to any access of
    /// `$C000-$C005` writes and `$C054/$C055`; the IIgs registers to writes only.
    fn io_snoop(&mut self, addr: u16, write: bool, value: u8, ctx: &mut CardCtx) {
        let set = |m: &mut u32, flag: u32, on: bool| *m = if on { *m | flag } else { *m & !flag };
        match (addr as u8, write) {
            (0x00 | 0x01, true) => set(&mut self.mem_mode, mf::STORE80, addr & 1 != 0),
            (0x02 | 0x03, true) => set(&mut self.mem_mode, mf::AUXREAD, addr & 1 != 0),
            (0x04 | 0x05, true) => set(&mut self.mem_mode, mf::AUXWRITE, addr & 1 != 0),
            (0x54 | 0x55, _) => set(&mut self.mem_mode, mf::PAGE2, addr & 1 != 0),
            (0x22, true) => self.screen_color = value,
            (0x29, true) => self.new_video = value,
            (0x34, true) => self.border_color = value,
            (0x35, true) => self.shadow = value,
            _ => {}
        }
        ctx.set_dual_write_aux(self.is_write_aux());
    }

    /// `VidHDCard::Reset` (:46): NEWVIDEO cleared (SHR off).
    fn reset(&mut self, _power_cycle: bool, _ctx: &mut CardCtx) {
        self.new_video = 0;
    }

    fn save(&self) -> Vec<u8> {
        let mut e = Enc::default();
        e.u32(self.mem_mode & MIRRORED).u8(self.screen_color).u8(self.new_video).u8(self.border_color).u8(self.shadow);
        e.0
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let mut d = Dec::new(state);
        match (d.u32(), d.u8(), d.u8(), d.u8(), d.u8()) {
            (Some(m), Some(sc), Some(nv), Some(bc), Some(sh)) if d.done() => {
                (self.mem_mode, self.screen_color, self.new_video, self.border_color, self.shadow) = (m & MIRRORED, sc, nv, bc, sh);
                true
            }
            _ => false,
        }
    }
}

/// The renderer's view of the card (the video crate defines the trait so it need not depend on
/// this one): hand it over with `apple2_video::WithVidHd`.
impl apple2_video::VidHdView for VidHd {
    fn is_shr(&self) -> bool {
        VidHd::is_shr(self)
    }
    fn shr_cell(&self, is_640: bool, color_fill: bool, palette: &[u16; 16], a: u32, prev: [u8; 3]) -> [[u8; 3]; 16] {
        shr_cell(is_640, color_fill, palette, a, prev)
    }
}

/// `ConvertIIgs2RGB` (VidHD.cpp:120): a IIgs `$0RGB` palette entry (4 bits per channel) to
/// 8-bit RGB.
pub fn iigs_color_to_rgb(color: u16) -> [u8; 3] {
    [((color >> 8) & 0xF) as u8 * 16, ((color >> 4) & 0xF) as u8 * 16, (color & 0xF) as u8 * 16]
}

/// `VidHDCard::UpdateSHRCell` (:131): four SHR bytes (`a`, first byte in the low bits) become 16
/// pixels. `palette` is the scanline's 16 IIgs colours, `prev` the pixel to the left (colour fill
/// mode repeats it for palette index 0). In 640 mode each byte holds four 2-bit pixels using
/// palette quarters `$8`, `$C`, `$0`, `$4`.
pub fn shr_cell(is_640: bool, color_fill: bool, palette: &[u16; 16], mut a: u32, mut prev: [u8; 3]) -> [[u8; 3]; 16] {
    let mut out = [[0u8; 3]; 16];
    let mut o = 0;
    for _ in 0..4 {
        if !is_640 {
            let p1 = ((a >> 4) & 0xF) as usize;
            let mut c1 = iigs_color_to_rgb(palette[p1]);
            if color_fill && p1 == 0 {
                c1 = prev;
            }
            let p2 = (a & 0xF) as usize;
            let mut c2 = iigs_color_to_rgb(palette[p2]);
            if color_fill && p2 == 0 {
                c2 = c1;
            }
            out[o..o + 4].copy_from_slice(&[c1, c1, c2, c2]);
            prev = c2;
            o += 4;
        } else {
            for (shift, base) in [(6, 0x8), (4, 0xC), (2, 0x0), (0, 0x4)] {
                out[o] = iigs_color_to_rgb(palette[base + ((a >> shift) & 3) as usize]);
                o += 1;
            }
        }
        a >>= 8;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_conversion_scales_nibbles_by_16() {
        assert_eq!(iigs_color_to_rgb(0x0F80), [0xF0, 0x80, 0x00]);
    }

    #[test]
    fn cell_320_expands_each_nibble_to_two_pixels() {
        let mut pal = [0u16; 16];
        pal[1] = 0x0F00;
        pal[2] = 0x00F0;
        // First byte 0x12 -> pixels 1,1,2,2; the other three bytes are index 0 (black).
        let c = shr_cell(false, false, &pal, 0x12, [9, 9, 9]);
        assert_eq!(&c[..4], &[[0xF0, 0, 0], [0xF0, 0, 0], [0, 0xF0, 0], [0, 0xF0, 0]]);
        assert_eq!(c[4], [0, 0, 0]);
    }

    #[test]
    fn color_fill_repeats_the_previous_pixel_for_index_zero() {
        let mut pal = [0u16; 16];
        pal[5] = 0x000F;
        let c = shr_cell(false, true, &pal, 0x05, [7, 7, 7]);
        assert_eq!(c[0], [7, 7, 7], "index 0 takes the left neighbour");
        assert_eq!(c[2], [0, 0, 0xF0]);
        let c = shr_cell(false, true, &pal, 0x50, [7, 7, 7]);
        assert_eq!(c[0], [0, 0, 0xF0]);
        assert_eq!(c[2], [0, 0, 0xF0], "fill continues from the first pixel of the byte");
    }

    #[test]
    fn cell_640_uses_the_four_palette_quarters() {
        let mut pal = [0u16; 16];
        pal[0x8 + 3] = 0x0100;
        pal[0xC + 2] = 0x0200;
        pal[1] = 0x0300;
        pal[0x4] = 0x0400;
        let c = shr_cell(true, false, &pal, 0b11_10_01_00, [0; 3]);
        assert_eq!([c[0][0], c[1][0], c[2][0], c[3][0]], [0x10, 0x20, 0x30, 0x40]);
    }
}
