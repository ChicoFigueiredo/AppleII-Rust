//! Video output seam: [`FrameSource`] turns the machine state into an RGBA framebuffer.
//!
//! [`VideoRenderer`] adapts `apple2-video`; [`TextPlaceholder`] is a minimal text-only fallback
//! (kept for tests of the shell without the NTSC renderer).

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
        Frame {
            width,
            height,
            rgba: vec![0; (width * height * 4) as usize],
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if (self.width, self.height) != (width, height) {
            *self = Frame::new(width, height);
        }
    }
}

/// Runs the machine and renders the screen. Runs on the emulation thread.
///
/// The source owns the frame loop because `apple2_video::Renderer` must be fed after every
/// 6502 instruction (AppleWin's `NTSC_VideoUpdateCycles`) to be exact and deterministic.
pub trait FrameSource {
    /// Runs one video frame; returns the cycles executed.
    fn run_frame(&mut self, machine: &mut Machine) -> u64 {
        machine.run_frame()
    }
    /// Runs one instruction (debugger single-step); returns its cycles.
    fn step(&mut self, machine: &mut Machine) -> u32 {
        machine.step()
    }
    /// The machine was reset or power-cycled.
    fn reset(&mut self) {}
    /// Re-renders the whole screen from the current state (used while the debugger, not the
    /// frame loop, advances the machine).
    fn redraw(&mut self, _machine: &Machine) {}
    /// Copies the current screen into `frame` (resizing it if the source changes resolution).
    fn render(&mut self, machine: &Machine, frame: &mut Frame);
    /// F9 / Shift+F9: next/previous video type (colour, mono, ...).
    fn cycle_video_mode(&mut self, _delta: i32) {}
    /// Ctrl+Shift+F9.
    fn toggle_scanlines(&mut self) {}
    /// F10 on the //e: video ROM rocker switch.
    fn toggle_rocker_switch(&mut self) {}
    /// Human-readable description of the current video type, for the status line.
    fn description(&self) -> String {
        String::new()
    }
}

/// `apple2-video` as a [`FrameSource`].
pub struct VideoRenderer {
    renderer: apple2_video::Renderer,
}

impl VideoRenderer {
    pub fn new(cfg: apple2_video::RenderConfig) -> Self {
        VideoRenderer {
            renderer: apple2_video::Renderer::new(cfg),
        }
    }

    pub fn from_settings(s: &apple2_config::Settings) -> Self {
        let mut cfg = render_config(&s.video);
        // `-videorom`: a file that cannot be used keeps the built-in character generator (the
        // warning is reported by the emulator).
        cfg.user_video_rom = s.read_video_rom().map(std::sync::Arc::new);
        Self::new(cfg)
    }

    fn edit(&mut self, f: impl FnOnce(&mut apple2_video::RenderConfig)) {
        let mut c = self.renderer.config().clone();
        f(&mut c);
        self.renderer.set_config(c);
    }
}

/// Settings -> renderer options.
pub fn render_config(v: &apple2_config::settings::VideoSettings) -> apple2_video::RenderConfig {
    use apple2_config::settings::RgbCardType;
    use apple2_video::{RgbCard, VideoType};
    let m = v.monochrome_rgb;
    apple2_video::RenderConfig {
        video_type: VideoType::ALL[(v.mode as usize).min(VideoType::ALL.len() - 1)],
        half_scanlines: v.half_scanlines,
        vertical_blend: v.vertical_blend,
        monochrome_rgb: [(m >> 16) as u8, (m >> 8) as u8, m as u8],
        rgb_card: match v.rgb_card {
            RgbCardType::Video7Sl7 => RgbCard::Video7Sl7,
            RgbCardType::LeChatMauveEve => RgbCard::LeChatMauveEve,
            RgbCardType::LeChatMauveFeline => RgbCard::LeChatMauveFeline,
        },
        rgb_text_fg: v.rgb_card_foreground,
        rgb_text_bg: v.rgb_card_background,
        rgb_invert_bit7: v.rgb_card_invert_bit7,
        rgb_mac_lc_dlgr: v.mac_lc_card_dlgr,
        video_rom_rocker: false,
        user_video_rom: None,
    }
}

/// The VidHD card seen by the renderer (apple2-video does not depend on apple2-cards).
struct HdView<'a>(&'a apple2_cards::VidHd);

