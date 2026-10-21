//! Reference trace v1: one CSV row per instruction, recorded BEFORE the instruction runs.
//!
//! ```text
//! # format=apple2-trace-v1
//! # source=apple2-rust
//! cycle,pc,op,a,x,y,sp,p
//! 0,FA62,D8,FF,FF,FF,FF,34
//! ```
//!
//! UTF-8; `cycle` is decimal (cycles accumulated since power-on); `pc` has four uppercase hex
//! digits and `op`, `a`, `x`, `y`, `sp`, `p` two. Lines starting with `#` are `key=value`
//! metadata. The same format is produced from AppleWin (C++) as the oracle.

use apple2_core::Machine;
use std::fmt;
use std::io::{self, BufRead, Write};

/// The exact header line.
pub const HEADER: &str = "cycle,pc,op,a,x,y,sp,p";
/// Value of the `format` metadata key written by [`TraceWriter`].
pub const FORMAT_ID: &str = "apple2-trace-v1";

/// Trace v1 records `p` with bits 4 (B) and 5 (reserved) set, as AppleWin keeps them in
/// `regs.ps`. The CPU register itself is unchanged; this is only its representation.
pub const P_FIXED_BITS: u8 = 0x30;

/// A column of the trace.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Cycle,
    Pc,
    Op,
    A,
    X,
    Y,
    Sp,
    P,
}

impl Field {
    pub const ALL: [Field; 8] = [
        Field::Cycle,
        Field::Pc,
        Field::Op,
        Field::A,
        Field::X,
        Field::Y,
        Field::Sp,
        Field::P,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Field::Cycle => "cycle",
            Field::Pc => "pc",
            Field::Op => "op",
            Field::A => "a",
            Field::X => "x",
            Field::Y => "y",
            Field::Sp => "sp",
            Field::P => "p",
        }
    }

    pub fn parse(s: &str) -> Option<Field> {
        Field::ALL.into_iter().find(|f| f.name() == s)
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Parses a comma-separated field list such as `cycle,p`.
pub fn parse_field_list(s: &str) -> Result<Vec<Field>, String> {
    s.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(|t| Field::parse(t).ok_or_else(|| format!("unknown trace field `{t}`")))
        .collect()
}

/// One instruction's pre-execution state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct TraceRow {
    pub cycle: u64,
    pub pc: u16,
    pub op: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub p: u8,
}

impl TraceRow {
    /// Snapshot of `m` just before its next instruction.
    pub fn from_machine(m: &Machine) -> TraceRow {
        let c = m.cpu();
        TraceRow {
            cycle: m.cycles(),
            pc: c.pc,
            op: m.peek(c.pc),
            a: c.a,
            x: c.x,
            y: c.y,
            sp: c.sp,
            p: c.p | P_FIXED_BITS,
        }
    }

    pub fn get(&self, f: Field) -> u64 {
        match f {
            Field::Cycle => self.cycle,
            Field::Pc => self.pc as u64,
            Field::Op => self.op as u64,
            Field::A => self.a as u64,
            Field::X => self.x as u64,
            Field::Y => self.y as u64,
            Field::Sp => self.sp as u64,
            Field::P => self.p as u64,
        }
    }

    /// The CSV line (without newline).
    pub fn to_csv(&self) -> String {
        format!(
            "{},{:04X},{:02X},{:02X},{:02X},{:02X},{:02X},{:02X}",
            self.cycle, self.pc, self.op, self.a, self.x, self.y, self.sp, self.p
        )
    }

