//! Dot-matrix glyphs (fonte própria, aproximação: ADR 0008) and character tables. Owned by the
//! font module; the API below is the contract the LX-310 layout uses.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

mod tables;
use tables::{Mark, ABICOMP_HI, BRASCII_HI, COMPOSED, PC437_HI, PC850_HI, PC860_HI};

/// Print quality. Draft: 9 rows 1/72" apart, columns 1/120" apart. NLQ: 18 rows 1/144" apart
/// (two passes), columns 1/240" apart.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Quality {
    #[default]
    Draft,
    Nlq,
}

/// Character tables the LX-310 offers that this project supports (ESC t / ESC R / ESC ( t).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum CharTable {
    /// Italic table / ASCII with an international variant from ESC R (`country` 0..=12).
    #[default]
    Pc437,
    Pc850,
    Pc860,
    Brascii,
    Abicomp,
    /// ESC t 0: italics in the upper half.
    Italic,
}

/// One character's dots. `columns[i]` is a bitmask of rows (bit 0 = top row) at column `i`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub quality: Quality,
    pub columns: Vec<u32>,
    /// Row (from the top) of the baseline, so descenders can go below it.
    pub baseline_row: u8,
}

/// Number of dot columns in the 10 CPI cell every returned glyph is padded to.
pub const fn cell_columns(quality: Quality) -> usize {
    match quality {
        Quality::Draft => DRAFT_COLS,
        Quality::Nlq => 2 * DRAFT_COLS,
    }
}

/// Number of pin rows (including the second pass in NLQ) of a glyph.
pub const fn cell_rows(quality: Quality) -> usize {
    match quality {
        Quality::Draft => DRAFT_ROWS,
        Quality::Nlq => 2 * DRAFT_ROWS,
    }
}

const DRAFT_COLS: usize = 12;
const DRAFT_ROWS: usize = 9;
const NLQ_COLS: usize = 2 * DRAFT_COLS;
const NLQ_ROWS: usize = 2 * DRAFT_ROWS;

/// Design lattice in Draft coordinates: `[row][column]`, 9 rows by 12 columns (1/72" x 1/120").
type Lattice = [[bool; DRAFT_COLS]; DRAFT_ROWS];

/// ESC R international variants: the 12 codes they replace, in order.
const NATIONAL_CODES: [u8; 12] = [
    0x23, 0x24, 0x40, 0x5B, 0x5C, 0x5D, 0x5E, 0x60, 0x7B, 0x7C, 0x7D, 0x7E,
];
/// ESC R international variants (n = 0..=12), from the Epson ESC/P reference manual (ESC R).
const NATIONAL: [[char; 12]; 13] = [
    ['#', '$', '@', '[', '\\', ']', '^', '`', '{', '|', '}', '~'],
    ['#', '$', 'à', '°', 'ç', '§', '^', '`', 'é', 'ù', 'è', '¨'],
    ['#', '$', '§', 'Ä', 'Ö', 'Ü', '^', '`', 'ä', 'ö', 'ü', 'ß'],
    ['£', '$', '@', '[', '\\', ']', '^', '`', '{', '|', '}', '~'],
    ['#', '$', '@', 'Æ', 'Ø', 'Å', '^', '`', 'æ', 'ø', 'å', '~'],
    ['#', '¤', 'É', 'Ä', 'Ö', 'Å', 'Ü', 'é', 'ä', 'ö', 'å', 'ü'],
    ['#', '$', '@', '°', '\\', 'é', '^', 'ù', 'à', 'ò', 'è', 'ì'],
    ['₧', '$', '@', '¡', 'Ñ', '¿', '^', '`', '¨', 'ñ', '}', '~'],
    ['#', '$', '@', '[', '¥', ']', '^', '`', '{', '|', '}', '~'],
    ['#', '¤', 'É', 'Æ', 'Ø', 'Å', 'Ü', 'é', 'æ', 'ø', 'å', 'ü'],
    ['#', '$', 'É', 'Æ', 'Ø', 'Å', 'Ü', 'é', 'æ', 'ø', 'å', 'ü'],
    ['#', '$', 'á', '¡', 'Ñ', '¿', 'é', '`', 'í', 'ñ', 'ó', 'ú'],
    ['#', '$', 'á', '¡', 'Ñ', '¿', 'é', 'ü', 'í', 'ñ', 'ó', 'ú'],
];

