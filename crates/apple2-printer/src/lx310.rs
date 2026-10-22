//! Epson LX-310 layout: turns `Command`s from the ESC/P parser into the physical `PageDocument`.
//!
//! Geometry conventions (all integers, unit = 1/2160", no accumulation, R01/T-016):
//! - `x_pos` is measured from the printable origin (`x0`, the left edge of the 8" printable area
//!   centred on the paper, never closer to the edge than the manual's minimum left margin);
//!   `y_pos` from the top of form of the current page.
//! - The print position is the centre of pin 1 (the top pin). Pin `p` (0 = top, 8 = the ninth,
//!   descender pin) is `30 * p` units below it (pins are 1/72" apart). A dot's page coordinates are
//!   `(x0 + x_pos + dx, y_pos + dy)`.
//! - Draft glyph columns are 1/120" (18 units) apart, NLQ 1/240" (9 units) and 1/144" (15 units)
//!   rows, as `font::Quality` documents.
//! - Characters are printed at the advance of the active pitch (216, 180, 144 units for 10/12/15 cpi;
//!   126 and 108 condensed), doubled by double width, plus ESC SP spacing.
//!
//! Glyph source: the matrix font (`font::glyph`); a code without a glyph draws a deterministic
//! placeholder cell (a hollow box) and is reported, so positions, advances and attributes stay
//! testable; `Glyphs::PlaceholderOnly` forces the box so goldens survive font retouching.

use crate::command::{Command, Parsed, Pitch};
use crate::document::{Diagnostic, Dot, Page, PageDocument, TextRun};
use crate::engine::PrinterEngine;
use crate::escp::{Parser, ParserLimits, DIAG_BYTES};
use crate::font::{self, CharTable, Glyph, Quality};
use crate::paper::{Paper, RenderOptions};
use crate::units;
use std::collections::HashMap;

/// Printable width of the LX-310: 8" (80 columns at 10 cpi), manual "Recommended printable area".
pub const PRINTABLE_WIDTH: u32 = 17_280;
/// Distance between pins: 1/72".
pub const PIN_PITCH: u32 = 30;
/// Draft dot column pitch: 1/120".
pub const DRAFT_COLUMN: u32 = 18;
/// NLQ dot column pitch: 1/240".
pub const NLQ_COLUMN: u32 = 9;
/// NLQ row pitch: 1/144".
pub const NLQ_ROW: u32 = 15;
/// Top of form: pin 1 of the first line sits at the manual's minimum top margin, 4.2 mm [M apspe_2]
/// (357 units), for continuous paper and single sheets alike.
pub const TOP_OFFSET: u32 = 357;
/// Baseline of the text layer: pin 8 (index 7); the descender pin is below it.
const TEXT_BASELINE: u32 = 7 * PIN_PITCH;
/// Height of a character cell (9 pins).
pub const CELL_HEIGHT: u32 = 270;
/// Default line spacing: 1/6".
pub const DEFAULT_LINE_SPACING: u32 = 360;
/// Largest page length: 22".
const MAX_PAGE_LENGTH: u32 = 22 * units::PER_INCH;
/// Emphasis (bold) prints the second pass 1/240" to the right.
const BOLD_SHIFT: u32 = 9;
/// Largest line distance (1/4") over which frame strokes are continued between lines.
const MAX_BRIDGE_GAP: u32 = 540;
/// Double strike prints the second pass 1/216" lower.
const DOUBLE_STRIKE_SHIFT: u32 = 10;

/// Where glyph dots come from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Glyphs {
    /// `font::glyph`; when it has none, the placeholder cell (transitional, until the font lands).
    #[default]
    FontWithPlaceholderFallback,
    /// Always the placeholder cell and ASCII text (stable goldens independent of the font).
    PlaceholderOnly,
    /// `font::glyph` only; a code without glyph prints nothing and gets a diagnostic.
    FontOnly,
}

#[derive(Clone, Copy, Debug)]
pub struct Lx310Config {
    pub glyphs: Glyphs,
    pub limits: ParserLimits,
    /// CR also feeds a line (the printer's "auto line feed" default setting; off by default).
    pub auto_lf_on_cr: bool,
    /// Diagnostics kept; one extra note is added when the limit is hit.
    pub max_diagnostics: usize,
    /// Pages one job may hold (blank pages included).
    pub max_pages: usize,
    /// Dots one job may hold (memory guard against adversarial streams: 12 bytes per dot). A real
    /// listing of hundreds of pages stays far below it; hitting it is announced once.
    pub max_dots: usize,
    /// Dots one page may hold. A page cannot physically hold more than about 1.8 million
    /// distinct positions, so this only stops streams that overstrike the same page forever.
    pub max_dots_per_page: usize,
}

