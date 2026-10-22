//! The table and international set of a job (the printer's DIP switches) are the Lx310's initial
//! state and what ESC @ returns to; old manifests without the fields keep PC437.

use apple2_printer::engine::PrinterEngine;
use apple2_printer::font::CharTable;
use apple2_printer::lx310::{Lx310, Lx310Config};
use apple2_printer::paper::RenderOptions;

fn text_of(options: RenderOptions, data: &[u8]) -> String {
    let mut e = Lx310::new(Lx310Config::default());
    e.begin(options);
    e.feed(0, data);
    e.finish();
    e.document()
        .pages
        .iter()
        .flat_map(|p| p.text.iter().map(|r| r.text.clone()))
        .collect()
}

fn with(table: CharTable, country: u8) -> RenderOptions {
    RenderOptions {
        table,
        country,
        ..RenderOptions::default()
    }
}

#[test]
fn initial_table_comes_from_the_options_without_any_escape_prefix() {
    assert_eq!(text_of(with(CharTable::Pc437, 0), &[0xE0]), "α");
    assert_eq!(text_of(with(CharTable::Pc850, 0), &[0xE0]), "Ó");
}

#[test]
fn esc_at_returns_to_the_configured_table_and_country() {
    let o = with(CharTable::Pc850, 3);
    let base = text_of(o, b"#\xE0");
    // The program switches to the default table and set, then resets: back to the job's setup.
    let t = text_of(o, b"\x1b(t\x03\x00\x01\x01\x00\x1bR\x00#\xE0\x1b@#\xE0");
    assert_eq!(t.chars().count(), 4);
    assert_eq!(t.chars().skip(2).collect::<String>(), base);
    assert_eq!(t.chars().take(2).collect::<String>(), "#α");
    assert_ne!(base, text_of(with(CharTable::Pc850, 0), b"#\xE0"));
}

#[test]
fn old_manifest_options_default_to_pc437_and_country_zero() {
    let o = RenderOptions::default();
    let mut v = serde_json::to_value(o).unwrap();
    let obj = v.as_object_mut().unwrap();
    obj.remove("table");
    obj.remove("country");
    let back: RenderOptions = serde_json::from_value(v).unwrap();
    assert_eq!(back, o);
    assert_eq!(back.table, CharTable::Pc437);
}
