//! Shared harness of the printer acceptance tests: a real Machine with a Generic Printer card in
//! slot 1, a program typed through the keyboard, and the captured events fed to a Spool with the
//! LX-310 engine writing the Arquivo de impressões in a temporary directory.
#![allow(dead_code)]

use apple2_core::{Machine, MachineConfig, Model};
use apple2_hostio::{
    capture_channel, CaptureReceiver, MemorySink, ParallelPrinterCard, PrinterOptions,
};
use apple2_printer::archive::Archive;
use apple2_printer::capture::{CaptureEvent, CloseReason, JobOrigin, Transport};
use apple2_printer::document::PageDocument;
use apple2_printer::lx310::{Glyphs, Lx310, Lx310Config};
use apple2_printer::paper::RenderOptions;
use apple2_printer::spool::Spool;
use sha2::{Digest, Sha256};
use std::path::Path;

pub struct Rig {
    pub machine: Machine,
    pub rx: CaptureReceiver,
    pub sink: MemorySink,
    pub events: Vec<CaptureEvent>,
}

impl Rig {
    pub fn new(model: Model, transport: Transport) -> Rig {
        Rig::with_cards(model, transport, |_| {})
    }

    /// Like [`Rig::new`], but `setup` may insert more cards (before the first reset).
    pub fn with_cards(model: Model, transport: Transport, setup: impl FnOnce(&mut Machine)) -> Rig {
        let mut machine = Machine::new(MachineConfig::for_model(model));
        let sink = MemorySink::new();
        let (tx, rx) = capture_channel();
        let name = match model {
            Model::Apple2 => "Apple ][",
            _ => "Apple //e Enhanced",
        };
        let card = ParallelPrinterCard::new(Box::new(sink.clone()), PrinterOptions::default())
            .with_virtual(
                tx,
                JobOrigin {
                    machine: name.into(),
                    slot: 1,
                },
                transport,
            );
        assert!(machine.insert_card(1, Box::new(card)));
        setup(&mut machine);
        machine.reset();
        let mut r = Rig {
            machine,
            rx,
            sink,
            events: Vec::new(),
        };
        r.frames(200);
        r
    }

    pub fn frames(&mut self, n: u32) {
        for _ in 0..n {
            self.machine.run_frame();
        }
        self.events.extend(self.rx.drain());
    }

    /// Types one line as the keyboard would, waiting for the monitor/BASIC to consume it.
    pub fn type_line(&mut self, line: &str) {
        while !self.machine.paste(&format!("{line}\r")) {
            self.frames(1);
        }
        self.frames(20 + line.len() as u32);
    }

    /// Runs frames until the card closed the job (idle limit elapsed) or `max` frames passed.
    pub fn run_until_closed(&mut self, max: u32) {
        for _ in 0..max {
            self.frames(1);
            if self
                .events
                .iter()
                .any(|e| matches!(e, CaptureEvent::Close { .. }))
            {
                return;
            }
        }
        panic!("job never closed");
    }

    /// All raw bytes of the captured stream.
    pub fn raw(&self) -> Vec<u8> {
        self.events
            .iter()
            .filter_map(|e| match e {
                CaptureEvent::Bytes { data, .. } => Some(data.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }
}

pub fn sha_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn lx310_factory() -> apple2_printer::spool::EngineFactory {
    Box::new(|| {
        Box::new(Lx310::new(Lx310Config {
            glyphs: Glyphs::FontOnly,
            ..Lx310Config::default()
        }))
    })
}

/// Feeds the events to a Spool whose archive lives in `dir`.
pub fn spool_events(dir: &Path, events: &[CaptureEvent]) -> Spool {
    let archive = Archive::open(dir).unwrap();
    let mut spool = Spool::new(archive, lx310_factory(), RenderOptions::default());
    for e in events {
        spool.handle(e.clone());
    }
    spool.publish_now();
    spool
}

/// Stable one-line-per-fact summary of the logical document (no floating point, no clock).
pub fn summary(doc: &PageDocument) -> String {
    let mut s = format!(
        "pages={} diagnostics={}\n",
        doc.pages.len(),
        doc.diagnostics.len()
    );
    for (i, p) in doc.pages.iter().enumerate() {
        let mut h = Sha256::new();
        for d in &p.dots {
            h.update(d.x.to_le_bytes());
            h.update(d.y.to_le_bytes());
            h.update([d.ink]);
        }
        let hash: String = h
            .finalize()
            .iter()
            .take(8)
            .map(|b| format!("{b:02x}"))
            .collect();
        s += &format!(
            "page {} size={}x{} dots={} dots_hash={}\n",
            i + 1,
            p.width,
            p.height,
            p.dots.len(),
            hash
        );
        for t in &p.text {
            s += &format!("  text x={} y={} {:?}\n", t.x, t.y, t.text);
        }
    }
    s
}

pub fn close_reasons(events: &[CaptureEvent]) -> Vec<CloseReason> {
    events
        .iter()
        .filter_map(|e| match e {
            CaptureEvent::Close { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect()
}

/// Directory of the fixtures generated by this project (no third-party software).
pub fn fixture_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/printer")
}

/// Compares `actual` with the golden file `name`; `UPDATE_GOLDEN=1` rewrites it.
pub fn golden(name: &str, actual: &[u8]) {
    let path = fixture_dir().join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let want = std::fs::read(&path).unwrap_or_else(|e| {
        panic!(
            "golden {} missing ({e}); run with UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert!(
        want == actual,
        "golden {name} differs; run with UPDATE_GOLDEN=1 to regenerate.\n--- expected\n{}\n--- actual\n{}",
        String::from_utf8_lossy(&want),
        String::from_utf8_lossy(actual)
    );
}

/// Engine whose glyph cells do not depend on the font (stable layout goldens).
pub fn placeholder_factory() -> apple2_printer::spool::EngineFactory {
    Box::new(|| {
        Box::new(Lx310::new(Lx310Config {
            glyphs: Glyphs::PlaceholderOnly,
            ..Lx310Config::default()
        }))
    })
}

/// Renders raw bytes straight through an engine (no machine involved).
pub fn render_raw(raw: &[u8], factory: &apple2_printer::spool::EngineFactory) -> PageDocument {
    let mut e = factory();
    e.begin(RenderOptions::default());
    for (i, c) in raw.chunks(61).enumerate() {
        e.feed((i * 61) as u64, c);
    }
    e.finish();
    e.document().clone()
}

/// Minimal structural validation of an exported PDF: header, trailer, xref offset and page count.
pub fn check_pdf_structure(pdf: &[u8], pages: usize) {
    assert!(pdf.starts_with(b"%PDF-1."), "header");
    let tail = &pdf[pdf.len().saturating_sub(64)..];
    assert!(
        String::from_utf8_lossy(tail).trim_end().ends_with("%%EOF"),
        "trailer"
    );
    let text = String::from_utf8_lossy(pdf);
    let sx = text.rfind("startxref").expect("startxref");
    let off: usize = text[sx + 9..]
        .split_whitespace()
        .next()
        .and_then(|n| n.parse().ok())
        .expect("xref offset");
    assert!(
        pdf[off..].starts_with(b"xref"),
        "startxref points to the xref table"
    );
    assert!(
        text.contains(&format!("/Count {pages}")),
        "page count {pages}"
    );
    let objs = text
        .match_indices("/Type /Page")
        .filter(|(i, _)| !text[i + 11..].starts_with('s'))
        .count();
    assert_eq!(objs, pages, "page objects");
}
