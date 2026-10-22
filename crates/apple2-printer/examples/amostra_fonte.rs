//! Generates a sample PDF of the dot-matrix font: every table in Draft and NLQ, three pitches,
//! Portuguese accents in four character tables and a PC437 frame. The page is assembled by hand from
//! the glyphs (it does not use the LX-310 layout engine).
//!
//! Usage: `cargo run -p apple2-printer --example amostra_fonte -- <output.pdf>`

use apple2_printer::document::{Dot, Page, PageDocument, TextRun};
use apple2_printer::font::{cell_rows, glyph, line_bridge, to_unicode, CharTable, Glyph, Quality};
use apple2_printer::paper::{Paper, PaperKind, RenderOptions};
use apple2_printer::pdf::{export, PageSelection, PdfMeta};
use apple2_printer::units::PER_INCH;

const MARGIN_X: u32 = 1080;
/// 1/6" line pitch (units of 1/2160").
const LINE: u32 = 360;

struct Sheet {
    page: Page,
    y: u32,
}

impl Sheet {
    fn new(paper: Paper) -> Sheet {
        Sheet {
            page: Page {
                width: paper.width,
                height: paper.height,
                ..Page::default()
            },
            y: 1080,
        }
    }
}

/// Codes of `text` in `table` (the first code whose Unicode matches; a blank for missing ones).
fn encode(table: CharTable, text: &str) -> Vec<u8> {
    text.chars()
        .map(|c| {
            (0x20..=0xFFu8)
                .find(|&code| to_unicode(table, 0, code) == Some(c))
                .unwrap_or(b' ')
        })
        .collect()
}

/// X offset (units) of dot column `col` inside a cell at `cpi`: columns scale with the pitch.
fn col_x(quality: Quality, col: usize, cpi: u32) -> u32 {
    let pitch10 = match quality {
        Quality::Draft => 18,
        Quality::Nlq => 9,
    };
    (col as u32 * pitch10 * 10 + cpi / 2) / cpi
}

fn row_y(quality: Quality, row: usize) -> u32 {
    match quality {
        Quality::Draft => row as u32 * 30,
        Quality::Nlq => row as u32 * 15,
    }
}

/// Prints one line; returns the glyphs of the cells for bridging.
fn line(
    sheet: &mut Sheet,
    text_codes: &[u8],
    table: CharTable,
    quality: Quality,
    cpi: u32,
) -> Vec<Option<Glyph>> {
    let advance = PER_INCH / cpi;
    let mut cells = Vec::new();
    let mut text = String::new();
    for (i, &code) in text_codes.iter().enumerate() {
        let x0 = MARGIN_X + i as u32 * advance;
        let g = glyph(table, 0, code, quality);
        if let Some(g) = &g {
            for (c, &mask) in g.columns.iter().enumerate() {
                for r in 0..cell_rows(quality) {
                    if mask & (1 << r) != 0 {
                        sheet.page.dots.push(Dot {
                            x: x0 + col_x(quality, c, cpi),
                            y: sheet.y + row_y(quality, r),
                            ink: 0,
                        });
                    }
                }
            }
        }
        text.push(to_unicode(table, 0, code).unwrap_or(' '));
        cells.push(g);
    }
    sheet.page.text.push(TextRun {
        x: MARGIN_X,
        y: sheet.y + row_y(quality, 8 * cell_rows(quality) / 9),
        advance,
        height: 270,
        text,
    });
    cells
}

/// Fills the gap between two frame lines by continuing the pins that fire on both sides.
fn bridge(
    sheet: &mut Sheet,
    upper: &[Option<Glyph>],
    lower: &[Option<Glyph>],
    quality: Quality,
    cpi: u32,
) {
    let advance = PER_INCH / cpi;
    let last = cell_rows(quality) - 1;
    for (i, (u, l)) in upper.iter().zip(lower).enumerate() {
        let (Some(u), Some(l)) = (u, l) else { continue };
        for (c, on) in line_bridge(u, l).into_iter().enumerate() {
            if !on {
                continue;
            }
            let x = MARGIN_X + i as u32 * advance + col_x(quality, c, cpi);
            let y_top = sheet.y - LINE + row_y(quality, last);
            let step = row_y(quality, 1);
            let mut y = y_top + step;
            while y < sheet.y {
                sheet.page.dots.push(Dot { x, y, ink: 0 });
                y += step;
            }
        }
    }
}