/// The glyph for `code` in `table` with ESC R `country` (0 = USA), or `None` if the table has no
/// glyph there (the layout reports a diagnostic and prints nothing, never `?`).
///
/// Every glyph is padded to the 10 CPI cell ([`cell_columns`]). Codes 0x00..=0x1F and 0x7F are
/// `None`. The ESC R `country` replaces the 12 national codes in every table.
pub fn glyph(table: CharTable, country: u8, code: u8, quality: Quality) -> Option<Glyph> {
    let ch = to_unicode(table, country, code)?;
    let mut g = match quality {
        Quality::Draft => lattice_to_draft(&lattice_for(ch)?),
        // Letters, digits, punctuation and the accented letters are drawn natively on the NLQ
        // grid; the rest (rule-based box/block characters and a few rare symbols) is derived from
        // the Draft lattice.
        Quality::Nlq => match nlq_glyph(ch) {
            Some(g) => g,
            None => lattice_to_nlq(&lattice_for(ch)?),
        },
    };
    if table == CharTable::Italic && !is_box_or_block(ch) {
        g = italicize(&g);
    }
    Some(g)
}

/// Unicode for the text layer (search/copy in the PDF); `None` for control or undefined codes.
pub fn to_unicode(table: CharTable, country: u8, code: u8) -> Option<char> {
    let ch = match code {
        0x20..=0x7E => {
            let national = NATIONAL_CODES.iter().position(|&c| c == code);
            match national {
                Some(i) => NATIONAL[usize::from(country).min(12)][i],
                None => char::from(code),
            }
        }
        0x80..=0xFF => {
            let i = usize::from(code - 0x80);
            match table {
                CharTable::Pc437 => PC437_HI[i],
                CharTable::Pc850 => PC850_HI[i],
                CharTable::Pc860 => PC860_HI[i],
                CharTable::Brascii => BRASCII_HI[i],
                CharTable::Abicomp => ABICOMP_HI[i],
                // Italic table: 0x80..=0x9F are undefined (ESC/P reference); 0xA0..=0xFF are
                // taken as ISO 8859-1 (assumption, see docs).
                CharTable::Italic if code >= 0xA0 => char::from(code),
                CharTable::Italic => '\0',
            }
        }
        _ => '\0',
    };
    (ch != '\0').then_some(ch)
}

/// Slants a glyph to the right with a deterministic shear: the Draft row `r` (0 = top) moves right
/// by `(9 - r) / 4` columns (2, 2, 1, 1, 1, 1, 0, 0, 0); NLQ rows use their Draft row (`r / 2`) and
/// twice the shift, so parity of column positions is kept. Columns pushed past the cell are dropped.
pub fn italicize(g: &Glyph) -> Glyph {
    let len = g.columns.len();
    let mut out = vec![0u32; len];
    for (i, &col) in g.columns.iter().enumerate() {
        for r in 0..32usize {
            if col & (1 << r) == 0 {
                continue;
            }
            let (draft_row, mul) = match g.quality {
                Quality::Draft => (r, 1),
                Quality::Nlq => (r / 2, 2),
            };
            let shift = mul * (9usize.saturating_sub(draft_row) / 4);
            if i + shift < len {
                out[i + shift] |= 1 << r;
            }
        }
    }
    enforce_no_adjacent(&mut out);
    Glyph {
        quality: g.quality,
        columns: out,
        baseline_row: g.baseline_row,
    }
}