impl Default for Lx310Config {
    fn default() -> Self {
        Lx310Config {
            glyphs: Glyphs::default(),
            limits: ParserLimits::default(),
            auto_lf_on_cr: false,
            max_diagnostics: 10_000,
            max_pages: 5_000,
            max_dots: 64_000_000,
            max_dots_per_page: 2_000_000,
        }
    }
}

/// Format state restored by ESC @.
#[derive(Clone, Debug)]
struct Format {
    line_spacing: u32,
    page_length: u32,
    bottom_margin: Option<u32>,
    left: u32,
    right: u32,
    htabs: Vec<u32>,
    /// `None` until ESC B is used (VT then acts as LF); `Some(empty)` after ESC B NUL (VT = CR).
    vtabs: Option<Vec<u32>>,
    pitch: Pitch,
    condensed: bool,
    double_width: bool,
    double_width_line: bool,
    extra_space: u32,
    bold: bool,
    double_strike: bool,
    underline: bool,
    italic: bool,
    nlq: bool,
    tables: [CharTable; 2],
    slot: usize,
    country: u8,
    upper_print: bool,
}

impl Format {
    fn initial(printable: u32, page_length: u32, table: CharTable, country: u8) -> Format {
        Format {
            line_spacing: DEFAULT_LINE_SPACING,
            page_length,
            bottom_margin: None,
            left: 0,
            right: printable,
            htabs: (1u32..)
                .map(|k| k * 8 * 216)
                .take_while(|&t| t < printable)
                .collect(),
            vtabs: None,
            pitch: Pitch::Cpi10,
            condensed: false,
            double_width: false,
            double_width_line: false,
            extra_space: 0,
            bold: false,
            double_strike: false,
            underline: false,
            italic: false,
            nlq: false,
            tables: [CharTable::Italic, table],
            slot: 1,
            country: country.min(12),
            // Graphic tables (PC437...) print codes 128..159 as characters; ESC 7 makes them
            // control codes (ignored), ESC 6 prints them again.
            upper_print: true,
        }
    }

    /// Width of one character cell at the active pitch, before double width and ESC SP spacing.
    fn cell_width(&self) -> u32 {
        match (self.pitch, self.condensed) {
            (Pitch::Cpi10, false) => 216,
            (Pitch::Cpi10, true) => 126,
            (Pitch::Cpi12, false) => 180,
            (Pitch::Cpi12, true) => 108,
            // The manual gives no condensed 15 cpi; condensed is ignored there.
            (Pitch::Cpi15, _) => 144,
        }
    }

    fn advance(&self) -> u32 {
        let base = self.cell_width();
        let mult = if self.double_width || self.double_width_line {
            2
        } else {
            1
        };
        (base + self.extra_space) * mult
    }

    fn table(&self) -> CharTable {
        self.tables[self.slot]
    }
}

#[derive(Clone, Copy)]
struct LastRun {
    page: usize,
    idx: usize,
    end_x: u32,
    y: u32,
    advance: u32,
}

/// A glyph printed on a recent line, kept so a frame character below it can continue its vertical
/// strokes through the gap between the lines (`font::line_bridge`).
struct LineGlyph {
    page: usize,
    y: u32,
    glyph: Glyph,
    cell: u32,
    mult: u32,
}

/// The Epson LX-310 virtual printer engine.
pub struct Lx310 {
    config: Lx310Config,
    options: RenderOptions,
    parser: Parser,
    fmt: Format,
    /// Printable origin and width on the paper, in units.
    x0: u32,
    printable: u32,
    x_pos: u32,
    y_pos: u32,
    page_no: usize,
    doc: PageDocument,
    changes: u64,
    last_run: Option<LastRun>,
    overscan_page: Option<usize>,
    limit_notes: u8,
    /// Dots held by the whole document (kept as a counter: summing the pages per push is O(pages)).
    dot_total: usize,
    /// Page that already got its "page full" note.
    page_full_noted: Option<usize>,
    diag_overflow: bool,
    /// Dots of the line in progress that fell below a continuous form: they land on the next page.
    spill: Vec<Dot>,
    /// Glyphs of the line in progress and of the line above, by `x_pos` (frame continuity).
    line_cur: HashMap<u32, LineGlyph>,
    line_prev: HashMap<u32, LineGlyph>,
}

impl Default for Lx310 {
    fn default() -> Self {
        Lx310::new(Lx310Config::default())
    }
}

/// Minimum left/right margins (manual): 13 mm continuous, 3 mm single sheet.
fn min_side_margin(paper: &Paper) -> u32 {
    if paper.is_continuous() {
        units::from_mm(13.0)
    } else {
        units::from_mm(3.0)
    }
}

