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

/// True when poppler can be used: the tool runs and APPLE2_PRINTER_NO_POPPLER is not set (that
/// variable simulates a machine without poppler, e.g. the Windows/macOS CI).
pub fn available(cmd: &str) -> bool {
    std::env::var_os("APPLE2_PRINTER_NO_POPPLER").is_none()
        && Command::new(cmd).arg("-v").output().is_ok()
}

pub fn poppler_present() -> bool {
    ["pdfinfo", "pdftoppm", "pdftotext"]
        .iter()
        .all(|t| available(t))
}

/// Installed poppler version, from `pdftoppm -v` (printed on stderr).
pub fn poppler_version() -> Option<String> {
    if !available("pdftoppm") {
        return None;
    }
    let out = Command::new("pdftoppm").arg("-v").output().ok()?;
    let text =
        String::from_utf8_lossy(&out.stderr).to_string() + &String::from_utf8_lossy(&out.stdout);
    text.lines()
        .next()?
        .split_whitespace()
        .last()
        .map(str::to_string)
}

/// Skips the rest of a test (prints BLOCKED, passes) when poppler is not available.
#[allow(unused_macros)]
macro_rules! need_poppler {
    () => {
        if !pdf_common::poppler_present() {
            eprintln!("BLOCKED: poppler (pdfinfo/pdftoppm/pdftotext) not available; external PDF check skipped");
            return;
        }
    };
}
#[allow(unused_imports)]
pub(crate) use need_poppler;

/// Runs a poppler tool (callers check `need_poppler!` first); a failing run fails the test.
pub fn tool(cmd: &str, args: &[&str]) -> Vec<u8> {
    let out = Command::new(cmd)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("`{cmd}` could not run ({e}); call need_poppler!() first"));
    assert!(
        out.status.success(),
        "{cmd} {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}

/// Structural check of a PDF written by the exporter, in pure Rust: header, trailer, xref offsets
/// pointing at their objects, and the page list. Returns the MediaBox of each page.
pub fn structure(b: &[u8]) -> Vec<(f64, f64)> {
    assert!(b.starts_with(b"%PDF-1.7\n"), "header");
    assert!(b.ends_with(b"%%EOF\n"), "trailer end");
    // Non-ASCII bytes become '?', one byte each, so string offsets equal file offsets.
    let text =
        String::from_utf8(b.iter().map(|&c| if c < 128 { c } else { b'?' }).collect()).unwrap();
    let sx: usize = text
        .rsplit("startxref\n")
        .next()
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(b[sx..].starts_with(b"xref\n"), "startxref points at xref");
    let xref = &text[sx..];
    let mut lines = xref.lines().skip(1);
    let count: usize = lines
        .next()
        .unwrap()
        .split(' ')
        .nth(1)
        .unwrap()
        .parse()
        .unwrap();
    lines.next(); // free entry
    for id in 1..count {
        let off: usize = lines.next().unwrap()[..10].parse().unwrap();
        assert!(
            b[off..].starts_with(format!("{id} 0 obj\n").as_bytes()),
            "object {id} offset"
        );
    }
    assert!(text.contains(&format!("/Size {count}")));
    let mut boxes = Vec::new();
    let mut rest = text.as_str();
    while let Some(i) = rest.find("/Type /Page /Parent") {
        let tail = &rest[i..];
        let m = tail.find("/MediaBox [0 0 ").unwrap() + "/MediaBox [0 0 ".len();
        let nums: Vec<f64> = tail[m..]
            .split(']')
            .next()
            .unwrap()
            .split(' ')
            .map(|s| s.parse().unwrap())
            .collect();
        boxes.push((nums[0], nums[1]));
        rest = &tail[m..];
    }
    let declared: usize = text
        .split("/Type /Pages /Count ")
        .nth(1)
        .unwrap()
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(declared, boxes.len(), "/Count matches the page objects");
    boxes
}

/// All content/metadata streams of the PDF, inflated when deflated, concatenated.
pub fn all_streams(b: &[u8]) -> Vec<u8> {
    use std::io::Read;
    let mut out = Vec::new();
    let mut pos = 0;
    while let Some(i) = find(&b[pos..], b">>\nstream\n") {
        let start = pos + i + 10;
        let dict_start = b[..start].windows(3).rposition(|w| w == b"<< ").unwrap();
        let dict = String::from_utf8_lossy(&b[dict_start..start]).to_string();
        let len: usize = dict
            .split("/Length ")
            .nth(1)
            .unwrap()
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let end = start + len;
        if dict.contains("FlateDecode") {
            let mut d = Vec::new();
            flate2::read::ZlibDecoder::new(&b[start..end])
                .read_to_end(&mut d)
                .unwrap();
            out.extend(d);
        } else {
            out.extend_from_slice(&b[start..end]);
        }
        out.push(b'\n');
        pos = end;
    }
    out
}

pub fn find(h: &[u8], n: &[u8]) -> Option<usize> {
    h.windows(n.len()).position(|w| w == n)
}

pub fn hex(s: &str) -> String {
    s.bytes().map(|b| format!("{b:02X}")).collect()
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
        ..RenderOptions::default()
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
        ..RenderOptions::default()
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
