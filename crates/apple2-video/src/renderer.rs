//! The per-cycle NTSC video renderer: a port of AppleWin NTSC.cpp's scanner and screen updaters.
//!
//! # Design
//!
//! AppleWin advances the renderer after every 6502 opcode (`NTSC_VideoUpdateCycles`), reading
//! the video switches and memory as they are at that moment. [`Renderer::update`] does the same
//! against a [`VideoSource`]: call it with the cycles each [`Machine::step`] took and mid-frame
//! mode changes, mid-frame memory writes and TEXT/MIXED switching delays come out exactly as in
//! AppleWin. [`Renderer::run_frame`] is that loop for a [`Machine`]; [`Renderer::redraw`] is the
//! fallback that renders a whole frame from the current state (`NTSC_VideoRedrawWholeScreen`).
//! The core needs no hook: mode changes are detected by comparing the switches between calls.
//!
//! The framebuffer is AppleWin's: 560x384 visible pixels (each Apple scan line is two rows) inside
//! a border of 20 pixels left and right and 18 rows above and below (`Video::GetFrameBuffer*`),
//! stored top-down as `0xAARRGGBB`.

use crate::charset::{char_sets, Glyphs};
use crate::chroma::{chroma, ALPHA, NUM_SEQUENCES};
use crate::config::{RenderConfig, RgbCard, VideoType};
use crate::lookup::{lookup, PALETTE_FELINE, PALETTE_SIZE};
use crate::source::{mode_snapshot, VideoFlags, VideoSource};
use apple2_core::video_scanner::{Switches, VideoScanner};
use apple2_core::{Machine, Model, VideoStandard};

pub const BORDER_W: usize = 20;
pub const BORDER_H: usize = 18;
pub const VISIBLE_W: usize = 560;
pub const VISIBLE_H: usize = 384;
pub const FB_W: usize = VISIBLE_W + 2 * BORDER_W;
pub const FB_H: usize = VISIBLE_H + 2 * BORDER_H;

const MAX_HORZ: u32 = 65;
const Y_DISPLAY: u32 = 192;
const Y_MIXED: u32 = 160;
const HORZ_START: u32 = 25;
const COLORBURST_BEG: u32 = 12;
const COLORBURST_END: u32 = 16;
const OPAQUE_BLACK: u32 = ALPHA;
/// `HGR_MATRIX_YOFFSET` and the matrix height (`FRAMEBUFFER_H/2 + 2*2`) of the vertical blend.
pub(crate) const MAT_Y: usize = 2;
pub(crate) const MAT_H: usize = VISIBLE_H / 2 + 2 * MAT_Y;

/// `UpdateScreenFunc_t` choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Screen {
    DoubleHires40,
    DoubleHires80,
    DoubleHires80Simplified,
    DoubleHires80Rgb,
    DoubleLores40,
    DoubleLores80,
    DoubleLores80Simplified,
    Hires40Simplified,
    SingleHires40Duochrome,
    SingleHires40Rgb,
    SingleHires40,
    SingleLores40Simplified,
    SingleLores40,
    Text40,
    Text40Rgb,
    Text80,
    Text80Rgb,
}

impl Screen {
    fn is_text(self) -> bool {
        matches!(self, Screen::Text40 | Screen::Text40Rgb | Screen::Text80 | Screen::Text80Rgb)
    }
}

/// One of `g_pFuncUpdateBnWPixel`/`g_pFuncUpdateHuePixel`.
#[derive(Clone, Copy)]
struct PixFn {
    tv: bool,
    mono: bool,
    single: bool,
}

pub(crate) struct RgbState {
    pub flags: u8,
    pub mode: u8,
    pub text_fb: bool,
    pub dhgr_last_cell_is_color: bool,
    pub dhgr_last_bit: u32,
}

pub struct Renderer {
    pub(crate) cfg: RenderConfig,
    fb: Vec<u32>,

    // Video scanner (NTSC.cpp g_nVideoClockVert/Horz).
    vert: u32,
    horz: u32,
    max_vert: u32,
    scanner: VideoScanner,
    configured: Option<(Model, VideoStandard)>,
    model: Model,
    pal: bool,
    /// `g_pVideoAddress` as an index into `fb`.
    pub(crate) addr: isize,

    // Mode state.
    flags: VideoFlags,
    /// `g_uNewVideoModeFlags`; differs from `flags` while a delayed mode change is pending.
    new_flags: VideoFlags,
    delay_mode: bool,
    col80_text: bool,
    prev_dhires: bool,
    gfx: Screen,
    txt: Screen,
    pub(crate) char_set: usize,
    csets: &'static [Glyphs],
    pub(crate) mixed: bool,
    text_page2: bool,
    hires_page2: bool,

    // Signal state.
    flash_counter: u8,
    flash_mask: u16,
    last_col_pixel: u32,
    color_burst: i32,
    color_phase: usize,
    signal_bits: usize,

