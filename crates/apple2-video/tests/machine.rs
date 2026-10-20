//! Rendering a real booted [`Machine`], and mid-frame video mode changes.

mod common;
use apple2_core::{Machine, MachineConfig, Model, VideoStandard};
use apple2_video::screen_text::screen_text;
use apple2_video::{RenderConfig, Renderer, VideoType};
use common::*;

fn booted(model: Model) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(model));
    for _ in 0..300 {
        m.run_frame();
        if screen_text(&m).iter().any(|r| r.starts_with(']') || r.starts_with('*')) && m.frames() > 90 {
            break;
        }
    }
    m
}

/// True if any pixel of the 16 pixel rows of text row `row` is lit.
fn text_row_lit(r: &Renderer, row: usize) -> bool {
    (16 * row..16 * row + 16).any(|y| (0..560).any(|x| lit(r, x, y)))
}

#[test]
fn boot_banner_and_prompt_rows_are_lit_and_blank_rows_are_dark() {
    for model in [Model::Apple2Plus, Model::Apple2e, Model::Apple2eEnhanced] {
        for vt in VideoType::ALL {
            let mut m = booted(model);
            let mut r = Renderer::new(RenderConfig { video_type: vt, ..Default::default() });
            r.run_frame(&mut m);
            r.run_frame(&mut m);
            let rows = screen_text(&m);
            // The banner is on row 0, the prompt on row 2 and the rest of the screen is blank.
            assert!(rows[0].trim().len() >= 8 && rows[2].starts_with(']'), "{model:?}: {rows:?}");
            assert!(text_row_lit(&r, 0), "{model:?} {vt:?} banner");
            assert!(text_row_lit(&r, 2), "{model:?} {vt:?} prompt");
            // (Not row 1 or 3: the TV in-between scan line takes half of the line below.)
            for blank in [10, 22] {
                assert!(!text_row_lit(&r, blank), "{model:?} {vt:?} blank row {blank}");
            }
        }
    }
}

#[test]
fn machine_run_frame_renders_the_text_screen_row_by_row() {
    let mut m = booted(Model::Apple2Plus);
    let mut r = Renderer::new(RenderConfig { video_type: VideoType::MonoWhite, ..Default::default() });
    r.run_frame(&mut m);
    // Compare the banner against what the screen memory says: 'A' in "APPLE ][" is the first lit
    // glyph; check the left edge of the banner row has the glyph dots of a letter, not noise.
    let rows = screen_text(&m);
    let col = rows[0].find("APPLE ][").unwrap_or_else(|| panic!("banner on text row 0: {rows:?}"));
    // The banner's glyphs light pixels within their own 14-pixel cells and nowhere left of them.
    let x0 = col * 14;
    assert!((0..16).any(|y| (x0..x0 + 8 * 14).any(|x| lit(&r, x, y))));
    assert!(!(0..16).any(|y| (0..x0).any(|x| lit(&r, x, y))));
}

#[test]
fn rendering_does_not_depend_on_how_the_cycles_are_chunked() {
    // The same frame rendered from per-opcode updates and from a whole-frame redraw agree when
    // nothing changes mid-frame.
    let mut m = booted(Model::Apple2e);
    let cfg = RenderConfig { video_type: VideoType::MonoWhite, ..Default::default() };
    let mut a = Renderer::new(cfg.clone());
    let mut b = Renderer::new(cfg);
    b.redraw(&m);
    a.resync(m.cycles_in_frame());
    // Two whole frames of per-opcode rendering; keep the screen static by ignoring the keyboard.
    for _ in 0..2 {
        a.run_frame(&mut m);
    }
    b.redraw(&m);
    let (pa, pb) = (a.rgba8(), b.rgba8());
    // The cursor blink may differ; compare everything except the cursor row region by rows.
    let row_bytes = 560 * 4 * 16;
    let diff_rows = (0..24).filter(|&r| pa[r * row_bytes..(r + 1) * row_bytes] != pb[r * row_bytes..(r + 1) * row_bytes]).count();
    assert!(diff_rows <= 1, "{diff_rows} text rows differ");
}

#[test]
fn mid_frame_mode_change_splits_the_picture_at_the_cycle() {
    // Programs that flip TEXT/GR mid-frame (raster effects) show both modes in one frame: run the
    // machine for part of a frame, change the mode, finish the frame.
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    for a in 0x400u16..0x800 {
        m.poke_main(a, 0xA0);
    }
    for a in 0x2000u16..0x4000 {
        m.poke_main(a, 0x7F);
    }
    let mut r = Renderer::new(RenderConfig { video_type: VideoType::MonoWhite, ..Default::default() });
    // Frame 1: text. Frame 2: text until line 96, then switch to hi-res.
    r.run_frame(&mut m);
    let line = 96u32;
    m.board_mut().video.text = true;
    let target = line * 65 + 40;
    r.resync(m.cycles_in_frame());
    while m.cycles_in_frame() < target || m.cycles_in_frame() > 17000 {
        let c = m.step();
        r.update(&m, c);
    }
    m.board_mut().video.text = false;
    m.board_mut().memory.set_paging(0x57, 0); // HIRES on
    let target = m.frames() + 1;
    while m.frames() < target {
        let c = m.step();
        r.update(&m, c);
    }
    // Lines above 96: text (blank, spaces); below: solid hi-res.
    assert!(!lit(&r, 100, 2 * 40));
    assert!(lit(&r, 100, 2 * 150));
    assert!(lit(&r, 500, 2 * 150));
}

#[test]
fn pal_machines_render_312_lines() {
    let mut cfg = MachineConfig::for_model(Model::Apple2e);
    cfg.video_standard = VideoStandard::Pal;
    let mut m = Machine::new(cfg);
    for a in 0x400u16..0x800 {
        m.poke_main(a, 0xA0);
    }
    m.poke_main(0x400, 0xC1);
    let mut r = Renderer::new(RenderConfig { video_type: VideoType::MonoWhite, ..Default::default() });
    let cycles = r.run_frame(&mut m);
    assert!(cycles >= 20280);
    assert!(lit(&r, 6, 0));
}