    /// Parses one CSV data line, enforcing the v1 digit counts and uppercase hex.
    pub fn parse_csv(line: &str) -> Result<TraceRow, String> {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() != 8 {
            return Err(format!("expected 8 columns, found {}", cols.len()));
        }
        let cycle = cols[0]
            .parse::<u64>()
            .map_err(|_| format!("bad cycle `{}`", cols[0]))?;
        if cols[0].is_empty() || !cols[0].bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("bad cycle `{}`", cols[0]));
        }
        let hex = |name: &str, s: &str, digits: usize| -> Result<u32, String> {
            if s.len() != digits
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
            {
                return Err(format!(
                    "bad {name} `{s}`: need {digits} uppercase hex digits"
                ));
            }
            u32::from_str_radix(s, 16).map_err(|_| format!("bad {name} `{s}`"))
        };
        Ok(TraceRow {
            cycle,
            pc: hex("pc", cols[1], 4)? as u16,
            op: hex("op", cols[2], 2)? as u8,
            a: hex("a", cols[3], 2)? as u8,
            x: hex("x", cols[4], 2)? as u8,
            y: hex("y", cols[5], 2)? as u8,
            sp: hex("sp", cols[6], 2)? as u8,
            p: hex("p", cols[7], 2)? as u8,
        })
    }
}

/// Writes trace v1. Not buffered by itself: wrap files in a `BufWriter`.
pub struct TraceWriter<W: Write> {
    out: W,
    rows: u64,
    /// Cycles executed since the last [`Machine::update_cards`].
    batch: u64,
}

/// Length of the oracle's execution batch (`CpuExecute` slice of 1 ms at 1.0205 MHz). Cards are
/// updated at the end of each one: the Disk II counts its motor-off delay there.
pub const BATCH_CYCLES: u64 = 1020;

impl<W: Write> TraceWriter<W> {
    /// Starts a trace: writes `# format=...`, the given metadata (`key=value`, in order) and the
    /// header.
    pub fn new(mut out: W, meta: &[(&str, &str)]) -> io::Result<Self> {
        writeln!(out, "# format={FORMAT_ID}")?;
        for (k, v) in meta {
            if k.is_empty() || k.contains(['=', '\n', '\r']) || v.contains(['\n', '\r']) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("bad trace metadata `{k}={v}`"),
                ));
            }
            writeln!(out, "# {k}={v}")?;
        }
        writeln!(out, "{HEADER}")?;
        Ok(TraceWriter {
            out,
            rows: 0,
            batch: 0,
        })
    }

    /// Records the state `m` is in right now (call before running the instruction).
    pub fn record(&mut self, m: &Machine) -> io::Result<()> {
        self.record_row(&TraceRow::from_machine(m))
    }

    pub fn record_row(&mut self, row: &TraceRow) -> io::Result<()> {
        self.rows += 1;
        writeln!(self.out, "{}", row.to_csv())
    }

    /// Records, then executes one instruction; returns its cycles. This is how a trace is
    /// "attached" to a [`Machine`] without any hook in the core. A step that only enters an
    /// interrupt (IRQ/NMI) writes no row, like the oracle (see the Trace v1 conventions).
    pub fn step(&mut self, m: &mut Machine) -> io::Result<u32> {
        let enters_interrupt = m.next_step_is_interrupt_entry();
        let mut row = TraceRow::from_machine(m);
        let cycles = m.step();
        self.batch += cycles as u64;
        if self.batch >= BATCH_CYCLES {
            m.update_cards(self.batch);
            self.batch = 0;
        }
        if enters_interrupt {
            return Ok(cycles);
        }
        if (0xC000..=0xCFFF).contains(&row.pc) {
            // `peek` cannot read soft switches or slot ROM without side effects (code running in
            // $C000-$C0FF, GH#728, fetches the floating bus / keyboard latch): take the opcode
            // the CPU fetched.
            row.op = m.cpu().opcode;
        }
        self.record_row(&row)?;
        Ok(cycles)
    }

    /// Traced equivalent of `run_cycles`: until at least `n` more cycles have passed.
    pub fn run_cycles(&mut self, m: &mut Machine, n: u64) -> io::Result<()> {
        let end = m.cycles() + n;
        while m.cycles() < end {
            self.step(m)?;
        }
        Ok(())
    }

    /// Traced equivalent of `run_frame`.
    pub fn run_frame(&mut self, m: &mut Machine) -> io::Result<()> {
        let target = m.frames() + 1;
        while m.frames() < target {
            self.step(m)?;
        }
        Ok(())
    }

    pub fn rows_written(&self) -> u64 {
        self.rows
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.out.flush()?;
        Ok(self.out)
    }
}