    // Style.
    bnw_fn: PixFn,
    hue_fn: PixFn,
    mono_monitor: Vec<u32>,
    mono_tv: Vec<u32>,
    pub(crate) palette: [u32; PALETTE_SIZE],
    pub(crate) rgb: RgbState,
    pub(crate) hgr_matrix: Vec<u8>,
    double_mask: [u16; 128],
    pixel_mask_gr: [u16; 16],
}

impl Default for Renderer {
    fn default() -> Self {
        Renderer::new(RenderConfig::default())
    }
}

fn blend_half(c: u32) -> u32 {
    (c & 0x00FE_FEFE) >> 1
}

impl Renderer {
    pub fn new(cfg: RenderConfig) -> Self {
        let mut double_mask = [0u16; 128];
        for (b, m) in double_mask.iter_mut().enumerate() {
            for bit in 0..7 {
                if b & (1 << bit) != 0 {
                    *m |= 3 << (bit * 2);
                }
            }
        }
        let mut pixel_mask_gr = [0u16; 16];
        for (c, m) in pixel_mask_gr.iter_mut().enumerate() {
            let c = c as u16;
            *m = (c << 12) | (c << 8) | (c << 4) | c;
        }
        let mut r = Renderer {
            cfg,
            fb: vec![OPAQUE_BLACK; FB_W * FB_H],
            vert: 0,
            horz: 0,
            max_vert: 262,
            scanner: VideoScanner::new(VideoStandard::Ntsc, false),
            configured: None,
            model: Model::Apple2e,
            pal: false,
            addr: 0,
            flags: VideoFlags { text: true, ..Default::default() },
            new_flags: VideoFlags { text: true, ..Default::default() },
            delay_mode: false,
            col80_text: false,
            prev_dhires: false,
            gfx: Screen::Text40,
            txt: Screen::Text40,
            char_set: 0,
            csets: char_sets(Model::Apple2e, false, false),
            mixed: false,
            text_page2: false,
            hires_page2: false,
            flash_counter: 0,
            flash_mask: 0,
            last_col_pixel: 0,
            color_burst: 0,
            color_phase: 0,
            signal_bits: 0,
            bnw_fn: PixFn { tv: true, mono: true, single: true },
            hue_fn: PixFn { tv: true, mono: false, single: true },
            mono_monitor: vec![0; NUM_SEQUENCES],
            mono_tv: vec![0; NUM_SEQUENCES],
            palette: lookup().palette_ntsc,
            rgb: RgbState { flags: 0, mode: 0, text_fb: false, dhgr_last_cell_is_color: true, dhgr_last_bit: 0 },
            hgr_matrix: vec![0; VISIBLE_W * MAT_H],
            double_mask,
            pixel_mask_gr,
        };
        r.set_style();
        r.addr = r.scan_line(0);
        r
    }

    // ---- Public API. ----

    pub fn config(&self) -> &RenderConfig {
        &self.cfg
    }

    /// Changes the rendering options (video type, scan lines, monochrome colour, RGB card).
    pub fn set_config(&mut self, cfg: RenderConfig) {
        let reinit = cfg.video_type != self.cfg.video_type || cfg.rgb_card != self.cfg.rgb_card;
        self.cfg = cfg;
        self.set_style();
        if reinit {
            self.reselect_screen_funcs();
        }
    }

    /// Framebuffer width and height in pixels, borders included.
    pub fn size(&self) -> (usize, usize) {
        (FB_W, FB_H)
    }

    /// The borderless visible area: `(x, y, width, height)` inside the framebuffer.
    pub fn visible_rect(&self) -> (usize, usize, usize, usize) {
        (BORDER_W, BORDER_H, VISIBLE_W, VISIBLE_H)
    }

    /// The whole framebuffer, top-down, one `0xAARRGGBB` per pixel.
    pub fn pixels(&self) -> &[u32] {
        &self.fb
    }

    /// Pixel `(x, y)` of the visible 560x384 area as `[r, g, b, a]`.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let c = self.fb[(BORDER_H + y) * FB_W + BORDER_W + x];
        [(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]
    }

    /// The visible 560x384 area as tightly packed RGBA8 (for texture upload or screenshots).
    pub fn rgba8(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(VISIBLE_W * VISIBLE_H * 4);
        for y in 0..VISIBLE_H {
            for x in 0..VISIBLE_W {
                out.extend_from_slice(&self.pixel(x, y));
            }
        }
        out
    }

    /// `VideoResetState` + `RGB_ResetState`: call when the machine is reset.
    pub fn reset(&mut self) {
        self.rgb.flags = 0;
        self.rgb.mode = 0;
        self.prev_dhires = false;
        self.col80_text = false;
    }

    /// `NTSC_VideoClockResync`: realign the scanner with the machine's cycle in the frame.
    pub fn resync(&mut self, cycles_in_frame: u32) {
        self.vert = (cycles_in_frame / MAX_HORZ) % self.max_vert;
        self.horz = cycles_in_frame % MAX_HORZ;
    }

    /// Current scanner position `(vert, horz)`.
    pub fn position(&self) -> (u32, u32) {
        (self.vert, self.horz)
    }

