//! Parallel printer interface card (ParallelPrinter.cpp).
//!
//! The card prints a byte written to `$C0s0` ("load output port") and answers every read of its I/O
//! page with `$FF`. Like AppleWin, the output file is opened lazily at the first access and closed
//! after the printer has been idle for [`PrinterOptions::idle_limit`] "seconds" (710000 cycles
//! each, :105); output goes to a [`PrinterSink`] so tests and frontends need no file.

use apple2_core::card::CardCtx;
use apple2_core::Card;
use std::io;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Cycles AppleWin counts per idle-limit second (ParallelPrinter.cpp:105; not the real clock).
pub const CYCLES_PER_IDLE_SECOND: u64 = 710_000;

/// Where printed bytes go. `open`/`close` bracket one print job (file open/close in AppleWin).
pub trait PrinterSink: Send {
    /// Starts a job; `append` keeps earlier output (`"ab"` vs `"wb"`, :65-68).
    fn open(&mut self, append: bool) -> io::Result<()>;
    fn write(&mut self, byte: u8) -> io::Result<()>;
    /// Ends the job. `dump_to_printer`: also copy it to the system printer (`copy file prn`, :81-86,
    /// Windows only).
    fn close(&mut self, dump_to_printer: bool);
}

/// Writes to a file (the default `Printer.txt` of AppleWin, :162).
pub struct FileSink {
    path: PathBuf,
    file: Option<io::BufWriter<std::fs::File>>,
}

impl FileSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        FileSink { path: path.into(), file: None }
    }
}

impl PrinterSink for FileSink {
    fn open(&mut self, append: bool) -> io::Result<()> {
        let f = std::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append).open(&self.path)?;
        self.file = Some(io::BufWriter::new(f));
        Ok(())
    }

    fn write(&mut self, byte: u8) -> io::Result<()> {
        use io::Write;
        match &mut self.file {
            Some(f) => f.write_all(&[byte]),
            None => Err(io::ErrorKind::NotConnected.into()),
        }
    }

    fn close(&mut self, dump_to_printer: bool) {
        use io::Write;
        if let Some(mut f) = self.file.take() {
            let _ = f.flush();
            drop(f);
            #[cfg(windows)]
            if dump_to_printer {
                let _ = std::process::Command::new("cmd").arg("/C").arg("copy").arg(&self.path).arg("prn").status();
            }
            #[cfg(not(windows))]
            let _ = dump_to_printer;
        }
    }
}

#[derive(Default)]
struct MemInner {
    data: Vec<u8>,
    open: bool,
    jobs: u32,
}

/// In-memory sink for tests; clone the handle before boxing the sink.
#[derive(Clone, Default)]
pub struct MemorySink(Arc<Mutex<MemInner>>);

impl MemorySink {
    pub fn new() -> Self {
        Self::default()
    }

    /// Everything printed so far.
    pub fn contents(&self) -> Vec<u8> {
        self.0.lock().unwrap().data.clone()
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.contents()).into_owned()
    }

    pub fn is_open(&self) -> bool {
        self.0.lock().unwrap().open
    }

    /// Number of times a job was opened.
    pub fn jobs(&self) -> u32 {
        self.0.lock().unwrap().jobs
    }
}

impl PrinterSink for MemorySink {
    fn open(&mut self, append: bool) -> io::Result<()> {
        let mut s = self.0.lock().unwrap();
        if !append {
            s.data.clear();
        }
        s.open = true;
        s.jobs += 1;
        Ok(())
    }

    fn write(&mut self, byte: u8) -> io::Result<()> {
        self.0.lock().unwrap().data.push(byte);
        Ok(())
    }

    fn close(&mut self, _dump_to_printer: bool) {
        self.0.lock().unwrap().open = false;
    }
}

/// Bulgarian keyboard-to-printer conversion (`Pravets::ConvertToPrinterChar`, Pravets.cpp:208-262).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PrinterEncoding {
    Pravets8A,
    Pravets82,
}

