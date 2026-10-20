//! RGB card rendering (VT_COLOR_VIDEOCARD_RGB): text, hi-res, and the DHGR modes selected by
//! clocking AN3 (RGBMonitor.cpp RGB_SetVideoMode).

mod common;
use apple2_core::Model;
use apple2_video::chroma::chroma;
use apple2_video::{RenderConfig, Renderer, RgbCard, VideoType};
use common::*;

fn rgb_renderer(card: RgbCard) -> Renderer {
    Renderer::new(RenderConfig { video_type: VideoType::ColorVideocardRgb, rgb_card: card, ..Default::default() })
}

fn lores(n: usize) -> u32 {
    chroma().base_colors[n] & 0xFFFFFF
}

fn dhgr_fake() -> Fake {
    let mut s = Fake::new(Model::Apple2eEnhanced).hires();
    s.mode.col80 = true;
    s.mode.dhires = true;
    s
}

/// Clocks the card's mode register: AN3 on (`dhires` false->true) then off, with 80COL as given.
fn clock(r: &mut Renderer, s: &mut Fake, col80: bool) {
    s.mode.col80 = col80;
    s.mode.dhires = false;
    r.update(s, 1);
    s.mode.dhires = true;
    r.update(s, 1);
    s.mode.dhires = false;
    r.update(s, 1);
    s.mode.dhires = true;
    s.mode.col80 = true;
    r.update(s, 1);
}

#[test]
fn text_is_white_on_black_with_the_card_default_colors() {
    let mut s = Fake::new(Model::Apple2e).text();
    s.poke(0x400, &[0xC1]);
    let mut r = rgb_renderer(RgbCard::Video7Sl7);
    render(&mut r, &s, 1);
    // Glyph row 0 of the //e 'A': dot 3; each dot is two pixels; the 2nd row is black (scan lines).
    assert_eq!(art(&r, 0, 0, 14), "......##......");
    assert_eq!(rgb(&r, 6, 0), 0xFFFFFF);
    assert_eq!(rgb(&r, 0, 0), 0);
    assert_eq!(art(&r, 0, 1, 14), "..............");
}

#[test]
fn text_colors_follow_the_configured_foreground() {
    let mut s = Fake::new(Model::Apple2e).text();
    s.poke(0x400, &[0xC1]);
    let mut r = Renderer::new(RenderConfig {
        video_type: VideoType::ColorVideocardRgb,
        rgb_text_fg: 12, // green
        rgb_text_bg: 2,  // dark blue
        ..Default::default()
    });
    render(&mut r, &s, 1);
    assert_eq!(rgb(&r, 6, 0), lores(12));
    assert_eq!(rgb(&r, 0, 0), lores(2));
}

#[test]
fn video7_renders_inverse_text_as_black_and_white_ignoring_the_colors() {
    let mut s = Fake::new(Model::Apple2e).text();
    s.poke(0x400, &[0x01]); // inverse 'A'
    let cfg = RenderConfig { video_type: VideoType::ColorVideocardRgb, rgb_text_fg: 12, rgb_text_bg: 2, ..Default::default() };
    let mut sl7 = Renderer::new(cfg.clone());
    render(&mut sl7, &s, 1);
    assert_eq!(rgb(&sl7, 0, 0), 0xFFFFFF, "SL7: inverse is B&W");
    assert_eq!(rgb(&sl7, 6, 0), 0);
    let mut eve = Renderer::new(RenderConfig { rgb_card: RgbCard::LeChatMauveEve, ..cfg });
    render(&mut eve, &s, 1);
    // Inverse 'A': the dots around the glyph are the lit ones.
    assert_eq!(rgb(&eve, 0, 0), lores(12), "EVE keeps the text colours");
    assert_eq!(rgb(&eve, 6, 0), lores(2));
}

#[test]
fn dhgr_default_mode_is_the_140_pixel_colour_mode() {
    let mut s = dhgr_fake();
    s.poke_aux(0x2000, &[0x01]);
    let mut r = rgb_renderer(RgbCard::Video7Sl7);
    render(&mut r, &s, 1);
    // Nibble 0001 -> colour 2 (DHGR colours are rotated one bit): four dots wide, no fringe.
    for x in 0..4 {
        assert_eq!(rgb(&r, x, 0), lores(2), "x={x}");
    }
    assert_eq!(rgb(&r, 4, 0), 0);
}