/// Origin and width of the printable area for a paper.
pub fn printable_area(paper: &Paper) -> (u32, u32) {
    let min = min_side_margin(paper);
    let avail = paper.width.saturating_sub(2 * min);
    let width = PRINTABLE_WIDTH.min(avail);
    let x0 = (paper.width - width) / 2;
    (x0, width)
}

/// Horizontal step between graphics columns for ESC * `m` (units), `None` if the LX-310 lacks it.
fn graphics_step(m: u8) -> Option<u32> {
    match m {
        0 => Some(36),
        1 | 2 => Some(18),
        3 => Some(9),
        4 => Some(27),
        5 => Some(30),
        6 => Some(24),
        _ => None,
    }
}

impl Lx310 {
    pub fn new(config: Lx310Config) -> Lx310 {
        let options = RenderOptions::default();
        let (x0, printable) = printable_area(&options.paper);
        Lx310 {
            parser: Parser::new(config.limits),
            fmt: Format::initial(
                printable,
                options.paper.height,
                options.table,
                options.country,
            ),
            config,
            options,
            x0,
            printable,
            x_pos: 0,
            y_pos: 0,
            page_no: 0,
            doc: PageDocument::default(),
            changes: 0,
            last_run: None,
            overscan_page: None,
            limit_notes: 0,
            dot_total: 0,
            page_full_noted: None,
            diag_overflow: false,
            spill: Vec::new(),
            line_cur: HashMap::new(),
            line_prev: HashMap::new(),
        }
    }

    /// Left edge of the printable area on the page, in units.
    pub fn printable_origin(&self) -> u32 {
        self.x0
    }

    fn push_diag(&mut self, d: Diagnostic) {
        if self.doc.diagnostics.len() < self.config.max_diagnostics {
            self.doc.diagnostics.push(d);
        } else if !self.diag_overflow {
            self.diag_overflow = true;
            self.doc.diagnostics.push(Diagnostic {
                offset: d.offset,
                bytes: Vec::new(),
                message: "limite de diagnósticos atingido; os seguintes foram omitidos".into(),
            });
        }
    }

    fn note(&mut self, offset: u64, bytes: Vec<u8>, message: &str) {
        let bytes = bytes.into_iter().take(DIAG_BYTES).collect();
        self.push_diag(Diagnostic {
            offset,
            bytes,
            message: message.to_string(),
        });
    }

    // ---- vertical motion -------------------------------------------------------------------

    fn usable_bottom(&self) -> u32 {
        let bottom = match (self.fmt.bottom_margin, self.options.paper.is_continuous()) {
            (Some(m), true) => self.fmt.page_length.saturating_sub(m),
            _ => self.fmt.page_length,
        };
        bottom.max(1)
    }

    fn new_page(&mut self) {
        self.page_no = self.page_no.saturating_add(1);
        self.y_pos = 0;
    }

    /// Moves down `delta` units; past the bottom it goes to the top of form of the next page.
    fn advance_y(&mut self, delta: u32) {
        let ny = self.y_pos.saturating_add(delta);
        if ny >= self.usable_bottom() {
            self.new_page();
        } else {
            self.y_pos = ny;
        }
    }

    fn carriage_return(&mut self) {
        self.x_pos = self.fmt.left;
        self.fmt.double_width_line = false;
    }

    fn line_feed(&mut self) {
        self.x_pos = self.fmt.left;
        self.fmt.double_width_line = false;
        self.advance_y(self.fmt.line_spacing);
    }

    // ---- page and dot output ---------------------------------------------------------------

    fn ensure_page_n(&mut self, n: usize, offset: u64) -> bool {
        if n >= self.config.max_pages {
            if self.limit_notes & 1 == 0 {
                self.limit_notes |= 1;
                self.note(
                    offset,
                    Vec::new(),
                    "limite de páginas do trabalho atingido; impressão descartada",
                );
            }
            return false;
        }
        while self.doc.pages.len() <= n {
            self.doc.pages.push(Page {
                width: self.options.paper.width,
                height: self.options.paper.height,
                dots: Vec::new(),
                text: Vec::new(),
            });
            self.changes += 1;
        }
        self.doc.last_in_progress = true;
        true
    }

    /// Appends dots to the current page, applying the paper bounds (overscan is reported once per
    /// page) and the per-job dot limit.
    fn commit_dots(&mut self, dots: Vec<Dot>, dropped: u32, offset: u64, why: &str) {
        if dropped > 0 && self.overscan_page != Some(self.page_no) {
            self.overscan_page = Some(self.page_no);
            self.note(offset, Vec::new(), why);
        }
        let spill = std::mem::take(&mut self.spill);
        self.push_dots(self.page_no, dots, offset);
        self.push_dots(self.page_no + 1, spill, offset);
    }

