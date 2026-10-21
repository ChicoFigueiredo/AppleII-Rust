//! Atomic writing of the exported PDF: no partial files, no silent overwrite, odd file names.
mod pdf_common;
use apple2_printer::paper::{InkColor, PaperKind, Ribbon};
use apple2_printer::pdf::{export_to_path, write_atomic, PageSelection, PdfMeta};
use pdf_common::*;
use std::io::ErrorKind;

fn run(path: &std::path::Path, overwrite: bool) -> std::io::Result<()> {
    let doc = doc_of(vec![pattern_page(20520, 23760)]);
    let o = opts(PaperKind::GreenBar, Ribbon::New, InkColor::Black);
    export_to_path(
        path,
        &doc,
        &o,
        &PdfMeta::default(),
        &PageSelection::All,
        overwrite,
    )
    .map(|_| ())
}

fn names(dir: &std::path::Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    v.sort();
    v
}

#[test]
fn writes_a_valid_pdf_and_leaves_no_temporary_file() {
    let d = scratch("atomic_ok");
    let p = d.join("a.pdf");
    run(&p, false).unwrap();
    assert!(tool_text("pdfinfo", &[p.to_str().unwrap()]).contains("Pages:"));
    assert_eq!(names(&d), ["a.pdf"]);
}

#[test]
fn never_overwrites_without_the_flag() {
    let d = scratch("atomic_exists");
    let p = d.join("a.pdf");
    std::fs::write(&p, b"precious").unwrap();
    let e = run(&p, false).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::AlreadyExists);
    assert_eq!(std::fs::read(&p).unwrap(), b"precious");
    assert_eq!(names(&d), ["a.pdf"]);
    // Explicit overwrite replaces it atomically.
    run(&p, true).unwrap();
    assert!(std::fs::read(&p).unwrap().starts_with(b"%PDF-"));
    assert_eq!(names(&d), ["a.pdf"]);
}

#[test]
fn failure_leaves_nothing_behind() {
    let d = scratch("atomic_fail");
    // Missing directory.
    assert!(run(&d.join("nope/a.pdf"), false).is_err());
    assert!(names(&d).is_empty());
    // Destination is a directory: the final move fails and the temporary file is removed.
    std::fs::create_dir(d.join("dir.pdf")).unwrap();
    assert!(run(&d.join("dir.pdf"), true).is_err());
    assert!(run(&d.join("dir.pdf"), false).is_err());
    assert_eq!(names(&d), ["dir.pdf"]);
    // No file name.
    assert!(write_atomic(std::path::Path::new("/"), b"x", false).is_err());
}

#[test]
fn odd_file_names_work() {
    let d = scratch("atomic_names");
    let all = [
        "relatório de março #1.pdf",
        "日本語 ファイル.pdf",
        "Ação (cópia) 100%.pdf",
        "with  two  spaces.pdf",
        "#hash.pdf",
        "emoji 🖨️.pdf",
    ];
    for n in all {
        let p = d.join(n);
        run(&p, false).unwrap();
        assert!(
            tool_text("pdfinfo", &[p.to_str().unwrap()]).contains("Pages:"),
            "{n}"
        );
    }
    let mut want: Vec<String> = all.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(names(&d), want);
}

#[test]
fn concurrent_exports_to_one_name_leave_exactly_one_winner() {
    let d = scratch("atomic_race");
    let p = d.join("race.pdf");
    let results: Vec<_> = (0..8)
        .map(|_| {
            let p = p.clone();
            std::thread::spawn(move || run(&p, false))
        })
        .collect::<Vec<_>>()
        .into_iter()
        .map(|h| h.join().unwrap())
        .collect();
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    assert_eq!(names(&d), ["race.pdf"]);
}