/// Columns whose pin fires on the last pin row of `upper` and on the first pin row of `lower`: a
/// vertical stroke that crosses the line break (box drawing). With the 9-pin head the line pitch of
/// 1/6" (12 rows of 1/72") leaves 3 unprinted rows between consecutive lines, so the layout (or
/// the sample generator) continues those columns through the gap, one dot per Draft row (two per
/// 1/144" row in NLQ), to make the frame continuous like the real printer does with a graphics
/// line pitch of 9/72" or less.
pub fn line_bridge(upper: &Glyph, lower: &Glyph) -> Vec<bool> {
    let last = cell_rows(upper.quality) - 1;
    upper
        .columns
        .iter()
        .zip(&lower.columns)
        .map(|(&u, &l)| u & (1 << last) != 0 && l & 1 != 0)
        .collect()
}

/// Renders a glyph as ASCII art (`#` dot, `.` blank), one line per pin row, top first.
pub fn ascii_art(g: &Glyph) -> String {
    let mut s = String::new();
    for r in 0..cell_rows(g.quality) {
        for &c in &g.columns {
            s.push(if c & (1 << r) != 0 { '#' } else { '.' });
        }
        s.push('\n');
    }
    s
}

/// The 9-pin rule: the same pin never fires in two adjacent columns (it needs a column to
/// recover). Applied left to right, keeping the earlier dot.
fn enforce_no_adjacent(cols: &mut [u32]) {
    for i in 1..cols.len() {
        cols[i] &= !cols[i - 1];
    }
}

fn is_box_or_block(c: char) -> bool {
    ('\u{2500}'..='\u{259F}').contains(&c)
}

fn lattice_to_draft(lat: &Lattice) -> Glyph {
    let mut cols = vec![0u32; DRAFT_COLS];
    for (r, row) in lat.iter().enumerate() {
        for (x, &d) in row.iter().enumerate() {
            if d {
                cols[x] |= 1 << r;
            }
        }
    }
    enforce_no_adjacent(&mut cols);
    Glyph {
        quality: Quality::Draft,
        columns: cols,
        baseline_row: 8,
    }
}

/// NLQ glyph derived from a Draft lattice (box/block characters and rare symbols only).
fn lattice_to_nlq(lat: &Lattice) -> Glyph {
    // Each Draft dot becomes a 1x2 vertical pair (both passes) at the doubled column, and
    // every pair of dots two Draft columns apart is bridged by a midpoint, so strokes
    // are continuous. All NLQ dots land on even columns, which satisfies the same
    // adjacency rule as Draft (see docs).
    let mut cols = vec![0u32; 2 * DRAFT_COLS];
    let mut put = |x: usize, r0: usize| {
        cols[x] |= 0b11 << r0;
    };
    for r in 0..DRAFT_ROWS {
        for x in 0..DRAFT_COLS {
            if !lat[r][x] {
                continue;
            }
            put(2 * x, 2 * r);
            // A horizontal run that reaches the cell edge keeps its 2-column rhythm into
            // the next cell (box drawing).
            if x == DRAFT_COLS - 2 && lat[r][x - 2] {
                put(2 * x + 2, 2 * r);
            }
            if x + 2 < DRAFT_COLS {
                if lat[r][x + 2] {
                    put(2 * x + 2, 2 * r);
                }
                if r + 1 < DRAFT_ROWS && lat[r + 1][x + 2] {
                    put(2 * x + 2, 2 * r + 1);
                }
                if r >= 1 && lat[r - 1][x + 2] {
                    put(2 * x + 2, 2 * r - 1);
                }
            }
        }
    }
    enforce_no_adjacent(&mut cols);
    Glyph {
        quality: Quality::Nlq,
        columns: cols,
        baseline_row: 16,
    }
}

// ---------------------------------------------------------------------------------------------
// Glyph designs
// ---------------------------------------------------------------------------------------------

