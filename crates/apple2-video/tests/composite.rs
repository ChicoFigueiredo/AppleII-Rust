//! Composite (NTSC) colour output: artifact colours, DHGR and the video-type styles.

mod common;
use apple2_core::Model;
use apple2_video::chroma::chroma;
use apple2_video::{RenderConfig, Renderer, VideoType};
use common::*;

fn channels(c: u32) -> (i32, i32, i32) {
    (
        ((c >> 16) & 0xFF) as i32,
        ((c >> 8) & 0xFF) as i32,
        (c & 0xFF) as i32,
    )
}

fn hires_line(bytes: &[u8]) -> Fake {
    let mut s = Fake::new(Model::Apple2Plus).hires();
    s.poke(0x2000, bytes);
    s
}

#[test]
fn hires_artifact_colors_on_the_composite_monitor() {
    // 0x55 = dots 0,2,4,6 on (even dots, bit 7 clear): violet. 0x2A = odd dots: green.
    // 0xD5 / 0xAA with bit 7 set: blue / orange.
    for vt in [VideoType::ColorMonitorNtsc, VideoType::ColorTv] {
        let mut r = renderer(vt);
        let mut s = hires_line(&[]);
        for col in 0..40 {
            s.poke(0x2000 + col, &[if col % 2 == 0 { 0x55 } else { 0x2A }]);
        }
        render(&mut r, &s, 1);
        let (red, green, blue) = channels(rgb(&r, 280, 0));
        assert!(
            red > green + 40 && blue > green + 40,
            "{vt:?} violet {red},{green},{blue}"
        );
        // Bit 7 set: the 0xD5 0xAA pair is blue then orange.
        for col in 0..40 {
            s.poke(0x2000 + col, &[if col % 2 == 0 { 0xD5 } else { 0xAA }]);
        }
        render(&mut r, &s, 1);
        let (red, _, blue) = channels(rgb(&r, 280, 0));
        assert!(blue > red + 60, "{vt:?} blue {red} {blue}");
        for col in 0..40 {
            s.poke(0x2000 + col, &[if col % 2 == 0 { 0xAA } else { 0xD5 }]);
        }
        render(&mut r, &s, 1);
        let (red, _, blue) = channels(rgb(&r, 280, 0));
        assert!(red > blue + 60, "{vt:?} orange {red} {blue}");
    }
}

#[test]
fn solid_white_hires_is_pure_white_without_chroma_ringing() {
    let mut s = hires_line(&[]);
    for col in 0..40 {
        s.poke(0x2000 + col, &[0x7F]);
    }
    let mut r = renderer(VideoType::ColorMonitorNtsc);
    render(&mut r, &s, 1);
    assert_eq!(rgb(&r, 280, 0), 0xFFFFFF);
}

#[test]
fn lores_colors_on_the_tv_are_the_base_palette_averaged_over_phases() {
    let mut s = Fake::new(Model::Apple2Plus).lores();
    // Colour n in the top half of every cell of text row 0.
    for n in 1..16u8 {
        for col in 0..40 {
            s.poke(0x400 + col, &[n]);
        }
        let mut r = renderer(VideoType::ColorTv);
        render(&mut r, &s, 1);
        let c = chroma().base_colors[n as usize];
        // The cell's colour repeats every 4 dots; averaging 4 neighbouring pixels recovers the
        // table's average (what GenerateBaseColors computes).
        let avg = |sh: u32| {
            (0..4)
                .map(|i| (rgb(&r, 200 + i, 0) >> sh) & 0xFF)
                .sum::<u32>()
                / 4
        };
        let got = (avg(16), avg(8), avg(0));
        let want = ((c >> 16) & 0xFF, (c >> 8) & 0xFF, c & 0xFF);
        let d = |a: u32, b: u32| (a as i32 - b as i32).abs();
        assert!(
            d(got.0, want.0) < 6 && d(got.1, want.1) < 6 && d(got.2, want.2) < 6,
            "colour {n}: {got:?} vs {want:?}"
        );
    }
}

#[test]
fn dhires_nibbles_give_sixteen_distinct_colors_in_the_idealized_mode() {
    let mut seen = std::collections::BTreeSet::new();
    for n in 0..16u32 {
        let mut s = Fake::new(Model::Apple2eEnhanced).hires();
        s.mode.col80 = true;
        s.mode.dhires = true;
        // Period-4 dot pattern: dot i is bit (i & 3) of the nibble; 28 dots = aux0 main0 aux1 main1.
        let dots: Vec<u8> = (0..28).map(|i| ((n >> (i & 3)) & 1) as u8).collect();
        let byte = |j: usize| (0..7).fold(0u8, |b, k| b | dots[7 * j + k] << k);
        for cell in 0..20u16 {
            s.poke_aux(0x2000 + 2 * cell, &[byte(0), byte(2)]);
            s.poke(0x2000 + 2 * cell, &[byte(1), byte(3)]);
        }
        let mut r = renderer(VideoType::ColorIdealized);
        render(&mut r, &s, 1);
        let c = rgb(&r, 300, 0);
        assert!(
            (300..304).all(|x| rgb(&r, x, 0) == c || n == 0),
            "nibble {n} is a flat colour"
        );
        seen.insert(c);
    }
    // 16 palette entries, but the NTSC palette's two grays (colours 5 and 10) are the same RGB.
    assert_eq!(seen.len(), 15);
}

