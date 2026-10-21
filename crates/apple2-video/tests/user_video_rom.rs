//! `-videorom` (GH#574 French //e video ROM, GH#629 Pig Font for the ][+, GH#773 J-Plus).
//! Oraculo: AppleWin a8f0d1d, NTSC_CharSet.cpp:109-310 (`userVideoRomForIIe`,
//! `userVideoRomForIIPlus`, `userVideoRom2K`). Resultados em
//! `docs/conformance/results/p23-config-outras.csv`.

use apple2_core::Model;
use apple2_video::charset::user_char_sets;

/// Transcription of `userVideoRom2K` for a plain ][/][+ (NTSC_CharSet.cpp:183-234).
fn oracle_2k(rom: &[u8]) -> [[u8; 8]; 256] {
    let mut g = [[0u8; 8]; 256];
    for (i, glyph) in g.iter_mut().enumerate() {
        let ra = i * 8;
        for (y, out) in glyph.iter_mut().enumerate() {
            let mut n = rom[ra + y];
            if ra < 1024 && n & 0x80 == 0 {
                n ^= 0x7F; // UTAII:8-30/31: bit 7 clear => inverse in the lower 1K
            }
            let mut d = 0u8;
            for _ in 0..7 {
                d = (d << 1) | (n & 1);
                n >>= 1;
            }
            *out = d;
        }
    }
    g
}

fn pattern(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| (i as u32).wrapping_mul(2654435761).to_le_bytes()[2])
        .collect()
}

#[test]
fn gh629_a_2k_pig_font_rom_replaces_the_apple_ii_plus_characters_like_applewin() {
    let rom = pattern(2048);
    for m in [Model::Apple2, Model::Apple2Plus] {
        let s = user_char_sets(m, &rom).expect("2K user video ROM for the ][/][+");
        assert_eq!(s.len(), 1);
        assert_eq!(s[0], oracle_2k(&rom), "{m:?}");
    }
    // userVideoRomForIIPlus (:241): only exactly 2K; userVideoRomForIIe (:162): at least 4K
    assert!(user_char_sets(Model::Apple2Plus, &pattern(4096)).is_none());
    assert!(user_char_sets(Model::Apple2e, &rom).is_none());
}

#[test]
fn gh574_a_4k_or_8k_video_rom_serves_the_original_and_the_enhanced_iie() {
    // :165-176: 4K = US only; 8K = PAL/other first, US (the one the 60Hz //e shows) second.
    let r4 = pattern(4096);
    let a = user_char_sets(Model::Apple2e, &r4).expect("original //e");
    let b = user_char_sets(Model::Apple2eEnhanced, &r4).expect("enhanced //e");
    assert_eq!(
        a, b,
        ":175 same custom US video ROM for Original & Enhanced //e"
    );
    let r8 = pattern(8192);
    let c = user_char_sets(Model::Apple2e, &r8).expect("8K");
    let d = user_char_sets(Model::Apple2e, &r8[4096..]).expect("second 4K alone");
    assert_eq!(c, d, "the second 4K of an 8K ROM is the one used");
    // a French ROM is not a 2K one: it never replaces a ][ set
    assert!(user_char_sets(Model::Apple2e, &pattern(3000)).is_none());
}

#[test]
fn gh773_j_plus_and_base64a_keep_their_own_video_rom_whatever_videorom_says() {
    // :264-275 build csbits_a2j / csbits_base64a from resources; :244 touches csbits_a2 only
    let rom = pattern(2048);
    assert!(user_char_sets(Model::Apple2JPlus, &rom).is_none());
    assert!(user_char_sets(Model::Base64A, &rom).is_none());
}