/// A parsed trace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Trace {
    /// `# key=value` lines, in file order.
    pub meta: Vec<(String, String)>,
    pub rows: Vec<TraceRow>,
    /// File line number (1-based) of each row.
    pub lines: Vec<usize>,
}

impl Trace {
    pub fn meta(&self, key: &str) -> Option<&str> {
        self.meta
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Error from [`TraceReader`], with the file line it happened at (0 for I/O before any line).
#[derive(Debug)]
pub struct TraceError {
    pub line: usize,
    pub message: String,
}

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.message)
    }
}

impl std::error::Error for TraceError {}

/// Reads trace v1.
pub struct TraceReader;

impl TraceReader {
    pub fn read<R: BufRead>(input: R) -> Result<Trace, TraceError> {
        let mut t = Trace::default();
        let mut header_seen = false;
        for (i, line) in input.lines().enumerate() {
            let n = i + 1;
            let line = line.map_err(|e| TraceError {
                line: n,
                message: e.to_string(),
            })?;
            let line = line.trim_end_matches('\r');
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix('#') {
                let rest = rest.trim_start();
                let (k, v) = rest.split_once('=').ok_or_else(|| TraceError {
                    line: n,
                    message: format!("metadata line without `=`: `{line}`"),
                })?;
                t.meta.push((k.trim().to_string(), v.trim().to_string()));
                continue;
            }
            if !header_seen {
                if line != HEADER {
                    return Err(TraceError {
                        line: n,
                        message: format!("expected header `{HEADER}`, found `{line}`"),
                    });
                }
                header_seen = true;
                continue;
            }
            let row =
                TraceRow::parse_csv(line).map_err(|message| TraceError { line: n, message })?;
            t.rows.push(row);
            t.lines.push(n);
        }
        if !header_seen {
            return Err(TraceError {
                line: 0,
                message: format!("missing header `{HEADER}`"),
            });
        }
        Ok(t)
    }

    pub fn read_str(s: &str) -> Result<Trace, TraceError> {
        Self::read(s.as_bytes())
    }

    pub fn read_file(path: &std::path::Path) -> Result<Trace, TraceError> {
        let f = std::fs::File::open(path).map_err(|e| TraceError {
            line: 0,
            message: format!("{}: {e}", path.display()),
        })?;
        Self::read(io::BufReader::new(f))
    }
}

/// How [`compare_with`] behaves.
#[derive(Clone, Debug, Default)]
pub struct CompareOptions {
    /// Rows of expected trace kept before the divergence.
    pub context_lines: usize,
    /// Fields that are not compared (e.g. `cycle` when the oracle has no reliable cycle count).
    pub ignore: Vec<Field>,
}

/// One field that differs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldDiff {
    pub field: Field,
    pub expected: u64,
    pub actual: u64,
}

/// What kind of divergence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DivergenceKind {
    /// Same row index, different values.
    Mismatch,
    /// `actual` ended while `expected` still has rows.
    ActualTooShort,
    /// `actual` has rows after `expected` ended.
    ActualTooLong,
}

/// The first point where two traces differ.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Divergence {
    pub kind: DivergenceKind,
    /// 1-based index among instruction rows.
    pub row: usize,
    /// File line (1-based) in the expected trace, if that row exists.
    pub expected_line: Option<usize>,
    /// File line (1-based) in the actual trace, if that row exists.
    pub actual_line: Option<usize>,
    pub fields: Vec<FieldDiff>,
    pub expected: Option<TraceRow>,
    pub actual: Option<TraceRow>,
    /// Up to `context_lines` expected rows preceding the divergence (oldest first).
    pub context: Vec<TraceRow>,
    pub expected_len: usize,
    pub actual_len: usize,
}

