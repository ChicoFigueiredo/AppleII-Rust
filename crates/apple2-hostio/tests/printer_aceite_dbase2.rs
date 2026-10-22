//! Acceptance of the LX-310 virtual printer with dBase II on CP/M (issue #104): a ][+ with a Z80
//! SoftCard in slot 4, the Generic Printer in slot 1 and the owner's CP/M + dBase II disc in the
//! Disk II of slot 6. The test boots CP/M, starts dBase II, creates and fills a small database
//! typed by the test, prints with `SET PRINT ON` / `LIST` (CP/M `LST:` -> SoftCard BIOS ->
//! parallel card) and checks the resulting Trabalho, its text layer and the PDF.
//!
//! The disc is third-party software: it lives in `target/conformance-cache/printer-apps/dbase2/`
//! (never versioned). It is read into memory and never written back (the test checks its SHA-256
//! before and after). Without it the test ends as BLOCKED, without failing. Set
//! `DBASE2_PDF_OUT=<file.pdf>` to also save the PDF.

mod printer_aceite_common;

use apple2_cards::SoftCard;
use apple2_core::text_screen::text_screen;
use apple2_core::Model;
use apple2_printer::archive::JobState;
use apple2_printer::capture::{CaptureEvent, CloseReason, Transport};
use apple2_printer::paper::RenderOptions;
use apple2_printer::pdf::{export, PageSelection, PdfMeta};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use printer_aceite_common::*;
use std::path::PathBuf;

fn disc_path() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/conformance-cache/printer-apps/dbase2");
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .find(|p| {
            p.extension()
                .and_then(|x| x.to_str())
                .is_some_and(|x| x.eq_ignore_ascii_case("dsk"))
        })
}

/// Types a line one character at a time: CP/M and dBase poll the keyboard and drop keys that
/// arrive in bursts (a whole-line paste loses characters).
fn type_slow(r: &mut Rig, line: &str) {
    for ch in line.chars().chain(['\r']) {
        while !r.machine.paste(&ch.to_string()) {
            r.frames(1);
        }
        r.frames(4);
    }
}

/// Types `line`, lets the system settle and checks `expect` appears on the 40-column screen.
fn step(r: &mut Rig, line: &str, expect: &str) {
    type_slow(r, line);
    for _ in 0..40 {
        r.frames(10);
        let seen = text_screen(&r.machine).iter().any(|l| {
            if expect == "." {
                l.trim_end() == "."
            } else {
                l.contains(expect)
            }
        });
        if seen {
            r.frames(60);
            return;
        }
    }
    panic!(
        "after {line:?} the screen never showed {expect:?}:\n{}",
        text_screen(&r.machine).join("\n")
    );
}

const REPORT_TITLE: &str = "RELATORIO DBASE II";
const ROWS: &[&str] = &[
    "00001  MARIA SILVA  CAMPINAS     123.45",
    "00002  JOAO LIMA    CURITIBA       9.90",
    "00003  ANA COSTA    RECIFE      1500.00",
];