    fn push_dots(&mut self, page: usize, dots: Vec<Dot>, offset: u64) {
        if dots.is_empty() || !self.ensure_page_n(page, offset) {
            return;
        }
        if self.dot_total.saturating_add(dots.len()) > self.config.max_dots {
            if self.limit_notes & 2 == 0 {
                self.limit_notes |= 2;
                self.note(
                    offset,
                    Vec::new(),
                    "limite de pontos do trabalho atingido; impressão descartada",
                );
            }
            return;
        }
        if self.doc.pages[page].dots.len().saturating_add(dots.len())
            > self.config.max_dots_per_page
        {
            if self.page_full_noted != Some(page) {
                self.page_full_noted = Some(page);
                self.note(
                    offset,
                    Vec::new(),
                    "limite de pontos da página atingido; o restante desta página foi descartado",
                );
            }
            return;
        }
        self.dot_total += dots.len();
        self.changes += dots.len() as u64;
        self.doc.pages[page].dots.extend(dots);
    }

    /// A dot at `(x_pos + dx, y_pos + dy)` (plus the top-of-form offset). Off the printable width
    /// it is dropped; below the page it goes to the next form on continuous paper (the paper is
    /// one strip) and is dropped on a single sheet.
    fn dot(&mut self, dx: i64, dy: i64, out: &mut Vec<Dot>, dropped: &mut u32) {
        let x = i64::from(self.x0) + i64::from(self.x_pos) + dx;
        let y = i64::from(self.y_pos) + i64::from(TOP_OFFSET) + dy;
        let h = i64::from(self.options.paper.height);
        let in_x = x >= i64::from(self.x0) && x < i64::from(self.x0) + i64::from(self.printable);
        if !in_x || y < 0 {
            *dropped += 1;
        } else if y < h {
            out.push(Dot {
                x: x as u32,
                y: y as u32,
                ink: 0,
            });
        } else if self.options.paper.is_continuous() && y < 2 * h {
            self.spill.push(Dot {
                x: x as u32,
                y: (y - h) as u32,
                ink: 0,
            });
        } else {
            *dropped += 1;
        }
    }

    fn add_text(&mut self, c: char, advance: u32, baseline: u32, offset: u64) {
        let x = self.x0 + self.x_pos;
        let mut y = self.y_pos + TOP_OFFSET + baseline;
        let line_y = self.y_pos;
        let mut page = self.page_no;
        let h = self.options.paper.height;
        if y >= h && self.options.paper.is_continuous() {
            // Baseline below the form: it prints on the next form (continuous strip).
            y -= h;
            page += 1;
        }
        if !self.ensure_page_n(page, offset) {
            return;
        }
        if let Some(l) = self.last_run {
            if l.page == page && l.y == line_y && l.advance == advance && l.end_x == x {
                if let Some(run) = self.doc.pages[page].text.get_mut(l.idx) {
                    run.text.push(c);
                    self.last_run = Some(LastRun {
                        end_x: x + advance,
                        ..l
                    });
                    self.changes += 1;
                    return;
                }
            }
        }
        let text = &mut self.doc.pages[page].text;
        text.push(TextRun {
            x,
            y,
            advance,
            height: CELL_HEIGHT,
            text: c.to_string(),
        });
        self.last_run = Some(LastRun {
            page,
            idx: text.len() - 1,
            end_x: x + advance,
            y: line_y,
            advance,
        });
        self.changes += 1;
    }

    // ---- characters ------------------------------------------------------------------------

    fn placeholder(advance_cols: u32) -> Glyph {
        // Hollow box 7 rows high; first column blank, as a letter-spacing gap.
        let w = advance_cols.saturating_sub(3).max(2);
        let mut columns = vec![0u32];
        for i in 0..w {
            columns.push(if i == 0 || i == w - 1 { 0x7F } else { 0x41 });
        }
        Glyph {
            quality: Quality::Draft,
            columns,
            baseline_row: 6,
        }
    }

    fn glyph_for(&mut self, code: u8, offset: u64) -> Option<Glyph> {
        let quality = if self.fmt.nlq {
            Quality::Nlq
        } else {
            Quality::Draft
        };
        let single = self.fmt.cell_width();
        let ph = || Self::placeholder(single / DRAFT_COLUMN);
        match self.config.glyphs {
            Glyphs::PlaceholderOnly => Some(ph()),
            Glyphs::FontWithPlaceholderFallback => Some(
                font::glyph(self.fmt.table(), self.fmt.country, code, quality).unwrap_or_else(ph),
            ),
            Glyphs::FontOnly => {
                let g = font::glyph(self.fmt.table(), self.fmt.country, code, quality);
                if g.is_none() {
                    self.note(
                        offset,
                        vec![code],
                        "código sem glifo na tabela de caracteres ativa; nada impresso",
                    );
                }
                g
            }
        }
    }

