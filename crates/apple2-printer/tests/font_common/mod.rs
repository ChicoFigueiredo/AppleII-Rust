//! Shared helpers for the font tests: printable ranges, golden comparison.
#![allow(dead_code)]

use apple2_printer::font::{ascii_art, glyph, to_unicode, CharTable, Quality};
use std::path::PathBuf;

pub const TABLES: [(CharTable, &str); 6] = [
    (CharTable::Pc437, "pc437"),
    (CharTable::Pc850, "pc850"),
    (CharTable::Pc860, "pc860"),
    (CharTable::Brascii, "brascii"),
    (CharTable::Abicomp, "abicomp"),
    (CharTable::Italic, "italic"),
];

pub const QUALITIES: [(Quality, &str); 2] = [(Quality::Draft, "draft"), (Quality::Nlq, "nlq")];

/// Codes each table must print (ASCII plus its defined upper half).
pub fn printable_codes(table: CharTable) -> Vec<u8> {
    let mut v: Vec<u8> = (0x20..=0x7E).collect();
    match table {
        CharTable::Pc437 | CharTable::Pc850 | CharTable::Pc860 => v.extend(0x80..=0xFF),
        CharTable::Brascii | CharTable::Italic => v.extend(0xA0..=0xFF),
        CharTable::Abicomp => v.extend(0xA0..=0xDF),
    }
    v
}

pub fn dump(table: CharTable, country: u8, codes: &[u8], quality: Quality) -> String {
    let mut s = String::new();
    for &code in codes {
        let ch = to_unicode(table, country, code).expect("unicode");
        let g = glyph(table, country, code, quality).expect("glyph");
        s.push_str(&format!("== 0x{code:02X} U+{:04X} {ch}\n", ch as u32));
        s.push_str(&ascii_art(&g));
    }
    s
}

/// Compares with `tests/golden/font/<name>`; `UPDATE_GOLDEN=1` (or a missing file) writes it.
pub fn check_golden(name: &str, actual: &str) {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "golden", "font", name]
        .iter()
        .collect();
    if std::env::var_os("UPDATE_GOLDEN").is_some() || !path.exists() {
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap();
    assert!(
        expected == actual,
        "golden {name} differs; rerun with UPDATE_GOLDEN=1 after reviewing the change"
    );
}
