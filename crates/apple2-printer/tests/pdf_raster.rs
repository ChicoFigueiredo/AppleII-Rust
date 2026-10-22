//! Rasterisation of the exported PDF: golden pixels, dot/background sampling, paper decoration,
//! ribbon determinism. Goldens live in tests/golden/pdf (regenerate with UPDATE_GOLDEN=1 after a
//! deliberate change; poppler version used: see docs/printer/pdf.md).
mod pdf_common;
use apple2_printer::document::{Dot, PageDocument};
use apple2_printer::paper::{InkColor, PaperKind, RenderOptions, Ribbon};
use apple2_printer::pdf::{export, PageSelection, PdfDate, PdfMeta};
use pdf_common::*;

const W: u32 = 3 * 2160;
const H: u32 = 2 * 2160;

fn export_file(
    name: &str,
    doc: &PageDocument,
    o: &RenderOptions,
    meta: &PdfMeta,
) -> std::path::PathBuf {
    let p = scratch(name).join("out.pdf");
    std::fs::write(&p, export(doc, o, meta, &PageSelection::All)).unwrap();
    p
}

/// Exact golden comparison only with the poppler version the goldens were made with.
fn golden_version_matches() -> bool {
    let want = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden/pdf/POPPLER_VERSION"),
    )
    .unwrap_or_default();
    let have = poppler_version().unwrap_or_default();
    if want.trim() == have {
        return true;
    }
    eprintln!("golden exato pulado: poppler {} != {}", have, want.trim());
    false
}

fn golden(name: &str, img: &(usize, usize, usize, Vec<u8>)) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/pdf")
        .join(format!("{name}.pgm"));
    let mut bytes = format!("P5\n{} {}\n255\n", img.0, img.1).into_bytes();
    bytes.extend_from_slice(&img.3);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, &bytes).unwrap();
        let v = poppler_version().unwrap();
        std::fs::write(path.with_file_name("POPPLER_VERSION"), format!("{v}\n")).unwrap();
        return;
    }
    if !golden_version_matches() {
        return;
    }
    let want = std::fs::read(&path)
        .unwrap_or_else(|_| panic!("golden {path:?} missing: run with UPDATE_GOLDEN=1"));
    let want = parse_pnm(&want);
    assert_eq!((want.0, want.1), (img.0, img.1), "{name}: size");
    let diff = want.3.iter().zip(&img.3).filter(|(a, b)| a != b).count();
    assert_eq!(diff, 0, "{name}: {diff} pixels differ from the golden");
}

fn doc() -> PageDocument {
    doc_of(vec![pattern_page(W, H)])
}

#[test]
fn golden_plain_paper_72_and_150_dpi() {
    need_poppler!();
    let o = opts_sized(PaperKind::A4, W, H, Ribbon::New, InkColor::Black);
    let p = export_file("g_plain", &doc(), &o, &PdfMeta::default());
    golden("plain_72", &raster(&p, 72, true));
    golden("plain_150", &raster(&p, 150, true));
}

#[test]
fn golden_green_bar_72_dpi() {
    need_poppler!();
    let o = opts_sized(PaperKind::GreenBar, W, H, Ribbon::New, InkColor::Black);
    let p = export_file("g_green", &doc(), &o, &PdfMeta::default());
    golden("greenbar_72", &raster(&p, 72, true));
}

#[test]
fn golden_worn_ribbon_150_dpi() {
    need_poppler!();
    let o = opts_sized(PaperKind::A4, W, H, Ribbon::Worn, InkColor::Black);
    let p = export_file("g_worn", &doc(), &o, &PdfMeta::default());
    golden("worn_150", &raster(&p, 150, true));
}

#[test]
fn dot_and_background_samples() {
    need_poppler!();
    let mut page = blank_page(W, H);
    // Dot centred on pixel (150, 150) at 150 dpi.
    let (dx, dy) = (2167, 2167);
    page.dots.push(Dot {
        x: dx,
        y: dy,
        ink: 0,
    });
    let doc = doc_of(vec![page]);
    type Check = fn(&[u8]) -> bool;
    let cases: [(InkColor, Check); 3] = [
        (InkColor::Black, |p| p == [0, 0, 0]),
        (InkColor::Blue, |p| p[2] > p[0] + 60 && p[0] < 40),
        (InkColor::Purple, |p| p[0] > 60 && p[2] > 60 && p[1] < 60),
    ];
    for (ink, check) in cases {
        let o = opts_sized(PaperKind::A4, W, H, Ribbon::New, ink);
        let img = raster(
            &export_file("samples", &doc, &o, &PdfMeta::default()),
            150,
            false,
        );
        let c = px(&img, at(dx, 150), at(dy, 150));
        assert!(check(&c), "{ink:?}: dot pixel {c:?}");
        assert_eq!(px(&img, 10, 10), [255, 255, 255], "background");
        assert_eq!(
            px(&img, at(dx, 150) + 6, at(dy, 150)),
            [255, 255, 255],
            "next to the dot"
        );
    }
}