    fn text_char(&self, code: u8) -> Option<char> {
        match self.config.glyphs {
            Glyphs::PlaceholderOnly => (0x20..=0x7E).contains(&code).then_some(code as char),
            _ => font::to_unicode(self.fmt.table(), self.fmt.country, code),
        }
    }

    fn print_char(&mut self, code: u8, offset: u64) {
        if (0x80..=0x9F).contains(&code) && !self.fmt.upper_print {
            self.note(
                offset,
                vec![code],
                "código 128..159 tratado como controle (ESC 7); ESC 6 habilita impressão; ignorado",
            );
            return;
        }
        let adv = self.fmt.advance();
        if self.x_pos > self.fmt.left && self.x_pos.saturating_add(adv) > self.fmt.right {
            // Automatic wrap at the right margin.
            self.line_feed();
        }
        let mult: u32 = if self.fmt.double_width || self.fmt.double_width_line {
            2
        } else {
            1
        };
        let mut dots = Vec::new();
        let mut dropped = 0u32;
        if code != b' ' {
            if let Some(g) = self.glyph_for(code, offset) {
                let ru = match g.quality {
                    Quality::Draft => PIN_PITCH,
                    Quality::Nlq => NLQ_ROW,
                };
                // The glyph matrix is the same at every pitch: the dot columns are spaced
                // proportionally to the cell width (10 cpi: 1/120" per Draft column; 17 cpi:
                // 7/12 of that), so a narrower cell never makes a glyph invade its neighbour.
                let cell = self.fmt.cell_width();
                let ncols = font::cell_columns(g.quality) as u32;
                let bold_shift = BOLD_SHIFT * cell / 216;
                // Passes: emphasis shifts right, double strike shifts down (not in NLQ).
                let mut passes: Vec<(u32, u32)> = vec![(0, 0)];
                if self.fmt.bold {
                    passes.push((bold_shift, 0));
                }
                if self.fmt.double_strike && g.quality == Quality::Draft {
                    let again: Vec<(u32, u32)> = passes
                        .iter()
                        .map(|&(x, y)| (x, y + DOUBLE_STRIKE_SHIFT))
                        .collect();
                    passes.extend(again);
                }
                if !self.fmt.italic {
                    self.bridge_frame(&g, cell, mult, ru, &mut dots, &mut dropped);
                }
                for &(px, py) in &passes {
                    for (ci, &col) in g.columns.iter().enumerate() {
                        for r in 0..32u32 {
                            if col & (1 << r) == 0 {
                                continue;
                            }
                            let shear = if self.fmt.italic {
                                (8 * PIN_PITCH).saturating_sub(r * ru) * 3 / 20
                            } else {
                                0
                            };
                            // Double width: every matrix column becomes `mult` adjacent columns.
                            // Double width also fills the empty matrix column between two dots
                            // of the same row (a stroke drawn as `#.#.#`), so horizontal strokes
                            // stay continuous instead of breaking into dashes.
                            let fill = mult > 1
                                && g.columns.get(ci + 2).is_some_and(|&n| n & (1 << r) != 0);
                            let span = if fill { 2 * mult } else { mult };
                            for k in 0..span {
                                let dx = (ci as u32 * mult + k) * cell / ncols + px + shear;
                                self.dot(
                                    i64::from(dx),
                                    i64::from(r * ru + py),
                                    &mut dots,
                                    &mut dropped,
                                );
                            }
                        }
                    }
                }
            }
        }
        if self.fmt.underline {
            // Pin 9 across the whole cell, following the active passes.
            let mut passes: Vec<(u32, u32)> = vec![(0, 0)];
            if self.fmt.bold {
                passes.push((BOLD_SHIFT * self.fmt.cell_width() / 216, 0));
            }
            if self.fmt.double_strike && !self.fmt.nlq {
                let again: Vec<(u32, u32)> = passes
                    .iter()
                    .map(|&(x, y)| (x, y + DOUBLE_STRIKE_SHIFT))
                    .collect();
                passes.extend(again);
            }
            for &(px, py) in &passes {
                for k in 0..adv / DRAFT_COLUMN {
                    self.dot(
                        i64::from(k * DRAFT_COLUMN + px),
                        i64::from(8 * PIN_PITCH + py),
                        &mut dots,
                        &mut dropped,
                    );
                }
            }
        }
        self.commit_dots(
            dots,
            dropped,
            offset,
            "pontos fora da área de impressão do papel descartados (overscan)",
        );
        if let Some(c) = self.text_char(code) {
            self.add_text(c, adv, TEXT_BASELINE, offset);
        }
        self.x_pos = self.x_pos.saturating_add(adv);
    }