    /// `NTSC_VideoUpdateCycles`: renders the `cycles` just executed (one opcode), reading the
    /// video state from `s` as it is now.
    pub fn update(&mut self, s: &dyn VideoSource, cycles: u32) {
        let frame = self.max_vert * MAX_HORZ;
        let left = cycles % frame;
        if left == 0 {
            return;
        }
        let mut cycles = left as i32;
        self.sync(s);
        if self.delay_mode {
            // A TEXT/MIXED change takes effect one cycle late (GH#656).
            self.video_update_cycles(s, 1);
            self.delay_mode = false;
            self.apply_mode(self.new_flags, true);
            cycles -= 1;
            if cycles == 0 {
                return;
            }
        }
        self.video_update_cycles(s, cycles);
    }

    /// Runs `m` for one frame, rendering after every instruction. Returns the cycles executed.
    pub fn run_frame(&mut self, m: &mut Machine) -> u64 {
        self.resync(m.cycles_in_frame());
        let target = m.frames() + 1;
        let mut done = 0u64;
        while m.frames() < target {
            let c = m.step();
            done += c as u64;
            self.update(m, c);
        }
        done
    }

    /// `NTSC_VideoRedrawWholeScreen`: renders a full frame from the current state, for when the
    /// per-opcode feed was not available (e.g. a paused or fast-forwarding machine).
    pub fn redraw(&mut self, s: &dyn VideoSource) {
        self.sync(s);
        if self.delay_mode {
            self.delay_mode = false;
            self.apply_mode(self.new_flags, false);
        }
        let horz = self.horz;
        self.horz = 0;
        self.update_video_scanner_address();
        let frame = (self.max_vert * MAX_HORZ) as i32;
        self.video_update_cycles(s, frame);
        self.video_update_cycles(s, horz as i32);
    }

    // ---- Configuration of the renderer for a machine. ----

    /// `NTSC_VideoInitAppleType` + `NTSC_SetRefreshRate`; and mode-change detection.
    fn sync(&mut self, s: &dyn VideoSource) {
        let key = (s.model(), s.standard());
        if self.configured != Some(key) {
            self.configured = Some(key);
            self.model = key.0;
            self.pal = key.1 == VideoStandard::Pal;
            self.max_vert = if self.pal { 312 } else { 262 };
            self.scanner = VideoScanner::new(key.1, key.0.value() & 0xFFFE == 0);
            self.vert %= self.max_vert;
            self.palette = self.palette_for_type();
            let snap = mode_snapshot(s);
            self.flags = snap.ntsc;
            self.col80_text = snap.raw.col80;
            self.prev_dhires = s.mode().dhires;
            self.reselect_text_mode();
            self.apply_mode(snap.ntsc, false);
            self.resync(s.cycles_in_frame());
            self.update_video_scanner_address();
            return;
        }
        let snap = mode_snapshot(s);
        let m = s.mode();
        // RGB_SetVideoMode: $C05E followed by $C05F clocks !80COL into the 2-bit card mode.
        if !self.model.is_apple2() {
            if self.prev_dhires && !m.dhires {
                self.rgb.flags = ((self.rgb.flags << 1) & 3) | (!snap.raw.col80) as u8;
                self.rgb.mode = self.rgb.flags;
            }
            self.prev_dhires = m.dhires;
        }
        if snap.raw.col80 != self.col80_text {
            self.col80_text = snap.raw.col80;
            self.reselect_text_mode();
        }
        if snap.ntsc != self.flags {
            let old = self.flags;
            self.flags = snap.ntsc;
            self.new_flags = snap.ntsc;
            if old.text != snap.ntsc.text || old.mixed != snap.ntsc.mixed {
                self.delay_mode = true;
            } else {
                self.apply_mode(snap.ntsc, true);
            }
        }
    }

    fn palette_for_type(&self) -> [u32; PALETTE_SIZE] {
        if self.cfg.video_type == VideoType::ColorVideocardRgb && self.cfg.rgb_card == RgbCard::LeChatMauveFeline {
            PALETTE_FELINE
        } else {
            lookup().palette_ntsc
        }
    }

    fn reselect_screen_funcs(&mut self) {
        self.palette = self.palette_for_type();
        self.reselect_text_mode();
        self.reselect_screen_funcs_for(self.flags);
    }

    /// `NTSC_SetVideoTextMode`.
    fn reselect_text_mode(&mut self) {
        let rgb = self.cfg.video_type == VideoType::ColorVideocardRgb;
        self.txt = match (rgb, self.col80_text) {
            (true, false) => Screen::Text40Rgb,
            (true, true) => Screen::Text80Rgb,
            (false, false) => Screen::Text40,
            (false, true) => Screen::Text80,
        };
    }

