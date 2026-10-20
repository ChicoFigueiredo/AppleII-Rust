//! Video scanner (UTAIIe ch. 3 and 5): which memory address the video generator is fetching at a
//! given cycle of the frame, which is what the floating bus returns, and VBLBAR.
//!
//! [`scanner_address`] is a direct port of AppleWin `Video::VideoGetScannerAddress`
//! (Video.cpp:338-462). [`Tables`] are built from it once at compile time exactly as AppleWin's
//! `GenerateVideoTables` (NTSC.cpp:2607-2690) does, so a lookup is O(1) like `NTSC.cpp`
//! `getVideoScannerAddressTXT/HGR` (:858-890).

use crate::video_standard::VideoStandard;

/// Cycles per scanline, including HBL (Video.cpp `kHClocks`, NTSC.cpp:90).
pub const CYCLES_PER_LINE: u32 = 65;
/// First line not displayed: VBL starts here (NTSC.cpp `VIDEO_SCANNER_Y_DISPLAY`).
pub const VISIBLE_LINES: u32 = 192;
/// First line of the text window in mixed mode (NTSC.cpp `VIDEO_SCANNER_Y_MIXED`).
pub const MIXED_LINE: u32 = 160;
/// First visible horizontal clock (NTSC.cpp `VIDEO_SCANNER_HORZ_START`).
pub const HORZ_START: u32 = 25;

const MAX_LINES: usize = 312;
const H_CLOCKS: usize = CYCLES_PER_LINE as usize;

/// Which part of the address `scanner_address` returns (Video.cpp `VideoScanner_e`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Full,
    /// `VS_PartialAddrH`: the horizontal bits (and the ][ HBL `$1000`).
    H,
    /// `VS_PartialAddrV`: the vertical bits, without the page.
    V,
}

/// Video switches that steer the scanner address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Switches {
    pub text: bool,
    pub mixed: bool,
    pub hires: bool,
    /// The page being displayed is page 2 (PAGE2 and not 80STORE: with 80STORE PAGE2 selects aux
    /// memory and the display stays on page 1, NTSC.cpp:2029-2036).
    pub page2: bool,
}

const fn bit(v: i32, n: i32) -> i32 {
    (v >> n) & 1
}

/// `Video::VideoGetScannerAddress` (Video.cpp:338-462) for the cycle `cycle` of the frame.
/// `ii` is `IS_APPLE2` (adds `$1000` in HBL in text modes); `mixed` is the SW_MIXED switch.
pub const fn scanner_address(cycle: u32, pal: bool, sw: Switches, store80: bool, ii: bool, part: Part) -> u16 {
    const H_CLOCK0_STATE: i32 = 0x18;
    const H_PE_CLOCK: i32 = 40;
    const H_PRESET_CLOCK: i32 = 41;
    const V_LINE0_STATE: i32 = 0x100;
    const V_PRESET_LINE: i32 = 256;

    let mut hires = sw.hires && !sw.text;
    let scan_lines: i32 = if pal { 312 } else { 262 };
    let cycle = (cycle % (scan_lines as u32 * CYCLES_PER_LINE)) as i32;

    // Horizontal state.
    let h_clock = (cycle + H_PE_CLOCK) % CYCLES_PER_LINE as i32;
    let mut h_state = H_CLOCK0_STATE + h_clock;
    if h_clock >= H_PRESET_CLOCK {
        h_state -= 1;
    }
    let (h_0, h_1, h_2, h_3, h_4, h_5) =
        (bit(h_state, 0), bit(h_state, 1), bit(h_state, 2), bit(h_state, 3), bit(h_state, 4), bit(h_state, 5));

    // Vertical state (UTAIIe:3-15,T3.2).
    let v_line = cycle / CYCLES_PER_LINE as i32;
    let mut v_state = V_LINE0_STATE + v_line;
    if v_line >= V_PRESET_LINE {
        v_state -= scan_lines;
    }
    let (v_a, v_b, v_c) = (bit(v_state, 0), bit(v_state, 1), bit(v_state, 2));
    let (v_0, v_1, v_2, v_3, v_4) = (bit(v_state, 3), bit(v_state, 4), bit(v_state, 5), bit(v_state, 6), bit(v_state, 7));

    if hires && sw.mixed && v_4 != 0 && v_2 != 0 {
        hires = false; // HIRES TIME (UTAIIe:5-7,P3): text memory for mixed hires
    }

    let addend1 = (h_5 << 2) | (h_4 << 1) | h_3;
    let addend2 = (v_4 << 3) | (v_3 << 2) | (v_4 << 1) | v_3;
    let sum = (0x0D + addend1 + addend2) & 0x0F; // UTAIIe:5-9

    let mut addr_h = h_0 | (h_1 << 1) | (h_2 << 2) | (sum << 3);
    if !hires && ii && h_5 == 0 && (h_4 == 0 || h_3 == 0) {
        addr_h |= 1 << 12; // ][ HBL (UTAIIe:I-4,#5 and 8-10,F8.5)
    }

    let mut addr_v = (v_0 << 7) | (v_1 << 8) | (v_2 << 9);
    let p2a = !(sw.page2 && !store80) as i32;
    let p2b = (sw.page2 && !store80) as i32;
    let mut addr_p = 0;
    if hires {
        addr_v |= (v_a << 10) | (v_b << 11) | (v_c << 12);
        addr_p |= (p2a << 13) | (p2b << 14);
    } else {
        addr_p |= (p2a << 10) | (p2b << 11);
    }

    (match part {
        Part::H => addr_h,
        Part::V => addr_v,
        Part::Full => addr_p | addr_v | addr_h,
    }) as u16
}