    /// Remembers `g` (printed now at `x_pos`) and, when the glyph printed on the line above at the
    /// same position leaves a vertical stroke ending at its last pin row while this one starts at
    /// its first, continues the stroke through the gap between the lines (one dot per pin row, as
    /// the real printer's frame looks at 1/6" line pitch with the pins' ink spread).
    fn bridge_frame(
        &mut self,
        g: &Glyph,
        cell: u32,
        mult: u32,
        ru: u32,
        dots: &mut Vec<Dot>,
        dropped: &mut u32,
    ) {
        let key = self.x_pos;
        let page = self.page_no;
        // Rotate the line maps when the print position moved to another line.
        let moved = self.line_cur.values().next().map(|e| (e.page, e.y));
        if let Some((pg, y)) = moved {
            if pg != page || y > self.y_pos {
                self.line_cur.clear();
                self.line_prev.clear();
            } else if y < self.y_pos {
                self.line_prev = std::mem::take(&mut self.line_cur);
            }
        }
        if let Some(up) = self.line_prev.get(&key) {
            let same = up.page == page
                && up.cell == cell
                && up.mult == mult
                && up.glyph.quality == g.quality
                && up.glyph.columns.len() == g.columns.len();
            let last_row = font::cell_rows(g.quality) as u32 - 1;
            let dist = self.y_pos.saturating_sub(up.y);
            if same && dist > last_row * ru && dist <= MAX_BRIDGE_GAP {
                let ncols = font::cell_columns(g.quality) as u32;
                let bridge = font::line_bridge(&up.glyph, g);
                let mut gap_y = last_row * ru + ru;
                while gap_y + ru <= dist {
                    for (ci, _) in bridge.iter().enumerate().filter(|(_, &b)| b) {
                        for k in 0..mult {
                            let dx = (ci as u32 * mult + k) * cell / ncols;
                            self.dot(
                                i64::from(dx),
                                i64::from(gap_y) - i64::from(dist),
                                dots,
                                dropped,
                            );
                        }
                    }
                    gap_y += ru;
                }
            }
        }
        self.line_cur.insert(
            key,
            LineGlyph {
                page,
                y: self.y_pos,
                glyph: g.clone(),
                cell,
                mult,
            },
        );
    }

    // ---- graphics --------------------------------------------------------------------------

    fn graphics(&mut self, density: u8, nine_pin: bool, columns: u16, data: &[u8], offset: u64) {
        let Some(step) = graphics_step(density) else {
            let mut head = vec![0x1B, b'*', density];
            head.extend_from_slice(&columns.to_le_bytes());
            self.note(
                offset,
                head,
                "densidade de gráfico sem suporte na LX-310 (144 dpi); dados descartados",
            );
            return;
        };
        let adjacent_ok = !(!nine_pin && (density == 2 || density == 3));
        let per = if nine_pin { 2 } else { 1 };
        let mut dots = Vec::new();
        let mut dropped = 0u32;
        let mut prev = 0u16;
        for c in 0..usize::from(columns) {
            let b0 = data.get(c * per).copied().unwrap_or(0);
            let b1 = if nine_pin {
                data.get(c * per + 1).copied().unwrap_or(0)
            } else {
                0
            };
            // Bit 7 of the first byte is the top pin; bit 0 of the second is the ninth pin.
            let mut bits: u16 = u16::from(b0) << 1 | u16::from(b1 & 1);
            if !nine_pin {
                bits = u16::from(b0) << 1;
            }
            if !adjacent_ok {
                // The head ignores a dot whose pin fired in the previous column; the comparison
                // is against what was PRINTED, so a continuous stroke prints every other column.
                bits &= !prev;
            }
            prev = bits;
            let dx = c as u32 * step;
            if self.x_pos.saturating_add(dx) >= self.fmt.right {
                dropped += bits.count_ones().max(1);
                continue;
            }
            for pin in 0..9u32 {
                if bits & (1 << (8 - pin)) != 0 {
                    self.dot(
                        i64::from(dx),
                        i64::from(pin * PIN_PITCH),
                        &mut dots,
                        &mut dropped,
                    );
                }
            }
        }
        self.commit_dots(
            dots,
            dropped,
            offset,
            "dados gráficos além da margem direita ou da área do papel descartados",
        );
        self.x_pos = self.x_pos.saturating_add(u32::from(columns) * step);
    }

    // ---- commands --------------------------------------------------------------------------