    /// `NTSC_SetVideoStyle` (NTSC.cpp:2233).
    fn set_style(&mut self) {
        let single = self.cfg.half_scanlines;
        let (r, g, b);
        let (mut bnw, mut hue) = (
            PixFn { tv: false, mono: true, single },
            PixFn { tv: false, mono: false, single },
        );
        match self.cfg.video_type {
            VideoType::ColorTv => {
                (r, g, b) = (0xFF, 0xFF, 0xFF);
                bnw.tv = true;
                hue.tv = true;
            }
            VideoType::ColorMonitorNtsc => (r, g, b) = (0xFF, 0xFF, 0xFF),
            VideoType::MonoTv => {
                (r, g, b) = (0xFF, 0xFF, 0xFF);
                bnw.tv = true;
                hue = bnw;
            }
            VideoType::MonoAmber => {
                (r, g, b) = (0xFF, 0x80, 0x00);
                hue = bnw;
            }
            VideoType::MonoGreen => {
                (r, g, b) = (0x00, 0xC0, 0x00);
                hue = bnw;
            }
            VideoType::ColorIdealized | VideoType::ColorVideocardRgb | VideoType::MonoWhite => {
                (r, g, b) = (0xFF, 0xFF, 0xFF);
                hue = bnw;
            }
            VideoType::MonoCustom => {
                [r, g, b] = self.cfg.monochrome_rgb;
                hue = bnw;
            }
        }
        self.bnw_fn = bnw;
        self.hue_fn = hue;
        // updateMonochromeTables
        let c = chroma();
        let scale = |t: &[u32], out: &mut Vec<u32>| {
            for (o, &v) in out.iter_mut().zip(t) {
                let ch = |sh: u32, k: u8| ((((v >> sh) & 0xFF) * k as u32) >> 8) & 0xFF;
                *o = ALPHA | (ch(16, r) << 16) | (ch(8, g) << 8) | ch(0, b);
            }
        };
        scale(&c.bnw_monitor, &mut self.mono_monitor);
        scale(&c.bnw_tv, &mut self.mono_tv);
        self.clear_overscan();
        self.palette = self.palette_for_type();
    }

    /// `ClearOverscanVideoArea` (NTSC.cpp:815).
    fn clear_overscan(&mut self) {
        const OFFSET_L: isize = 3;
        const SPAN_L: isize = 3;
        const OVERLAP_L: isize = SPAN_L - OFFSET_L;
        const OFFSET_R: isize = 2;
        const SPAN_R: isize = 4;
        const OVERLAP_R: isize = SPAN_R - OFFSET_R;
        const HORZ_PIXELS: isize = (MAX_HORZ - HORZ_START) as isize * 14;
        let line0 = self.scan_line(0) - FB_W as isize - OFFSET_L;
        for i in 0..(HORZ_PIXELS + (SPAN_L - OVERLAP_L) + (SPAN_R - OVERLAP_R)) {
            self.put(line0 + i, OPAQUE_BLACK);
        }
        for i in 0..(Y_DISPLAY as usize * 2) {
            let start = self.scan_line(i) - OFFSET_L;
            for j in 0..SPAN_L + 1 {
                self.put(start + j, OPAQUE_BLACK);
            }
            let start = start + OFFSET_L + HORZ_PIXELS - OFFSET_R;
            for j in 0..SPAN_R {
                self.put(start + j, OPAQUE_BLACK);
            }
        }
    }

    // ---- Mode selection (NTSC_SetVideoMode, NTSC.cpp:1996). ----

    fn apply_mode(&mut self, f: VideoFlags, fixups: bool) {
        self.new_flags = f;
        self.mixed = f.mixed;
        let sets = char_sets(self.model, f.jplus_ascii, self.cfg.video_rom_rocker);
        self.csets = sets;
        self.char_set = (f.altcharset as usize).min(sets.len() - 1);
        self.rgb.text_fb = false;
        self.text_page2 = false;
        self.hires_page2 = false;
        if f.page2 && !f.store80 {
            // Apple IIe Technical Note #3: 80STORE must be off to display page 2.
            self.text_page2 = true;
            self.hires_page2 = true;
        }
        if self.pal && fixups {
            self.pal_mid_line_fixups(f);
        }
        self.reselect_screen_funcs_for(f);
    }

    /// The 50 Hz branch of `NTSC_SetVideoMode` (GH#763).
    fn pal_mid_line_fixups(&mut self, f: VideoFlags) {
        let monitor = self.cfg.video_type == VideoType::ColorMonitorNtsc;
        let gfx_text = self.gfx.is_text();
        if f.text {
            if self.cfg.video_type != VideoType::ColorTv {
                self.color_burst = 0; // instantaneously kill the colour burst
            }
            if monitor && !gfx_text {
                self.put(self.addr, 0);
                self.put(self.addr + 1, 0);
                self.addr += 2;
            }
        } else {
            if self.mixed && self.vert >= Y_MIXED {
                self.color_burst = 0;
            }
            if monitor && gfx_text {
                self.addr -= 2;
            }
        }
    }