impl Divergence {
    /// Short single-line description.
    pub fn summary(&self) -> String {
        match self.kind {
            DivergenceKind::Mismatch => {
                let f: Vec<String> = self
                    .fields
                    .iter()
                    .map(|d| {
                        format!(
                            "{} esperado {} obtido {}",
                            d.field,
                            fmt_field(d.field, d.expected),
                            fmt_field(d.field, d.actual)
                        )
                    })
                    .collect();
                format!(
                    "linha {} (instrução {}): {}",
                    self.line_for_report(),
                    self.row,
                    f.join("; ")
                )
            }
            DivergenceKind::ActualTooShort => format!(
                "trace obtido termina na instrução {} mas o esperado tem {}",
                self.actual_len, self.expected_len
            ),
            DivergenceKind::ActualTooLong => format!(
                "trace obtido tem {} instruções mas o esperado termina em {}",
                self.actual_len, self.expected_len
            ),
        }
    }

    fn line_for_report(&self) -> usize {
        self.expected_line.or(self.actual_line).unwrap_or(0)
    }

    /// Multi-line report: context rows, then the expected and actual rows.
    pub fn report(&self) -> String {
        let mut s = format!("divergência: {}\n", self.summary());
        s.push_str(&format!("# {HEADER}\n"));
        for r in &self.context {
            s.push_str(&format!("  {}\n", r.to_csv()));
        }
        match &self.expected {
            Some(r) => s.push_str(&format!("- {}\n", r.to_csv())),
            None => s.push_str("- (fim do trace esperado)\n"),
        }
        match &self.actual {
            Some(r) => s.push_str(&format!("+ {}\n", r.to_csv())),
            None => s.push_str("+ (fim do trace obtido)\n"),
        }
        s
    }
}

fn fmt_field(f: Field, v: u64) -> String {
    match f {
        Field::Cycle => v.to_string(),
        Field::Pc => format!("{v:04X}"),
        _ => format!("{v:02X}"),
    }
}

/// First divergence between `expected` and `actual`, comparing every field; `None` if equal.
pub fn compare(expected: &Trace, actual: &Trace, context_lines: usize) -> Option<Divergence> {
    compare_with(
        expected,
        actual,
        &CompareOptions {
            context_lines,
            ignore: Vec::new(),
        },
    )
}