/// Hand-drawn Draft designs: `font/draft.txt` (native 9 x 12 lattice, half-dots allowed) plus the
/// older 5-column designs of `font/legacy5.txt` for rare symbols that were not redrawn
/// (they sit on the odd columns of the lattice).
fn base_glyphs() -> &'static HashMap<char, Lattice> {
    static MAP: OnceLock<HashMap<char, Lattice>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        for (src, native) in [
            (include_str!("font/draft.txt"), true),
            (include_str!("font/legacy5.txt"), false),
        ] {
            for line in src.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with("//") {
                    continue;
                }
                let mut toks = line.split_whitespace();
                let ch = toks.next().and_then(single_char).expect("glyph char");
                let top: usize = toks.next().and_then(|t| t.parse().ok()).expect("top row");
                let mut lat: Lattice = [[false; DRAFT_COLS]; DRAFT_ROWS];
                for (i, row) in toks.enumerate() {
                    let width = if native { 10 } else { 5 };
                    assert!(top + i < DRAFT_ROWS && row.len() == width, "bad glyph {ch}");
                    for (c, b) in row.bytes().enumerate() {
                        if b == b'#' {
                            let x = if native { c + 1 } else { 2 * c + 1 };
                            lat[top + i][x] = true;
                        }
                    }
                }
                if native {
                    assert!(map.insert(ch, lat).is_none(), "duplicate glyph {ch}");
                } else {
                    // The legacy file only supplies what draft.txt does not draw.
                    map.entry(ch).or_insert(lat);
                }
            }
        }
        map
    })
}

fn single_char(t: &str) -> Option<char> {
    let mut it = t.chars();
    let c = it.next()?;
    it.next().is_none().then_some(c)
}

fn put_row(lat: &mut Lattice, row: usize, pattern: &str) {
    for (c, b) in pattern.bytes().enumerate() {
        if b == b'#' {
            lat[row][c + 1] = true;
        }
    }
}

fn first_row(lat: &Lattice) -> usize {
    lat.iter().position(|r| r.iter().any(|&d| d)).unwrap_or(0)
}

/// Draft accent marks (10-char rows like draft.txt). Capitals have a single free row above them;
/// lowercase letters have two (three for the ring), above the x-height.
fn draft_mark(mark: Mark, capital: bool) -> &'static [&'static str] {
    if capital {
        match mark {
            Mark::Acute => &[".....#...."],
            Mark::Grave => &["...#......"],
            Mark::Circ => &["...#.#...."],
            Mark::Tilde => &["..#.#.#..."],
            Mark::Diaer => &["..#...#..."],
            Mark::Ring => &["....#....."],
            Mark::Cedilla => &["....#....."],
        }
    } else {
        match mark {
            Mark::Acute => &[".....#....", "....#....."],
            Mark::Grave => &["...#......", "....#....."],
            Mark::Circ => &["....#.....", "...#.#...."],
            Mark::Tilde => &["...#.#....", "..#.#.#..."],
            Mark::Diaer => &["..#...#...", "..#...#..."],
            Mark::Ring => &["....#.....", "...#.#....", "....#....."],
            Mark::Cedilla => &["....#....."],
        }
    }
}

fn compose_draft(ch: char, base: char, mark: Mark) -> Option<Lattice> {
    let mut lat = *base_glyphs().get(&base)?;
    let top = first_row(&lat);
    let capital = !ch.is_lowercase();
    if matches!(mark, Mark::Cedilla) {
        put_row(&mut lat, 8, draft_mark(mark, capital)[0]);
        return Some(lat);
    }
    let rows = draft_mark(mark, capital);
    for (i, pattern) in rows.iter().enumerate() {
        put_row(&mut lat, top - rows.len() + i, pattern);
    }
    Some(lat)
}

fn lattice_for(ch: char) -> Option<Lattice> {
    if ch == ' ' || ch == '\u{a0}' {
        return Some([[false; DRAFT_COLS]; DRAFT_ROWS]);
    }
    if is_box_or_block(ch) {
        return box_or_block(ch);
    }
    if let Some(l) = base_glyphs().get(&ch) {
        return Some(*l);
    }
    let &(_, base, mark) = COMPOSED.iter().find(|(c, _, _)| *c == ch)?;
    compose_draft(ch, base, mark)
}

// ---------------------------------------------------------------------------------------------
// NLQ: strokes rasterised on the native 24 x 18 grid
// ---------------------------------------------------------------------------------------------

