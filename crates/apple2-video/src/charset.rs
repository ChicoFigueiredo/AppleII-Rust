//! Character generators (AppleWin NTSC_CharSet.cpp). A glyph is 8 rows of 7 dots; bit 0 is the
//! leftmost dot (the video ROM is stored reversed, UTAII:8-30). `[set][screen code][row]`,
//! where set 1 is the ALTCHARSET half on models that have one.

use apple2_core::Model;
use apple2_roms as roms;
use std::sync::OnceLock;

pub type Glyphs = [[u8; 8]; 256];

/// Which dot-pattern family a model uses; see [`char_sets`].
pub struct CharSets {
    a2: [Glyphs; 1],
    a2j: [Glyphs; 2],
    e_original: [Glyphs; 2],
    e_enhanced: [Glyphs; 2],
    pravets82: [Glyphs; 1],
    pravets8m: [Glyphs; 1],
    pravets8c: [Glyphs; 2],
    base64a: [Glyphs; 2],
}

/// How the 2K ROM decoder (`userVideoRom2K`, :183) treats the ROM.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Rom2k {
    Plain,
    JPlus { an2: bool },
    Base64a,
}

fn from_2k(rom: &[u8], kind: Rom2k) -> Glyphs {
    let mut g = [[0u8; 8]; 256];
    for (i, glyph) in g.iter_mut().enumerate() {
        let mut ra = i * 8;
        if let Rom2k::JPlus { an2 } = kind {
            // AN2=0: $00-3F twice, $80-BF twice (Katakana); AN2=1: $40-7F, $C0-FF (ASCII). UTAII:8-12.
            ra &= !(1 << 9);
            ra |= (an2 as usize) << 9;
        }
        for (y, row) in glyph.iter_mut().enumerate() {
            let mut n = rom[ra + y];
            if ra < 1024 {
                // UTAII:8-30: bit 7 (Base64A: bit 0) of the low 1K ROM controls flashing/inverse.
                match kind {
                    Rom2k::Base64a if n & 1 == 0 => n ^= 0xFE,
                    Rom2k::JPlus { .. } => n ^= 0x7F,
                    Rom2k::Plain if n & 0x80 == 0 => n ^= 0x7F,
                    _ => {}
                }
            }
            *row = if kind == Rom2k::Base64a {
                // The Base64A orders its bits 1345672.
                (n >> 2) | ((n & 2) >> 1) | ((n & 4) << 4)
            } else {
                // Just bits 0-6, reversed.
                (0..7).fold(0u8, |d, j| (d << 1) | ((n >> j) & 1))
            };
        }
    }
    g
}

/// `userVideoRom4K` (:109): 4K //e video ROM (US layout) into normal-set and ALTCHARSET tables.
fn from_4k(rom: &[u8]) -> [Glyphs; 2] {
    let mut s = [[[0u8; 8]; 256]; 2];
    let inv = |ra: usize| -> [u8; 8] { std::array::from_fn(|y| rom[ra + y] ^ 0xFF) }; // UTAIIe:8-11: stored inverted
    for i in 0..64 {
        let g = inv(i * 8);
        s[0][i] = g; // [00..3F] inverse
        s[0][i + 64] = g; // [40..7F] flash (same dots, flipped by the flash mask)
    }
    for (i, g) in s[0].iter_mut().enumerate().skip(128) {
        *g = inv(0x400 + (i - 128) * 8); // [80..FF] normal
    }
    for (i, g) in s[1].iter_mut().enumerate() {
        *g = inv(i * 8); // [00..7F] inverse, [80..FF] normal
    }
    s
}

fn from_bits(data: &[u8]) -> Vec<Glyphs> {
    data.chunks(2048)
        .map(|c| {
            let mut g = [[0u8; 8]; 256];
            for (i, glyph) in g.iter_mut().enumerate() {
                glyph.copy_from_slice(&c[i * 8..i * 8 + 8]);
            }
            g
        })
        .collect()
}

fn one(data: &[u8]) -> [Glyphs; 1] {
    from_bits(data).try_into().expect("one set")
}

fn build() -> CharSets {
    let enhanced = from_4k(roms::APPLE2E_ENHANCED_VIDEO);
    // Original //e: the Enhanced //e with the 32 MouseText glyphs [40..5F] replaced by the
    // normal-set ones (:299-301).
    let mut original = enhanced;
    original[1][64..96].copy_from_slice(&enhanced[0][64..96]);
    CharSets {
        a2: [from_2k(roms::APPLE2_VIDEO, Rom2k::Plain)],
        a2j: [
            from_2k(roms::APPLE2_JPLUS_VIDEO, Rom2k::JPlus { an2: false }),
            from_2k(roms::APPLE2_JPLUS_VIDEO, Rom2k::JPlus { an2: true }),
        ],
        e_original: original,
        e_enhanced: enhanced,
        // Decoded from AppleWin resource/CHARSET{82,8M,8C}.bmp by tools/gen_pravets_charsets.py.
        pravets82: one(include_bytes!("../data/pravets82.bin")),
        pravets8m: one(include_bytes!("../data/pravets8m.bin")),
        pravets8c: from_bits(include_bytes!("../data/pravets8c.bin")).try_into().expect("two sets"),
        base64a: [
            from_2k(&roms::BASE64A_GERMAN_VIDEO[..2048], Rom2k::Base64a),
            from_2k(&roms::BASE64A_GERMAN_VIDEO[2048..], Rom2k::Base64a),
        ],
    }
}

