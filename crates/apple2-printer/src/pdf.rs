//! PDF export of a [`PageDocument`] (Arquivo de impressões / "Salvar PDF").
//!
//! The PDF is written by hand (it only needs a handful of object kinds) so the crate stays free of
//! heavy dependencies; `flate2` deflates the content streams. See `docs/printer/pdf.md`.
//!
//! What the file contains, per Page:
//! - the page box = `Page.width x Page.height` (30 units = 1 pt, exact);
//! - the Papel decoration (bars, tractor holes, perforations), drawn *under* the ink;
//! - every [`Dot`](crate::document::Dot) as a vector circle (a zero-length stroke with round caps,
//!   one path per ribbon-intensity bucket);
//! - an invisible text layer (render mode 3) from `Page.text`, for search and copy.
//!
//! Everything is drawn in a coordinate system where one user unit is 1/2160", y pointing down, so
//! the dot coordinates are written as the exact integers of the document.

use crate::document::{Page, PageDocument};
use crate::paper::{InkColor, PaperKind, RenderOptions, Ribbon};
use crate::units::PER_POINT;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;

/// Diameter of a drawn dot, in units: 25 units = 0.294 mm = 0.833 pt. The Epson 9-pin print head
/// leaves a dot of roughly 0.29 mm (0.0114") on paper; the exact size depends on ribbon, paper and
/// head wear, so this is an approximation (documented in `docs/printer/pdf.md`).
pub const DOT_DIAMETER_UNITS: u32 = 25;

/// Tractor hole diameter: 5/32" = 337.5 units (drawn as radius 168.75).
const HOLE_RADIUS_UNITS: f64 = 168.75;
/// Tractor hole pitch: 1/2".
const HOLE_PITCH_UNITS: u32 = 1080;
/// Width of the tractor strip (hole centre line is half of it): 1/2".
const STRIP_UNITS: u32 = 1080;
/// Green/blue bar height: 1/2" (3 lines at 6 lpi).
const BAR_UNITS: u32 = 1080;
/// Number of ribbon intensity buckets of `Ribbon::Worn`.
const WORN_LEVELS: usize = 8;
/// Dots per stroked path (keeps single operators small for viewers).
const DOTS_PER_PATH: usize = 4000;

/// A UTC timestamp for the document properties. The caller supplies it (the exporter never reads
/// the clock, so exports are reproducible).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PdfDate {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl PdfDate {
    fn pdf(&self) -> String {
        format!(
            "D:{:04}{:02}{:02}{:02}{:02}{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
    fn iso(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
    fn human(&self) -> String {
        format!(
            "{:02}/{:02}/{:04} {:02}:{:02} UTC",
            self.day, self.month, self.year, self.hour, self.minute
        )
    }
}

/// Document properties. Always written (Info dictionary and XMP).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfMeta {
    pub title: String,
    pub author: String,
    /// `None` omits the dates (the output then does not depend on the time at all).
    pub date: Option<PdfDate>,
    /// Apple II model, e.g. "Apple //e Enhanced".
    pub apple2_model: String,
    /// Perfil de impressora, e.g. "Epson LX-310".
    pub printer_profile: String,
    /// Free personal note of the user.
    pub note: String,
    /// apple2-rust version that produced the file.
    pub app_version: String,
    /// Prepend a cover sheet (folha de rosto). Off by default.
    pub cover: bool,
    /// Seed of the worn ribbon (e.g. derived from the Trabalho de impressão's raw bytes). `None`
    /// derives it from each page's dots, which is deterministic too.
    pub ribbon_seed: Option<u64>,
}

impl Default for PdfMeta {
    fn default() -> Self {
        PdfMeta {
            title: "Impressão do Apple II".into(),
            author: String::new(),
            date: None,
            apple2_model: String::new(),
            printer_profile: "Epson LX-310".into(),
            note: String::new(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            cover: false,
            ribbon_seed: None,
        }
    }
}

/// Which pages of the document to export (0-based indices into `PageDocument::pages`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum PageSelection {
    #[default]
    All,
    /// `start..end` (end exclusive), clamped to the document.
    Range { start: usize, end: usize },
    /// Explicit pages, in the given order; out-of-range indices are ignored.
    List(Vec<usize>),
}

/// What the export had to approximate; never silent (R02).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExportReport {
    /// Pages written, cover excluded.
    pub pages: usize,
    /// Characters of the text layer that fit neither WinAnsi nor the extra 255-code table; they
    /// were replaced by a blank in the invisible layer (the dots are unaffected).
    pub omitted_chars: usize,
    /// Distinct non-WinAnsi characters mapped through the extra font + ToUnicode.
    pub extra_mapped_chars: usize,
}