#[test]
fn dot_diameter_matches_the_public_constant() {
    need_poppler!();
    // A row of 600 dots 1/2160" apart makes a bar as thick as one dot. At 600 dpi it is
    // DOT_DIAMETER / 2160 * 600 px (8.3 px for 30 units), within one pixel.
    let expect = (apple2_printer::pdf::DOT_DIAMETER * 600 / 2160) as usize;
    let mut page = blank_page(W, H);
    for i in 0..600u32 {
        page.dots.push(Dot {
            x: 1000 + i,
            y: 1000,
            ink: 0,
        });
    }
    let o = opts_sized(PaperKind::A4, W, H, Ribbon::New, InkColor::Black);
    let p = export_file("diam", &doc_of(vec![page]), &o, &PdfMeta::default());
    let img = raster(&p, 600, true);
    let x = at(1300, 600);
    let thick = (0..img.1).filter(|&y| px(&img, x, y)[0] < 128).count();
    assert!(
        (expect..=expect + 2).contains(&thick),
        "bar thickness {thick}px at 600 dpi"
    );
}

#[test]
fn paper_decoration_never_moves_dots() {
    need_poppler!();
    let d = doc();
    let gray = |kind| {
        let o = opts_sized(kind, W, H, Ribbon::New, InkColor::Black);
        raster(&export_file("deco", &d, &o, &PdfMeta::default()), 150, true).3
    };
    let base = gray(PaperKind::A4);
    assert!(base.iter().any(|&v| v < 64));
    for kind in [
        PaperKind::GreenBar,
        PaperKind::BlueBar,
        PaperKind::PlainContinuous,
        PaperKind::Letter,
    ] {
        let g = gray(kind);
        // Core ink pixels of the bare page stay ink; pixels without ink stay light (the
        // decoration is much lighter than the ink). Anti-aliased edge pixels blend with the
        // tint underneath and may differ slightly, so they are not compared.
        let lost = base
            .iter()
            .zip(&g)
            .filter(|(a, b)| **a <= 20 && **b > 64)
            .count();
        let gained = base
            .iter()
            .zip(&g)
            .filter(|(a, b)| **a == 255 && **b < 120)
            .count();
        assert_eq!((lost, gained), (0, 0), "{kind:?}");
        let edge = base
            .iter()
            .zip(&g)
            .filter(|(a, b)| **a < 255 && a.abs_diff(**b) > 60)
            .count();
        // A handful of anti-aliased edge pixels may blend with the tint (a few more with the 30-unit dot).
        assert!(edge <= 8, "{kind:?}: {edge} edge pixels differ a lot");
    }
}

#[test]
fn green_bar_pattern_holes_and_perforation() {
    need_poppler!();
    let (w, h) = (9 * 2160 + 1080, 3 * 2160);
    let o = opts_sized(PaperKind::GreenBar, w, h, Ribbon::New, InkColor::Black);
    let img = raster(
        &export_file(
            "bars",
            &doc_of(vec![blank_page(w, h)]),
            &o,
            &PdfMeta::default(),
        ),
        72,
        false,
    );
    let mid = at(w / 2, 72);
    // Band 0 (0..1/2") white, band 1 (1/2".."1") green, band 2 white again, band 3 green.
    let band = |k: u32| px(&img, mid, at(k * 1080 + 540, 72));
    assert_eq!(band(0), [255, 255, 255]);
    let g = band(1);
    assert!(g[1] > g[0] + 15 && g[1] > g[2] + 15, "green band {g:?}");
    assert_eq!(band(2), [255, 255, 255]);
    assert_eq!(band(3), g);
    // Tractor hole: centre 1/4" from the edge, 1/4" from the top; grey, darker than paper.
    let hole = px(&img, at(540, 72), at(540, 72));
    assert!(hole[0] < 235 && hole[0] > 150, "hole {hole:?}");
    // Strip outside the perforation stays white (bars end at the perforation).
    assert_eq!(px(&img, at(300, 72), at(1080 + 540, 72)), [255, 255, 255]);
    // Blue bars are blue.
    let o = opts_sized(PaperKind::BlueBar, w, h, Ribbon::New, InkColor::Black);
    let img = raster(
        &export_file(
            "bars2",
            &doc_of(vec![blank_page(w, h)]),
            &o,
            &PdfMeta::default(),
        ),
        72,
        false,
    );
    let b = px(&img, mid, at(1080 + 540, 72));
    assert!(b[2] > b[0] + 15, "blue band {b:?}");
    // A4 and Letter are plain white.
    let o = opts_sized(PaperKind::Letter, w, h, Ribbon::New, InkColor::Black);
    let img = raster(
        &export_file(
            "bars3",
            &doc_of(vec![blank_page(w, h)]),
            &o,
            &PdfMeta::default(),
        ),
        72,
        false,
    );
    assert!(img.3.iter().all(|&v| v == 255));
}