fn say(sheet: &mut Sheet, s: &str) {
    line(
        sheet,
        &encode(CharTable::Pc437, s),
        CharTable::Pc437,
        Quality::Draft,
        10,
    );
    sheet.y += LINE;
}

fn both(sheet: &mut Sheet, table: CharTable, s: &str, cpi: u32) {
    for q in [Quality::Draft, Quality::Nlq] {
        line(sheet, &encode(table, s), table, q, cpi);
        sheet.y += LINE;
    }
}

fn main() {
    let out = std::env::args()
        .nth(1)
        .expect("usage: amostra_fonte <output.pdf>");
    let paper = Paper::preset(PaperKind::A4);
    let mut doc = PageDocument::default();

    // Page 1: alphabets at 10 CPI.
    let mut s = Sheet::new(paper);
    say(
        &mut s,
        "Fonte matricial v2 - Draft (9 agulhas) e NLQ (18 linhas), 10 CPI",
    );
    s.y += LINE / 2;
    for text in [
        "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
        "abcdefghijklmnopqrstuvwxyz",
        "0123456789  !\"#$%&'()*+,-./:;<=>?",
        "@[\\]^_`{|}~  Hamburgefonstiv 1980",
    ] {
        both(&mut s, CharTable::Pc437, text, 10);
    }
    s.y += LINE / 2;
    say(&mut s, "Pitches: 10, 12 e 15 CPI");
    for cpi in [10, 12, 15] {
        both(
            &mut s,
            CharTable::Pc437,
            "The quick brown fox jumps over the lazy dog 0123456789",
            cpi,
        );
    }
    doc.pages.push(std::sync::Arc::new(s.page));

    // Page 2: Portuguese accents in four tables, then a frame.
    let mut s = Sheet::new(paper);
    let phrase = "Ação, coração, pão, avô, você, CAÇULA, Ü";
    for (table, name) in [
        (CharTable::Pc850, "PC850"),
        (CharTable::Pc860, "PC860"),
        (CharTable::Brascii, "BRASCII"),
        (CharTable::Abicomp, "ABICOMP"),
    ] {
        say(&mut s, name);
        both(&mut s, table, phrase, 10);
    }
    s.y += LINE;
    say(&mut s, "Moldura PC437 (continua entre as linhas)");
    for q in [Quality::Draft, Quality::Nlq] {
        let rows = [
            "╔══════════╦═══════╗",
            "║ Apple II ║ LX310 ║",
            "╠══════════╬═══════╣",
            "║ Draft    ║  NLQ  ║",
            "╚══════════╩═══════╝",
        ];
        let mut prev: Option<Vec<Option<Glyph>>> = None;
        for r in rows {
            let cells = line(
                &mut s,
                &encode(CharTable::Pc437, r),
                CharTable::Pc437,
                q,
                10,
            );
            if let Some(p) = &prev {
                bridge(&mut s, p, &cells, q, 10);
            }
            prev = Some(cells);
            s.y += LINE;
        }
        s.y += LINE;
    }
    doc.pages.push(std::sync::Arc::new(s.page));

    let opts = RenderOptions {
        paper,
        ..RenderOptions::default()
    };
    let meta = PdfMeta {
        title: "Amostra da fonte matricial v2".into(),
        ..PdfMeta::default()
    };
    let bytes = export(&doc, &opts, &meta, &PageSelection::All);
    std::fs::write(&out, bytes).expect("write pdf");
    eprintln!("wrote {out}");
}
