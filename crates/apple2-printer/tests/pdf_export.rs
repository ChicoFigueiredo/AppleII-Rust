//! Structure, geometry, text layer and metadata of the exported PDF. Pure-Rust structural checks
//! always run; the poppler cross-checks print BLOCKED and are skipped where poppler is missing.
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

fn pdfinfo_args(p: &std::path::Path, extra: &[&str]) -> String {
    let mut args: Vec<&str> = extra.to_vec();
    args.push(p.to_str().unwrap());
    squash(&tool_text("pdfinfo", &args))
}

fn pdfinfo(p: &std::path::Path) -> String {
    pdfinfo_args(p, &[])
}

fn a_doc(n: usize) -> apple2_printer::document::PageDocument {
    doc_of(vec![pattern_page(20520, 23760); n])
}

fn green() -> apple2_printer::paper::RenderOptions {
    opts(PaperKind::GreenBar, Ribbon::New, InkColor::Black)
}

/// One test that reports how many poppler-dependent checks cannot run here.
#[test]
fn poppler_summary() {
    let missing: Vec<&str> = ["pdfinfo", "pdftoppm", "pdftotext"]
        .into_iter()
        .filter(|t| !available(t))
        .collect();
    if missing.is_empty() {
        eprintln!("poppler available: {:?}", poppler_version());
    } else {
        eprintln!(
            "BLOCKED: {} poppler tool(s) missing ({}); 17 poppler-dependent checks across \
             pdf_export/pdf_raster/pdf_files are skipped, structural checks still run",
            missing.len(),
            missing.join(", ")
        );
    }
}

#[test]
fn one_page_is_valid_with_exact_size() {
    let bytes = export(
        &a_doc(1),
        &green(),
        &PdfMeta::default(),
        &PageSelection::All,
    );
    assert_eq!(structure(&bytes), [(684.0, 792.0)]);
    need_poppler!();
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
    let o = opts(PaperKind::GreenBar, Ribbon::Worn, InkColor::Blue);
    let bytes = export(&doc_of(pages), &o, &PdfMeta::default(), &PageSelection::All);
    assert_eq!(structure(&bytes), vec![(684.0, 792.0); 25]);
    assert!(
        bytes.len() < 200_000,
        "25 sparse pages took {} bytes",
        bytes.len()
    );
    need_poppler!();
    let info = pdfinfo_args(&write("p25", &bytes), &["-f", "1", "-l", "25"]);
    assert!(info.contains("Pages: 25"), "{info}");
    assert_eq!(info.matches("size: 684 x 792 pts").count(), 25, "{info}");
}