/// Exports the selected pages as a PDF.
pub fn export(
    doc: &PageDocument,
    opts: &RenderOptions,
    meta: &PdfMeta,
    pages: &PageSelection,
) -> Vec<u8> {
    export_with_report(doc, opts, meta, pages).0
}

/// Like [`export`], also reporting what was approximated. An empty selection yields one blank page
/// of the chosen paper (a PDF needs at least one page).
pub fn export_with_report(
    doc: &PageDocument,
    opts: &RenderOptions,
    meta: &PdfMeta,
    selection: &PageSelection,
) -> (Vec<u8>, ExportReport) {
    let selected: Vec<&Page> = match selection {
        PageSelection::All => doc.pages.iter().collect(),
        PageSelection::Range { start, end } => doc
            .pages
            .iter()
            .enumerate()
            .filter(|(i, _)| i >= start && i < end)
            .map(|(_, p)| p)
            .collect(),
        PageSelection::List(l) => l.iter().filter_map(|&i| doc.pages.get(i)).collect(),
    };

    let mut report = ExportReport {
        pages: selected.len(),
        ..Default::default()
    };
    let mut extra = ExtraFont::default();

    // Page bodies: (width, height, content stream bytes, visible?).
    let mut bodies: Vec<(u32, u32, Vec<u8>)> = Vec::new();
    if meta.cover {
        bodies.push((
            opts.paper.width,
            opts.paper.height,
            cover_content(opts.paper.width, opts.paper.height, meta),
        ));
    }
    for page in &selected {
        bodies.push((
            page.width,
            page.height,
            page_content(page, opts, meta, &mut extra, &mut report),
        ));
    }
    if bodies.is_empty() {
        let (w, h) = (opts.paper.width, opts.paper.height);
        let blank = Page {
            width: w,
            height: h,
            ..Default::default()
        };
        bodies.push((
            w,
            h,
            page_content(&blank, opts, meta, &mut extra, &mut report),
        ));
    }
    report.extra_mapped_chars = extra.chars.len();

    // Object ids.
    const CATALOG: usize = 1;
    const PAGES: usize = 2;
    const INFO: usize = 3;
    const XMP: usize = 4;
    const RES: usize = 5;
    const F_COURIER: usize = 6;
    const F_EXTRA: usize = 7;
    const F_TOUNI: usize = 8;
    const F_HELV: usize = 9;
    const FIRST_PAGE: usize = 10;

    let mut w = Writer::default();
    w.begin();

    w.obj(
        CATALOG,
        format!("<< /Type /Catalog /Pages {PAGES} 0 R /Metadata {XMP} 0 R /Lang (pt-BR) >>")
            .as_bytes(),
    );
    let kids: String = (0..bodies.len())
        .map(|i| format!("{} 0 R", FIRST_PAGE + 2 * i))
        .collect::<Vec<_>>()
        .join(" ");
    w.obj(
        PAGES,
        format!("<< /Type /Pages /Count {} /Kids [{kids}] >>", bodies.len()).as_bytes(),
    );
    w.obj(INFO, info_dict(meta).as_bytes());
    w.stream(
        XMP,
        "/Type /Metadata /Subtype /XML",
        xmp(meta).as_bytes(),
        false,
    );

    // Resources: ribbon alpha states and the fonts.
    let mut gs = String::new();
    for k in 0..WORN_LEVELS {
        let _ = write!(
            gs,
            "/A{k} << /CA {} /ca {} >> ",
            num(worn_alpha(k)),
            num(worn_alpha(k))
        );
    }
    let extra_ref = if extra.chars.is_empty() {
        String::new()
    } else {
        format!("/FX {F_EXTRA} 0 R ")
    };
    w.obj(
        RES,
        format!(
            "<< /ExtGState << {gs}>> /Font << /FC {F_COURIER} 0 R {extra_ref}/FH {F_HELV} 0 R >> >>"
        )
        .as_bytes(),
    );
    w.obj(
        F_COURIER,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding >>",
    );
    if extra.chars.is_empty() {
        // Keep the object table dense: an unused placeholder.
        w.obj(F_EXTRA, b"null");
        w.obj(F_TOUNI, b"null");
    } else {
        let diffs = vec!["/space"; extra.chars.len()].join(" ");
        w.obj(
            F_EXTRA,
            format!(
                "<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding << /Type /Encoding \
                 /BaseEncoding /WinAnsiEncoding /Differences [1 {diffs}] >> /ToUnicode {F_TOUNI} 0 R >>"
            )
            .as_bytes(),
        );
        w.stream(F_TOUNI, "", extra.to_unicode().as_bytes(), false);
    }
    w.obj(
        F_HELV,
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    );

    for (i, (pw, ph, content)) in bodies.iter().enumerate() {
        let pid = FIRST_PAGE + 2 * i;
        w.obj(
            pid,
            format!(
                "<< /Type /Page /Parent {PAGES} 0 R /MediaBox [0 0 {} {}] /Resources {RES} 0 R \
                 /Contents {} 0 R >>",
                pt(*pw),
                pt(*ph),
                pid + 1
            )
            .as_bytes(),
        );
        w.stream(pid + 1, "", content, true);
    }

    let total = FIRST_PAGE + 2 * bodies.len();
    (w.finish(total, CATALOG, INFO), report)
}

