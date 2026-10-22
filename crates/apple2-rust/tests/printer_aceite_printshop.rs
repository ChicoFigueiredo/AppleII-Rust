//! Acceptance of the virtual LX-310 with The New Print Shop (issue #104). The discs are
//! third-party software and never enter the repository: the owner drops
//! `The New Print Shop program disk side A.nib` and `... side B.nib` in
//! `target/conformance-cache/printer-apps/printshop/`. Without them the test ends as BLOCKED
//! (message on stderr, no failure).
//!
//! The test drives the real program through the keyboard: Setup or Convert -> Printer (Epson LX
//! 80/86/90/800), Interface Card (Apple II Parallel), Save and Exit; then Graphic Editor ->
//! Create a New Graphic, draws a box with a diagonal and prints it (P). The discs are copied to a
//! temporary directory because Setup saves the choice on side A (the originals are never
//! touched). Run it with `cargo test --release -p apple2-rust --test printer_aceite_printshop`
//! (in a debug build it is skipped: the emulator is too slow). Step by step with the keys: `docs/printer/aceite.md`.

use apple2_config::card::{PrinterDestination, PrinterSpec, PrinterTransport};
use apple2_config::{Settings, SlotCardSpec};
use apple2_core::keyboard::NavKey;
use apple2_hostio::capture_channel;
use apple2_printer::archive::{Archive, JobState};
use apple2_printer::capture::{CaptureEvent, CloseReason};
use apple2_printer::lx310::{Glyphs, Lx310, Lx310Config};
use apple2_printer::paper::RenderOptions;
use apple2_printer::pdf::{export, PageSelection, PdfMeta};
use apple2_printer::spool::Spool;
use apple2_rust_shell::backend::DefaultBackend;
use apple2_rust_shell::emu::{Command, Emulator};
use apple2_rust_shell::factory::set_thread_capture;
use apple2_rust_shell::keymap::KeyEvent;
use std::path::{Path, PathBuf};

const SIDE_A: &str = "The New Print Shop program disk side A.nib";
const SIDE_B: &str = "The New Print Shop program disk side B.nib";

fn cache_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/conformance-cache/printer-apps/printshop")
}

struct Session {
    emu: Emulator,
    rx: apple2_hostio::CaptureReceiver,
    events: Vec<CaptureEvent>,
}

impl Session {
    fn new(dir: &Path, transport: PrinterTransport) -> Session {
        let (tx, rx) = capture_channel();
        set_thread_capture(Some(tx));
        let mut s = Settings::default();
        s.slots.0[1] = SlotCardSpec::GenericPrinter(PrinterSpec {
            destination: PrinterDestination::VirtualOnly,
            transport,
            ..PrinterSpec::default()
        });
        let mut emu = Emulator::new(s, Box::new(DefaultBackend), 48_000).unwrap();
        for (drive, name) in [SIDE_A, SIDE_B].iter().enumerate() {
            emu.apply(Command::InsertDisk {
                drive,
                path: dir.join(name),
                read_only: false,
            });
        }
        emu.apply(Command::PowerCycle);
        Session {
            emu,
            rx,
            events: Vec::new(),
        }
    }

    fn run(&mut self, frames: u32) {
        for _ in 0..frames {
            self.emu.run_frame();
            self.events.extend(self.rx.drain());
        }
    }

    fn key(&mut self, k: KeyEvent) {
        self.emu.apply(Command::Key(k));
        self.run(6);
    }

    fn press(&mut self, k: KeyEvent, times: usize) {
        for _ in 0..times {
            self.key(k);
        }
    }

    /// Navigation of the program: Setup, then the Graphic Editor with the drawing, then Print.
    fn print_graphic(&mut self) {
        let ret = || KeyEvent::Ascii(0x0D);
        let esc = || KeyEvent::Ascii(0x1B);
        let down = || KeyEvent::Nav(NavKey::Down);
        let right = || KeyEvent::Nav(NavKey::Right);
        let left = || KeyEvent::Nav(NavKey::Left);
        let up = || KeyEvent::Nav(NavKey::Up);
        let space = || KeyEvent::Ascii(b' ');
        self.run(900);
        self.key(space()); // title screen
        self.run(1500);
        // Main Menu -> Setup or Convert -> Printer Setup -> Printer.
        self.press(down(), 7);
        self.key(ret());
        self.run(200);
        self.key(ret());
        self.run(1800);
        self.key(ret());
        self.run(400);
        self.key(ret());
        self.run(300);
        // Printer list: page 6 has "Epson LX 80/86/90/800" as 3rd item.
        for _ in 0..5 {
            self.key(right());
            self.run(200);
        }
        self.press(down(), 3);
        self.key(ret());
        self.run(300);
        // Interface Card -> Apple II Parallel (2nd item). Slot is already 1.
        self.key(ret());
        self.run(300);
        self.key(down());
        self.key(ret());
        self.run(300);
        self.key(ret());
        self.run(300);
        self.key(ret());
        self.run(300);
        // Back out of Setup with Save and Exit (writes the choice on side A).
        self.key(esc());
        self.run(600);
        self.press(down(), 2);
        self.key(ret());
        self.run(1500);
        // Main Menu -> Graphic Editor -> Create a New Graphic.
        self.press(down(), 6);
        self.key(ret());
        self.run(1500);
        self.key(down());
        self.key(ret());
        self.run(3000);
        // Draw: SPACE toggles the dot under the cursor. Box 24 x 12 plus a stair diagonal.
        self.key(space());
        for (k, n) in [(right(), 24), (down(), 12), (left(), 24), (up(), 12)] {
            for _ in 0..n {
                self.key(k);
                self.key(space());
            }
        }
        for _ in 0..11 {
            self.key(right());
            self.key(space());
            self.key(down());
            self.key(space());
        }
        self.run(60);
        self.key(KeyEvent::Ascii(b'P'));
        // Print: the job closes by inactivity.
        for _ in 0..12000 {
            self.run(1);
            if self
                .events
                .iter()
                .any(|e| matches!(e, CaptureEvent::Close { .. }))
            {
                return;
            }
        }
        panic!("the Print Shop job never closed");
    }