    fn reselect_screen_funcs_for(&mut self, f: VideoFlags) {
        let vt = self.cfg.video_type;
        let rgb_card = vt == VideoType::ColorVideocardRgb;
        let ideal = vt == VideoType::ColorIdealized;
        self.rgb.text_fb = false;
        // Video7 SL7 extra modes (the DHIRES switch on means regular //e modes are off).
        if rgb_card
            && self.cfg.rgb_card == RgbCard::Video7Sl7
            && !(!f.dhires || (f.dhires && !f.text && f.col80))
        {
            self.rgb.text_fb = true;
            if f.text {
                self.gfx = if f.col80 { Screen::Text80Rgb } else { Screen::Text40Rgb };
            } else if f.hires {
                self.gfx = Screen::SingleHires40Duochrome;
                self.txt = Screen::Text40Rgb;
            } else if f.col80 {
                self.gfx = Screen::DoubleLores80Simplified;
                self.txt = Screen::Text80Rgb;
            } else {
                self.gfx = Screen::SingleLores40Simplified;
                self.txt = Screen::Text40Rgb;
            }
            return;
        }
        self.gfx = if f.text {
            match (f.col80, rgb_card) {
                (true, true) => Screen::Text80Rgb,
                (true, false) => Screen::Text80,
                (false, true) => Screen::Text40Rgb,
                (false, false) => Screen::Text40,
            }
        } else if f.hires {
            if f.dhires {
                if f.col80 {
                    if ideal {
                        Screen::DoubleHires80Simplified
                    } else if rgb_card {
                        Screen::DoubleHires80Rgb
                    } else {
                        Screen::DoubleHires80
                    }
                } else if ideal {
                    Screen::Hires40Simplified
                } else {
                    Screen::DoubleHires40
                }
            } else if ideal {
                Screen::Hires40Simplified
            } else if rgb_card {
                Screen::SingleHires40Rgb
            } else {
                Screen::SingleHires40
            }
        } else if f.dhires {
            if f.col80 {
                if ideal || rgb_card {
                    Screen::DoubleLores80Simplified
                } else {
                    Screen::DoubleLores80
                }
            } else {
                Screen::DoubleLores40
            }
        } else if ideal || rgb_card {
            Screen::SingleLores40Simplified
        } else {
            Screen::SingleLores40
        };
    }

    // ---- Scanner. ----

    /// `g_pScanLines[k]`: index of pixel 0 of framebuffer scan line `k` (0-383).
    fn scan_line(&self, k: usize) -> isize {
        ((BORDER_H + k) * FB_W + BORDER_W) as isize
    }

    pub(crate) fn put(&mut self, idx: isize, v: u32) {
        if let Some(p) = usize::try_from(idx).ok().and_then(|i| self.fb.get_mut(i)) {
            *p = v;
        }
    }

    pub(crate) fn get(&self, idx: isize) -> u32 {
        usize::try_from(idx).ok().and_then(|i| self.fb.get(i)).copied().unwrap_or(0)
    }

    /// `updateVideoScannerAddress` (NTSC.cpp:748).
    fn update_video_scanner_address(&mut self) {
        let vt = self.cfg.video_type;
        if self.mixed && self.pal {
            // GH#763
            if self.vert >= Y_MIXED {
                self.color_burst = 0;
            } else if self.vert == 0 && !self.new_flags.text {
                self.color_burst = 1024;
            }
        }
        self.addr = self.scan_line(if self.vert < Y_DISPLAY { 2 * self.vert as usize } else { 0 });
        if vt == VideoType::MonoTv || vt == VideoType::ColorTv || (vt == VideoType::ColorMonitorNtsc && self.color_burst_on())
        {
            self.addr -= 2;
        }
        // GH#555: the 14M video modes start one 14M pixel early.
        let g = self.gfx;
        let wide = matches!(g, Screen::DoubleHires80 | Screen::DoubleLores80 | Screen::Text80 | Screen::Text80Rgb)
            || (self.mixed && self.vert >= Y_MIXED && (self.txt == Screen::Text80 || g == Screen::Text80Rgb));
        if wide && vt != VideoType::ColorIdealized && vt != VideoType::ColorVideocardRgb {
            self.addr -= 1;
        }
        self.color_phase = 0;
        self.last_col_pixel = 0;
        self.signal_bits = 0;
    }

    fn color_burst_on(&self) -> bool {
        self.color_burst >= 2
    }

    fn update_flash_rate(&mut self) {
        // Flash rate: NTSC 60/16 ~ 4 Hz; PAL 50/16 ~ 3 Hz.
        self.flash_counter = self.flash_counter.wrapping_add(1);
        if self.flash_counter & 0xF == 0 {
            self.flash_mask ^= 0xFFFF;
        }
    }

    /// `updateVideoScannerHorzEOLSimple`.
    fn eol_simple(&mut self) {
        self.horz += 1;
        if self.horz == MAX_HORZ {
            if self.vert < Y_DISPLAY {
                self.put(self.addr, OPAQUE_BLACK);
                self.put(self.addr + FB_W as isize, OPAQUE_BLACK);
            }
            self.horz = 0;
            self.vert += 1;
            if self.vert == self.max_vert {
                self.vert = 0;
                self.update_flash_rate();
            }
            if self.vert < Y_DISPLAY {
                self.update_video_scanner_address();
            }
        }
    }