/// Like [`compare`] with options (ignored fields).
pub fn compare_with(expected: &Trace, actual: &Trace, opts: &CompareOptions) -> Option<Divergence> {
    let common = expected.rows.len().min(actual.rows.len());
    let context_at = |i: usize| -> Vec<TraceRow> {
        expected.rows[i.saturating_sub(opts.context_lines)..i].to_vec()
    };
    for i in 0..common {
        let (e, a) = (&expected.rows[i], &actual.rows[i]);
        let fields: Vec<FieldDiff> = Field::ALL
            .into_iter()
            .filter(|f| !opts.ignore.contains(f) && e.get(*f) != a.get(*f))
            .map(|f| FieldDiff {
                field: f,
                expected: e.get(f),
                actual: a.get(f),
            })
            .collect();
        if !fields.is_empty() {
            return Some(Divergence {
                kind: DivergenceKind::Mismatch,
                row: i + 1,
                expected_line: expected.lines.get(i).copied(),
                actual_line: actual.lines.get(i).copied(),
                fields,
                expected: Some(*e),
                actual: Some(*a),
                context: context_at(i),
                expected_len: expected.rows.len(),
                actual_len: actual.rows.len(),
            });
        }
    }
    if expected.rows.len() == actual.rows.len() {
        return None;
    }
    let short = expected.rows.len() > actual.rows.len();
    Some(Divergence {
        kind: if short {
            DivergenceKind::ActualTooShort
        } else {
            DivergenceKind::ActualTooLong
        },
        row: common + 1,
        expected_line: expected.lines.get(common).copied(),
        actual_line: actual.lines.get(common).copied(),
        fields: Vec::new(),
        expected: expected.rows.get(common).copied(),
        actual: actual.rows.get(common).copied(),
        context: context_at(common),
        expected_len: expected.rows.len(),
        actual_len: actual.rows.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use apple2_core::{MachineConfig, Model};

    fn row(i: u64) -> TraceRow {
        TraceRow {
            cycle: i * 3,
            pc: 0xC000 + i as u16,
            op: 0xEA,
            a: i as u8,
            x: 1,
            y: 2,
            sp: 0xFF,
            p: 0x34,
        }
    }

    fn trace(n: u64) -> Trace {
        Trace {
            meta: vec![],
            rows: (0..n).map(row).collect(),
            lines: (0..n as usize).map(|i| i + 3).collect(),
        }
    }

    #[test]
    fn csv_row_round_trip_and_format() {
        let r = TraceRow {
            cycle: 12345,
            pc: 0xFA62,
            op: 0xD8,
            a: 0xFF,
            x: 0x0A,
            y: 0,
            sp: 0xFD,
            p: 0x34,
        };
        assert_eq!(r.to_csv(), "12345,FA62,D8,FF,0A,00,FD,34");
        assert_eq!(TraceRow::parse_csv(&r.to_csv()).unwrap(), r);
    }

    #[test]
    fn parse_rejects_wrong_digits_case_and_columns() {
        for bad in [
            "1,fa62,D8,FF,FF,FF,FF,34",
            "1,FA62,D,FF,FF,FF,FF,34",
            "1,FA6,D8,FF,FF,FF,FF,34",
            "1,FA62,D8,FF,FF,FF,FF",
            "x,FA62,D8,FF,FF,FF,FF,34",
            "+1,FA62,D8,FF,FF,FF,FF,34",
            "1,FA62,D8,FF,FF,FF,FF,3G",
        ] {
            assert!(TraceRow::parse_csv(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn writer_reader_round_trip_with_metadata() {
        let mut w =
            TraceWriter::new(Vec::new(), &[("model", "apple2plus"), ("note", "a=b")]).unwrap();
        for i in 0..5 {
            w.record_row(&row(i)).unwrap();
        }
        assert_eq!(w.rows_written(), 5);
        let bytes = w.finish().unwrap();
        let text = String::from_utf8(bytes).unwrap();
        assert!(text.starts_with("# format=apple2-trace-v1\n# model=apple2plus\n# note=a=b\ncycle,pc,op,a,x,y,sp,p\n0,C000,EA,00,01,02,FF,34\n"));
        let t = TraceReader::read_str(&text).unwrap();
        assert_eq!(t.meta("format"), Some(FORMAT_ID));
        assert_eq!(t.meta("note"), Some("a=b"));
        assert_eq!(t.rows, trace(5).rows);
        assert_eq!(t.lines, vec![5, 6, 7, 8, 9]);
    }

    #[test]
    fn reader_errors_carry_the_line() {
        let e = TraceReader::read_str("cycle,pc,op,a,x,y,sp,p\n0,C000,EA,00,01,02,FF,34\n1,zz\n")
            .unwrap_err();
        assert_eq!(e.line, 3);
        assert!(TraceReader::read_str("0,C000,EA,00,01,02,FF,34\n").is_err());
        assert!(TraceReader::read_str("# no equals\ncycle,pc,op,a,x,y,sp,p\n").is_err());
        assert!(TraceReader::read_str("# only=meta\n").is_err());
        // CRLF and blank lines are tolerated.
        let t = TraceReader::read_str("cycle,pc,op,a,x,y,sp,p\r\n\r\n0,C000,EA,00,01,02,FF,34\r\n")
            .unwrap();
        assert_eq!(t.rows.len(), 1);
    }

    #[test]
    fn writer_rejects_bad_metadata() {
        assert!(TraceWriter::new(Vec::new(), &[("a=b", "c")]).is_err());
        assert!(TraceWriter::new(Vec::new(), &[("a", "c\nd")]).is_err());
    }

    #[test]
    fn compare_equal() {
        assert_eq!(compare(&trace(10), &trace(10), 3), None);
        assert_eq!(compare(&trace(0), &trace(0), 3), None);
    }

    #[test]
    fn compare_difference_in_the_middle() {
        let e = trace(10);
        let mut a = trace(10);
        a.rows[6].a = 0x99;
        a.rows[6].p = 0x00;
        let d = compare(&e, &a, 3).unwrap();
        assert_eq!(d.kind, DivergenceKind::Mismatch);
        assert_eq!(d.row, 7);
        assert_eq!(d.expected_line, Some(9));
        assert_eq!(d.fields.len(), 2);
        assert_eq!(d.fields[0].field, Field::A);
        assert_eq!(d.fields[0].expected, 6);
        assert_eq!(d.fields[0].actual, 0x99);
        assert_eq!(d.fields[1].field, Field::P);
        assert_eq!(d.context, e.rows[3..6].to_vec());
        assert!(d.report().contains("- ") && d.report().contains("+ "));
    }

    #[test]
    fn compare_context_is_clamped_at_the_start() {
        let e = trace(5);
        let mut a = trace(5);
        a.rows[1].pc = 0;
        let d = compare(&e, &a, 10).unwrap();
        assert_eq!(d.row, 2);
        assert_eq!(d.context.len(), 1);
        let d0 = compare(&e, &a, 0).unwrap();
        assert!(d0.context.is_empty());
    }

    #[test]
    fn compare_difference_at_the_end() {
        let e = trace(10);
        let mut a = trace(10);
        a.rows[9].x = 7;
        let d = compare(&e, &a, 2).unwrap();
        assert_eq!(d.row, 10);
        assert_eq!(d.context.len(), 2);
    }

    #[test]
    fn compare_length_differences() {
        let d = compare(&trace(10), &trace(7), 2).unwrap();
        assert_eq!(d.kind, DivergenceKind::ActualTooShort);
        assert_eq!(d.row, 8);
        assert_eq!(d.expected, Some(row(7)));
        assert_eq!(d.actual, None);
        assert_eq!(d.context.len(), 2);
        let d = compare(&trace(7), &trace(10), 2).unwrap();
        assert_eq!(d.kind, DivergenceKind::ActualTooLong);
        assert_eq!(d.row, 8);
        assert_eq!(d.expected, None);
        assert_eq!(d.actual, Some(row(7)));
    }

    #[test]
    fn compare_ignored_fields() {
        let e = trace(6);
        let mut a = trace(6);
        for r in &mut a.rows {
            r.cycle += 100;
        }
        assert!(compare(&e, &a, 1).is_some());
        let opts = CompareOptions {
            context_lines: 1,
            ignore: vec![Field::Cycle],
        };
        assert_eq!(compare_with(&e, &a, &opts), None);
        // Another field still diverges, and the ignored one is not reported.
        a.rows[2].y = 9;
        let d = compare_with(&e, &a, &opts).unwrap();
        assert_eq!(d.fields.len(), 1);
        assert_eq!(d.fields[0].field, Field::Y);
    }

    #[test]
    fn field_list_parsing() {
        assert_eq!(
            parse_field_list("cycle, p").unwrap(),
            vec![Field::Cycle, Field::P]
        );
        assert!(parse_field_list("cycle,zz").is_err());
        assert!(parse_field_list("").unwrap().is_empty());
    }

    #[test]
    fn records_a_real_machine_before_each_instruction() {
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
        let mut w = TraceWriter::new(Vec::new(), &[]).unwrap();
        let first = TraceRow::from_machine(&m);
        assert_eq!(first.cycle, 0);
        w.run_cycles(&mut m, 200).unwrap();
        let n = w.rows_written();
        assert!(n > 20);
        let t = TraceReader::read(&w.finish().unwrap()[..]).unwrap();
        assert_eq!(t.rows[0], first);
        // The next row is the state after the previous instruction ran.
        assert!(t.rows[1].cycle > t.rows[0].cycle);
        assert!(t.rows.windows(2).all(|p| p[0].cycle < p[1].cycle));
    }
}