#[test]
fn clocking_an3_selects_160_mixed_and_560_modes() {
    // Two clocks with !80COL: 00 -> 01 -> 11 (560 B&W). !80COL then 80COL: 01 -> 10 (mixed).
    let mut r = rgb_renderer(RgbCard::Video7Sl7);
    let mut s = dhgr_fake();
    s.poke_aux(0x2000, &[0x55]);
    s.poke(0x2000, &[0x2A]);
    r.update(&s, 1);
    clock(&mut r, &mut s, false); // mode 1: 160 colours
    render(&mut r, &s, 1);
    let m160: Vec<u32> = (0..28).map(|x| rgb(&r, x, 0)).collect();
    clock(&mut r, &mut s, false); // mode 3: 560 B&W
    render(&mut r, &s, 1);
    let m560: Vec<u32> = (0..14).map(|x| rgb(&r, x, 0)).collect();
    // 560 mode: pure 1-bit pixels (the monochrome monitor white), aux dots then main dots.
    assert!(m560.iter().all(|&c| c == 0 || c == 0xFEFEFE), "{m560:x?}");
    let aux_dots: Vec<bool> = (0..7).map(|i| 0x55 >> i & 1 != 0).collect();
    for (i, on) in aux_dots.iter().enumerate() {
        assert_eq!(m560[i] != 0, *on, "aux dot {i}");
    }
    // The 160 mode used the colour source image, so it has colours.
    assert!(m160.iter().any(|&c| c != 0 && c != 0xFFFFFF));
}

#[test]
fn mixed_mode_uses_bit_7_to_pick_bw_or_colour_per_byte() {
    let mut r = rgb_renderer(RgbCard::Video7Sl7);
    let mut s = dhgr_fake();
    // Aux byte 0 has bit 7 clear (B&W), main byte 0 has it set (colour).
    s.poke_aux(0x2000, &[0x7F]);
    s.poke(0x2000, &[0x80 | 0x7F]);
    r.update(&s, 1);
    clock(&mut r, &mut s, false); // flags 01
    clock(&mut r, &mut s, true); // flags 10 = mixed
    render(&mut r, &s, 1);
    // First 7 pixels: B&W (all dots on -> white). Then colour cell 1 (all ones -> white too, but
    // via the colour path: nibble 1111 = colour 15 = white).
    for x in 0..7 {
        assert_eq!(rgb(&r, x, 0), 0xFFFFFF);
    }
    assert_eq!(rgb(&r, 10, 0), 0xFFFFFF);
}

#[test]
fn feline_has_its_own_palette_and_no_160_mode() {
    let mut s = Fake::new(Model::Apple2e).lores();
    s.poke(0x400, &[0x09]); // lo-res orange
    let mut r = rgb_renderer(RgbCard::LeChatMauveFeline);
    render(&mut r, &s, 1);
    assert_eq!(rgb(&r, 3, 0), 0xFF7247);
    let mut sl7 = rgb_renderer(RgbCard::Video7Sl7);
    render(&mut sl7, &s, 1);
    assert_eq!(rgb(&sl7, 3, 0), lores(9));
    assert_ne!(lores(9), 0xFF7247);
}

#[test]
fn hires_isolated_dots_are_colored_and_runs_are_white() {
    let mut s = Fake::new(Model::Apple2e).hires();
    s.poke(0x2000, &[0x01, 0x00, 0x7F]);
    let mut r = rgb_renderer(RgbCard::Video7Sl7);
    render(&mut r, &s, 1);
    // An isolated dot is a colour pixel (violet = NTSC magenta), a run of dots is white.
    assert_eq!(rgb(&r, 0, 0), lores(3));
    assert_eq!(rgb(&r, 28 + 6, 0), 0xFFFFFF);
    assert_eq!(rgb(&r, 14 + 2, 0), 0);
}

#[test]
fn video7_extra_modes_use_aux_memory_for_foreground_and_background() {
    // AN3 off (DHIRES set) with TEXT and not 80COL: 40-column F/B text; colours come from aux.
    let mut s = Fake::new(Model::Apple2e).text();
    s.mode.dhires = true;
    s.poke(0x400, &[0xC1]);
    s.poke_aux(0x400, &[0x4B]); // fg 4 (dark green), bg 11 (pink)
    let mut r = rgb_renderer(RgbCard::Video7Sl7);
    render(&mut r, &s, 1);
    assert_eq!(rgb(&r, 6, 0), lores(4));
    assert_eq!(rgb(&r, 0, 0), lores(11));
}

#[test]
fn hires_duochrome_in_the_video7_extra_modes() {
    // HIRES on, TEXT off, not 80COL, DHIRES set: F/B hi-res, colours from aux.
    let mut s = Fake::new(Model::Apple2e).hires();
    s.mode.dhires = true;
    s.poke(0x2000, &[0x01]);
    s.poke_aux(0x2000, &[0x62]); // fg 6, bg 2
    let mut r = rgb_renderer(RgbCard::Video7Sl7);
    render(&mut r, &s, 1);
    assert_eq!(rgb(&r, 0, 0), lores(6));
    assert_eq!(rgb(&r, 2, 0), lores(2));
}