    /// `updateVideoScannerHorzEOL`.
    fn eol(&mut self) {
        self.horz += 1;
        if self.horz == MAX_HORZ {
            if self.vert < Y_DISPLAY {
                let last = self.last_col_pixel;
                if !self.color_burst_on() {
                    self.dot(last, self.bnw_fn);
                    self.dot(0, self.bnw_fn);
                    self.put(self.addr, OPAQUE_BLACK);
                    self.addr += 1;
                    self.put(self.addr, OPAQUE_BLACK);
                    self.addr += 1;
                } else {
                    self.dot(last, self.hue_fn);
                    self.dot(0, self.hue_fn);
                    for _ in 0..2 {
                        self.put(self.addr, OPAQUE_BLACK);
                        self.put(self.addr + FB_W as isize, OPAQUE_BLACK);
                        self.addr += 1;
                    }
                }
            }
            self.horz = 0;
            self.vert += 1;
            if self.vert == self.max_vert {
                self.vert = 0;
                self.update_flash_rate();
            }
            if self.vert < Y_DISPLAY {
                self.update_video_scanner_address();
            }
        }
    }

    /// `getVideoScannerAddressTXT`.
    fn addr_txt(&self) -> u16 {
        let sw = Switches { text: true, mixed: false, hires: false, page2: self.text_page2 };
        self.scanner.address_at(self.vert, self.horz, sw)
    }

    /// `getVideoScannerAddressHGR`.
    fn addr_hgr(&self) -> u16 {
        let sw = Switches { text: false, mixed: false, hires: true, page2: self.hires_page2 };
        self.scanner.address_at(self.vert, self.horz, sw)
    }

    // ---- Pixel output (NTSC.cpp:511-650). ----

    fn dot(&mut self, signal: u32, f: PixFn) {
        let c = chroma();
        self.signal_bits = ((self.signal_bits << 1) | signal as usize) & 0xFFF;
        let sb = self.signal_bits;
        let color0 = match (f.tv, f.mono) {
            (false, true) => self.mono_monitor[sb],
            (true, true) => self.mono_tv[sb],
            (false, false) => c.hue_monitor[self.color_phase][sb],
            (true, false) => c.hue_tv[self.color_phase][sb],
        };
        let a = self.addr;
        let fw = FB_W as isize;
        if f.tv {
            let color2 = self.get(a - 2 * fw);
            let mut color1 = blend_half(color0) + blend_half(color2);
            if f.single {
                color1 = (color1 & 0x00FE_FEFE) >> 1; // then 50% brightness for the in-between line
            }
            self.put(a - fw, color1 | ALPHA);
            self.put(a, color0);
            // GH#650: draw the final in-between scan line to avoid residue from other modes.
            if self.vert == Y_DISPLAY - 1 {
                let last = if f.single { (color0 & 0x00FC_FCFC) >> 2 } else { (color0 & 0x00FE_FEFE) >> 1 };
                self.put(a + fw, last | ALPHA);
            }
        } else {
            let below = if f.single { OPAQUE_BLACK } else { color0 }; // no blending (GH#631)
            self.put(a + fw, below);
            self.put(a, color0);
        }
        self.addr += 1;
        self.color_phase = (self.color_phase + 1) & 3;
    }

    /// `updatePixels`: 14 dots, bit 0 first.
    fn update_pixels(&mut self, bits: u16) {
        let f = if self.color_burst_on() { self.hue_fn } else { self.bnw_fn };
        for k in 0..14 {
            self.dot(((bits >> k) & 1) as u32, f);
        }
        self.last_col_pixel = ((bits >> 13) & 1) as u32;
    }

    /// `update7MonoPixels`.
    pub(crate) fn update_7_mono_pixels(&mut self, mut bits: u16) {
        let f = self.bnw_fn;
        for _ in 0..7 {
            self.dot((bits & 1) as u32, f);
            bits >>= 1;
        }
    }

    // ---- Screen updaters. ----

    /// `VideoUpdateCycles` (NTSC.cpp:2476).
    fn video_update_cycles(&mut self, s: &dyn VideoSource, mut left: i32) {
        let frame = (self.max_vert * MAX_HORZ) as i32;
        let to_eol = (MAX_HORZ - self.horz) as i32;
        let g = self.gfx;
        if self.vert < Y_MIXED {
            let to_160 = MAX_HORZ as i32 * (Y_MIXED - self.vert - 1) as i32 + to_eol;
            let mut c = left.min(to_160);
            self.run(s, g, c);
            left -= c;
            let from_160 = frame - (MAX_HORZ * Y_MIXED) as i32;
            c = left.min(from_160);
            let g = self.gfx;
            self.run(s, g, c);
            left -= c;
        } else {
            let to_end = MAX_HORZ as i32 * (self.max_vert - self.vert - 1) as i32 + to_eol;
            let mut c = left.min(to_end);
            self.run(s, g, c);
            left -= c;
            let upto = (MAX_HORZ * Y_MIXED) as i32;
            c = left.min(upto);
            let g = self.gfx;
            self.run(s, g, c);
            left -= c;
        }
        if left != 0 {
            let g = self.gfx;
            self.run(s, g, left);
        }
    }