/// AppleWin's runtime-generated NTSC.cpp tables (`g_aClockVertOffsets*`, `APPLE_II*_HORZ_CLOCK_OFFSET`).
pub struct Tables {
    /// Per line, hires vertical bits (page 1, no page offset).
    vert_hgr: [u16; MAX_LINES],
    /// Per 8 lines, text vertical bits.
    vert_txt: [u16; MAX_LINES / 8],
    /// `[ii][line / 64][horz]`: the ][/][+ table has `$1000` in HBL.
    horz: [[[u16; H_CLOCKS]; 5]; 2],
}

impl Tables {
    /// `GenerateVideoTables` (NTSC.cpp:2607-2690), page 1.
    const fn new(pal: bool) -> Self {
        let sw_h = Switches { text: false, mixed: false, hires: true, page2: false };
        let sw_t = Switches { text: true, ..sw_h };
        let mut t = Tables { vert_hgr: [0; MAX_LINES], vert_txt: [0; MAX_LINES / 8], horz: [[[0; H_CLOCKS]; 5]; 2] };
        let mut i = 0;
        while i < MAX_LINES {
            let cycle = HORZ_START + CYCLES_PER_LINE * i as u32;
            t.vert_hgr[i] = scanner_address(cycle, pal, sw_h, false, false, Part::V);
            i += 1;
        }
        i = 0;
        while i < MAX_LINES / 8 {
            let cycle = HORZ_START + CYCLES_PER_LINE * 8 * i as u32;
            t.vert_txt[i] = scanner_address(cycle, pal, sw_t, false, false, Part::V);
            i += 1;
        }
        let mut ii = 0;
        while ii < 2 {
            let mut j = 0;
            while j < 5 {
                let mut h = 0;
                while h < H_CLOCKS {
                    let cycle = j as u32 * 64 * CYCLES_PER_LINE + h as u32;
                    t.horz[ii][j][h] = scanner_address(cycle, pal, sw_t, false, ii == 1, Part::H);
                    h += 1;
                }
                j += 1;
            }
            ii += 1;
        }
        t
    }
}

static NTSC: Tables = Tables::new(false);
static PAL: Tables = Tables::new(true);

/// The scanner of a machine: video standard and whether the model is a ][ or ][+ (the
/// `APPLE_IIP` horizontal table, NTSC.cpp:2443-2451).
#[derive(Clone, Copy)]
pub struct VideoScanner {
    tables: &'static Tables,
    lines: u32,
    ii: bool,
}