// ---------------------------------------------------------------------------------------------
// Low-level PDF writer
// ---------------------------------------------------------------------------------------------

#[derive(Default)]
struct Writer {
    buf: Vec<u8>,
    offsets: BTreeMap<usize, usize>,
}

impl Writer {
    fn begin(&mut self) {
        // The binary comment marks the file as binary for transfer tools.
        self.buf.extend_from_slice(b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n");
    }

    fn obj(&mut self, id: usize, body: &[u8]) {
        self.offsets.insert(id, self.buf.len());
        self.buf
            .extend_from_slice(format!("{id} 0 obj\n").as_bytes());
        self.buf.extend_from_slice(body);
        self.buf.extend_from_slice(b"\nendobj\n");
    }

    fn stream(&mut self, id: usize, dict: &str, data: &[u8], compress: bool) {
        let (payload, filter) = if compress {
            (deflate(data), "/Filter /FlateDecode ")
        } else {
            (data.to_vec(), "")
        };
        self.offsets.insert(id, self.buf.len());
        self.buf.extend_from_slice(
            format!(
                "{id} 0 obj\n<< {dict} {filter}/Length {} >>\nstream\n",
                payload.len()
            )
            .as_bytes(),
        );
        self.buf.extend_from_slice(&payload);
        self.buf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    fn finish(mut self, count: usize, root: usize, info: usize) -> Vec<u8> {
        let xref = self.buf.len();
        let mut x = format!("xref\n0 {count}\n0000000000 65535 f \n");
        for id in 1..count {
            let off = self.offsets.get(&id).copied().unwrap_or(0);
            let _ = writeln!(x, "{off:010} 00000 n ");
        }
        // Deterministic file identifier from the content written so far.
        let h1 = fnv64(&self.buf);
        let h2 = fnv64(&h1.to_le_bytes());
        let _ = write!(
            x,
            "trailer\n<< /Size {count} /Root {root} 0 R /Info {info} 0 R /ID [<{h1:016x}{h2:016x}> \
             <{h1:016x}{h2:016x}>] >>\nstartxref\n{xref}\n%%EOF\n"
        );
        self.buf.extend_from_slice(x.as_bytes());
        self.buf
    }
}

fn deflate(data: &[u8]) -> Vec<u8> {
    let mut enc = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(9));
    enc.write_all(data).expect("writing to a Vec cannot fail");
    enc.finish().expect("writing to a Vec cannot fail")
}

fn fnv64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Number with up to 4 decimals, trailing zeros trimmed (deterministic text).
fn num(v: f64) -> String {
    let mut s = format!("{v:.4}");
    while s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

/// Units to PDF points.
fn pt(units: u32) -> String {
    num(f64::from(units) / f64::from(PER_POINT))
}

// ---------------------------------------------------------------------------------------------
// Properties
// ---------------------------------------------------------------------------------------------

/// PDF text string: PDFDocEncoding-safe ASCII literal, or UTF-16BE with BOM as hex.
fn text_string(s: &str) -> String {
    if s.chars().all(|c| (' '..='~').contains(&c)) {
        let mut out = String::from("(");
        for c in s.chars() {
            if matches!(c, '(' | ')' | '\\') {
                out.push('\\');
            }
            out.push(c);
        }
        out.push(')');
        out
    } else {
        let mut out = String::from("<FEFF");
        for u in s.encode_utf16() {
            let _ = write!(out, "{u:04X}");
        }
        out.push('>');
        out
    }
}

fn producer(meta: &PdfMeta) -> String {
    format!("apple2-rust {}", meta.app_version)
}

fn subject(meta: &PdfMeta) -> String {
    let parts: Vec<&str> = [meta.apple2_model.as_str(), meta.printer_profile.as_str()]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect();
    format!("Impressão do Apple II — {}", parts.join(" — "))
}

fn info_dict(meta: &PdfMeta) -> String {
    let mut s = String::from("<< ");
    let _ = write!(s, "/Title {} ", text_string(&meta.title));
    let _ = write!(s, "/Author {} ", text_string(&meta.author));
    let _ = write!(s, "/Subject {} ", text_string(&subject(meta)));
    let _ = write!(s, "/Creator {} ", text_string(&producer(meta)));
    let _ = write!(s, "/Producer {} ", text_string(&producer(meta)));
    if let Some(d) = &meta.date {
        let _ = write!(s, "/CreationDate ({0}) /ModDate ({0}) ", d.pdf());
    }
    let _ = write!(s, "/Apple2Model {} ", text_string(&meta.apple2_model));
    let _ = write!(s, "/PrinterProfile {} ", text_string(&meta.printer_profile));
    let _ = write!(s, "/Note {} ", text_string(&meta.note));
    let _ = write!(s, "/Apple2RustVersion {} ", text_string(&meta.app_version));
    s.push_str(">>");
    s
}

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .chars()
        .filter(|&c| c == '\n' || c == '\t' || c >= ' ')
        .collect()
}

fn xmp(meta: &PdfMeta) -> String {
    let dates = match &meta.date {
        Some(d) => format!(
            "<xmp:CreateDate>{0}</xmp:CreateDate><xmp:ModifyDate>{0}</xmp:ModifyDate>",
            d.iso()
        ),
        None => String::new(),
    };
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
         <x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF \
         xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\
         <rdf:Description rdf:about=\"\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" \
         xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\" xmlns:pdf=\"http://ns.adobe.com/pdf/1.3/\" \
         xmlns:a2=\"https://github.com/ChicoFigueiredo/AppleII-Rust/ns/printer/1.0/\">\
         <dc:format>application/pdf</dc:format>\
         <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{title}</rdf:li></rdf:Alt></dc:title>\
         <dc:creator><rdf:Seq><rdf:li>{author}</rdf:li></rdf:Seq></dc:creator>\
         <dc:description><rdf:Alt><rdf:li xml:lang=\"x-default\">{subject}</rdf:li></rdf:Alt></dc:description>\
         <xmp:CreatorTool>{producer}</xmp:CreatorTool>{dates}\
         <pdf:Producer>{producer}</pdf:Producer>\
         <a2:Apple2Model>{model}</a2:Apple2Model>\
         <a2:PrinterProfile>{profile}</a2:PrinterProfile>\
         <a2:Note>{note}</a2:Note>\
         <a2:Apple2RustVersion>{ver}</a2:Apple2RustVersion>\
         </rdf:Description></rdf:RDF></x:xmpmeta>\n<?xpacket end=\"w\"?>",
        title = xml(&meta.title),
        author = xml(&meta.author),
        subject = xml(&subject(meta)),
        producer = xml(&producer(meta)),
        model = xml(&meta.apple2_model),
        profile = xml(&meta.printer_profile),
        note = xml(&meta.note),
        ver = xml(&meta.app_version),
    )
}