#[test]
fn worn_ribbon_is_deterministic_and_irregular() {
    let d = doc();
    let ow = opts_sized(PaperKind::A4, W, H, Ribbon::Worn, InkColor::Black);
    let no_date = PdfMeta::default();
    let a = export(&d, &ow, &no_date, &PageSelection::All);
    let b = export(&d, &ow, &no_date, &PageSelection::All);
    assert_eq!(a, b, "two exports must be byte-identical");
    // Dates change only the properties: the inflated page content is identical.
    let dated = |s| PdfMeta {
        date: Some(PdfDate {
            year: 2026,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: s,
        }),
        ..PdfMeta::default()
    };
    let pages = |m: &PdfMeta| {
        let all = all_streams(&export(&d, &ow, m, &PageSelection::All));
        let i = find(&all, b"0.03333333333333").unwrap();
        all[i..].to_vec()
    };
    assert_eq!(pages(&dated(1)), pages(&dated(59)));
    let seeded = |seed| PdfMeta {
        ribbon_seed: Some(seed),
        ..PdfMeta::default()
    };
    assert_eq!(pages(&seeded(7)), pages(&seeded(7)));
    assert_ne!(pages(&seeded(7)), pages(&seeded(8)));
    let on = opts_sized(PaperKind::A4, W, H, Ribbon::New, InkColor::Black);
    assert_ne!(
        all_streams(&export(&d, &on, &no_date, &PageSelection::All)),
        all_streams(&a)
    );
    need_poppler!();
    // Different dates change only the properties, not the drawing.
    let dated = |s| PdfMeta {
        date: Some(PdfDate {
            year: 2026,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: s,
        }),
        ..PdfMeta::default()
    };
    let p1 = export_file("w1", &d, &ow, &dated(1));
    let p2 = export_file("w2", &d, &ow, &dated(59));
    assert_eq!(raster(&p1, 150, true), raster(&p2, 150, true));
    // Irregular: dot intensities differ, and it differs from the new ribbon.
    let img = raster(&p1, 150, true);
    let mut levels: Vec<u8> = img.3.iter().copied().filter(|&v| v < 200).collect();
    levels.sort_unstable();
    levels.dedup();
    assert!(
        levels.len() >= 4,
        "worn ribbon produced {} gray levels",
        levels.len()
    );
    let on = opts_sized(PaperKind::A4, W, H, Ribbon::New, InkColor::Black);
    let pn = export_file("wn", &d, &on, &no_date);
    assert_ne!(raster(&pn, 150, true), img);
    // Explicit job seed: different seeds, different look; same seed, same look.
    let s = |seed| PdfMeta {
        ribbon_seed: Some(seed),
        ..PdfMeta::default()
    };
    let r = |n: &str, seed| raster(&export_file(n, &d, &ow, &s(seed)), 150, true);
    assert_eq!(r("s1", 7), r("s1b", 7));
    assert_ne!(r("s1", 7), r("s2", 8));
    // A repeated hit on the same spot is darker than a single one with a worn ribbon.
    let mut single = blank_page(W, H);
    single.dots.push(Dot {
        x: 2167,
        y: 2167,
        ink: 0,
    });
    let mut triple = single.clone();
    triple.dots.extend([single.dots[0]; 2]);
    let v = |pg| {
        raster(
            &export_file("hits", &doc_of(vec![pg]), &ow, &s(3)),
            150,
            true,
        )
        .3[150 * (3 * 150) + 150]
    };
    assert!(v(triple) <= v(single));
}