#[test]
fn dbase_ii_cpm_lst_prints_one_job_with_text_layer_and_pdf() {
    let Some(path) = disc_path() else {
        eprintln!("BLOCKED: disco ausente em target/conformance-cache/printer-apps/dbase2");
        return;
    };
    let bytes = std::fs::read(&path).unwrap();
    let disc_sha = sha_hex(&bytes);
    let image = DiskImage::from_bytes(&path.file_name().unwrap().to_string_lossy(), bytes).unwrap();

    let mut r = Rig::with_cards(Model::Apple2Plus, Transport::Auto, |m| {
        assert!(m.insert_card(4, Box::new(SoftCard::new())));
        let mut d = Disk2Card::new();
        d.insert_disk(0, image);
        assert!(m.insert_card(6, Box::new(d)));
    });
    r.frames(600);
    assert!(
        text_screen(&r.machine).iter().any(|l| l.starts_with("A>")),
        "CP/M did not reach the A> prompt:\n{}",
        text_screen(&r.machine).join("\n")
    );

    // The shipped disc is full (no room for a new .DBF): free space in memory only.
    for f in ["DBASEII.DOC", "ZIP.COM", "INSTALL.COM", "TESTE.DBF"] {
        step(&mut r, &format!("ERA {f}"), "A>ERA");
    }
    step(&mut r, "DBASE", "RETURN FOR NONE");
    step(&mut r, "", ".");
    step(&mut r, "CREATE TESTE", "001");
    step(&mut r, "NOME,C,12", "002");
    step(&mut r, "CIDADE,C,10", "003");
    step(&mut r, "VALOR,N,7,2", "004");
    step(&mut r, "", "INPUT DATA NOW");
    step(&mut r, "Y", "RECORD # 00001");
    // Numeric field of width 7 advances by itself when full; the others end with RETURN.
    for (i, rec) in [
        ("MARIA SILVA", "CAMPINAS", "0123.45"),
        ("JOAO LIMA", "CURITIBA", "0009.90"),
        ("ANA COSTA", "RECIFE", "1500.00"),
    ]
    .iter()
    .enumerate()
    {
        let next = format!("RECORD # {:05}", i + 2);
        type_slow(&mut r, rec.0);
        type_slow(&mut r, rec.1);
        step(&mut r, rec.2, &next);
    }
    step(&mut r, "", ".");
    step(&mut r, "USE TESTE", "USE TESTE");
    assert!(r.raw().is_empty(), "nothing printed before SET PRINT ON");

    step(&mut r, "SET PRINT ON", "SET PRINT ON");
    step(&mut r, &format!("? \"{REPORT_TITLE}\""), REPORT_TITLE);
    step(&mut r, "LIST", "00003");
    step(&mut r, "SET PRINT OFF", "SET PRINT OFF");
    // The card closes the Trabalho by inactivity.
    r.run_until_closed(1500);

    // The disc image was only read.
    assert_eq!(sha_hex(&std::fs::read(&path).unwrap()), disc_sha);

    // One Trabalho: Open, Bytes..., Close(Idle); CP/M 7-bit text, no ESC/P commands from dBase.
    let opens = r
        .events
        .iter()
        .filter(|e| matches!(e, CaptureEvent::Open { .. }))
        .count();
    assert_eq!(opens, 1);
    assert_eq!(close_reasons(&r.events), vec![CloseReason::Idle]);
    let raw = r.raw();
    let printed = String::from_utf8_lossy(&raw).into_owned();
    assert!(raw.iter().all(|b| *b < 0x80), "{printed:?}");
    assert!(!raw.contains(&0x1B));
    assert!(printed.contains(REPORT_TITLE));
    for row in ROWS {
        assert!(printed.contains(row), "{printed:?}");
    }
    // The legacy Printer.txt sink saw the same text.
    assert!(r.sink.text().contains(ROWS[2]));
    // Goldens hold only what the test typed and dBase printed (no bytes from the disc).
    golden("aceite_dbase2.raw", &raw);

    let dir = tempfile::tempdir().unwrap();
    let mut spool = spool_events(dir.path(), &r.events);
    let snap = spool.snapshot_handle().get();
    assert!(snap.active.is_none());
    assert_eq!(snap.jobs.len(), 1, "um único Trabalho de impressão");
    let job = &snap.jobs[0];
    assert_eq!(job.state, Some(JobState::Completed));
    assert_eq!(job.close_reason, Some(CloseReason::Idle));
    assert_eq!(job.pages, 1);
    assert_eq!(job.total_bytes, raw.len() as u64);
    let man = spool.archive().manifest(&job.id).unwrap();
    assert_eq!(man.sha256.as_deref(), Some(sha_hex(&raw).as_str()));

    let doc = spool.render(&job.id, RenderOptions::default()).unwrap();
    assert!(doc.diagnostics.is_empty(), "{:?}", doc.diagnostics);
    assert_eq!(doc.pages.len(), 1);
    let lines: Vec<&str> = doc.pages[0].text.iter().map(|t| t.text.as_str()).collect();
    let layer = lines.join("\n");
    assert!(layer.contains(REPORT_TITLE), "{layer}");
    for row in ROWS {
        assert!(layer.contains(row), "{layer}");
    }

    let pdf = export(
        &doc,
        &RenderOptions::default(),
        &PdfMeta::default(),
        &PageSelection::All,
    );
    check_pdf_structure(&pdf, 1);
    if let Some(out) = std::env::var_os("DBASE2_PDF_OUT") {
        std::fs::write(out, &pdf).unwrap();
    }

    let doc_ph = render_raw(&raw, &placeholder_factory());
    assert!(doc_ph.diagnostics.is_empty());
    golden("aceite_dbase2.doc.txt", summary(&doc_ph).as_bytes());
}
