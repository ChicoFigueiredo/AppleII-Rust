//! Character sets through the renderer for each model, ALTCHARSET/MouseText, and cycle-level
//! behaviour of mode switches.

mod common;
use apple2_core::{AuxCardKind, Model};
use apple2_video::charset::char_sets;
use apple2_video::VideoType;
use common::*;

const MODELS: [Model; 12] = [
    Model::Apple2,
    Model::Apple2Plus,
    Model::Apple2JPlus,
    Model::Apple2e,
    Model::Apple2eEnhanced,
    Model::Pravets82,
    Model::Pravets8M,
    Model::Pravets8A,
    Model::Tk3000e,
    Model::Base64A,
    Model::BadPravets82,
    Model::BadPravets8M,
];

/// The glyph's dots as 14 pixel characters per row, from the character-set tables.
fn glyph_art(g: &[u8; 8]) -> Vec<String> {
    g.iter().map(|row| (0..7).flat_map(|b| if row >> b & 1 != 0 { ['#', '#'] } else { ['.', '.'] }).collect()).collect()
}

#[test]
fn every_model_draws_every_screen_code_with_its_own_character_set() {
    for model in MODELS {
        let sets = char_sets(model, false, false);
        // Codes in the normal set: inverse (<$40), flash ($40-7F, mask 0 in the first frame), normal.
        for code in [0x01u8, 0x1B, 0x2A, 0x41, 0x5F, 0x7F, 0x80, 0xA0, 0xC1, 0xDD, 0xE1, 0xFF] {
            let mut s = Fake::new(model).text();
            s.poke(0x400, &[code]);
            let mut r = renderer(VideoType::MonoWhite);
            render(&mut r, &s, 1);
            let want = glyph_art(&sets[0][code as usize]);
            for (y, w) in want.iter().enumerate() {
                // Pravets/][ fonts have 8 rows; the second pixel row of each line is black.
                assert_eq!(&art(&r, 0, 2 * y, 14), w, "{model:?} code {code:02x} row {y}");
            }
        }
    }
}

#[test]
fn alt_charset_shows_mousetext_on_the_enhanced_but_not_the_original_iie() {
    let draw = |model: Model, alt: bool| {
        let mut s = Fake::new(model).text();
        s.mode.altcharset = alt;
        s.poke(0x400, &[0x4F]); // MouseText 'open apple'? ($4F) vs flashing 'O' normally
        let mut r = renderer(VideoType::MonoWhite);
        render(&mut r, &s, 1);
        (0..8).map(|y| art(&r, 0, 2 * y, 14)).collect::<Vec<_>>()
    };
    let enh_alt = draw(Model::Apple2eEnhanced, true);
    let enh = draw(Model::Apple2eEnhanced, false);
    assert_ne!(enh_alt, enh);
    assert_eq!(enh_alt, glyph_art(&char_sets(Model::Apple2eEnhanced, false, false)[1][0x4F]));
    // The original //e has no MouseText: ALTCHARSET shows the same glyph as the normal set.
    assert_eq!(draw(Model::Apple2e, true), draw(Model::Apple2e, false));
    // MouseText does not flash.
    let mut s = Fake::new(Model::Apple2eEnhanced).text();
    s.mode.altcharset = true;
    s.poke(0x400, &[0x4F]);
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 40);
    assert_eq!((0..8).map(|y| art(&r, 0, 2 * y, 14)).collect::<Vec<_>>(), enh_alt);
}

#[test]
fn jplus_an2_switches_between_katakana_and_ascii_inverse_halves() {
    let draw = |ascii: bool| {
        let mut s = Fake::new(Model::Apple2JPlus).text();
        s.mode.jplus_ascii = ascii;
        s.poke(0x400, &[0x01]);
        let mut r = renderer(VideoType::MonoWhite);
        render(&mut r, &s, 1);
        (0..8).map(|y| art(&r, 0, 2 * y, 14)).collect::<Vec<_>>()
    };
    assert_ne!(draw(true), draw(false));
    assert_eq!(draw(true), glyph_art(&char_sets(Model::Apple2JPlus, true, false)[0][1]));
}

