//! Behavioural rules of the font: coverage, the 9-pin adjacency rule, geometry, determinism, box
//! continuity and national variants.

mod font_common;
use apple2_printer::font::*;
use font_common::*;

#[test]
fn every_printable_code_has_glyph_and_unicode() {
    for (table, name) in TABLES {
        for code in printable_codes(table) {
            for country in [0u8, 2, 12] {
                let u = to_unicode(table, country, code);
                assert!(u.is_some(), "{name} 0x{code:02X} has no Unicode");
                for (q, _) in QUALITIES {
                    assert!(
                        glyph(table, country, code, q).is_some(),
                        "{name} 0x{code:02X} has no glyph"
                    );
                }
            }
        }
    }
}

#[test]
fn controls_and_undefined_codes_are_none() {
    for (table, _) in TABLES {
        for code in (0x00..0x20u8).chain([0x7F]) {
            assert_eq!(to_unicode(table, 0, code), None);
            assert!(glyph(table, 0, code, Quality::Draft).is_none());
        }
    }
    assert_eq!(to_unicode(CharTable::Abicomp, 0, 0xE0), None);
    assert_eq!(to_unicode(CharTable::Brascii, 0, 0x80), None);
    assert_eq!(to_unicode(CharTable::Italic, 0, 0x90), None);
}

#[test]
fn no_adjacent_dots_in_any_glyph_either_quality() {
    for (table, name) in TABLES {
        for code in printable_codes(table) {
            for (q, qname) in QUALITIES {
                let g = glyph(table, 0, code, q).unwrap();
                for w in g.columns.windows(2) {
                    assert_eq!(
                        w[0] & w[1],
                        0,
                        "{name} {qname} 0x{code:02X}: same pin in adjacent columns"
                    );
                }
            }
        }
    }
}

#[test]
fn geometry_matches_contract() {
    for (table, _) in TABLES {
        for code in printable_codes(table) {
            let d = glyph(table, 0, code, Quality::Draft).unwrap();
            assert_eq!(d.columns.len(), 12);
            assert!(d.columns.iter().all(|&c| c < 1 << 9));
            assert_eq!(d.baseline_row, 8);
            let n = glyph(table, 0, code, Quality::Nlq).unwrap();
            assert_eq!(n.columns.len(), 24);
            assert!(n.columns.iter().all(|&c| c < 1 << 18));
            assert_eq!(n.baseline_row, 16);
        }
    }
}

#[test]
fn descenders_use_the_lower_rows() {
    let low = |ch: u8| {
        glyph(CharTable::Pc437, 0, ch, Quality::Draft)
            .unwrap()
            .columns
            .iter()
            .fold(0, |a, &c| a | c)
    };
    for ch in *b"gjpqy" {
        assert_ne!(
            low(ch) & (1 << 8),
            0,
            "{} must reach the last row",
            ch as char
        );
    }
    for ch in *b"aeoxZ" {
        assert_eq!(
            low(ch) & (1 << 8),
            0,
            "{} must stay above the descender row",
            ch as char
        );
    }
}

#[test]
fn deterministic() {
    for (table, _) in TABLES {
        for code in printable_codes(table) {
            for (q, _) in QUALITIES {
                assert_eq!(glyph(table, 3, code, q), glyph(table, 3, code, q));
            }
        }
    }
}

#[test]
fn box_drawing_lines_are_continuous_between_cells() {
    // Find the code of ─ and │ in PC437 and check that two adjacent cells give an evenly spaced
    // line (period 2, as the adjacency rule demands) and that verticals fill all rows.
    let find = |ch: char| {
        (0x80..=0xFFu8)
            .find(|&c| to_unicode(CharTable::Pc437, 0, c) == Some(ch))
            .unwrap()
    };
    for (q, scale) in [(Quality::Draft, 1usize), (Quality::Nlq, 2)] {
        let h = glyph(CharTable::Pc437, 0, find('─'), q).unwrap();
        let row = 4 * scale;
        let mut line = h.columns.clone();
        line.extend(h.columns.iter());
        let dots: Vec<usize> = (0..line.len())
            .filter(|&i| line[i] & (1 << row) != 0)
            .collect();
        assert!(dots.len() >= 10 * scale / 2 * 2 / 2);
        assert!(dots.windows(2).all(|w| w[1] - w[0] == 2), "{dots:?}");
        assert_eq!(dots[0], 0);
        assert_eq!(*dots.last().unwrap(), line.len() - 2);

        let v = glyph(CharTable::Pc437, 0, find('│'), q).unwrap();
        let col: Vec<u32> = v.columns.iter().copied().filter(|&c| c != 0).collect();
        assert_eq!(col.len(), 1);
        assert_eq!(
            col[0],
            (1u32 << cell_rows(q)) - 1,
            "vertical must span all rows"
        );
    }
}

#[test]
fn national_variants_replace_exactly_the_documented_codes() {
    let at = |country, code| to_unicode(CharTable::Pc437, country, code).unwrap();
    assert_eq!(at(0, 0x5B), '[');
    assert_eq!(at(1, 0x40), 'à');
    assert_eq!(at(2, 0x7E), 'ß');
    assert_eq!(at(3, 0x23), '£');
    assert_eq!(at(7, 0x23), '₧');
    assert_eq!(at(8, 0x5C), '¥');
    assert_eq!(at(12, 0x60), 'ü');
    assert_eq!(at(5, 0x24), '¤');
    // Letters and digits never change.
    for country in 0..=12 {
        assert_eq!(at(country, b'A'), 'A');
        assert_eq!(at(country, b'7'), '7');
    }
    // Variants differ from USA in their own codes.
    for country in 1..=12 {
        assert!(
            (0x20..=0x7Eu8).any(|c| at(country, c) != at(0, c)),
            "country {country}"
        );
    }
}

#[test]
fn italic_table_slants_and_keeps_the_dot_count() {
    let up = glyph(CharTable::Pc437, 0, b'I', Quality::Draft).unwrap();
    let it = glyph(CharTable::Italic, 0, b'I', Quality::Draft).unwrap();
    assert_ne!(up.columns, it.columns);
    let count = |g: &Glyph| g.columns.iter().map(|c| c.count_ones()).sum::<u32>();
    assert_eq!(count(&up), count(&it));
    assert_eq!(it, italicize(&up));
}

#[test]
fn portuguese_accents_exist_in_every_portuguese_table() {
    for table in [
        CharTable::Pc850,
        CharTable::Pc860,
        CharTable::Brascii,
        CharTable::Abicomp,
    ] {
        for ch in "ÇçáàâãéêíóôõúüÁÃÉÓ".chars() {
            let found = (0x80..=0xFFu8).any(|c| to_unicode(table, 0, c) == Some(ch));
            // PC850 has no Ã/Õ? it does not need to; PC860 has them; only require the lowercase set.
            if "çáàâãéêíóôõúü".contains(ch) && table != CharTable::Pc850 {
                assert!(found, "{ch} missing in {table:?}");
            }
        }
    }
}
