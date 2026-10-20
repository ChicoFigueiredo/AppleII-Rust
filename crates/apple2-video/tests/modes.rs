//! Golden checks per video mode against hand-computed dot patterns and palette values.

mod common;
use apple2_core::memory::mf;
use apple2_core::Model;
use apple2_video::VideoType;
use common::*;

#[test]
fn framebuffer_geometry_matches_applewin() {
    let r = renderer(VideoType::MonoWhite);
    // 560x384 visible inside 20-pixel side and 18-row top/bottom borders (Video.cpp:107-127).
    assert_eq!(r.size(), (600, 420));
    assert_eq!(r.visible_rect(), (20, 18, 560, 384));
    assert_eq!(r.pixels().len(), 600 * 420);
    assert_eq!(r.rgba8().len(), 560 * 384 * 4);
}

#[test]
fn text40_normal_inverse_and_scanline_pairing() {
    let mut s = Fake::new(Model::Apple2Plus).text();
    s.poke(0x400, &[0xC1, 0x01]); // 'A' normal, 'A' inverse
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    // 'A': rows 00 08 14 22 22 3E 22 22; each dot is two pixels wide; the second pixel row of
    // every Apple scan line is black with the default 50% scan lines.
    let rows = [".......", "...#...", "..#.#..", ".#...#.", ".#...#.", ".#####.", ".#...#.", ".#...#."];
    for (y, dots) in rows.iter().enumerate() {
        let want: String = dots.chars().flat_map(|c| [c, c]).collect();
        assert_eq!(art(&r, 0, 2 * y, 14), want, "row {y}");
        assert_eq!(art(&r, 0, 2 * y + 1, 14), "..............", "scan line {y}");
        // Inverse: all 7 dots inverted.
        let inv: String = dots.chars().flat_map(|c| if c == '#' { ['.', '.'] } else { ['#', '#'] }).collect();
        assert_eq!(art(&r, 14, 2 * y, 14), inv, "inverse row {y}");
    }
}

#[test]
fn full_scanlines_repeat_the_line() {
    let mut s = Fake::new(Model::Apple2Plus).text();
    s.poke(0x400, &[0xC1]);
    let mut r = apple2_video::Renderer::new(apple2_video::RenderConfig {
        video_type: VideoType::MonoWhite,
        half_scanlines: false,
        ..Default::default()
    });
    render(&mut r, &s, 1);
    for y in 0..8 {
        assert_eq!(art(&r, 0, 2 * y, 14), art(&r, 0, 2 * y + 1, 14));
    }
}

#[test]
fn flash_toggles_every_16_frames() {
    let mut s = Fake::new(Model::Apple2Plus).text();
    s.poke(0x400, &[0x41]); // flashing 'A'
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    let first = art(&r, 0, 2, 14); // glyph row 1 of 'A': dot 3
    assert_eq!(first, "......##......");
    render(&mut r, &s, 14);
    assert_eq!(art(&r, 0, 2, 14), first);
    render(&mut r, &s, 2);
    // The mask has toggled once the counter reaches 16.
    assert_eq!(art(&r, 0, 2, 14), "######..######");
    render(&mut r, &s, 16);
    assert_eq!(art(&r, 0, 2, 14), first);
}

#[test]
fn text80_interleaves_aux_then_main() {
    let mut s = Fake::new(Model::Apple2eEnhanced).text();
    s.mode.col80 = true;
    s.poke_aux(0x400, &[0xC1]);
    s.poke(0x400, &[0xC2]); // 'B'
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    // 7 single-width dots of the aux char, then the main char's; glyph row 0 of the //e 'A' is
    // dot 3 and of 'B' ($C2) the top bar. The 14M text80 modes are shifted one dot (GH#555) and
    // then back.
    let row1 = art(&r, 0, 0, 14);
    assert_eq!(&row1[..7], "...#...");
    assert_eq!(&row1[7..], ".####..");
}

#[test]
fn lores_colors_from_both_nibbles() {
    let mut s = Fake::new(Model::Apple2Plus).lores();
    s.poke(0x400, &[0xE5]); // top nibble E (aqua) bottom; low nibble 5 (grey) on scan lines 0-3
    let mut r = renderer(VideoType::ColorIdealized);
    render(&mut r, &s, 1);
    let c = apple2_video::chroma::chroma().base_colors;
    assert_eq!(rgb(&r, 3, 0), c[5] & 0xFFFFFF);
    assert_eq!(rgb(&r, 3, 2 * 3), c[5] & 0xFFFFFF);
    assert_eq!(rgb(&r, 3, 2 * 4), c[0xE] & 0xFFFFFF);
    assert_eq!(rgb(&r, 13, 2 * 7), c[0xE] & 0xFFFFFF);
    // The neighbouring cell is black (memory is 0).
    assert_eq!(rgb(&r, 14, 0), 0);
    // 50% scan lines blacken the second row of each line in the table-driven modes.
    assert_eq!(rgb(&r, 3, 1), 0);
}