/// Stroke programs of `font/nlq.txt` (command format in the file header), by character.
fn nlq_strokes() -> &'static HashMap<char, &'static str> {
    static MAP: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        for line in include_str!("font/nlq.txt").lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            let (head, rest) = line.split_once(char::is_whitespace).expect("nlq line");
            let ch = single_char(head).expect("nlq glyph char");
            assert!(map.insert(ch, rest.trim()).is_none(), "duplicate nlq {ch}");
        }
        map
    })
}

/// NLQ accent marks as stroke programs: capitals use rows 0..=1, lowercase rows 1..=5 above the
/// x-height (row 6); the cedilla hangs below the baseline (row 14).
fn nlq_mark(mark: Mark, capital: bool) -> &'static str {
    match (mark, capital) {
        (Mark::Cedilla, _) => "m 10 14 l 11 15 l 12 16 l 10 17 l 8 16",
        (Mark::Acute, true) => "m 9 1 l 13 0",
        (Mark::Grave, true) => "m 7 0 l 11 1",
        (Mark::Circ, true) => "m 7 1 l 10 0 l 13 1",
        (Mark::Tilde, true) => "m 6 1 l 8 0 l 12 1 l 14 0",
        (Mark::Diaer, true) => "M 7 0 L 7 1 M 13 0 L 13 1",
        (Mark::Ring, true) => "e 10 0.8 2 0.9 0 360",
        (Mark::Acute, false) => "m 9 5 l 13 2",
        (Mark::Grave, false) => "m 7 2 l 11 5",
        (Mark::Circ, false) => "m 7 5 l 10 2 l 13 5",
        (Mark::Tilde, false) => "e 8 3.5 2.5 1.2 180 360 e 13 3.5 2.5 1.2 180 0",
        (Mark::Diaer, false) => "M 7 3 L 7 4 M 13 3 L 13 4",
        (Mark::Ring, false) => "E 10 3 2 1.8 0 360",
    }
}

/// The natively drawn NLQ glyph of `ch`, or `None` when it has no NLQ design (box/block characters,
/// rare symbols): those are derived from the Draft lattice.
fn nlq_glyph(ch: char) -> Option<Glyph> {
    let program = match nlq_strokes().get(&ch) {
        Some(p) => (*p).to_string(),
        None => {
            let &(_, base, mark) = COMPOSED.iter().find(|(c, _, _)| *c == ch)?;
            let base = nlq_strokes().get(&base)?;
            format!("{base} {}", nlq_mark(mark, !ch.is_lowercase()))
        }
    };
    Some(rasterize_nlq(&program).0)
}

/// Rasterises a stroke program on the NLQ grid. Returns the glyph and how many dots fell outside the
/// cell (a design error, checked by the unit tests).
fn rasterize_nlq(program: &str) -> (Glyph, usize) {
    let mut cols = vec![0u32; NLQ_COLS];
    let mut clipped = 0usize;
    let mut plot = |x: f64, y: f64, thick: bool| {
        let (xi, yi) = ((x + 0.5).floor() as i32, (y + 0.5).floor() as i32);
        let xs: &[i32] = if thick { &[-1, 1] } else { &[0] };
        for dx in xs {
            let xx = xi + dx;
            if (0..NLQ_COLS as i32).contains(&xx) && (0..NLQ_ROWS as i32).contains(&yi) {
                cols[xx as usize] |= 1 << yi;
            } else {
                clipped += 1;
            }
        }
    };
    let toks: Vec<&str> = program.split_whitespace().collect();
    let num = |i: usize| -> f64 { toks[i].parse().expect("number in nlq program") };
    let (mut cx, mut cy) = (0.0f64, 0.0f64);
    let mut i = 0;
    while i < toks.len() {
        let cmd = toks[i];
        let thick = cmd.chars().all(|c| c.is_ascii_uppercase());
        match cmd {
            "m" | "M" => {
                (cx, cy) = (num(i + 1), num(i + 2));
                plot(cx, cy, thick);
                i += 3;
            }
            "l" | "L" => {
                let (x1, y1) = (num(i + 1), num(i + 2));
                let steps = ((x1 - cx).abs().max((y1 - cy).abs()) * 4.0).ceil().max(1.0) as u32;
                for k in 0..=steps {
                    let t = f64::from(k) / f64::from(steps);
                    plot(cx + (x1 - cx) * t, cy + (y1 - cy) * t, thick);
                }
                (cx, cy) = (x1, y1);
                i += 3;
            }
            "e" | "E" => {
                let (ox, oy, rx, ry) = (num(i + 1), num(i + 2), num(i + 3), num(i + 4));
                let (a0, a1) = (num(i + 5), num(i + 6));
                let steps = (((a1 - a0).abs().to_radians()) * rx.max(ry) * 4.0)
                    .ceil()
                    .max(8.0) as u32;
                for k in 0..=steps {
                    let a = (a0 + (a1 - a0) * f64::from(k) / f64::from(steps)).to_radians();
                    (cx, cy) = (ox + rx * a.cos(), oy + ry * a.sin());
                    plot(cx, cy, thick);
                }
                i += 7;
            }
            other => panic!("unknown nlq command {other}"),
        }
    }
    enforce_no_adjacent(&mut cols);
    (
        Glyph {
            quality: Quality::Nlq,
            columns: cols,
            baseline_row: 16,
        },
        clipped,
    )
}