    fn run(&mut self, s: &dyn VideoSource, g: Screen, cycles: i32) {
        match g {
            Screen::Text40 => self.text40(s, cycles),
            Screen::Text40Rgb => self.text40_rgb(s, cycles),
            Screen::Text80 => self.text80(s, cycles),
            Screen::Text80Rgb => self.text80_rgb(s, cycles),
            _ => {
                if self.mixed && self.vert >= Y_MIXED {
                    let t = self.txt;
                    self.run(s, t, cycles);
                } else {
                    self.graphics(s, g, cycles);
                }
            }
        }
    }

    /// The shared loop of the graphics updaters: colour burst bookkeeping, then `draw` for each
    /// visible cell.
    fn graphics(&mut self, s: &dyn VideoSource, g: Screen, cycles: i32) {
        let simple_eol = matches!(
            g,
            Screen::DoubleHires80Simplified
                | Screen::DoubleHires80Rgb
                | Screen::DoubleLores80Simplified
                | Screen::Hires40Simplified
                | Screen::SingleHires40Duochrome
                | Screen::SingleHires40Rgb
                | Screen::SingleLores40Simplified
        );
        for _ in 0..cycles {
            if self.vert < Y_DISPLAY {
                if (COLORBURST_BEG..COLORBURST_END).contains(&self.horz) {
                    self.color_burst = 1024;
                } else if self.horz >= HORZ_START {
                    self.draw(s, g);
                }
            }
            if simple_eol {
                self.eol_simple();
            } else {
                self.eol();
            }
        }
    }

    fn draw(&mut self, s: &dyn VideoSource, g: Screen) {
        let x = (self.horz - HORZ_START) as usize;
        let y = self.vert as usize;
        match g {
            Screen::DoubleHires40 => {
                let a = self.addr_hgr();
                let bits = self.double_mask[(s.main(a) & 0x7F) as usize];
                self.update_pixels(bits);
            }
            Screen::DoubleHires80 => {
                let a = self.addr_hgr();
                let (m, au) = (s.main(a) as u16, s.aux(a) as u16);
                let mut bits = ((m & 0x7F) << 7) | (au & 0x7F);
                bits = (bits << 1) | self.last_col_pixel as u16;
                self.update_pixels(bits);
                self.last_col_pixel = ((bits >> 14) & 1) as u32;
            }
            Screen::DoubleHires80Simplified => {
                let a = self.addr_hgr();
                self.dhires_cell(s, x, a, true, true);
                self.addr += 14;
            }
            Screen::DoubleHires80Rgb => {
                let a = self.addr_hgr();
                let (mut au, mut m) = (s.aux(a), s.main(a));
                if self.rgb_mix_mode() && self.cfg.rgb_invert_bit7 {
                    au ^= 0x80;
                    m ^= 0x80;
                }
                if self.rgb_160_mode() {
                    let w = self.dhires160_cell(s, x, a);
                    self.addr += w;
                } else if self.rgb_560_mode() {
                    self.update_7_mono_pixels(au as u16);
                    self.update_7_mono_pixels(m as u16);
                } else {
                    let invert = self.rgb_mix_mode() && self.cfg.rgb_invert_bit7;
                    self.dhires_cell_rgb(s, x, a, self.rgb_mix_mode(), invert);
                    self.addr += 14;
                }
            }
            Screen::DoubleLores40 => {
                let a = self.addr_txt();
                let lo = self.lores_bits(s.main(a));
                let sh = (1 - (self.horz & 1)) * 2;
                let bits = self.double_mask[(((lo >> sh) & 0xFF) & 0x7F) as usize];
                self.update_pixels(bits);
            }
            Screen::DoubleLores80 => {
                let a = self.addr_txt();
                let lo = self.lores_bits(s.main(a));
                let hi = self.lores_bits(s.aux(a));
                let sh = (1 - (self.horz & 1)) * 2 + 3;
                let (main, aux) = (lo >> sh, hi >> sh);
                let bits = (main << 7) | (aux & 0x7F);
                self.update_pixels(bits);
                self.last_col_pixel = ((bits >> 14) & 1) as u32;
            }
            Screen::DoubleLores80Simplified => {
                let a = self.addr_txt();
                self.dlores_cell(s, x, y, a);
                self.addr += 14;
            }
            Screen::Hires40Simplified => {
                let a = self.addr_hgr();
                self.hires_cell(s, x, y, a);
                self.addr += 14;
            }
            Screen::SingleHires40Duochrome => {
                let a = self.addr_hgr();
                self.hires_duochrome_cell(s, a);
                self.addr += 14;
            }
            Screen::SingleHires40Rgb => {
                let a = self.addr_hgr();
                self.hires_rgb_cell(s, x, a);
                self.addr += 14;
            }
            Screen::SingleHires40 => {
                let a = self.addr_hgr();
                let m = s.main(a);
                let mut bits = self.double_mask[(m & 0x7F) as usize];
                if m & 0x80 != 0 {
                    bits = (bits << 1) | self.last_col_pixel as u16;
                }
                self.update_pixels(bits);
                // GH#555: no third 14M pixel at the end of the line.
                if self.horz == MAX_HORZ - 1 {
                    self.last_col_pixel = 0;
                }
            }
            Screen::SingleLores40Simplified => {
                let a = self.addr_txt();
                self.lores_cell(s, x, y, a);
                self.addr += 14;
            }
            Screen::SingleLores40 => {
                let a = self.addr_txt();
                let lo = self.lores_bits(s.main(a));
                let bits = lo >> ((1 - (self.horz & 1)) * 2);
                self.update_pixels(bits);
            }
            Screen::Text40 | Screen::Text40Rgb | Screen::Text80 | Screen::Text80Rgb => {}
        }
    }

