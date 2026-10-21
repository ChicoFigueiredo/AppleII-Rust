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
const BASE_W: usize = 5;

/// Design lattice in Draft coordinates: `[row][column]`, 9 rows by 12 columns.
type Lattice = [[bool; DRAFT_COLS]; DRAFT_ROWS];
/// A hand-drawn 5-column bitmap: `[row][column]`.
type Bitmap = [[bool; BASE_W]; DRAFT_ROWS];

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
    let lattice = lattice_for(ch)?;
    let mut g = lattice_to_glyph(&lattice, quality);
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

fn lattice_to_glyph(lat: &Lattice, quality: Quality) -> Glyph {
    match quality {
        Quality::Draft => {
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
                quality,
                columns: cols,
                baseline_row: 8,
            }
        }
        Quality::Nlq => {
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
                quality,
                columns: cols,
                baseline_row: 16,
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Glyph designs
// ---------------------------------------------------------------------------------------------

fn base_glyphs() -> &'static HashMap<char, Bitmap> {
    static MAP: OnceLock<HashMap<char, Bitmap>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut map = HashMap::new();
        for line in include_str!("font/glyphs.txt").lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with("//") {
                continue;
            }
            let mut toks = line.split_whitespace();
            let ch = toks.next().and_then(single_char).expect("glyph char");
            let top: usize = toks.next().and_then(|t| t.parse().ok()).expect("top row");
            let mut bm: Bitmap = [[false; BASE_W]; DRAFT_ROWS];
            for (i, row) in toks.enumerate() {
                assert!(
                    top + i < DRAFT_ROWS && row.len() == BASE_W,
                    "bad glyph {ch}"
                );
                for (c, b) in row.bytes().enumerate() {
                    bm[top + i][c] = b == b'#';
                }
            }
            assert!(map.insert(ch, bm).is_none(), "duplicate glyph {ch}");
        }
        map
    })
}

fn single_char(t: &str) -> Option<char> {
    let mut it = t.chars();
    let c = it.next()?;
    it.next().is_none().then_some(c)
}

fn bitmap_from_rows(top: usize, rows: &[&str], bm: &mut Bitmap) {
    for (i, row) in rows.iter().enumerate() {
        for (c, b) in row.bytes().enumerate() {
            if b == b'#' {
                bm[top + i][c] = true;
            }
        }
    }
}

fn first_row(bm: &Bitmap) -> usize {
    bm.iter().position(|r| r.iter().any(|&d| d)).unwrap_or(0)
}

fn compose(ch: char, base: char, mark: Mark) -> Option<Bitmap> {
    let mut bm = *base_glyphs().get(&base)?;
    let top = first_row(&bm);
    let capital = !ch.is_lowercase();
    match mark {
        Mark::Cedilla => bitmap_from_rows(8, &[".##.."], &mut bm),
        _ if capital => {
            let row = match mark {
                Mark::Acute => "...#.",
                Mark::Grave => ".#...",
                Mark::Circ | Mark::Ring => "..#..",
                Mark::Tilde => ".##.#",
                _ => ".#.#.",
            };
            bitmap_from_rows(top.saturating_sub(1), &[row], &mut bm);
        }
        _ => {
            let rows: &[&str] = match mark {
                Mark::Acute => &["...#.", "..#.."],
                Mark::Grave => &[".#...", "..#.."],
                Mark::Circ => &["..#..", ".#.#."],
                Mark::Tilde => &[".##.#", "#..#."],
                Mark::Diaer => &[".#.#.", "....."],
                _ => &["..#..", ".#.#.", "..#.."],
            };
            bitmap_from_rows(top - rows.len(), rows, &mut bm);
        }
    }
    Some(bm)
}

fn lattice_for(ch: char) -> Option<Lattice> {
    if ch == ' ' || ch == '\u{a0}' {
        return Some([[false; DRAFT_COLS]; DRAFT_ROWS]);
    }
    if is_box_or_block(ch) {
        return box_or_block(ch);
    }
    let bm = match base_glyphs().get(&ch) {
        Some(b) => *b,
        None => {
            let &(_, base, mark) = COMPOSED.iter().find(|(c, _, _)| *c == ch)?;
            compose(ch, base, mark)?
        }
    };
    let mut lat: Lattice = [[false; DRAFT_COLS]; DRAFT_ROWS];
    for (r, row) in bm.iter().enumerate() {
        for (c, &d) in row.iter().enumerate() {
            lat[r][2 * c + 1] = d;
        }
    }
    Some(lat)
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