impl PrinterEncoding {
    /// Maps the Latin transliteration the Pravets prints to its Cyrillic (CP-1251) byte.
    pub fn convert(self, value: u8) -> u8 {
        const LAT_8A: &[u8] = b"abwgdevzijklmnoprstufhc~{}yx`q|]";
        const LAT_82: &[u8] = b"abwgdevzijklmnoprstufhc^[]yx@q{}~`";
        // CP-1251 À..ß minus the letters the Pravets 82 does not use, then its four symbols.
        const KIR_82: &[u8] = &[
            0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD, 0xCE, 0xCF, 0xD0, 0xD1,
            0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDC, 0xDE, 0xDF, b'[', b']', b'^', b'@',
        ];
        let map = |lat: &[u8], kir: &dyn Fn(usize) -> u8, n: usize, c: u8| {
            lat.iter().take(n).position(|&l| l == c).map_or(c, kir)
        };
        match self {
            PrinterEncoding::Pravets8A => {
                // The C++ loops over 31 table entries (the 32nd, `]`, never matches).
                let capital = |i: usize| 0xC0 + i as u8;
                let lower = |i: usize| 0xE0 + i as u8;
                if value > 90 && value < 128 {
                    map(LAT_8A, &lower, 31, value)
                } else if value > 64 && value < 91 {
                    value + 32
                } else {
                    map(LAT_8A, &capital, 31, value & 0x7F)
                }
            }
            PrinterEncoding::Pravets82 => map(LAT_82, &|i| KIR_82[i], 34, value & 0x7F),
        }
    }
}

/// `m_printerIdleLimit`, `m_bDumpToPrinter`, ... (ParallelPrinter.h:63-71).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrinterOptions {
    /// Idle seconds (of [`CYCLES_PER_IDLE_SECOND`] cycles) before the job is closed.
    pub idle_limit: u32,
    /// Copy each finished job to the system printer (`copy file prn`, Windows).
    pub dump_to_printer: bool,
    /// Pravets only: convert the Latin transliteration to Cyrillic.
    pub convert_encoding: Option<PrinterEncoding>,
    /// Drop control characters other than CR and LF.
    pub filter_unprintable: bool,
    /// Append to the file instead of overwriting at each job.
    pub append: bool,
}

impl Default for PrinterOptions {
    fn default() -> Self {
        PrinterOptions { idle_limit: 10, dump_to_printer: false, convert_encoding: None, filter_unprintable: false, append: false }
    }
}

/// The state AppleWin saves for the printer (`ParallelPrinterCard::SaveSnapshot`,
/// ParallelPrinter.cpp:223). The file name is the sink's business and is not kept here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrinterSnapshot {
    /// `m_inactivity` (cycles since the last access while a job is open).
    pub inactivity: u64,
    pub options: PrinterOptions,
    /// `m_file != NULL`: a job is open.
    pub is_open: bool,
}

pub struct ParallelPrinterCard {
    sink: Box<dyn PrinterSink>,
    options: PrinterOptions,
    is_open: bool,
    inactivity: u64,
}

impl ParallelPrinterCard {
    pub fn new(sink: Box<dyn PrinterSink>, options: PrinterOptions) -> Self {
        ParallelPrinterCard { sink, options, is_open: false, inactivity: 0 }
    }