/// (up, down, left, right) line styles: 0 none, 1 single, 2 double.
fn box_segments(c: char) -> Option<(u8, u8, u8, u8)> {
    Some(match c {
        '─' => (0, 0, 1, 1),
        '│' => (1, 1, 0, 0),
        '┌' => (0, 1, 0, 1),
        '┐' => (0, 1, 1, 0),
        '└' => (1, 0, 0, 1),
        '┘' => (1, 0, 1, 0),
        '├' => (1, 1, 0, 1),
        '┤' => (1, 1, 1, 0),
        '┬' => (0, 1, 1, 1),
        '┴' => (1, 0, 1, 1),
        '┼' => (1, 1, 1, 1),
        '═' => (0, 0, 2, 2),
        '║' => (2, 2, 0, 0),
        '╒' => (0, 1, 0, 2),
        '╓' => (0, 2, 0, 1),
        '╔' => (0, 2, 0, 2),
        '╕' => (0, 1, 2, 0),
        '╖' => (0, 2, 1, 0),
        '╗' => (0, 2, 2, 0),
        '╘' => (1, 0, 0, 2),
        '╙' => (2, 0, 0, 1),
        '╚' => (2, 0, 0, 2),
        '╛' => (1, 0, 2, 0),
        '╜' => (2, 0, 1, 0),
        '╝' => (2, 0, 2, 0),
        '╞' => (1, 1, 0, 2),
        '╟' => (2, 2, 0, 1),
        '╠' => (2, 2, 0, 2),
        '╡' => (1, 1, 2, 0),
        '╢' => (2, 2, 1, 0),
        '╣' => (2, 2, 2, 0),
        '╤' => (0, 1, 2, 2),
        '╥' => (0, 2, 1, 1),
        '╦' => (0, 2, 2, 2),
        '╧' => (1, 0, 2, 2),
        '╨' => (2, 0, 1, 1),
        '╩' => (2, 0, 2, 2),
        '╪' => (1, 1, 2, 2),
        '╫' => (2, 2, 1, 1),
        '╬' => (2, 2, 2, 2),
        _ => return None,
    })
}

fn v_cols(style: u8) -> &'static [usize] {
    if style == 2 {
        &[4, 8]
    } else {
        &[6]
    }
}

fn h_rows(style: u8) -> &'static [usize] {
    if style == 2 {
        &[3, 5]
    } else {
        &[4]
    }
}