#[test]
fn dhires_solid_white_and_black_on_the_composite_monitor() {
    let mut s = Fake::new(Model::Apple2eEnhanced).hires();
    s.mode.col80 = true;
    s.mode.dhires = true;
    for a in 0x2000u16..0x2028 {
        s.poke(a, &[0x7F]);
        s.poke_aux(a, &[0x7F]);
    }
    let mut r = renderer(VideoType::ColorMonitorNtsc);
    render(&mut r, &s, 1);
    assert_eq!(rgb(&r, 300, 0), 0xFFFFFF);
    assert_eq!(
        rgb(&r, 300, 1),
        0,
        "50% scan lines: the second row is black on a monitor"
    );
}

#[test]
fn mono_types_use_their_phosphor_colors() {
    let mut s = Fake::new(Model::Apple2Plus).text();
    s.poke(0x400, &[0xFF]); // inverse/normal block? $FF is a normal solid block-ish glyph
    let cases = [
        (VideoType::MonoWhite, 0xFEFEFE),
        (VideoType::MonoAmber, 0xFE7F00),
        (VideoType::MonoGreen, 0x00BF00),
        (VideoType::MonoCustom, 0xBFBFBF),
        (VideoType::ColorIdealized, 0xFEFEFE),
    ];
    for (vt, want) in cases {
        let mut r = renderer(vt);
        render(&mut r, &s, 1);
        let lit_px = (0..14)
            .map(|x| rgb(&r, x, 8))
            .find(|&c| c != 0)
            .expect("a lit dot");
        assert_eq!(lit_px, want, "{vt:?}");
    }
    // The custom colour is configurable.
    let mut r = Renderer::new(RenderConfig {
        video_type: VideoType::MonoCustom,
        monochrome_rgb: [0xFF, 0x00, 0x80],
        ..Default::default()
    });
    render(&mut r, &s, 1);
    assert_eq!(
        (0..14).map(|x| rgb(&r, x, 8)).find(|&c| c != 0),
        Some(0xFE007F)
    );
}

#[test]
fn tv_scanlines_blend_with_the_line_above() {
    let mut s = Fake::new(Model::Apple2Plus).hires();
    s.poke(0x2000, &[0x7F]); // line 0 white
                             // Line 1 is blank, so on a TV its in-between rows (y=1 above, y=2... ) show a 50% mix.
    let mut r = renderer(VideoType::MonoTv);
    render(&mut r, &s, 1);
    let line0 = rgb(&r, 5, 0) & 0xFF;
    let between = rgb(&r, 5, 1) & 0xFF;
    // Row 1 is the in-between line preceding Apple line 1 (blank): a quarter of the white line 0.
    assert!(line0 > 200);
    assert!(between < line0 / 2, "{between} vs {line0}");
    // Double (full) scan lines: the in-between line blends 50%.
    let mut full = Renderer::new(RenderConfig {
        video_type: VideoType::MonoTv,
        half_scanlines: false,
        ..Default::default()
    });
    render(&mut full, &s, 1);
    assert!(rgb(&full, 5, 1) & 0xFF > between);
}

#[test]
fn vertical_blend_mixes_hires_colors_of_adjacent_lines() {
    // Line 0 violet (px 0-1), line 1 blue (px 1-2): px 1 mixes violet+blue (GH#616).
    let mut s = Fake::new(Model::Apple2Plus).hires();
    s.poke(0x2000, &[0x01]);
    s.poke(0x2400, &[0x81]);
    let plain = {
        let mut r = renderer(VideoType::ColorIdealized);
        render(&mut r, &s, 2);
        (rgb(&r, 0, 0), rgb(&r, 1, 0), rgb(&r, 2, 0))
    };
    let mut r = Renderer::new(RenderConfig {
        video_type: VideoType::ColorIdealized,
        vertical_blend: true,
        ..Default::default()
    });
    render(&mut r, &s, 2);
    let blended = (rgb(&r, 0, 0), rgb(&r, 1, 0), rgb(&r, 2, 0));
    assert_ne!(plain, blended);
    // Violet/blue neighbours mix to the "purple" TV-emulation colour (RGBMonitor.cpp HGR_PURPLE).
    assert!([blended.0, blended.1, blended.2].contains(&0x6161FF));
}

#[test]
fn video_type_enum_matches_applewin() {
    use VideoType::*;
    assert_eq!(VideoType::ALL.map(|v| v as u8), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(VideoType::DEFAULT, ColorTv);
    assert_eq!(ColorTv.description(), "Color TV");
    assert_eq!(MonoCustom.prev(), MonoWhite);
    assert_eq!(MonoWhite.next(), MonoCustom);
    assert_eq!(ColorTv.next(), MonoTv);
}

#[test]
fn every_video_type_renders_every_mode_without_panicking() {
    for vt in VideoType::ALL {
        for (text, hires, col80, dhires, mixed) in [
            (true, false, false, false, false),
            (true, false, true, false, false),
            (false, false, false, false, false),
            (false, false, true, true, false),
            (false, true, false, false, true),
            (false, true, true, true, false),
            (false, true, false, true, false),
        ] {
            let mut s = Fake::new(Model::Apple2eEnhanced);
            s.mode.text = text;
            s.mode.col80 = col80;
            s.mode.dhires = dhires;
            s.mode.mixed = mixed;
            if hires {
                s = s.hires();
                s.mode.text = false;
            }
            s.poke(0x400, &[0xC1; 40]);
            s.poke(0x2000, &[0x55; 40]);
            s.poke_aux(0x2000, &[0x2A; 40]);
            let mut r = renderer(vt);
            render(&mut r, &s, 2);
            assert!(
                r.pixels().iter().all(|p| p >> 24 == 0xFF),
                "{vt:?} left transparent pixels"
            );
        }
    }
}