#[test]
fn hires_isolated_dots_are_the_artifact_colors_in_idealized_mode() {
    let mut s = Fake::new(Model::Apple2Plus).hires();
    s.poke(0x2000, &[0x01, 0x81]);
    let mut r = renderer(VideoType::ColorIdealized);
    render(&mut r, &s, 1);
    let c = apple2_video::chroma::chroma().base_colors;
    // Violet = the NTSC magenta (lo-res 3) and blue = lo-res 6; one 14M dot pair wide.
    assert_eq!(rgb(&r, 0, 0), c[3] & 0xFFFFFF);
    assert_eq!(rgb(&r, 1, 0), c[3] & 0xFFFFFF);
    assert_eq!(rgb(&r, 2, 0), 0);
    // Odd column with bit 7 set: shifted half a dot right and orange (blue in an even column).
    assert_eq!(rgb(&r, 14 + 1, 0), c[9] & 0xFFFFFF);
    assert_eq!(rgb(&r, 14 + 2, 0), c[9] & 0xFFFFFF);
    assert_eq!(rgb(&r, 14, 0), 0);
}

#[test]
fn hires_adjacent_dots_are_white_in_idealized_mode() {
    let mut s = Fake::new(Model::Apple2Plus).hires();
    s.poke(0x2000, &[0x7F, 0x7F]);
    let mut r = renderer(VideoType::ColorIdealized);
    render(&mut r, &s, 1);
    assert_eq!(rgb(&r, 7, 0), 0xFFFFFF);
    assert_eq!(rgb(&r, 20, 0), 0xFFFFFF);
}

#[test]
fn hires_scanline_addresses_are_interleaved() {
    let mut s = Fake::new(Model::Apple2Plus).hires();
    s.poke(0x2000, &[0x7F]); // line 0
    s.poke(0x2400, &[0x7F]); // line 1
    s.poke(0x2080, &[0x7F]); // line 8
    s.poke(0x2028, &[0x7F]); // line 64
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    for line in [0usize, 1, 8, 64] {
        assert!(lit(&r, 3, 2 * line), "line {line}");
    }
    assert!(!lit(&r, 3, 2 * 2));
}

#[test]
fn page2_is_ignored_with_80store_and_selects_the_second_page_otherwise() {
    let mut s = Fake::new(Model::Apple2e).text();
    s.poke(0x400, &[0xC1]);
    s.poke(0x800, &[0xC2]);
    s.memmode |= mf::PAGE2;
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    assert_eq!(art(&r, 0, 0, 14), "..########....", "page 2 shows 'B'");
    s.memmode |= mf::STORE80;
    render(&mut r, &s, 1);
    assert_eq!(art(&r, 0, 0, 14), "......##......", "80STORE keeps page 1");
}

#[test]
fn mixed_mode_shows_four_text_rows_at_the_bottom() {
    let mut s = Fake::new(Model::Apple2Plus).hires();
    s.mode.mixed = true;
    for y in 0..192usize {
        let a = 0x2000 + (y % 8) * 0x400 + ((y / 8) % 8) * 0x80 + (y / 64) * 0x28;
        s.poke(a as u16, &[0x7F]);
    }
    s.poke(0x650, &[0xC1]); // text row 20 col 0 (y = 160)
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    // Graphics up to line 159: solid 14 pixels; from 160: the text glyph row 0 ('A': dot 3).
    assert_eq!(art(&r, 0, 2 * 159, 14), "##############");
    assert_eq!(art(&r, 0, 2 * 160, 14), "..............");
    assert_eq!(art(&r, 0, 2 * 161, 14), "......##......");
    assert_eq!(art(&r, 0, 2 * 162, 14), "....##..##....");
}

#[test]
fn dhires_idealized_solid_colours() {
    let mut s = Fake::new(Model::Apple2eEnhanced).hires();
    s.mode.col80 = true;
    s.mode.dhires = true;
    for a in 0x2000u16..0x2028 {
        s.poke(a, &[0x7F]);
        s.poke_aux(a, &[0x7F]);
    }
    let mut r = renderer(VideoType::ColorIdealized);
    render(&mut r, &s, 1);
    assert_eq!(rgb(&r, 7 + 14 * 3, 0), 0xFFFFFF);
    // All zero -> black.
    let s2 = Fake { ..Fake::new(Model::Apple2eEnhanced).hires() };
    let mut s2 = s2;
    s2.mode.col80 = true;
    s2.mode.dhires = true;
    let mut r2 = renderer(VideoType::ColorIdealized);
    render(&mut r2, &s2, 1);
    assert_eq!(rgb(&r2, 100, 0), 0);
}

#[test]
fn user_video_rom_replaces_the_character_generator() {
    // A 2K ][+ ROM that draws every character with a solid row 0 (bit 7 set = normal).
    let mut rom = vec![0u8; 2048];
    for c in 0..256 {
        rom[c * 8] = 0x7F;
    }
    let mut s = Fake::new(Model::Apple2Plus).text();
    s.poke(0x400, &[0xC1]);
    let mut r = apple2_video::Renderer::new(apple2_video::RenderConfig {
        video_type: VideoType::MonoWhite,
        user_video_rom: Some(std::sync::Arc::new(rom)),
        ..Default::default()
    });
    render(&mut r, &s, 1);
    // glyph row 0 of the built-in 'A' is blank; the user's is a full 7-dot bar
    assert_eq!(art(&r, 0, 0, 14), "##############");
}