#[test]
fn text80_with_an_empty_aux_slot_reads_the_floating_bus_for_aux() {
    let mut s = Fake::new(Model::Apple2e).text();
    s.aux_card = AuxCardKind::Empty;
    s.mode.col80 = true;
    s.poke(0x400, &[0xC1]);
    s.poke_aux(0x400, &[0xC2]); // ignored: no aux card (the fake's floating bus is 0)
    let mut r = renderer(VideoType::MonoWhite);
    render(&mut r, &s, 1);
    let row = art(&r, 0, 0, 14);
    // Aux half is the glyph for byte $00 (inverse '@'), not 'B'.
    let inv_at = &glyph_art(&char_sets(Model::Apple2e, false, false)[0][0])[0];
    assert_eq!(&row[..7], &inv_at.chars().step_by(2).collect::<String>());
}

#[test]
fn text_to_graphics_switch_is_delayed_by_one_cycle() {
    // Switch TEXT off with the scanner at line 0, cycle 40 (cell 15): the cell fetched on that
    // cycle is still drawn in text, the next cell in lo-res (GH#656).
    let mut s = Fake::new(Model::Apple2Plus).text();
    for a in 0x400u16..0x428 {
        s.poke(a, &[0xA0]); // spaces
    }
    s.poke(0x400, &[0x00; 40]);
    for a in 0x400..0x428u16 {
        s.main[a as usize] = 0x0F; // lo-res: white top half; as text: inverse '/'
    }
    let mut r = renderer(VideoType::ColorIdealized);
    s.cycle = 0;
    r.resync(0);
    r.update(&s, 1); // initialises the renderer
    r.resync(0);
    r.update(&s, 40); // cycles 0..39
    s.mode.text = false;
    r.update(&s, 3); // cycles 40, 41, 42
    r.update(&s, 65 - 43); // the rest of the line
    // Cell for horz 40 is index 15 (x = 15*14 ..). Still text: not a flat lo-res white block.
    let text_cell: Vec<u32> = (0..14).map(|x| rgb(&r, 15 * 14 + x, 0)).collect();
    let lores_cell: Vec<u32> = (0..14).map(|x| rgb(&r, 16 * 14 + x, 0)).collect();
    assert!(lores_cell.iter().all(|&c| c == 0xFFFFFF), "{lores_cell:x?}");
    assert!(text_cell.iter().any(|&c| c != 0xFFFFFF), "{text_cell:x?}");
    // Cells before the switch are text, too.
    assert!((0..14).any(|x| rgb(&r, 14 * 14 + x, 0) != 0xFFFFFF));
}

#[test]
fn immediate_switches_take_effect_on_the_same_cycle() {
    // HIRES on at cycle 40: no delay for non-TEXT/MIXED changes.
    let mut s = Fake::new(Model::Apple2Plus).lores();
    for a in 0x400u16..0x428 {
        s.main[a as usize] = 0x11; // lo-res colour 1 (deep red)
    }
    for a in 0x2000u16..0x2028 {
        s.main[a as usize] = 0x7F;
    }
    let mut r = renderer(VideoType::ColorIdealized);
    r.update(&s, 1);
    r.resync(0);
    r.update(&s, 40);
    s.memmode |= apple2_core::memory::mf::HIRES;
    r.update(&s, 3);
    r.update(&s, 65 - 43);
    // Cell 14 (horz 39) is lo-res deep red; cell 15 (horz 40, the switching cycle) is already
    // hi-res white.
    let red = apple2_video::chroma::chroma().base_colors[1] & 0xFFFFFF;
    assert_eq!(rgb(&r, 14 * 14 + 5, 0), red);
    assert_eq!(rgb(&r, 15 * 14 + 5, 0), 0xFFFFFF);
}
