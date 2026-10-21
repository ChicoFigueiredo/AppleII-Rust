//! Shared helpers for the archive and spool tests: a dummy engine and event builders.
#![allow(dead_code)]

use apple2_printer::archive::{Archive, NewJob};
use apple2_printer::capture::{CaptureEvent, CloseReason, JobOrigin, Transport};
use apple2_printer::document::{Diagnostic, Page, PageDocument, TextRun};
use apple2_printer::engine::PrinterEngine;
use apple2_printer::paper::{Paper, PaperKind, RenderOptions};
use apple2_printer::spool::{EngineFactory, Spool};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub const LINES_PER_PAGE: usize = 60;

/// Line-based engine: one TextRun per '\n'-terminated line, 60 lines per page; byte 0xFF is a
/// diagnostic. The page width records the paper so reprints with other paper differ.
pub struct DummyEngine {
    options: RenderOptions,
    doc: PageDocument,
    line: Vec<u8>,
    lines: usize,
    changes: u64,
    pub feeds: Arc<AtomicUsize>,
}

impl DummyEngine {
    pub fn new(feeds: Arc<AtomicUsize>) -> Self {
        DummyEngine {
            options: RenderOptions::default(),
            doc: PageDocument::default(),
            line: Vec::new(),
            lines: 0,
            changes: 0,
            feeds,
        }
    }

    fn end_line(&mut self) {
        if self.lines.is_multiple_of(LINES_PER_PAGE) {
            self.doc.pages.push(Page {
                width: self.options.paper.width,
                height: self.options.paper.height,
                ..Page::default()
            });
        }
        let text = String::from_utf8_lossy(&self.line).into_owned();
        let y = (self.lines % LINES_PER_PAGE) as u32 * 100;
        self.doc.pages.last_mut().unwrap().text.push(TextRun {
            x: 0,
            y,
            advance: 10,
            height: 10,
            text,
        });
        self.lines += 1;
        self.line.clear();
        self.doc.last_in_progress = true;
        self.changes += 1;
    }
}

impl PrinterEngine for DummyEngine {
    fn begin(&mut self, options: RenderOptions) {
        *self = DummyEngine {
            options,
            feeds: self.feeds.clone(),
            ..DummyEngine::new(self.feeds.clone())
        };
    }
    fn feed(&mut self, offset: u64, data: &[u8]) {
        self.feeds.fetch_add(1, Ordering::SeqCst);
        for (i, &b) in data.iter().enumerate() {
            match b {
                b'\n' => self.end_line(),
                0xFF => {
                    self.doc.diagnostics.push(Diagnostic {
                        offset: offset + i as u64,
                        bytes: vec![b],
                        message: "byte 0xFF".into(),
                    });
                    self.changes += 1;
                }
                _ => self.line.push(b),
            }
        }
    }
    fn document(&self) -> &PageDocument {
        &self.doc
    }
    fn take_revision(&mut self) -> u64 {
        std::mem::take(&mut self.changes)
    }
    fn finish(&mut self) {
        if !self.line.is_empty() {
            self.end_line();
        }
        self.doc.last_in_progress = false;
        self.changes += 1;
    }
}

pub fn factory(feeds: Arc<AtomicUsize>) -> EngineFactory {
    Box::new(move || Box::new(DummyEngine::new(feeds.clone())))
}

pub fn origin() -> JobOrigin {
    JobOrigin {
        machine: "Apple //e Enhanced #1 ção".into(),
        slot: 1,
    }
}

pub fn new_job(n: u64) -> NewJob {
    NewJob {
        capture_job: n,
        origin: origin(),
        transport: Transport::EightBit,
        profile: "Epson LX-310".into(),
        options: RenderOptions::default(),
    }
}

pub fn a4() -> RenderOptions {
    RenderOptions {
        paper: Paper::preset(PaperKind::A4),
        ..RenderOptions::default()
    }
}

pub fn open(job: u64) -> CaptureEvent {
    CaptureEvent::Open {
        job,
        origin: origin(),
        transport: Transport::EightBit,
    }
}

pub fn bytes(job: u64, offset: u64, data: &[u8]) -> CaptureEvent {
    CaptureEvent::Bytes {
        job,
        offset,
        data: data.to_vec(),
    }
}

pub fn close(job: u64, reason: CloseReason) -> CaptureEvent {
    CaptureEvent::Close { job, reason }
}

pub fn spool_in(dir: &std::path::Path) -> (Spool, Arc<AtomicUsize>) {
    let feeds = Arc::new(AtomicUsize::new(0));
    let archive = Archive::open(dir).unwrap();
    (
        Spool::new(archive, factory(feeds.clone()), RenderOptions::default()),
        feeds,
    )
}

pub fn sha_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