    fn raw(&self) -> Vec<u8> {
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

fn stage_discs() -> Option<tempdir::Dir> {
    let cache = cache_dir();
    if !cache.join(SIDE_A).exists() || !cache.join(SIDE_B).exists() {
        eprintln!("BLOCKED: disco ausente em {}", cache.display());
        return None;
    }
    let d = tempdir::Dir::new("printshop");
    for n in [SIDE_A, SIDE_B] {
        let to = d.path().join(n);
        std::fs::copy(cache.join(n), &to).unwrap();
    }
    Some(d)
}

/// Tiny self-cleaning temporary directory (this crate has no `tempfile` dev-dependency).
mod tempdir {
    use std::path::{Path, PathBuf};
    pub struct Dir(PathBuf);
    impl Dir {
        pub fn new(tag: &str) -> Dir {
            let p = std::env::temp_dir().join(format!("apple2-rust-{tag}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(&p).unwrap();
            Dir(p)
        }
        pub fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn dots_of(raw: &[u8]) -> (usize, Vec<apple2_printer::document::PageDocument>) {
    let mut e = Lx310::new(Lx310Config {
        glyphs: Glyphs::FontOnly,
        ..Lx310Config::default()
    });
    use apple2_printer::engine::PrinterEngine;
    e.begin(RenderOptions::default());
    e.feed(0, raw);
    e.finish();
    let doc = e.document().clone();
    (doc.pages.iter().map(|p| p.dots.len()).sum(), vec![doc])
}

#[test]
fn print_shop_graphic_through_the_virtual_lx310() {
    // Boot plus navigation is ~13 000 emulated frames: seconds in release, ~10 minutes in debug.
    if cfg!(debug_assertions) && std::env::var_os("PRINTSHOP_FORCE_DEBUG").is_none() {
        eprintln!(
            "SKIPPED: aceite do Print Shop só roda com --release (ou PRINTSHOP_FORCE_DEBUG=1)"
        );
        return;
    }
    let Some(dir) = stage_discs() else { return };
    let mut s = Session::new(dir.path(), PrinterTransport::Auto);
    s.print_graphic();
    let raw = s.raw();

    // One job closed by inactivity; the driver starts with ESC @ and uses ESC L bit-image.
    let opens = s
        .events
        .iter()
        .filter(|e| matches!(e, CaptureEvent::Open { .. }))
        .count();
    assert_eq!(opens, 1, "um único Trabalho");
    assert!(
        raw.starts_with(&[0x1B, 0x40]),
        "{:02x?}",
        &raw[..8.min(raw.len())]
    );
    assert!(raw.windows(2).any(|w| w == [0x1B, 0x4C]), "ESC L bit-image");
    // The driver writes straight to the port: bit 7 (top pin) arrives in Auto.
    assert!(raw.iter().any(|b| b & 0x80 != 0), "bit 7 chegou inteiro");

    // Spool + LX-310: one job, dots, no diagnostics, valid PDF.
    let adir = tempdir::Dir::new("printshop-archive");
    let archive = Archive::open(adir.path()).unwrap();
    let factory: apple2_printer::spool::EngineFactory = Box::new(|| {
        Box::new(Lx310::new(Lx310Config {
            glyphs: Glyphs::FontOnly,
            ..Lx310Config::default()
        }))
    });
    let mut spool = Spool::new(archive, factory, RenderOptions::default());
    for e in &s.events {
        spool.handle(e.clone());
    }
    spool.publish_now();
    let snap = spool.snapshot_handle().get();
    assert_eq!(snap.jobs.len(), 1);
    let job = &snap.jobs[0];
    assert_eq!(job.state, Some(JobState::Completed));
    assert_eq!(job.close_reason, Some(CloseReason::Idle));
    let doc = spool.render(&job.id, RenderOptions::default()).unwrap();
    assert!(
        doc.diagnostics.is_empty(),
        "comando desconhecido: {:?}",
        doc.diagnostics
    );
    let dots: usize = doc.pages.iter().map(|p| p.dots.len()).sum();
    assert!(dots > 50, "pontos: {dots}");
    let pdf = export(
        &doc,
        &RenderOptions::default(),
        &PdfMeta::default(),
        &PageSelection::All,
    );
    assert!(pdf.starts_with(b"%PDF-1."));
    assert!(
        String::from_utf8_lossy(&pdf[pdf.len().saturating_sub(64)..])
            .trim_end()
            .ends_with("%%EOF")
    );
    if let Some(out) = std::env::var_os("PRINTSHOP_PDF") {
        std::fs::write(out, &pdf).unwrap();
    }

    // Transporte de 7 bits: o mesmo desenho perde a agulha de cima (menos pontos).
    let (dots8, _) = dots_of(&raw);
    let raw7: Vec<u8> = raw.iter().map(|b| b & 0x7F).collect();
    let (dots7, _) = dots_of(&raw7);
    assert!(
        dots7 < dots8,
        "7 bits {dots7} deveria ter menos pontos que {dots8}"
    );
}