    fn apply(&mut self, parsed: Parsed) {
        let off = parsed.offset;
        match parsed.command {
            Command::Char(b) => self.print_char(b, off),
            Command::CarriageReturn => {
                self.carriage_return();
                if self.config.auto_lf_on_cr {
                    self.advance_y(self.fmt.line_spacing);
                }
            }
            Command::LineFeed => self.line_feed(),
            Command::FormFeed => {
                self.x_pos = self.fmt.left;
                self.fmt.double_width_line = false;
                self.new_page();
            }
            Command::Backspace => {
                let adv = self.fmt.advance();
                if self.x_pos >= self.fmt.left.saturating_add(adv) {
                    self.x_pos -= adv;
                }
            }
            Command::HorizontalTab => {
                let left = self.fmt.left;
                let next = self
                    .fmt
                    .htabs
                    .iter()
                    .map(|&t| left.saturating_add(t))
                    .find(|&t| t > self.x_pos);
                if let Some(t) = next {
                    if t < self.fmt.right {
                        self.x_pos = t;
                    }
                }
            }
            Command::VerticalTab => {
                self.x_pos = self.fmt.left;
                self.fmt.double_width_line = false;
                match self.fmt.vtabs.clone() {
                    None => self.advance_y(self.fmt.line_spacing),
                    Some(v) if v.is_empty() => {}
                    Some(v) => match v.iter().copied().find(|&t| t > self.y_pos) {
                        Some(t) if t < self.usable_bottom() => self.y_pos = t,
                        _ => self.new_page(),
                    },
                }
            }
            Command::Reset => {
                let page_length = self.options.paper.height;
                self.fmt = Format::initial(
                    self.printable,
                    page_length,
                    self.options.table,
                    self.options.country,
                );
                self.x_pos = 0;
            }
            Command::LineSpacing216(n) => self.fmt.line_spacing = u32::from(n) * units::PER_216,
            Command::AdvanceVertical216(n) => self.advance_y(u32::from(n) * units::PER_216),
            Command::PageLengthLines(n) => {
                let len = u32::from(n) * self.fmt.line_spacing;
                self.set_page_length(len, off, &[0x1B, b'C', n]);
            }
            Command::PageLengthInches(n) => {
                self.set_page_length(u32::from(n) * units::PER_INCH, off, &[0x1B, b'C', 0, n]);
            }
            Command::BottomMargin(n) => {
                let m = u32::from(n) * self.fmt.line_spacing;
                if m == 0 || m >= self.fmt.page_length {
                    self.note(off, vec![0x1B, b'N', n], "ESC N: margem inferior fora da faixa (deve ser menor que o comprimento da página); ignorado");
                } else {
                    self.fmt.bottom_margin = Some(m);
                }
            }
            Command::CancelBottomMargin => self.fmt.bottom_margin = None,
            Command::LeftMargin(n) => {
                let m = u32::from(n) * self.fmt.advance();
                if m >= self.fmt.right {
                    self.note(
                        off,
                        vec![0x1B, b'l', n],
                        "ESC l: margem esquerda deve ser menor que a direita; ignorado",
                    );
                } else {
                    self.fmt.left = m;
                    self.x_pos = self.x_pos.max(m);
                }
            }
            Command::RightMargin(n) => {
                let m = u32::from(n) * self.fmt.advance();
                if m <= self.fmt.left || m > self.printable {
                    self.note(off, vec![0x1B, b'Q', n], "ESC Q: margem direita fora da faixa (maior que a esquerda e dentro da área imprimível); ignorado");
                } else {
                    self.fmt.right = m;
                }
            }
            Command::AbsoluteHorizontal(v) => {
                let t = u32::from(v) * 36 + self.fmt.left;
                if t > self.fmt.right {
                    self.note(
                        off,
                        vec![0x1B, b'$', v as u8, (v >> 8) as u8],
                        "ESC $: posição além da margem direita; ignorado",
                    );
                } else {
                    self.x_pos = t;
                }
            }
            Command::RelativeHorizontal(v) => {
                let t = i64::from(self.x_pos) + i64::from(v) * 18;
                if t < 0 || t > i64::from(self.printable) {
                    self.note(
                        off,
                        vec![0x1B, b'\\', v as u8, ((v as u16) >> 8) as u8],
                        "ESC \\: movimento sairia da área imprimível; ignorado",
                    );
                } else {
                    self.x_pos = t as u32;
                }
            }
            Command::HorizontalTabs(v) => {
                let adv = self.fmt.advance();
                self.fmt.htabs = v.iter().map(|&n| u32::from(n) * adv).collect();
            }
            Command::VerticalTabs(v) => {
                let ls = self.fmt.line_spacing;
                self.fmt.vtabs = Some(v.iter().map(|&n| u32::from(n) * ls).collect());
            }
            Command::Pitch(p) => self.fmt.pitch = p,
            Command::Condensed(b) => self.fmt.condensed = b,
            Command::DoubleWidthLine(b) => self.fmt.double_width_line = b,
            Command::DoubleWidth(b) => {
                self.fmt.double_width = b;
                if !b {
                    self.fmt.double_width_line = false;
                }
            }
            Command::Bold(b) => self.fmt.bold = b,
            Command::DoubleStrike(b) => self.fmt.double_strike = b,
            Command::Underline(b) => self.fmt.underline = b,
            Command::Italic(b) => self.fmt.italic = b,
            Command::MasterSelect(n) => {
                self.fmt.pitch = if n & 1 != 0 {
                    Pitch::Cpi12
                } else {
                    Pitch::Cpi10
                };
                if n & 2 != 0 {
                    self.note(
                        off,
                        vec![0x1B, b'!', n],
                        "ESC !: espaçamento proporcional (bit 1) sem suporte; usando passo fixo",
                    );
                }
                self.fmt.condensed = n & 4 != 0;
                self.fmt.bold = n & 8 != 0;
                self.fmt.double_strike = n & 16 != 0;
                self.fmt.double_width = n & 32 != 0;
                self.fmt.italic = n & 64 != 0;
                self.fmt.underline = n & 128 != 0;
            }
            Command::CharTable(slot) => self.fmt.slot = usize::from(slot & 1),
            Command::AssignCharTable { slot, id, variant } => {
                let table = match (id, variant) {
                    (0, 0) => Some(CharTable::Italic),
                    (1, 0) => Some(CharTable::Pc437),
                    (3, 0) => Some(CharTable::Pc850),
                    (7, 0) => Some(CharTable::Pc860),
                    (25, 0) => Some(CharTable::Brascii),
                    (26, 0) => Some(CharTable::Abicomp),
                    _ => None,
                };
                match table {
                    Some(t) => self.fmt.tables[usize::from(slot & 1)] = t,
                    None => self.note(off, vec![0x1B, b'(', b't', 3, 0, slot, id, variant], "ESC ( t: tabela de caracteres sem suporte neste projeto; atribuição ignorada"),
                }
            }
            Command::International(n) => self.fmt.country = n,
            Command::Nlq(b) => self.fmt.nlq = b,
            Command::Typeface(_) | Command::Unidirectional(_) => {}
            Command::ExtraSpace(n) => self.fmt.extra_space = u32::from(n) * 18,
            Command::UpperControlCodes(b) => self.fmt.upper_print = b,
            Command::Graphics {
                density,
                nine_pin,
                columns,
                data,
            } => self.graphics(density, nine_pin, columns, &data, off),
            Command::Unsupported { .. } => {}
        }
    }

