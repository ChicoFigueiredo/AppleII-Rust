//! Shared helpers of the PDF tests: hand-built documents and wrappers of poppler's tools.
#![allow(dead_code)]

use apple2_printer::document::{Dot, Page, PageDocument, TextRun};
use apple2_printer::paper::{InkColor, Paper, PaperKind, RenderOptions, Ribbon};
use std::path::{Path, PathBuf};
use std::process::Command;

/// A scratch directory unique to the test.
pub fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("pdf_tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Runs a poppler tool; a missing tool makes the test fail loudly (BLOCKED), never pass silently.
pub fn tool(cmd: &str, args: &[&str]) -> Vec<u8> {
    let out = Command::new(cmd).args(args).output().unwrap_or_else(|e| {
        panic!("BLOCKED: external tool `{cmd}` is not available ({e}); install poppler-utils")
    });
    assert!(
        out.status.success(),
        "{cmd} {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

pub fn tool_text(cmd: &str, args: &[&str]) -> String {
    String::from_utf8(tool(cmd, args)).unwrap()
}

/// A page of `w_in x h_in` inches.
pub fn blank_page(w_units: u32, h_units: u32) -> Page {
    Page {
        width: w_units,
        height: h_units,
        ..Default::default()
    }
}

/// A small page with a ruler of dots, a 2x repeated dot and a diagonal.
pub fn pattern_page(w: u32, h: u32) -> Page {
    let mut p = blank_page(w, h);
    for i in 0..40u32 {
        p.dots.push(Dot {
            x: 600 + i * 36,
            y: 900,
            ink: 0,
        });
        p.dots.push(Dot {
            x: 600 + i * 36,
            y: 900 + 72 * (i % 5),
            ink: 0,
        });
    }
    // Repeated hits (double strike) on the same spot.
    for _ in 0..3 {
        p.dots.push(Dot {
            x: 1500,
            y: 2000,
            ink: 0,
        });
    }
    for i in 0..30u32 {
        p.dots.push(Dot {
            x: 300 + i * 27,
            y: 300 + i * 27,
            ink: 0,
        });
    }
    p.text.push(TextRun {
        x: 600,
        y: 1200,
        advance: 216,
        height: 360,
        text: "HELLO PRINTER".into(),
    });
    p.text.push(TextRun {
        x: 600,
        y: 1500,
        advance: 216,
        height: 360,
        text: "Ação, coração".into(),
    });
    p
}

pub fn doc_of(pages: Vec<Page>) -> PageDocument {
    PageDocument {
        pages,
        ..Default::default()
    }
}

pub fn opts(kind: PaperKind, ribbon: Ribbon, ink: InkColor) -> RenderOptions {
    RenderOptions {
        paper: Paper::preset(kind),
        ribbon,
        ink,
    }
}

/// Options for a custom-sized paper of the given kind.
pub fn opts_sized(kind: PaperKind, w: u32, h: u32, ribbon: Ribbon, ink: InkColor) -> RenderOptions {
    RenderOptions {
        paper: Paper {
            kind,
            width: w,
            height: h,
        },
        ribbon,
        ink,
    }
}

/// Rasterises page 1 to a binary PNM (gray or RGB) and returns (width, height, channels, pixels).
pub fn raster(pdf: &Path, dpi: u32, gray: bool) -> (usize, usize, usize, Vec<u8>) {
    let dir = pdf.parent().unwrap();
    let stem = dir.join(format!("r{dpi}{}", if gray { "g" } else { "c" }));
    let mut args = vec!["-r".to_string(), dpi.to_string(), "-singlefile".into()];
    if gray {
        args.push("-gray".into());
    }
    args.push(pdf.to_str().unwrap().into());
    args.push(stem.to_str().unwrap().into());
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    tool("pdftoppm", &a);
    let file = stem.with_extension(if gray { "pgm" } else { "ppm" });
    parse_pnm(&std::fs::read(file).unwrap())
}

pub fn parse_pnm(b: &[u8]) -> (usize, usize, usize, Vec<u8>) {
    // "P5\nW H\n255\n" or "P6...": three whitespace-separated header tokens after the magic.
    let mut pos = 2;
    let mut vals = Vec::new();
    while vals.len() < 3 {
        while b[pos].is_ascii_whitespace() {
            pos += 1;
        }
        let s = pos;
        while !b[pos].is_ascii_whitespace() {
            pos += 1;
        }
        vals.push(
            std::str::from_utf8(&b[s..pos])
                .unwrap()
                .parse::<usize>()
                .unwrap(),
        );
    }
    pos += 1;
    let ch = if &b[..2] == b"P6" { 3 } else { 1 };
    (vals[0], vals[1], ch, b[pos..].to_vec())
}

pub fn px(img: &(usize, usize, usize, Vec<u8>), x: usize, y: usize) -> Vec<u8> {
    let (w, _, ch, d) = img;
    d[(y * w + x) * ch..(y * w + x) * ch + ch].to_vec()
}

/// Pixel index of unit coordinate `u` at `dpi` (centre of the pixel containing it).
pub fn at(u: u32, dpi: u32) -> usize {
    (u as usize * dpi as usize) / 2160
}