    /// `getLoResBits`.
    fn lores_bits(&self, byte: u8) -> u16 {
        self.pixel_mask_gr[((byte >> (self.vert & 4)) & 0xF) as usize]
    }

    /// `getCharSetBits`.
    fn char_bits(&self, c: u8) -> u8 {
        self.csets[self.char_set][c as usize][(self.vert & 7) as usize]
    }

    /// The text updaters' colour burst bookkeeping; true if a cell should be drawn.
    fn text_cell_due(&mut self) -> bool {
        if (COLORBURST_BEG..COLORBURST_END).contains(&self.horz) {
            if self.color_burst > 0 {
                self.color_burst -= 1;
            }
            false
        } else {
            self.vert < Y_DISPLAY && self.horz >= HORZ_START
        }
    }

    fn flashes(&self, c: u8) -> bool {
        self.char_set == 0 && c & 0xC0 == 0x40 // flash only if MouseText is not active
    }

    fn text40(&mut self, s: &dyn VideoSource, cycles: i32) {
        for _ in 0..cycles {
            let a = self.addr_txt();
            if self.text_cell_due() {
                let m = s.main(a);
                let c = self.char_bits(m);
                let mut bits = self.double_mask[(c & 0x7F) as usize];
                if self.flashes(m) {
                    bits ^= self.flash_mask;
                }
                self.update_pixels(bits);
            }
            self.eol();
        }
    }

    fn text40_rgb(&mut self, s: &dyn VideoSource, cycles: i32) {
        for _ in 0..cycles {
            let a = self.addr_txt();
            if self.text_cell_due() {
                let m = s.main(a);
                let mut c = self.char_bits(m);
                if self.flashes(m) {
                    c ^= self.flash_mask as u8;
                }
                self.text40_color_cell(s, a, c, m);
                self.addr += 14;
            }
            self.eol_simple();
        }
    }

    fn text80(&mut self, s: &dyn VideoSource, cycles: i32) {
        for _ in 0..cycles {
            let a = self.addr_txt();
            if self.text_cell_due() {
                let m = s.main(a);
                let mut au = s.aux(a);
                if self.new_flags.aux_empty {
                    au = s.floating_bus();
                }
                let mut main = self.char_bits(m) as u16;
                let mut aux = self.char_bits(au) as u16;
                if self.flashes(m) {
                    main ^= self.flash_mask;
                }
                if self.flashes(au) {
                    aux ^= self.flash_mask;
                }
                let mut bits = (main << 7) | (aux & 0x7F);
                let vt = self.cfg.video_type;
                if vt != VideoType::ColorIdealized && vt != VideoType::ColorVideocardRgb {
                    bits = (bits << 1) | self.last_col_pixel as u16; // GH#555: align TEXT80 with DHGR
                }
                self.update_pixels(bits);
                self.last_col_pixel = ((bits >> 14) & 1) as u32;
            }
            self.eol();
        }
    }

    fn text80_rgb(&mut self, s: &dyn VideoSource, cycles: i32) {
        for _ in 0..cycles {
            let a = self.addr_txt();
            if self.text_cell_due() {
                let (m, au) = (s.main(a), s.aux(a));
                let mut main = self.char_bits(m) as u16;
                let mut aux = self.char_bits(au) as u16;
                if self.flashes(m) {
                    main ^= self.flash_mask;
                }
                if self.flashes(au) {
                    aux ^= self.flash_mask;
                }
                self.text80_color_cell(aux as u8, au);
                self.addr += 7;
                self.text80_color_cell(main as u8, m);
                self.addr += 7;
                let bits = (main << 7) | (aux & 0x7F);
                self.last_col_pixel = ((bits >> 14) & 1) as u32;
            }
            self.eol();
        }
    }

    // ---- Accessors for the cell drawers (rgb.rs). ----

    pub(crate) fn fb_w(&self) -> isize {
        FB_W as isize
    }

    pub(crate) fn half_scanlines(&self) -> bool {
        self.cfg.half_scanlines
    }

    pub(crate) fn current_flags(&self) -> VideoFlags {
        self.flags
    }

    pub(crate) fn is_mixed(&self) -> bool {
        self.mixed
    }
}