    fn set_page_length(&mut self, len: u32, off: u64, raw: &[u8]) {
        if len == 0 || len > MAX_PAGE_LENGTH {
            self.note(
                off,
                raw.to_vec(),
                "comprimento de página fora da faixa (máximo 22 polegadas); ignorado",
            );
        } else {
            self.fmt.page_length = len;
            self.fmt.bottom_margin = None;
        }
    }

    /// Applies parser output in stream order: diagnostics and commands of one batch are merged by
    /// offset (a sequence's diagnostic first), so the document does not depend on how the stream
    /// was split into `feed` calls.
    fn process(&mut self, parsed: Vec<Parsed>, diags: Vec<Diagnostic>) {
        let mut diags = diags.into_iter().peekable();
        for p in parsed {
            while diags.peek().is_some_and(|d| d.offset <= p.offset) {
                if let Some(d) = diags.next() {
                    self.push_diag(d);
                }
            }
            self.apply(p);
        }
        for d in diags {
            self.push_diag(d);
        }
    }
}

impl PrinterEngine for Lx310 {
    fn begin(&mut self, options: RenderOptions) {
        let (x0, printable) = printable_area(&options.paper);
        self.options = options;
        self.x0 = x0;
        self.printable = printable;
        self.fmt = Format::initial(
            printable,
            options.paper.height,
            options.table,
            options.country,
        );
        self.parser.reset();
        self.x_pos = 0;
        self.y_pos = 0;
        self.page_no = 0;
        self.doc = PageDocument::default();
        self.changes = 0;
        self.last_run = None;
        self.overscan_page = None;
        self.limit_notes = 0;
        self.dot_total = 0;
        self.page_full_noted = None;
        self.diag_overflow = false;
        self.spill.clear();
        self.line_cur.clear();
        self.line_prev.clear();
    }

    fn feed(&mut self, offset: u64, data: &[u8]) {
        let mut out = Vec::new();
        let mut diags = Vec::new();
        self.parser.feed(offset, data, &mut out, &mut diags);
        self.process(out, diags);
    }

    fn document(&self) -> &PageDocument {
        &self.doc
    }

    fn take_revision(&mut self) -> u64 {
        std::mem::take(&mut self.changes)
    }

    fn finish(&mut self) {
        let mut out = Vec::new();
        let mut diags = Vec::new();
        self.parser.finish(&mut out, &mut diags);
        self.process(out, diags);
        self.doc.last_in_progress = false;
    }
}