    pub fn options(&self) -> &PrinterOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: PrinterOptions) {
        self.options = options;
    }

    pub fn snapshot(&self) -> PrinterSnapshot {
        PrinterSnapshot { inactivity: self.inactivity, options: self.options, is_open: self.is_open }
    }

    /// `LoadSnapshot` (:256). The options come from the snapshot except the Pravets encoding
    /// (AppleWin picks it from the machine type): `convert_encoding` keeps this card's choice,
    /// or `Pravets82` if it had none. A job that was open is resumed in append mode through
    /// `CheckPrint`, which also restarts the idle counter.
    pub fn apply_snapshot(&mut self, s: &PrinterSnapshot) {
        let encoding = self.options.convert_encoding.unwrap_or(PrinterEncoding::Pravets82);
        self.options = s.options;
        self.options.convert_encoding = s.options.convert_encoding.map(|_| encoding);
        self.inactivity = s.inactivity;
        if s.is_open {
            self.options.append = true;
            self.check_print();
        }
    }

    /// `CheckPrint` (:56-71): opens the job on first use and restarts the idle counter.
    fn check_print(&mut self) -> bool {
        self.inactivity = 0;
        if !self.is_open {
            self.is_open = self.sink.open(self.options.append).is_ok();
        }
        self.is_open
    }

    /// `ClosePrint` (:74-90).
    fn close_print(&mut self) {
        if self.is_open {
            self.sink.close(self.options.dump_to_printer);
            self.is_open = false;
        }
        self.inactivity = 0;
    }

    pub fn is_open(&self) -> bool {
        self.is_open
    }
}

impl Card for ParallelPrinterCard {
    fn name(&self) -> &'static str {
        "Generic Printer"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn io_read(&mut self, _lo: u8, _ctx: &mut CardCtx) -> u8 {
        self.check_print();
        0xFF // status: always ready (:125)
    }

    fn io_write(&mut self, lo: u8, value: u8, _ctx: &mut CardCtx) {
        if !self.check_print() || lo != 0 {
            return;
        }
        let mut c = value & 0x7F;
        if let Some(enc) = self.options.convert_encoding {
            c = enc.convert(value);
        }
        // `c > 0x7F` keeps Cyrillic output when conversion is on (:149).
        if !self.options.filter_unprintable || c > 31 || c == 13 || c == 10 {
            let _ = self.sink.write(c);
        }
    }

    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        apple2_roms::PARALLEL[offset as usize]
    }

    fn reset(&mut self, _power_cycle: bool, _ctx: &mut CardCtx) {
        self.close_print();
    }

    fn update(&mut self, cycles: u64, _ctx: &mut CardCtx) {
        if !self.is_open {
            return;
        }
        self.inactivity += cycles;
        if self.inactivity > self.options.idle_limit as u64 * CYCLES_PER_IDLE_SECOND {
            self.close_print(); // the next print overwrites or appends, per the options
        }
    }
}

impl Drop for ParallelPrinterCard {
    fn drop(&mut self) {
        self.close_print();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pravets_82_transliterates_latin_to_cyrillic() {
        let e = PrinterEncoding::Pravets82;
        assert_eq!(e.convert(b'a'), 0xC0);
        assert_eq!(e.convert(b'c'), 0xD6);
        assert_eq!(e.convert(b'q'), 0xDF);
        assert_eq!(e.convert(b'{'), b'[');
        assert_eq!(e.convert(b'A'), b'A'); // not in the table
        assert_eq!(e.convert(b'1' | 0x80), b'1');
    }

    #[test]
    fn pravets_8a_has_separate_capital_and_lower_case() {
        let e = PrinterEncoding::Pravets8A;
        assert_eq!(e.convert(b'a'), 0xE0); // lower-case range: Cyrillic lower case
        assert_eq!(e.convert(b'A'), b'a'); // capitals print as lower-case Latin first...
        assert_eq!(e.convert(b'0'), b'0'); // below 64: only the capital table applies
    }

    #[test]
    fn file_sink_appends_or_truncates() {
        let dir = std::env::temp_dir().join(format!("apple2_hostio_printer_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("Printer.txt");
        let mut s = FileSink::new(&path);
        for append in [false, true, false] {
            s.open(append).unwrap();
            s.write(b'x').unwrap();
            s.close(false);
            let len = std::fs::read(&path).unwrap().len();
            assert_eq!(len, if append { 2 } else { 1 });
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
}