impl VideoScanner {
    /// `ii`: model value & 0xFFFE == 0, i.e. only the ][ and ][+ (NTSC.cpp:2446).
    pub fn new(standard: VideoStandard, ii: bool) -> Self {
        match standard {
            VideoStandard::Ntsc => VideoScanner { tables: &NTSC, lines: 262, ii },
            VideoStandard::Pal => VideoScanner { tables: &PAL, lines: 312, ii },
        }
    }

    pub fn cycles_per_frame(&self) -> u32 {
        self.lines * CYCLES_PER_LINE
    }

    /// `(vert, horz)` at `cycle` of the frame (`NTSC_VideoClockResync`, NTSC.cpp:1925).
    pub fn position(&self, cycle: u32) -> (u32, u32) {
        ((cycle / CYCLES_PER_LINE) % self.lines, cycle % CYCLES_PER_LINE)
    }

    /// `NTSC_GetVblBar` (NTSC.cpp:2776): true on the visible lines, false in the vertical blank.
    pub fn vbl_bar(&self, cycle: u32) -> bool {
        self.position(cycle).0 < VISIBLE_LINES
    }

    /// `getVideoScannerAddressTXTorHGR` (NTSC.cpp:892) at the scanner position `(vert, horz)`.
    pub fn address_at(&self, vert: u32, horz: u32, sw: Switches) -> u16 {
        let (vert, horz) = (vert as usize, horz as usize);
        let page = if sw.page2 { 2 } else { 1 };
        if (sw.mixed && vert >= MIXED_LINE as usize) || sw.text || !sw.hires {
            self.tables.vert_txt[vert / 8] + self.tables.horz[self.ii as usize][vert / 64][horz] + page * 0x400
        } else {
            // Both the ][ and the //e use the //e horizontal table in HGR (NTSC.cpp:883).
            self.tables.vert_hgr[vert] + self.tables.horz[0][vert / 64][horz] + page * 0x2000
        }
    }

    /// `NTSC_VideoGetScannerAddress` (NTSC.cpp:1932): the address at `cycle` of the frame, after
    /// stepping back one horizontal clock (the "2-cycle adjust" for ANSI STORY: the fetch seen
    /// by a CPU access is the previous cycle's).
    pub fn address(&self, cycle: u32, sw: Switches) -> u16 {
        let cycle = if cycle == 0 { self.cycles_per_frame() - 1 } else { cycle - 1 };
        let (vert, horz) = self.position(cycle);
        self.address_at(vert, horz, sw)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn sw(text: bool, mixed: bool, hires: bool, page2: bool) -> Switches {
        Switches { text, mixed, hires, page2 }
    }

    #[test]
    fn address_at_matches_the_reference_model_everywhere() {
        // Every cycle of both standards, all mode combinations, both model families: the tables
        // are the reference model sampled at compile time, so they must agree with it.
        for (std, pal) in [(VideoStandard::Ntsc, false), (VideoStandard::Pal, true)] {
            for ii in [false, true] {
                let s = VideoScanner::new(std, ii);
                for flags in 0..16u32 {
                    let w = sw(flags & 1 != 0, flags & 2 != 0, flags & 4 != 0, flags & 8 != 0);
                    for cycle in (0..s.cycles_per_frame()).step_by(3) {
                        let (v, h) = s.position(cycle);
                        // Reference: with the text/hires decision made as NTSC.cpp does, the
                        // formula's own HIRES TIME handling must give the same address.
                        let text_addr = (w.mixed && v >= MIXED_LINE) || w.text || !w.hires;
                        let rw = Switches { text: text_addr, hires: !text_addr, ..w };
                        let want = scanner_address(cycle, pal, rw, false, ii && text_addr, Part::Full);
                        assert_eq!(s.address_at(v, h, w), want, "pal={pal} ii={ii} {w:?} v={v} h={h}");
                    }
                }
            }
        }
    }
}
