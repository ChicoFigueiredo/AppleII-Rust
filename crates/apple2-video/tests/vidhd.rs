//! VidHD: the larger framebuffer, centred legacy modes and Super Hi-Res (NTSC.cpp
//! updateScreenSHR / VidHD.cpp UpdateSHRCell), plus the debugger's pseudo pages.

mod common;
use apple2_core::Model;
use apple2_video::source::VideoFlags;
use apple2_video::{DebugView, VideoType};
use common::*;

fn shr_machine() -> Fake {
    let mut s = Fake::new(Model::Apple2Plus).text();
    s.vidhd = Some(FakeHd { shr: true });
    s
}

#[test]
fn a_vidhd_enlarges_the_framebuffer_to_the_iigs_size() {
    // Video.cpp:96-137: 640x400 inside the same 20/18 borders.
    let mut s = Fake::new(Model::Apple2Plus).text();
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    assert_eq!(r.size(), (600, 420));
    s.vidhd = Some(FakeHd { shr: false });
    render(&mut r, &s, 1);
    assert_eq!(r.size(), (680, 436));
    assert_eq!(r.visible_rect(), (20, 18, 640, 400));
    assert_eq!(r.rgba8().len(), 640 * 400 * 4);
}

#[test]
fn legacy_modes_are_centred_in_the_larger_framebuffer() {
    let mut s = Fake::new(Model::Apple2Plus).text();
    s.vidhd = Some(FakeHd { shr: false });
    s.poke(0x400, &[0xC1]); // 'A'
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    // (640-560)/2 = 40 pixels in and (400-384)/2 = 8 rows down.
    let rows = [
        ".......", "...#...", "..#.#..", ".#...#.", ".#...#.", ".#####.", ".#...#.", ".#...#.",
    ];
    for (y, dots) in rows.iter().enumerate() {
        let want: String = dots.chars().flat_map(|c| [c, c]).collect();
        assert_eq!(art(&r, 40, 8 + 2 * y, 14), want, "row {y}");
    }
}

#[test]
fn shr_320_cells_come_from_aux_memory_with_per_line_palette_and_control() {
    let mut s = shr_machine();
    s.poke_aux(0x2000, &[1, 2, 3, 4, 5, 6, 7, 8]); // line 0: cells 0 and 1
    s.poke_aux(0x20A0, &[9, 0, 0, 0]); // line 1 (160 bytes later)
    s.poke_aux(0x9D00, &[0x05]); // line 0: palette 5, 320 mode, no fill
    s.poke_aux(0x9D01, &[0xA0]); // line 1: 640 mode + fill, palette 0
    for i in 0..16u8 {
        s.poke_aux(0x9E00 + 5 * 32 + 2 * i as u16, &[0x40 + i, 0]);
        s.poke_aux(0x9E00 + 2 * i as u16, &[0x80 + i, 0]);
    }
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    let px = |x, y| r.pixel(x, y);
    for i in 0..16 {
        assert_eq!(
            px(i, 0),
            [1 + (i / 4) as u8, 0x40 + i as u8, 0, 255],
            "line 0 cell 0 pixel {i}"
        );
        assert_eq!(
            px(16 + i, 0),
            [5 + (i / 4) as u8, 0x40 + i as u8, 0, 255],
            "cell 1"
        );
    }
    // AppleWin draws IIgs line y on framebuffer row 2*y only (UpdateSHRCell writes one row);
    // the rows in between stay black (framebuffer differential, DIV-002).
    for x in 0..640 {
        assert_eq!(r.pixel(x, 1), [0, 0, 0, 255], "row 1, x {x}");
    }
    assert_eq!(
        px(0, 2),
        [9, 0x80, 3, 255],
        "line 1: 640 + fill flags, palette 0"
    );
    assert_eq!(px(639, 0)[2], 0);
}

#[test]
fn leaving_shr_clears_the_residue() {
    let mut s = shr_machine();
    s.poke_aux(0x2000, &[0xFF; 4]);
    for i in 0..16u16 {
        s.poke_aux(0x9E00 + 2 * i, &[0xFF, 0x0F]);
    }
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    assert!(lit(&r, 0, 0));
    s.vidhd = Some(FakeHd { shr: false });
    render(&mut r, &s, 1);
    assert!(!lit(&r, 0, 0));
    assert!(!lit(&r, 639, 399));
}

#[test]
fn debugger_pseudo_pages_view_other_hires_pages() {
    // NTSC.cpp:2039-2069 / Debug.cpp _ViewOutput: HGR page n starts at aPageAddr[n].
    let mut s = Fake::new(Model::Apple2Plus).hires();
    s.poke(0x8000, &[0x7F]);
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    assert!(!lit(&r, 0, 0), "page 1 ($2000) is empty");
    let flags = VideoFlags {
        hires: true,
        ..Default::default()
    };
    for (page, addr) in [
        (0u8, 0x0000u16),
        (3, 0x6000),
        (4, 0x8000),
        (5, 0xA000),
        (6, 0xC000),
        (7, 0xD000),
        (8, 0xE000),
    ] {
        let mut s2 = Fake::new(Model::Apple2Plus).hires();
        s2.poke(addr, &[0x7F]);
        let view = DebugView::new(&s2, flags, Some(page));
        let mut r = renderer(VideoType::MonoWhite);
        r.redraw(&view);
        assert!(lit(&r, 0, 0), "pseudo page {page}");
    }
    let view = DebugView::new(&s, flags, Some(4));
    r.redraw(&view);
    r.redraw(&view);
    assert!(lit(&r, 0, 0), "$8000 shown");
}

#[test]
fn debug_view_overrides_the_machine_switches() {
    // A text-mode machine viewed as HGR (the debugger's `hgr1`).
    let mut s = Fake::new(Model::Apple2Plus).text();
    s.poke(0x2000, &[0x7F]);
    let view = DebugView::new(
        &s,
        VideoFlags {
            hires: true,
            ..Default::default()
        },
        None,
    );
    let mut r = renderer(VideoType::MonoWhite);
    r.redraw(&view);
    r.redraw(&view);
    assert!(lit(&r, 0, 0));
}