impl apple2_video::VidHdView for HdView<'_> {
    fn is_shr(&self) -> bool {
        self.0.is_shr()
    }
    fn shr_cell(
        &self,
        is_640: bool,
        color_fill: bool,
        palette: &[u16; 16],
        a: u32,
        prev: [u8; 3],
    ) -> [[u8; 3]; 16] {
        apple2_cards::vidhd::shr_cell(is_640, color_fill, palette, a, prev)
    }
}

/// The slot of the VidHD card, if the machine has one.
fn vidhd_slot(m: &Machine) -> Option<u8> {
    crate::factory::slot_of::<apple2_cards::VidHd>(m)
}

impl VideoRenderer {
    /// Feeds the renderer the cycles of one instruction, with the VidHD view when there is one.
    fn feed(&mut self, m: &Machine, slot: Option<u8>, cycles: u32) {
        match slot.and_then(|s| m.card::<apple2_cards::VidHd>(s)) {
            Some(hd) => {
                let view = HdView(hd);
                self.renderer.update(
                    &apple2_video::WithVidHd {
                        source: m,
                        vidhd: &view,
                    },
                    cycles,
                );
            }
            None => self.renderer.update(m, cycles),
        }
    }
}

impl FrameSource for VideoRenderer {
    fn run_frame(&mut self, m: &mut Machine) -> u64 {
        let slot = vidhd_slot(m);
        if slot.is_none() {
            return self.renderer.run_frame(m);
        }
        self.renderer.resync(m.cycles_in_frame());
        let target = m.frames() + 1;
        let mut done = 0u64;
        while m.frames() < target {
            let c = m.step();
            done += u64::from(c);
            self.feed(m, slot, c);
        }
        done
    }
    fn step(&mut self, m: &mut Machine) -> u32 {
        let c = m.step();
        let slot = vidhd_slot(m);
        self.feed(m, slot, c);
        c
    }
    fn reset(&mut self) {
        self.renderer.reset();
    }
    fn redraw(&mut self, m: &Machine) {
        match vidhd_slot(m).and_then(|s| m.card::<apple2_cards::VidHd>(s)) {
            Some(hd) => {
                let view = HdView(hd);
                self.renderer.redraw(&apple2_video::WithVidHd {
                    source: m,
                    vidhd: &view,
                });
            }
            None => self.renderer.redraw(m),
        }
    }
    fn render(&mut self, _m: &Machine, frame: &mut Frame) {
        // 560x384, or 640x400 with a VidHD card (Video.cpp:96-103)
        let (_, _, w, h) = self.renderer.visible_rect();
        frame.resize(w as u32, h as u32);
        frame.rgba = self.renderer.rgba8();
    }
    fn cycle_video_mode(&mut self, delta: i32) {
        self.edit(|c| {
            c.video_type = if delta >= 0 {
                c.video_type.next()
            } else {
                c.video_type.prev()
            }
        });
    }
    fn toggle_scanlines(&mut self) {
        self.edit(|c| c.half_scanlines = !c.half_scanlines);
    }
    fn toggle_rocker_switch(&mut self) {
        self.edit(|c| c.video_rom_rocker = !c.video_rom_rocker);
    }
    fn description(&self) -> String {
        self.renderer.config().video_type.description().to_string()
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
        TextPlaceholder {
            charset,
            is_e,
            fg: [0xE8, 0xE8, 0xE8],
        }
    }

    /// 7 pixel columns (bit 0 = leftmost); true = lit. `flash_on`: phase of FLASH characters.
    fn glyph(&self, code: u8, alt: bool, flash_on: bool) -> [u8; 8] {
        let mut rows = [0u8; 8];
        if self.is_e {
            // The ROM stores inverted pixels (1 = dark). First 256 characters = alternate set
            // (inverse, MouseText, inverse lowercase, normal).
            let idx = if alt || !(0x40..0x80).contains(&code) {
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
        let flash_on = (m.frames() / 16).is_multiple_of(2);
        let col80 = v.col80 && self.is_e;
        let alt = v.altcharset && self.is_e;
        let w = SCREEN_W as usize;
        frame.rgba.fill(0);
        for a in frame.rgba.iter_mut().skip(3).step_by(4) {
            *a = 255;
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
                        let (x0, nx) = if col80 {
                            (col * 7 + gx, 1)
                        } else {
                            (col * cell_w + gx * 2, 2)
                        };
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
}
