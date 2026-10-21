//! Golden dumps of every glyph, per table and quality, plus national variants and the Portuguese
//! accents the project cares about most.

mod font_common;
use apple2_printer::font::{CharTable, Quality};
use font_common::*;

#[test]
fn golden_per_table_and_quality() {
    for (table, name) in TABLES {
        for (q, qname) in QUALITIES {
            let s = dump(table, 0, &printable_codes(table), q);
            check_golden(&format!("{name}_{qname}.txt"), &s);
        }
    }
}

#[test]
fn golden_national_variants() {
    // Only the 12 codes ESC R can change, for each of the 13 variants.
    let codes = [
        0x23, 0x24, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x60, 0x7B, 0x7C, 0x7D, 0x7E,
    ];
    for (q, qname) in QUALITIES {
        let mut s = String::new();
        for country in 0..=12u8 {
            s.push_str(&format!("##### country {country}\n"));
            s.push_str(&dump(CharTable::Pc437, country, &codes, q));
        }
        check_golden(&format!("national_{qname}.txt"), &s);
    }
}

#[test]
fn golden_portuguese_accents() {
    // Ç ç á ã é ó ú, by Unicode, looked up in each Portuguese-capable table.
    let wanted = ['Ç', 'ç', 'á', 'ã', 'é', 'ó', 'ú'];
    for (table, name) in [
        (CharTable::Pc850, "pc850"),
        (CharTable::Pc860, "pc860"),
        (CharTable::Brascii, "brascii"),
        (CharTable::Abicomp, "abicomp"),
    ] {
        let codes: Vec<u8> = wanted
            .iter()
            .map(|&w| {
                (0x80..=0xFFu8)
                    .find(|&c| apple2_printer::font::to_unicode(table, 0, c) == Some(w))
                    .unwrap_or_else(|| panic!("{w} missing from {name}"))
            })
            .collect();
        for (q, qname) in QUALITIES {
            check_golden(
                &format!("accents_{name}_{qname}.txt"),
                &dump(table, 0, &codes, q),
            );
        }
    }
    let _ = Quality::Draft;
}