// ---------------------------------------------------------------------------------------------
// Page content
// ---------------------------------------------------------------------------------------------

/// Colour of the ink as DeviceRGB.
fn ink_rgb(ink: InkColor) -> (f64, f64, f64) {
    match ink {
        InkColor::Black => (0.0, 0.0, 0.0),
        InkColor::Blue => (0.05, 0.12, 0.55),
        InkColor::Purple => (0.35, 0.10, 0.50),
    }
}

/// Opacity of one worn-ribbon hit at intensity bucket `k` (0 = faintest).
fn worn_alpha(k: usize) -> f64 {
    0.40 + 0.60 * (k as f64) / ((WORN_LEVELS - 1) as f64)
}

fn mix(a: u64, b: u64) -> u64 {
    // splitmix64 finaliser over a combination of both inputs.
    let mut z = a ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Seed of a page's worn ribbon: the job seed if given, else a hash of the page's dot positions.
fn page_seed(page: &Page, meta: &PdfMeta) -> u64 {
    if let Some(s) = meta.ribbon_seed {
        return s;
    }
    let mut h = mix(0x5EED, u64::from(page.width) << 32 | u64::from(page.height));
    for d in &page.dots {
        h = mix(h, u64::from(d.x) << 32 | u64::from(d.y));
    }
    h
}

fn page_content(
    page: &Page,
    opts: &RenderOptions,
    meta: &PdfMeta,
    extra: &mut ExtraFont,
    report: &mut ExportReport,
) -> Vec<u8> {
    let mut c = String::new();
    // Unit space: 1 unit = 1/2160", origin top-left, y down.
    let _ = writeln!(
        c,
        "q 0.03333333333333 0 0 -0.03333333333333 0 {} cm",
        pt(page.height)
    );
    paper_decoration(&mut c, page, opts.paper.kind);
    dots(&mut c, page, opts, meta);
    text_layer(&mut c, page, extra, report);
    c.push_str("Q\n");
    c.into_bytes()
}

fn paper_decoration(c: &mut String, page: &Page, kind: PaperKind) {
    let (w, h) = (page.width, page.height);
    let _ = writeln!(c, "1 1 1 rg 0 0 {w} {h} re f");
    let continuous = matches!(
        kind,
        PaperKind::GreenBar | PaperKind::BlueBar | PaperKind::PlainContinuous
    );
    if !continuous {
        return;
    }
    // Bars between the two tractor strips: 1/2" white, 1/2" tinted, starting with white.
    if matches!(kind, PaperKind::GreenBar | PaperKind::BlueBar) && w > 2 * STRIP_UNITS {
        let (r, g, b) = if kind == PaperKind::GreenBar {
            (0.80, 0.92, 0.80)
        } else {
            (0.80, 0.87, 0.95)
        };
        let _ = writeln!(c, "{r} {g} {b} rg");
        let mut y = BAR_UNITS;
        while y < h {
            let bh = BAR_UNITS.min(h - y);
            let _ = writeln!(c, "{STRIP_UNITS} {y} {} {bh} re", w - 2 * STRIP_UNITS);
            y += 2 * BAR_UNITS;
        }
        c.push_str("f\n");
    }
    // Tear-off perforations: along the strips and between forms (top/bottom edge).
    c.push_str("0.62 G 8 w 0 J [90 90] 0 d\n");
    if w > 2 * STRIP_UNITS {
        let _ = writeln!(c, "{STRIP_UNITS} 0 m {STRIP_UNITS} {h} l");
        let _ = writeln!(c, "{0} 0 m {0} {h} l", w - STRIP_UNITS);
    }
    let _ = writeln!(
        c,
        "0 12 m {w} 12 l 0 {0} m {w} {0} l S [] 0 d",
        h.saturating_sub(12)
    );
    // Tractor holes (backdrop shows through).
    c.push_str("0.86 g 0.55 G 6 w\n");
    let k = 0.552_284_75 * HOLE_RADIUS_UNITS;
    let r = HOLE_RADIUS_UNITS;
    for cx in [STRIP_UNITS / 2, w.saturating_sub(STRIP_UNITS / 2)] {
        let mut cy = HOLE_PITCH_UNITS / 2;
        while cy + HOLE_PITCH_UNITS / 4 < h {
            let (x, y) = (f64::from(cx), f64::from(cy));
            let _ = writeln!(
                c,
                "{} {} m {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c {} {} {} {} {} {} c B",
                num(x + r), num(y),
                num(x + r), num(y + k), num(x + k), num(y + r), num(x), num(y + r),
                num(x - k), num(y + r), num(x - r), num(y + k), num(x - r), num(y),
                num(x - r), num(y - k), num(x - k), num(y - r), num(x), num(y - r),
                num(x + k), num(y - r), num(x + r), num(y - k), num(x + r), num(y),
            );
            cy += HOLE_PITCH_UNITS;
        }
    }
}

fn dots(c: &mut String, page: &Page, opts: &RenderOptions, meta: &PdfMeta) {
    if page.dots.is_empty() {
        return;
    }
    let (r, g, b) = ink_rgb(opts.ink);
    let _ = writeln!(
        c,
        "{} {} {} RG 1 J {DOT_DIAMETER_UNITS} w",
        num(r),
        num(g),
        num(b)
    );

    // Hits per position (BTreeMap: sorted, deterministic output).
    let mut hits: BTreeMap<(u32, u32), u32> = BTreeMap::new();
    for d in &page.dots {
        *hits.entry((d.y, d.x)).or_insert(0) += 1;
    }

    match opts.ribbon {
        Ribbon::New => {
            // Opaque ink: a repeated hit on the same spot changes nothing visible.
            let pts: Vec<(u32, u32)> = hits.keys().copied().collect();
            emit_paths(c, &pts);
        }
        Ribbon::Worn => {
            let seed = page_seed(page, meta);
            let mut buckets: Vec<Vec<(u32, u32)>> = vec![Vec::new(); WORN_LEVELS];
            for (&(y, x), &n) in &hits {
                // Slow variation along the page: each 1/6" band is a little lighter or darker.
                let band = mix(seed ^ 0xBA0D, u64::from(y / 360));
                let bias = (band % 3) as i32 - 1;
                for i in 0..n {
                    let h = mix(
                        seed,
                        u64::from(x) << 32 | u64::from(y) ^ (u64::from(i) << 56),
                    );
                    let level = ((h >> 20) % WORN_LEVELS as u64) as i32 + bias;
                    let level = level.clamp(0, WORN_LEVELS as i32 - 1) as usize;
                    buckets[level].push((y, x));
                }
            }
            for (k, pts) in buckets.iter().enumerate() {
                if pts.is_empty() {
                    continue;
                }
                let _ = writeln!(c, "/A{k} gs");
                emit_paths(c, pts);
            }
        }
    }
}

/// `pts` are `(y, x)`; one zero-length round-capped stroke per dot.
fn emit_paths(c: &mut String, pts: &[(u32, u32)]) {
    for chunk in pts.chunks(DOTS_PER_PATH) {
        for &(y, x) in chunk {
            let _ = writeln!(c, "{x} {y} m {x} {y} l");
        }
        c.push_str("S\n");
    }
}

// ---------------------------------------------------------------------------------------------
// Invisible text layer
// ---------------------------------------------------------------------------------------------

/// Byte of a character in Windows-1252 (WinAnsiEncoding), if it has one.
fn winansi(c: char) -> Option<u8> {
    let u = c as u32;
    match u {
        0x20..=0x7E | 0xA0..=0xFF => Some(u as u8),
        _ => Some(match c {
            '€' => 0x80,
            '‚' => 0x82,
            'ƒ' => 0x83,
            '„' => 0x84,
            '…' => 0x85,
            '†' => 0x86,
            '‡' => 0x87,
            'ˆ' => 0x88,
            '‰' => 0x89,
            'Š' => 0x8A,
            '‹' => 0x8B,
            'Œ' => 0x8C,
            'Ž' => 0x8E,
            '‘' => 0x91,
            '’' => 0x92,
            '“' => 0x93,
            '”' => 0x94,
            '•' => 0x95,
            '–' => 0x96,
            '—' => 0x97,
            '˜' => 0x98,
            '™' => 0x99,
            'š' => 0x9A,
            '›' => 0x9B,
            'œ' => 0x9C,
            'ž' => 0x9E,
            'Ÿ' => 0x9F,
            _ => return None,
        }),
    }
}

/// Characters outside WinAnsi (box drawing of PC437, Greek, ...) get codes 1..=255 in a second
/// Courier font whose glyphs are all blank and whose ToUnicode maps each code back to the real
/// character. The layer is invisible, so only the extracted text matters.
#[derive(Default)]
struct ExtraFont {
    chars: Vec<char>,
}

impl ExtraFont {
    fn code(&mut self, c: char) -> Option<u8> {
        if let Some(i) = self.chars.iter().position(|&x| x == c) {
            return Some(i as u8 + 1);
        }
        if self.chars.len() >= 255 {
            return None;
        }
        self.chars.push(c);
        Some(self.chars.len() as u8)
    }

    fn to_unicode(&self) -> String {
        let mut s = String::from(
            "/CIDInit /ProcSet findresource begin 12 dict begin begincmap\n\
             /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n\
             /CMapName /Adobe-Identity-UCS def /CMapType 2 def\n\
             1 begincodespacerange <01> <FF> endcodespacerange\n",
        );
        for chunk in self.chars.chunks(100).enumerate() {
            let (n, chars) = chunk;
            let _ = writeln!(s, "{} beginbfchar", chars.len());
            for (j, ch) in chars.iter().enumerate() {
                let code = n * 100 + j + 1;
                let mut buf = [0u16; 2];
                let hex: String = ch
                    .encode_utf16(&mut buf)
                    .iter()
                    .map(|u| format!("{u:04X}"))
                    .collect();
                let _ = writeln!(s, "<{code:02X}> <{hex}>");
            }
            s.push_str("endbfchar\n");
        }
        s.push_str("endcmap CMapName currentdict /CMap defineresource pop end end\n");
        s
    }
}

fn text_layer(c: &mut String, page: &Page, extra: &mut ExtraFont, report: &mut ExportReport) {
    if page.text.is_empty() {
        return;
    }
    c.push_str("BT 3 Tr\n");
    for run in &page.text {
        let size = run.height.max(1);
        // Courier advances 0.6 em; Tc pads or squeezes to the printer's pitch.
        let tc = f64::from(run.advance) - 0.6 * f64::from(size);
        let _ = writeln!(c, "{} Tc", num(tc));
        // Split into segments of equal font so each is a single Tj.
        let mut seg: Vec<u8> = Vec::new();
        let mut seg_font = "FC";
        let mut seg_start = 0u32;
        for (i, ch) in run.text.chars().enumerate() {
            let ch = if ch.is_control() { ' ' } else { ch };
            let (font, byte) = match winansi(ch) {
                Some(b) => ("FC", b),
                None => match extra.code(ch) {
                    Some(b) => ("FX", b),
                    None => {
                        report.omitted_chars += 1;
                        ("FC", b' ')
                    }
                },
            };
            if font != seg_font && !seg.is_empty() {
                flush_segment(c, run, size, seg_font, seg_start, &seg);
                seg.clear();
            }
            if seg.is_empty() {
                seg_start = i as u32;
            }
            seg_font = font;
            seg.push(byte);
        }
        if !seg.is_empty() {
            flush_segment(c, run, size, seg_font, seg_start, &seg);
        }
    }
    c.push_str("ET\n");
}

fn flush_segment(
    c: &mut String,
    run: &crate::document::TextRun,
    size: u32,
    font: &str,
    start: u32,
    bytes: &[u8],
) {
    let x = u64::from(run.x) + u64::from(start) * u64::from(run.advance);
    let hex: String = bytes.iter().map(|b| format!("{b:02X}")).collect();
    // The page space is y-down, so the text matrix flips y back for upright glyphs.
    let _ = writeln!(c, "/{font} {size} Tf 1 0 0 -1 {x} {} Tm <{hex}> Tj", run.y);
}

// ---------------------------------------------------------------------------------------------
// Cover sheet
// ---------------------------------------------------------------------------------------------

fn winansi_bytes(s: &str) -> String {
    s.chars()
        .map(|ch| format!("{:02X}", winansi(ch).unwrap_or(b'?')))
        .collect()
}

fn wrap(s: &str, max: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > max {
                out.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    out
}

fn cover_content(width: u32, height: u32, meta: &PdfMeta) -> Vec<u8> {
    let (wpt, hpt) = (f64::from(width) / 30.0, f64::from(height) / 30.0);
    let margin = 72.0_f64.min(wpt / 6.0);
    let mut c = String::new();
    let _ = writeln!(c, "1 1 1 rg 0 0 {} {} re f 0 g", num(wpt), num(hpt));
    let mut y = hpt - margin - 24.0;
    let line = |c: &mut String, y: &mut f64, size: f64, text: &str| {
        let _ = writeln!(
            c,
            "BT /FH {} Tf 1 0 0 1 {} {} Tm <{}> Tj ET",
            num(size),
            num(margin),
            num(*y),
            winansi_bytes(text)
        );
        *y -= size * 1.5;
    };
    for l in wrap(&meta.title, 40) {
        line(&mut c, &mut y, 24.0, &l);
    }
    y -= 18.0;
    if let Some(d) = &meta.date {
        line(&mut c, &mut y, 12.0, &format!("Data: {}", d.human()));
    }
    if !meta.apple2_model.is_empty() {
        line(
            &mut c,
            &mut y,
            12.0,
            &format!("Máquina: {}", meta.apple2_model),
        );
    }
    line(
        &mut c,
        &mut y,
        12.0,
        &format!("Impressora: {}", meta.printer_profile),
    );
    if !meta.author.is_empty() {
        line(&mut c, &mut y, 12.0, &format!("Autor: {}", meta.author));
    }
    if !meta.note.trim().is_empty() {
        y -= 12.0;
        line(&mut c, &mut y, 12.0, "Nota:");
        for l in wrap(&meta.note, 80) {
            line(&mut c, &mut y, 11.0, &l);
        }
    }
    let _ = writeln!(
        c,
        "BT /FH 9 Tf 1 0 0 1 {} {} Tm <{}> Tj ET",
        num(margin),
        num(margin / 2.0),
        winansi_bytes(&format!("Gerado por {}", producer(meta)))
    );
    c.into_bytes()
}

// ---------------------------------------------------------------------------------------------
// Files
// ---------------------------------------------------------------------------------------------

/// Writes `bytes` to `path` atomically: a temporary file in the same directory is written and
/// synced, then moved into place. A failure leaves no partial file behind.
///
/// Unless `overwrite` is set, an existing `path` is never replaced (`ErrorKind::AlreadyExists`).
/// Any file name is accepted (spaces, accents, `#`, Unicode): the path is never interpreted.
#[cfg(feature = "fs")]
pub fn write_atomic(path: &std::path::Path, bytes: &[u8], overwrite: bool) -> std::io::Result<()> {
    use std::io::{Error, ErrorKind};
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let name = path
        .file_name()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "path has no file name"))?;
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p.to_path_buf(),
        _ => std::path::PathBuf::from("."),
    };
    if !overwrite && path.exists() {
        return Err(Error::new(ErrorKind::AlreadyExists, "destination exists"));
    }
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(
        ".{}.{}.tmp",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let tmp = dir.join(tmp_name);

    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        if overwrite {
            std::fs::rename(&tmp, path)
        } else {
            // hard_link fails if the destination exists, closing the check/rename race. Where
            // hard links are unavailable (FAT/exFAT), fall back to the checked rename.
            match std::fs::hard_link(&tmp, path) {
                Ok(()) => std::fs::remove_file(&tmp),
                Err(e) if e.kind() == ErrorKind::AlreadyExists => Err(e),
                Err(_) => {
                    if path.exists() {
                        Err(Error::new(ErrorKind::AlreadyExists, "destination exists"))
                    } else {
                        std::fs::rename(&tmp, path)
                    }
                }
            }
        }
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// Exports and writes the PDF atomically (see [`write_atomic`]).
#[cfg(feature = "fs")]
pub fn export_to_path(
    path: &std::path::Path,
    doc: &PageDocument,
    opts: &RenderOptions,
    meta: &PdfMeta,
    pages: &PageSelection,
    overwrite: bool,
) -> std::io::Result<ExportReport> {
    let (bytes, report) = export_with_report(doc, opts, meta, pages);
    write_atomic(path, &bytes, overwrite)?;
    Ok(report)
}
