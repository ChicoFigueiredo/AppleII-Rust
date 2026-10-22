//! Goldens that depend on the real font (`Glyphs::default()`, i.e. `font::glyph`). They live apart
//! from the placeholder goldens and must be regenerated (`UPDATE_GOLDEN=1`) and reviewed whenever
//! the font changes (v1 -> v2): `tests/golden/lx310_font/`.

mod lx310_common;
use apple2_printer::lx310::Lx310Config;
use apple2_printer::paper::PaperKind;
use lx310_common::*;
use std::path::PathBuf;

fn cases() -> Vec<(&'static str, &'static [u8])> {
    vec![
        (
            "title_bold_double_width",
            b"\x1b@\x1bE\x1bW\x01APPLE II-RUST\x1bW\x00\x1bF",
        ),
        (
            "draft_10_12_15_cpi",
            b"Hello\r\n\x1bMHello\r\n\x1bgHello\r\n\x1bPHello",
        ),
        ("condensed", b"\x0fCondensed 17 cpi\x12"),
        ("nlq_line", b"\x1bx\x01Near Letter Quality\x1bx\x00"),
        (
            "underline_italic",
            b"\x1b-\x01under\x1b-\x00 \x1b4italic\x1b5",
        ),
        ("bold_and_double_strike", b"\x1bEbold\x1bF \x1bGstrike\x1bH"),
        ("expanded_so", b"\x0eWIDE"),
        ("brazilian_437", b"\x1bR\x00\x87\xa0\xc6"),
        (
            "frame_draft",
            b"\x1bt\x01\xda\xc4\xbf\r\n\xb3 \xb3\r\n\xc0\xc4\xd9\r\n\xc9\xcd\xbb\r\n\xba \xba\r\n\xc8\xcd\xbc",
        ),
        (
            "frame_nlq",
            b"\x1bx\x01\x1bt\x01\xda\xc4\xbf\r\n\xb3 \xb3\r\n\xc0\xc4\xd9\r\n\xc9\xcd\xbb\r\n\xba \xba\r\n\xc8\xcd\xbc",
        ),
        ("portuguese_symbols", b"\xa8 \xa6\xa7 \xf8 \xf1 \xfd\xfc \xf6 \x9e \xe6"),
    ]
}

fn path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/lx310_font")
        .join(format!("{name}.golden"))
}

#[test]
fn font_goldens_match() {
    for (name, data) in cases() {
        let d = run_chunks(Lx310Config::default(), PaperKind::GreenBar, &[data]);
        let actual = render(&d);
        let p = path(name);
        if std::env::var_os("UPDATE_GOLDEN").is_some() {
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, &actual).unwrap();
            continue;
        }
        let expected = std::fs::read_to_string(&p)
            .unwrap_or_else(|_| panic!("{} missing; UPDATE_GOLDEN=1 regenerates", p.display()));
        assert!(
            expected == actual,
            "font golden {name} differs (font changed? regenerate and review)"
        );
    }
}

#[test]
fn real_font_output_is_split_independent_and_inside_the_page() {
    for (name, data) in cases() {
        let whole = run_chunks(Lx310Config::default(), PaperKind::GreenBar, &[data]);
        let bytes: Vec<&[u8]> = data.chunks(1).collect();
        let one = run_chunks(Lx310Config::default(), PaperKind::GreenBar, &bytes);
        assert_eq!(render(&whole), render(&one), "{name}");
        assert!(
            whole.diagnostics.is_empty(),
            "{name}: {:?}",
            whole.diagnostics
        );
        for p in &whole.pages {
            assert!(p.dots.iter().all(|d| d.y >= 357 && d.y < p.height));
        }
    }
}