/// Box-drawing and block characters are drawn by rule, not from the 5-column font: all dots sit
/// on even columns of the 12-column cell (so horizontals are periodic across cells, with a dot
/// every 2 columns, and satisfy the adjacency rule); verticals are at column 6 (single) or 4 and 8
/// (double), horizontals at row 4 (single) or 3 and 5 (double).
fn box_or_block(c: char) -> Option<Lattice> {
    let mut lat: Lattice = [[false; DRAFT_COLS]; DRAFT_ROWS];
    if let Some((u, d, l, r)) = box_segments(c) {
        let vdouble = u == 2 || d == 2;
        let hdouble = l == 2 || r == 2;
        let corner = (u != 0) != (d != 0) && (l != 0) != (r != 0);
        let hseg = |lat: &mut Lattice, row: usize, from: usize, to: usize| {
            let mut x = from;
            while x <= to {
                lat[row][x] = true;
                x += 2;
            }
        };
        if corner {
            let down = d != 0;
            let right = r != 0;
            let (vs, hs) = (if down { d } else { u }, if right { r } else { l });
            let vc = v_cols(vs);
            let hr = h_rows(hs);
            // Outer column/row are the ones farther from the interior of the corner.
            let outer_col = if right { vc[0] } else { vc[vc.len() - 1] };
            let outer_row = if down { hr[0] } else { hr[hr.len() - 1] };
            for &col in vc {
                let row_edge = if hs == 2 && vs == 1 {
                    outer_row
                } else if hs == 2 {
                    if col == outer_col {
                        outer_row
                    } else {
                        hr[0] + hr[hr.len() - 1] - outer_row
                    }
                } else {
                    hr[0]
                };
                let (a, b) = if down { (row_edge, 8) } else { (0, row_edge) };
                for row in lat.iter_mut().take(b + 1).skip(a) {
                    row[col] = true;
                }
            }
            for &row in hr {
                let start_col = if hs == 2 && vs == 2 && row != outer_row {
                    vc[0] + vc[vc.len() - 1] - outer_col
                } else if vs == 2 {
                    outer_col
                } else {
                    vc[0]
                };
                let (a, b) = if right {
                    (start_col, 10)
                } else {
                    (0, start_col)
                };
                hseg(&mut lat, row, a, b);
            }
        } else {
            let yu = if hdouble { 3 } else { 4 };
            let yd = if hdouble { 5 } else { 4 };
            let xl = if vdouble { 4 } else { 6 };
            let xr = if vdouble { 8 } else { 6 };
            if u != 0 {
                for &col in v_cols(u) {
                    for row in lat.iter_mut().take(yu + 1) {
                        row[col] = true;
                    }
                }
            }
            if d != 0 {
                for &col in v_cols(d) {
                    for row in lat.iter_mut().skip(yd) {
                        row[col] = true;
                    }
                }
            }
            if l != 0 {
                for &row in h_rows(l) {
                    hseg(&mut lat, row, 0, xl);
                }
            }
            if r != 0 {
                for &row in h_rows(r) {
                    hseg(&mut lat, row, xr, 10);
                }
            }
        }
        return Some(lat);
    }
    let on = |r: usize, x: usize| -> bool {
        let even = x.is_multiple_of(2);
        match c {
            '░' => even && x.is_multiple_of(4) && r.is_multiple_of(3),
            '▒' => even && (x / 2 + r).is_multiple_of(2),
            '▓' => even && !(x.is_multiple_of(4) && r.is_multiple_of(3) && (x / 2 + r) % 2 == 1),
            '█' => even,
            '▀' => even && r <= 4,
            '▄' => even && r >= 4,
            '▌' => even && x < 6,
            '▐' => even && x >= 6,
            _ => false,
        }
    };
    if !"░▒▓█▀▄▌▐".contains(c) {
        return None;
    }
    for (r, row) in lat.iter_mut().enumerate() {
        for (x, d) in row.iter_mut().enumerate() {
            *d = on(r, x);
        }
    }
    Some(lat)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nlq_designs_stay_inside_the_cell() {
        for (ch, program) in nlq_strokes() {
            assert_eq!(rasterize_nlq(program).1, 0, "NLQ glyph {ch} is clipped");
        }
        for (_, base, mark) in COMPOSED {
            if let Some(b) = nlq_strokes().get(base) {
                for cap in [true, false] {
                    let p = format!("{b} {}", nlq_mark(*mark, cap));
                    assert_eq!(rasterize_nlq(&p).1, 0, "{base} with a mark is clipped");
                }
            }
        }
    }
}
