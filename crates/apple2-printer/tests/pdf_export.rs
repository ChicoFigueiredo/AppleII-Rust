//! Structure, geometry, text layer and metadata of the exported PDF (validated with poppler).
mod pdf_common;
use apple2_printer::document::{Dot, TextRun};
use apple2_printer::paper::{InkColor, PaperKind, Ribbon};
use apple2_printer::pdf::{export, export_with_report, PageSelection, PdfDate, PdfMeta};
use pdf_common::*;

fn write(name: &str, bytes: &[u8]) -> std::path::PathBuf {
    let p = scratch(name).join("out.pdf");
    std::fs::write(&p, bytes).unwrap();
    p
}

/// pdfinfo output with runs of blanks collapsed (its column padding varies).
fn squash(s: &str) -> String {
    s.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn pdfinfo(p: &std::path::Path) -> String {
    squash(&tool_text("pdfinfo", &[p.to_str().unwrap()]))
}

#[test]
fn poppler_tools_are_present() {
    for t in ["pdfinfo", "pdftoppm", "pdftotext"] {
        assert!(
            std::process::Command::new(t).arg("-v").output().is_ok(),
            "BLOCKED: {t} not installed"
        );
    }
}

#[test]
fn one_page_is_valid_with_exact_size() {
    let doc = doc_of(vec![pattern_page(684 * 30, 792 * 30)]);
    let o = opts(PaperKind::GreenBar, Ribbon::New, InkColor::Black);
    let bytes = export(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    assert!(bytes.starts_with(b"%PDF-1.7"));
    assert!(bytes.ends_with(b"%%EOF\n"));
    let info = pdfinfo(&write("one", &bytes));
    assert!(info.contains("Pages: 1"), "{info}");
    assert!(info.contains("Page size: 684 x 792 pts"), "{info}");
}

#[test]
fn twenty_five_pages_are_valid_and_small() {
    let pages: Vec<_> = (0..25u32)
        .map(|i| {
            let mut p = pattern_page(20520, 23760);
            p.dots.push(Dot {
                x: 1200 + i,
                y: 1200,
                ink: 0,
            });
            p
        })
        .collect();
    let doc = doc_of(pages);
    let o = opts(PaperKind::GreenBar, Ribbon::Worn, InkColor::Blue);
    let bytes = export(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    let path = write("p25", &bytes);
    let info = squash(&tool_text(
        "pdfinfo",
        &["-f", "1", "-l", "25", path.to_str().unwrap()],
    ));
    assert!(info.contains("Pages: 25"), "{info}");
    assert_eq!(info.matches("size: 684 x 792 pts").count(), 25, "{info}");
    assert!(
        bytes.len() < 200_000,
        "25 sparse pages took {} bytes",
        bytes.len()
    );
}

#[test]
fn page_size_in_points_is_units_over_30() {
    // A4 in units is not a whole number of points: the box must still be exact (4 decimals).
    let p = apple2_printer::paper::Paper::preset(PaperKind::A4);
    let doc = doc_of(vec![blank_page(p.width, p.height)]);
    let o = opts(PaperKind::A4, Ribbon::New, InkColor::Black);
    let bytes = export(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    let text = String::from_utf8_lossy(&bytes);
    let w = p.width as f64 / 30.0;
    let h = p.height as f64 / 30.0;
    assert!(
        text.contains(&format!("/MediaBox [0 0 {:.4} {:.4}]", w, h).replace(".0000", "")) || {
            // Trailing zeros are trimmed: compare numerically instead.
            let i = text.find("/MediaBox [0 0 ").unwrap() + "/MediaBox [0 0 ".len();
            let nums: Vec<f64> = text[i..]
                .split(']')
                .next()
                .unwrap()
                .split(' ')
                .map(|s| s.parse().unwrap())
                .collect();
            (nums[0] - w).abs() < 1e-4 && (nums[1] - h).abs() < 1e-4
        }
    );
    let info = pdfinfo(&write("a4", &bytes));
    assert!(info.contains("595.267 x 841.9 pts"), "{info}");
    // Mixed sizes in one document keep their own box.
    let doc = doc_of(vec![
        blank_page(30 * 100, 30 * 200),
        blank_page(30 * 300, 30 * 50),
    ]);
    let bytes = export(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    let info = squash(&tool_text(
        "pdfinfo",
        &[
            "-f",
            "1",
            "-l",
            "2",
            write("mixed", &bytes).to_str().unwrap(),
        ],
    ));
    assert!(info.contains("Page 1 size: 100 x 200 pts"), "{info}");
    assert!(info.contains("Page 2 size: 300 x 50 pts"), "{info}");
}

#[test]
fn text_layer_is_searchable() {
    let mut page = pattern_page(20520, 23760);
    // Characters outside WinAnsi (PC437 box drawing) go through the extra font + ToUnicode.
    page.text.push(TextRun {
        x: 600,
        y: 2000,
        advance: 216,
        height: 360,
        text: "┌─┐ ΣΩ".into(),
    });
    let doc = doc_of(vec![page]);
    let o = opts(PaperKind::A4, Ribbon::New, InkColor::Black);
    let (bytes, rep) = export_with_report(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    assert_eq!(rep.omitted_chars, 0);
    assert_eq!(rep.extra_mapped_chars, 5);
    let path = write("text", &bytes);
    let txt = tool_text("pdftotext", &["-enc", "UTF-8", path.to_str().unwrap(), "-"]);
    assert!(txt.contains("HELLO PRINTER"), "{txt:?}");
    assert!(txt.contains("Ação"), "{txt:?}");
    assert!(txt.contains("coração"), "{txt:?}");
    assert!(txt.contains("┌─┐"), "{txt:?}");
    assert!(txt.contains("ΣΩ"), "{txt:?}");
}

#[test]
fn too_many_distinct_exotic_chars_are_reported_not_silent() {
    let mut page = blank_page(20520, 23760);
    let s: String = (0x4E00u32..0x4E00 + 300)
        .filter_map(char::from_u32)
        .collect();
    page.text.push(TextRun {
        x: 0,
        y: 500,
        advance: 100,
        height: 300,
        text: s,
    });
    let o = opts(PaperKind::A4, Ribbon::New, InkColor::Black);
    let (bytes, rep) = export_with_report(
        &doc_of(vec![page]),
        &o,
        &PdfMeta::default(),
        &PageSelection::All,
    );
    assert_eq!(rep.extra_mapped_chars, 255);
    assert_eq!(rep.omitted_chars, 45);
    pdfinfo(&write("manychars", &bytes));
}

fn full_meta() -> PdfMeta {
    PdfMeta {
        title: "Relatório de Ação #1 (teste)".into(),
        author: "Chico Figueiredo".into(),
        date: Some(PdfDate {
            year: 2026,
            month: 10,
            day: 21,
            hour: 12,
            minute: 30,
            second: 5,
        }),
        apple2_model: "Apple //e Enhanced".into(),
        printer_profile: "Epson LX-310".into(),
        note: "Listagem do programa de teste\nsegunda linha".into(),
        app_version: "9.8.7".into(),
        cover: false,
        ribbon_seed: None,
    }
}

#[test]
fn metadata_is_always_in_the_properties() {
    let doc = doc_of(vec![pattern_page(20520, 23760)]);
    let o = opts(PaperKind::GreenBar, Ribbon::New, InkColor::Black);
    let bytes = export(&doc, &o, &full_meta(), &PageSelection::All);
    let path = write("meta", &bytes);
    let info = pdfinfo(&path);
    assert!(
        info.contains("Title: Relatório de Ação #1 (teste)"),
        "{info}"
    );
    assert!(info.contains("Author: Chico Figueiredo"), "{info}");
    assert!(info.contains("Apple //e Enhanced"), "{info}");
    assert!(info.contains("Epson LX-310"), "{info}");
    assert!(info.contains("Producer: apple2-rust 9.8.7"), "{info}");
    assert!(info.contains("2026"), "{info}");
    let xmp = tool_text("pdfinfo", &["-meta", path.to_str().unwrap()]);
    for needle in [
        "<dc:title>",
        "Relatório de Ação #1 (teste)",
        "<a2:Apple2Model>Apple //e Enhanced</a2:Apple2Model>",
        "<a2:PrinterProfile>Epson LX-310</a2:PrinterProfile>",
        "Listagem do programa de teste",
        "<a2:Apple2RustVersion>9.8.7</a2:Apple2RustVersion>",
        "2026-10-21T12:30:05Z",
    ] {
        assert!(xmp.contains(needle), "missing {needle:?} in {xmp}");
    }
    // Even the default metadata writes the properties.
    let bytes = export(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    let info = pdfinfo(&write("meta0", &bytes));
    assert!(info.contains("Title: Impressão do Apple II"), "{info}");
}

#[test]
fn cover_is_optional_and_off_by_default() {
    let doc = doc_of(vec![pattern_page(20520, 23760); 2]);
    let o = opts(PaperKind::GreenBar, Ribbon::New, InkColor::Black);
    assert!(!PdfMeta::default().cover);
    let plain = export(&doc, &o, &full_meta(), &PageSelection::All);
    assert!(pdfinfo(&write("nocover", &plain)).contains("Pages: 2"));
    let meta = PdfMeta {
        cover: true,
        ..full_meta()
    };
    let bytes = export(&doc, &o, &meta, &PageSelection::All);
    let path = write("cover", &bytes);
    assert!(pdfinfo(&path).contains("Pages: 3"));
    let first = tool_text(
        "pdftotext",
        &[
            "-f",
            "1",
            "-l",
            "1",
            "-enc",
            "UTF-8",
            path.to_str().unwrap(),
            "-",
        ],
    );
    for needle in [
        "Relatório de Ação #1 (teste)",
        "21/10/2026",
        "Apple //e Enhanced",
        "Epson LX-310",
        "segunda linha",
        "apple2-rust 9.8.7",
    ] {
        assert!(first.contains(needle), "cover lacks {needle:?}: {first}");
    }
    assert!(!first.contains("HELLO PRINTER"));
}

#[test]
fn page_selection() {
    let pages: Vec<_> = (1..=5u32)
        .map(|i| blank_page(30 * 100 * i, 30 * 100))
        .collect();
    let doc = doc_of(pages);
    let o = opts(PaperKind::A4, Ribbon::New, InkColor::Black);
    let widths = |sel: PageSelection| {
        let b = export(&doc, &o, &PdfMeta::default(), &sel);
        let p = write("sel", &b);
        squash(&tool_text(
            "pdfinfo",
            &["-f", "1", "-l", "9", p.to_str().unwrap()],
        ))
        .lines()
        .filter(|l| l.contains(" size:") && l.starts_with("Page "))
        .map(|l| l.split_whitespace().nth(3).unwrap().to_string())
        .collect::<Vec<_>>()
    };
    assert_eq!(
        widths(PageSelection::All),
        ["100", "200", "300", "400", "500"]
    );
    assert_eq!(
        widths(PageSelection::Range { start: 1, end: 3 }),
        ["200", "300"]
    );
    assert_eq!(widths(PageSelection::List(vec![4, 0, 99])), ["500", "100"]);
    // Empty selection still yields a valid, blank page.
    assert_eq!(widths(PageSelection::List(vec![])).len(), 1);
}

#[test]
fn in_progress_document_exports_consistently() {
    let mut doc = doc_of(vec![pattern_page(20520, 23760)]);
    doc.last_in_progress = true;
    let o = opts(PaperKind::GreenBar, Ribbon::New, InkColor::Black);
    let b = export(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    assert!(pdfinfo(&write("inprog", &b)).contains("Pages: 1"));
}
