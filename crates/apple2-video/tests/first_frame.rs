//! Regression for the first frame after power-on (found by the AppleWin framebuffer differential,
//! docs/conformance/audit/video-differential.md, DIV-001).
//!
//! AppleWin sets the scanner address of line 0 from the power-on mode (TEXT40) in
//! `NTSC_VideoInit`; a mode switched by the very first opcode changes the offset only from the
//! next line on. The Renderer used to configure itself lazily in its first `update`, after that
//! opcode, so line 0 of frame 1 started one pixel early in the 80-column modes, and the residue
//! (the Color TV "in-between" scan line above line 0 is never rewritten) differed from AppleWin.

use apple2_core::{Machine, MachineConfig, Model};
use apple2_video::{RenderConfig, Renderer, VideoType};

fn run_80col_text(first_frame_only: bool) -> Renderer {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    // $0300: STA $C00D (80COL on); STA $C051 (TEXT); JMP $0306
    for (i, b) in [0x8D, 0x0D, 0xC0, 0x8D, 0x51, 0xC0, 0x4C, 0x06, 0x03]
        .iter()
        .enumerate()
    {
        m.poke_main(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    // Solid text (inverse spaces), so the last pixels of every line are lit.
    for row in 0..24u16 {
        for col in 0..40u16 {
            let a = 0x400 + (row % 8) * 0x80 + (row / 8) * 0x28 + col;
            m.poke_main(a, 0x20); // inverse space: every dot lit
            m.poke_aux(a, 0x20);
        }
    }
    let mut r = Renderer::new(RenderConfig {
        video_type: VideoType::ColorTv,
        ..Default::default()
    });
    r.resync(m.cycles_in_frame());
    r.run_frame(&mut m);
    if !first_frame_only {
        r.run_frame(&mut m);
    }
    r
}

#[test]
fn line_zero_of_the_first_frame_starts_at_the_power_on_text40_offset() {
    let r = run_80col_text(true);
    let (w, _) = r.size();
    let at = |x: usize, y: usize| r.pixels()[y * w + x];
    // Line 0 is framebuffer row 18. With the power-on offset (-2 for Color TV, no -1 for 80 columns
    // yet) its 560 pixels end at x=577 and the end-of-line "ringing" pixels land on 578 and 579; the
    // in-between row above (17) receives the same two pixels, so (579, 17) is lit.
    assert_ne!(
        at(579, 17),
        0xFF00_0000,
        "residue of the first line 0 above the picture"
    );
    // The later lines of the frame already use the 80-column offset (-3).
    assert_eq!(at(579, 19 + 2), 0xFF00_0000);
}

#[test]
fn from_the_second_frame_line_zero_uses_the_80_column_offset() {
    let r = run_80col_text(false);
    let (w, _) = r.size();
    let at = |x: usize, y: usize| r.pixels()[y * w + x];
    // The residue of frame 1 stays (the row above line 0 is not redrawn at x=579); line 0 itself
    // now ends one pixel earlier.
    assert_eq!(at(579, 18), 0xFF00_0000);
    assert_ne!(at(578, 18), 0xFF00_0000);
}
