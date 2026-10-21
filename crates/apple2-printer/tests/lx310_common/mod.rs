//! Shared helpers for the LX-310 integration tests (not a test crate by itself).
#![allow(dead_code)]

use apple2_printer::document::PageDocument;
use apple2_printer::engine::PrinterEngine;
use apple2_printer::lx310::{Glyphs, Lx310, Lx310Config};
use apple2_printer::paper::{Paper, PaperKind, RenderOptions};
use std::fmt::Write as _;
use std::path::PathBuf;

pub const ESC: u8 = 0x1B;

/// Config with the placeholder cell and ASCII text so goldens do not depend on `font`.
pub fn golden_config() -> Lx310Config {
    Lx310Config {
        glyphs: Glyphs::PlaceholderOnly,
        ..Lx310Config::default()
    }
}

pub fn options(kind: PaperKind) -> RenderOptions {
    RenderOptions {
        paper: Paper::preset(kind),
        ..RenderOptions::default()
    }
}

/// Runs a whole job with the stream cut into `chunks` and returns the document.
pub fn run_chunks(config: Lx310Config, kind: PaperKind, chunks: &[&[u8]]) -> PageDocument {
    let mut e = Lx310::new(config);
    e.begin(options(kind));
    let mut off = 0u64;
    for c in chunks {
        e.feed(off, c);
        off += c.len() as u64;
    }
    e.finish();
    e.document().clone()
}

pub fn run(data: &[u8]) -> PageDocument {
    run_chunks(golden_config(), PaperKind::GreenBar, &[data])
}

pub fn run_paper(kind: PaperKind, data: &[u8]) -> PageDocument {
    run_chunks(golden_config(), kind, &[data])
}

/// Stable, ordered, exact text form of a document (no tolerance anywhere).
pub fn render(doc: &PageDocument) -> String {
    let mut s = String::new();
    for (i, p) in doc.pages.iter().enumerate() {
        let _ = writeln!(
            s,
            "page {} {}x{} dots={} text={}",
            i + 1,
            p.width,
            p.height,
            p.dots.len(),
            p.text.len()
        );
        for d in &p.dots {
            let _ = writeln!(s, "d {} {} {}", d.x, d.y, d.ink);
        }
        for t in &p.text {
            let _ = writeln!(
                s,
                "t {} {} adv={} h={} {:?}",
                t.x, t.y, t.advance, t.height, t.text
            );
        }
    }
    let _ = writeln!(s, "in_progress={}", doc.last_in_progress);
    for d in &doc.diagnostics {
        let hex: Vec<String> = d.bytes.iter().map(|b| format!("{b:02X}")).collect();
        let _ = writeln!(s, "diag @{} [{}] {}", d.offset, hex.join(" "), d.message);
    }
    s
}

/// FNV-1a 64 of the rendered document: the "hash" of T-010.
pub fn hash(doc: &PageDocument) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in render(doc).bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

pub fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/lx310")
        .join(format!("{name}.golden"))
}

/// Compares `actual` with the stored golden; `UPDATE_GOLDEN=1` rewrites it.
pub fn check_golden(name: &str, actual: &str) {
    let path = golden_path(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).expect("write golden");
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "golden {} missing; run with UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert!(
        expected == actual,
        "golden {name} differs (UPDATE_GOLDEN=1 to regenerate and review the diff)"
    );
}

pub fn dots_of(doc: &PageDocument, page: usize) -> Vec<(u32, u32)> {
    doc.pages[page].dots.iter().map(|d| (d.x, d.y)).collect()
}