#[test]
fn page_size_in_points_is_units_over_30() {
    let p = apple2_printer::paper::Paper::preset(PaperKind::A4);
    let doc = doc_of(vec![blank_page(p.width, p.height)]);
    let o = opts(PaperKind::A4, Ribbon::New, InkColor::Black);
    let bytes = export(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    let boxes = structure(&bytes);
    assert!((boxes[0].0 - p.width as f64 / 30.0).abs() < 1e-4);
    assert!((boxes[0].1 - p.height as f64 / 30.0).abs() < 1e-4);
    // Mixed sizes in one document keep their own box.
    let doc = doc_of(vec![
        blank_page(30 * 100, 30 * 200),
        blank_page(30 * 300, 30 * 50),
    ]);
    let mixed = export(&doc, &o, &PdfMeta::default(), &PageSelection::All);
    assert_eq!(structure(&mixed), [(100.0, 200.0), (300.0, 50.0)]);
    need_poppler!();
    let info = pdfinfo(&write("a4", &bytes));
    assert!(info.contains("595.267 x 841.9 pts"), "{info}");
    let info = pdfinfo_args(&write("mixed", &mixed), &["-f", "1", "-l", "2"]);
    assert!(info.contains("Page 1 size: 100 x 200 pts"), "{info}");
    assert!(info.contains("Page 2 size: 300 x 50 pts"), "{info}");
}

fn text_doc() -> apple2_printer::document::PageDocument {
    let mut page = pattern_page(20520, 23760);
    // Characters outside WinAnsi (PC437 box drawing) go through the extra font + ToUnicode.
    page.text.push(TextRun {
        x: 600,
        y: 2000,
        advance: 216,
        height: 360,
        text: "┌─┐ ΣΩ".into(),
    });
    doc_of(vec![page])
}

#[test]
fn text_layer_is_in_the_file() {
    let o = opts(PaperKind::A4, Ribbon::New, InkColor::Black);
    let (bytes, rep) =
        export_with_report(&text_doc(), &o, &PdfMeta::default(), &PageSelection::All);
    assert_eq!((rep.omitted_chars, rep.extra_mapped_chars), (0, 5));
    structure(&bytes);
    let streams = all_streams(&bytes);
    let s = String::from_utf8_lossy(&streams);
    assert!(s.contains("3 Tr"), "invisible render mode");
    assert!(s.contains(&format!("<{}> Tj", hex("HELLO PRINTER"))));
    // "Ação, coração" in WinAnsi: ã = E3, ç = E7.
    assert!(s.contains("<41E7E36F2C20636F7261E7E36F> Tj"), "{s}");
    // ToUnicode of the extra font maps code 1 to U+250C.
    assert!(s.contains("<01> <250C>"));
}

#[test]
fn text_layer_is_searchable() {
    need_poppler!();
    let o = opts(PaperKind::A4, Ribbon::New, InkColor::Black);
    let bytes = export(&text_doc(), &o, &PdfMeta::default(), &PageSelection::All);
    let path = write("text", &bytes);
    let txt = tool_text("pdftotext", &["-enc", "UTF-8", path.to_str().unwrap(), "-"]);
    for needle in ["HELLO PRINTER", "Ação", "coração", "┌─┐", "ΣΩ"] {
        assert!(txt.contains(needle), "{needle:?} not in {txt:?}");
    }
}

#[test]
fn double_width_text_stays_searchable_as_whole_words() {
    need_poppler!();
    let mut page = blank_page(20520, 23760);
    // Double width at 10 cpi: the advance is twice the 216-unit pitch.
    for (y, text) in [(1000, "Largura dupla"), (9000, "APPLE II-RUST")] {
        page.text.push(TextRun {
            x: 600,
            y,
            advance: 432,
            height: 360,
            text: text.into(),
        });
    }
    let o = opts(PaperKind::A4, Ribbon::New, InkColor::Black);
    let (bytes, _) = export_with_report(
        &doc_of(vec![page]),
        &o,
        &PdfMeta::default(),
        &PageSelection::All,
    );
    let s = String::from_utf8_lossy(&all_streams(&bytes)).into_owned();
    assert!(
        !s.contains(" Tc") && !s.contains(" Tz"),
        "no spacing operators: {s}"
    );
    let path = write("double", &bytes);
    let txt = tool_text("pdftotext", &["-enc", "UTF-8", path.to_str().unwrap(), "-"]);
    for needle in ["Largura dupla", "APPLE II-RUST"] {
        assert!(txt.contains(needle), "{needle:?} not in {txt:?}");
    }
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
    structure(&bytes);
    need_poppler!();
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

fn utf16_hex(s: &str) -> String {
    let mut o = String::from("<FEFF");
    for u in s.encode_utf16() {
        o.push_str(&format!("{u:04X}"));
    }
    o + ">"
}

#[test]
fn metadata_is_in_the_file_properties() {
    let bytes = export(&a_doc(1), &green(), &full_meta(), &PageSelection::All);
    structure(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains(&format!(
        "/Title {}",
        utf16_hex("Relatório de Ação #1 (teste)")
    )));
    assert!(text.contains("/Author (Chico Figueiredo)"));
    assert!(text.contains("/Producer (apple2-rust 9.8.7)"));
    assert!(text.contains("/CreationDate (D:20261021123005Z)"));
    assert!(text.contains(&format!("/Apple2Model {}", "(Apple //e Enhanced)")));
    assert!(text.contains("/PrinterProfile (Epson LX-310)"));
    assert!(text.contains("/Apple2RustVersion (9.8.7)"));
    let xmp = String::from_utf8_lossy(&all_streams(&bytes)).to_string();
    for needle in [
        "Relatório de Ação #1 (teste)",
        "<a2:Apple2Model>Apple //e Enhanced</a2:Apple2Model>",
        "<a2:PrinterProfile>Epson LX-310</a2:PrinterProfile>",
        "<xmp:CreateDate>2026-10-21T12:30:05Z</xmp:CreateDate>",
    ] {
        assert!(xmp.contains(needle), "XMP lacks {needle:?}");
    }
    // Even the default metadata writes the properties, and no dates without a date.
    let d = export(
        &a_doc(1),
        &green(),
        &PdfMeta::default(),
        &PageSelection::All,
    );
    let dt = String::from_utf8_lossy(&d);
    assert!(dt.contains("/Producer (apple2-rust "));
    assert!(!dt.contains("/CreationDate"));
}

#[test]
fn metadata_is_read_by_poppler() {
    need_poppler!();
    let bytes = export(&a_doc(1), &green(), &full_meta(), &PageSelection::All);
    let path = write("meta", &bytes);
    let info = pdfinfo(&path);
    assert!(
        info.contains("Title: Relatório de Ação #1 (teste)"),
        "{info}"
    );
    assert!(info.contains("Author: Chico Figueiredo"), "{info}");
    assert!(info.contains("Apple //e Enhanced"), "{info}");
    assert!(info.contains("Producer: apple2-rust 9.8.7"), "{info}");
    let xmp = tool_text("pdfinfo", &["-meta", path.to_str().unwrap()]);
    assert!(
        xmp.contains("<a2:Apple2Model>Apple //e Enhanced</a2:Apple2Model>"),
        "{xmp}"
    );
    assert!(xmp.contains("2026-10-21T12:30:05Z"), "{xmp}");
}

#[test]
fn cover_is_optional_and_off_by_default() {
    assert!(!PdfMeta::default().cover);
    let plain = export(&a_doc(2), &green(), &full_meta(), &PageSelection::All);
    assert_eq!(structure(&plain).len(), 2);
    let meta = PdfMeta {
        cover: true,
        ..full_meta()
    };
    let bytes = export(&a_doc(2), &green(), &meta, &PageSelection::All);
    assert_eq!(structure(&bytes).len(), 3);
    let cover_text = String::from_utf8_lossy(&all_streams(&bytes)).to_string();
    assert!(cover_text.contains(&format!("<{}> Tj", hex("Impressora: Epson LX-310"))));
    need_poppler!();
    let path = write("cover", &bytes);
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
    let widths = |sel: PageSelection| -> Vec<f64> {
        structure(&export(&doc, &o, &PdfMeta::default(), &sel))
            .iter()
            .map(|b| b.0)
            .collect()
    };
    assert_eq!(
        widths(PageSelection::All),
        [100.0, 200.0, 300.0, 400.0, 500.0]
    );
    assert_eq!(
        widths(PageSelection::Range { start: 1, end: 3 }),
        [200.0, 300.0]
    );
    assert_eq!(widths(PageSelection::List(vec![4, 0, 99])), [500.0, 100.0]);
    // Empty selection still yields a valid, blank page.
    assert_eq!(widths(PageSelection::List(vec![])).len(), 1);
}

#[test]
fn in_progress_document_exports_consistently() {
    let mut doc = a_doc(1);
    doc.last_in_progress = true;
    let b = export(&doc, &green(), &PdfMeta::default(), &PageSelection::All);
    assert_eq!(structure(&b).len(), 1);
    need_poppler!();
    assert!(pdfinfo(&write("inprog", &b)).contains("Pages: 1"));
}

/// The PDF names the version it was built from (the workspace package version, resolved at
/// compile time), in /Creator, /Producer and the XMP tool.
#[test]
fn the_pdf_names_the_crate_version_it_was_built_from() {
    let meta = PdfMeta::default();
    assert_eq!(meta.app_version, env!("CARGO_PKG_VERSION"));
    let bytes = export(&a_doc(1), &green(), &meta, &PageSelection::All);
    let text = String::from_utf8_lossy(&bytes);
    let want = format!("apple2-rust {}", env!("CARGO_PKG_VERSION"));
    assert!(text.contains(&format!("/Creator ({want})")), "{want}");
}