static SETS: OnceLock<CharSets> = OnceLock::new();

/// The built-in character generators.
pub fn all() -> &'static CharSets {
    SETS.get_or_init(build)
}

/// `set_csbits` (NTSC.cpp:355): the glyph sets of `model`. One entry means the model has no
/// ALTCHARSET. `jplus_ascii` is the J-Plus AN2 (selects the ASCII half); `base64a_rocker` is the
/// Base64A video-ROM rocker switch (AppleWin default off, which picks the second half).
pub fn char_sets(model: Model, jplus_ascii: bool, base64a_rocker: bool) -> &'static [Glyphs] {
    let s = all();
    match model {
        Model::Apple2 | Model::Apple2Plus => &s.a2,
        Model::Apple2JPlus => &s.a2j[jplus_ascii as usize..][..1],
        Model::Apple2e => &s.e_original,
        Model::Pravets82 => &s.pravets82,
        Model::Pravets8M => &s.pravets8m,
        Model::Pravets8A => &s.pravets8c,
        Model::Base64A => &s.base64a[!base64a_rocker as usize..][..1],
        // Enhanced //e, TK3000 //e, and AppleWin's fallback for the //c and the "bad" Pravets tags.
        Model::Apple2eEnhanced | Model::Tk3000e | Model::Apple2c | Model::BadPravets82 | Model::BadPravets8M => &s.e_enhanced,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn art(g: &[u8; 8]) -> Vec<String> {
        g.iter().map(|r| (0..7).map(|b| if r >> b & 1 != 0 { '#' } else { '.' }).collect()).collect()
    }

    const F_NORMAL: [&str; 8] =
        [".......", ".#####.", ".#.....", ".#.....", ".####..", ".#.....", ".#.....", ".#....."];


    #[test]
    fn apple2_f_is_normal_inverse_and_flash_alike() {
        let g = &char_sets(Model::Apple2, false, false)[0];
        assert_eq!(art(&g[0xC6]), F_NORMAL);
        // $06 is inverse: the same dots inverted over all seven columns.
        let inv: Vec<String> =
            F_NORMAL.iter().map(|r| r.chars().map(|c| if c == '#' { '.' } else { '#' }).collect()).collect();
        assert_eq!(art(&g[0x06]), inv);
        // $46 is the flash half: the normal dots (the flash mask inverts them on alternate phases).
        assert_eq!(art(&g[0x46]), F_NORMAL);
    }

    #[test]
    fn model_character_set_counts() {
        for (m, n) in [
            (Model::Apple2, 1),
            (Model::Apple2Plus, 1),
            (Model::Apple2JPlus, 1),
            (Model::Apple2e, 2),
            (Model::Apple2eEnhanced, 2),
            (Model::Pravets82, 1),
            (Model::Pravets8M, 1),
            (Model::Pravets8A, 2),
            (Model::Tk3000e, 2),
            (Model::Base64A, 1),
        ] {
            assert_eq!(char_sets(m, false, false).len(), n, "{m:?}");
        }
    }

    #[test]
    fn mousetext_only_in_the_enhanced_alt_set() {
        let enh = char_sets(Model::Apple2eEnhanced, false, false);
        let orig = char_sets(Model::Apple2e, false, false);
        // Enhanced alt set $40-$5F is MouseText, unlike the normal set's glyphs there.
        assert_ne!(enh[1][0x41], enh[0][0x41]);
        // The original //e has no MouseText: the alt set repeats the normal glyphs there.
        assert_eq!(orig[1][0x41], orig[0][0x41]);
        // Everything else is shared.
        assert_eq!(orig[1][0xC1], enh[1][0xC1]);
        // The alt set has inverse lowercase at $60-$7F; the normal set has flashing uppercase there.
        assert_ne!(enh[1][0x61], enh[0][0x61]);
    }

    #[test]
    fn jplus_an2_selects_ascii_or_katakana_half() {
        let kata = &char_sets(Model::Apple2JPlus, false, false)[0];
        let asc = &char_sets(Model::Apple2JPlus, true, false)[0];
        // AN2 only matters for the inverse/flash half ($00-$7F); normal 'A' is the ][+ glyph.
        assert_ne!(kata[0x01], asc[0x01]);
        assert_eq!(kata[0xC1], asc[0xC1]);
        assert_eq!(asc[0xC1], char_sets(Model::Apple2Plus, false, false)[0][0xC1]);
        // Katakana half repeats $00-$3F at $40-$7F.
        assert_eq!(kata[0x01], kata[0x41]);
    }

    #[test]
    fn base64a_halves_and_rocker() {
        let off = &char_sets(Model::Base64A, false, false)[0];
        let on = &char_sets(Model::Base64A, false, true)[0];
        // The German half differs at the bracket positions ($5B-$5E become umlauts).
        assert_ne!(off[0xDB], on[0xDB]);
        assert_eq!(off[0xC1], on[0xC1]);
        assert!(off[0xC1].iter().any(|&r| r != 0));
    }

    #[test]
    fn pravets_sets_decode() {
        let p82 = &char_sets(Model::Pravets82, false, false)[0];
        // Normal 'A' ($C1) of the Pravets 82, decoded from CHARSET82.bmp.
        assert_eq!(
            art(&p82[0xC1]),
            ["....#..", "...#.#.", "..#...#", "..#...#", "..#####", "..#...#", "..#...#", "......."]
        );
        let c = char_sets(Model::Pravets8A, false, false);
        assert_ne!(c[0], c[1]);
    }
}
