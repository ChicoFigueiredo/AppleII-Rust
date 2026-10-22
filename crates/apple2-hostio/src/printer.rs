//! Parallel printer interface card (ParallelPrinter.cpp).
//!
//! The card prints a byte written to `$C0s0` ("load output port") and answers every read of its I/O
//! page with `$FF`. Like AppleWin, the output file is opened lazily at the first access and closed
//! after the printer has been idle for [`PrinterOptions::idle_limit`] "seconds" (710000 cycles
//! each, :105); output goes to a [`PrinterSink`] so tests and frontends need no file.

use apple2_core::card::CardCtx;
use apple2_core::Card;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::printer_capture::CaptureSender;
use apple2_printer::capture::{CloseReason, JobOrigin, Transport};

/// Cycles between deliveries of the bytes accumulated for the virtual printer (about one frame).
pub const VIRTUAL_FLUSH_CYCLES: u64 = 17_030;
/// Accumulated bytes that force a delivery before the cycle count does.
pub const VIRTUAL_FLUSH_BYTES: usize = 4096;

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

    /// Destination file, for sinks that have one. Saved in save-states (`Print Filename`).
    fn path(&self) -> Option<&Path> {
        None
    }

    /// Redirects the sink to another file (`LoadSnapshot` restores the saved name, :255).
    fn set_path(&mut self, _path: PathBuf) {}
}

/// Why a printer file name was refused.
fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, msg.into())
}

/// Canonical, safe form of a user-given printer file name (AUD-P-033).
///
/// AppleWin uses the registry string as is (relative to the current directory, which
/// `-current-dir` can change). We resolve a relative name against `base` (the configuration
/// directory) or, without one, the current directory, drop `.` and `..` components, and
/// canonicalise the parent directory when it exists (so symbolic links and `..` through them
/// cannot hide the real destination). The file itself need not exist.
///
/// Refused: empty names, names with a NUL, directories (existing, or ending in a separator) and
/// names without a final component.
pub fn sanitize_printer_path(raw: &Path, base: Option<&Path>) -> io::Result<PathBuf> {
    let text = raw.to_string_lossy();
    if text.trim().is_empty() {
        return Err(invalid("the printer file name is empty"));
    }
    if text.contains('\0') {
        return Err(invalid("the printer file name contains a NUL character"));
    }
    if text.ends_with('/') || (cfg!(windows) && text.ends_with('\\')) {
        return Err(invalid(format!("'{text}' is a directory, not a file")));
    }
    let absolute = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        match base {
            Some(b) => b.join(raw),
            None => std::env::current_dir()?.join(raw),
        }
    };
    let mut clean = PathBuf::new();
    for c in absolute.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                clean.pop();
            }
            other => clean.push(other.as_os_str()),
        }
    }
    let Some(name) = clean.file_name().map(|n| n.to_os_string()) else {
        return Err(invalid(format!("'{text}' has no file name")));
    };
    let parent = clean.parent().unwrap_or(Path::new("")).to_path_buf();
    let mut resolved = std::fs::canonicalize(&parent).unwrap_or(parent);
    resolved.push(name);
    if resolved.is_dir() {
        return Err(invalid(format!(
            "'{}' is a directory, not a file",
            resolved.display()
        )));
    }
    Ok(resolved)
}

/// Writes to a file (the default `Printer.txt` of AppleWin, :162).
pub struct FileSink {
    /// As given by the user; resolved by [`sanitize_printer_path`] each time a job opens.
    path: PathBuf,
    base_dir: Option<PathBuf>,
    file: Option<io::BufWriter<std::fs::File>>,
    /// Error of the last flush at the end of a job.
    last_error: Option<String>,
}

impl FileSink {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        FileSink {
            path: path.into(),
            base_dir: None,
            file: None,
            last_error: None,
        }
    }

    /// Resolves a relative name against `dir` (the configuration directory) instead of the
    /// current directory.
    pub fn with_base_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.base_dir = Some(dir.into());
        self
    }

    /// The error of the last job that failed to flush to disk (disk full, device gone).
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    fn resolved(&self) -> io::Result<PathBuf> {
        sanitize_printer_path(&self.path, self.base_dir.as_deref())
    }
}

impl PrinterSink for FileSink {
    fn open(&mut self, append: bool) -> io::Result<()> {
        let path = self.resolved()?;
        let f = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&path)
            .map_err(|e| {
                io::Error::new(
                    e.kind(),
                    format!("cannot open printer file '{}': {e}", path.display()),
                )
            })?;
        self.file = Some(io::BufWriter::new(f));
        Ok(())
    }

    fn path(&self) -> Option<&Path> {
        Some(&self.path)
    }

    fn set_path(&mut self, path: PathBuf) {
        self.path = path;
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
            match f.flush() {
                Ok(()) => self.last_error = None,
                Err(e) => {
                    let msg = format!("printer file '{}': write failed: {e}", self.path.display());
                    crate::diagnostics::report("printer-write", msg.clone());
                    self.last_error = Some(msg);
                }
            }
            drop(f);
            #[cfg(windows)]
            if dump_to_printer {
                let _ = std::process::Command::new("cmd")
                    .arg("/C")
                    .arg("copy")
                    .arg(self.resolved().unwrap_or_else(|_| self.path.clone()))
                    .arg("prn")
                    .status();
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
            0xC0, 0xC1, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xCB, 0xCC, 0xCD,
            0xCE, 0xCF, 0xD0, 0xD1, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xDC,
            0xDE, 0xDF, b'[', b']', b'^', b'@',
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
        PrinterOptions {
            idle_limit: 10,
            dump_to_printer: false,
            convert_encoding: None,
            filter_unprintable: false,
            append: false,
        }
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

/// The virtual printer's path out of the card (independent of the legacy file sink): the byte
/// goes through the [`Transport`] only, with no unprintable filter and no Pravets conversion.
/// Bytes are accumulated and handed over in blocks (see [`VIRTUAL_FLUSH_CYCLES`]).
struct VirtualOutput {
    tx: CaptureSender,
    origin: JobOrigin,
    transport: Transport,
    job: Option<VirtualJob>,
}

struct VirtualJob {
    id: u64,
    /// Bytes of the job already handed to the queue.
    delivered: u64,
    pending: Vec<u8>,
    inactivity: u64,
    since_flush: u64,
}

impl VirtualOutput {
    /// `from_firmware`: the writing instruction is in the card's ROM (see [`Transport::Auto`]).
    fn write(&mut self, value: u8, from_firmware: bool) {
        let byte = match self.transport {
            Transport::Auto if from_firmware => value & 0x7F,
            Transport::Auto | Transport::EightBit => value,
            Transport::SevenBitAppleWin => value & 0x7F,
        };
        let job = self.job.get_or_insert_with(|| VirtualJob {
            id: self.tx.open(self.origin.clone(), self.transport),
            delivered: 0,
            pending: Vec::new(),
            inactivity: 0,
            since_flush: 0,
        });
        job.inactivity = 0;
        job.pending.push(byte);
        if job.pending.len() >= VIRTUAL_FLUSH_BYTES {
            self.flush();
        }
    }

    /// Any access to the card's I/O page keeps an open job alive (like the legacy counter); a read
    /// never opens one (CA02).
    fn touch(&mut self) {
        if let Some(j) = &mut self.job {
            j.inactivity = 0;
        }
    }

    fn flush(&mut self) {
        if let Some(j) = &mut self.job {
            j.since_flush = 0;
            if !j.pending.is_empty() {
                let data = std::mem::take(&mut j.pending);
                let n = data.len() as u64;
                self.tx.bytes(j.id, j.delivered, data);
                j.delivered += n;
            }
        }
    }

    fn close(&mut self, reason: CloseReason) {
        self.flush();
        if let Some(j) = self.job.take() {
            self.tx.close(j.id, reason);
        }
    }

    fn update(&mut self, cycles: u64, idle_cycles: u64) {
        let Some(j) = &mut self.job else { return };
        j.inactivity += cycles;
        j.since_flush += cycles;
        if j.inactivity > idle_cycles {
            self.close(CloseReason::Idle);
        } else if j.since_flush >= VIRTUAL_FLUSH_CYCLES {
            self.flush();
        }
    }
}

pub struct ParallelPrinterCard {
    /// The legacy file output (AppleWin's `Printer.txt`); `None` for a virtual-only card.
    sink: Option<Box<dyn PrinterSink>>,
    virtual_out: Option<VirtualOutput>,
    options: PrinterOptions,
    is_open: bool,
    inactivity: u64,
    last_error: Option<String>,
}

impl ParallelPrinterCard {
    pub fn new(sink: Box<dyn PrinterSink>, options: PrinterOptions) -> Self {
        ParallelPrinterCard {
            sink: Some(sink),
            virtual_out: None,
            options,
            is_open: false,
            inactivity: 0,
            last_error: None,
        }
    }

    /// A card without the legacy file output: only the virtual printer receives the bytes.
    pub fn virtual_only(options: PrinterOptions) -> Self {
        let mut c = Self::new(Box::new(MemorySink::new()), options);
        c.sink = None;
        c
    }

    /// Adds the virtual printer's output (see the module docs of [`crate::printer_capture`]).
    /// `origin` says where jobs come from; the byte reaches the queue as `transport` dictates.
    pub fn with_virtual(
        mut self,
        tx: CaptureSender,
        origin: JobOrigin,
        transport: Transport,
    ) -> Self {
        self.virtual_out = Some(VirtualOutput {
            tx,
            origin,
            transport,
            job: None,
        });
        self
    }

    /// Ends the virtual job (if any) with `reason`, delivering the pending bytes first. The
    /// emulator calls it with `CardRemoved` / `StateLoaded` before it replaces the machine.
    pub fn close_virtual(&mut self, reason: CloseReason) {
        if let Some(v) = &mut self.virtual_out {
            v.close(reason);
        }
    }

    /// Id of the open virtual job.
    pub fn virtual_job(&self) -> Option<u64> {
        self.virtual_out.as_ref()?.job.as_ref().map(|j| j.id)
    }

    /// Why the last attempt to open or write the print file failed, until one succeeds. The
    /// emulation goes on with the printer "disconnected" (AppleWin ignores the failure silently;
    /// we also report it through [`crate::diagnostics`]).
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    /// The print file name as configured (saved in save-states).
    pub fn filename(&self) -> Option<&Path> {
        self.sink.as_ref().and_then(|s| s.path())
    }

    /// Redirects the output (`LoadSnapshot` restores the name saved by AppleWin, :255).
    pub fn set_filename(&mut self, path: PathBuf) {
        if let Some(s) = &mut self.sink {
            s.set_path(path);
        }
    }

    pub fn options(&self) -> &PrinterOptions {
        &self.options
    }

    pub fn set_options(&mut self, options: PrinterOptions) {
        self.options = options;
    }

    pub fn snapshot(&self) -> PrinterSnapshot {
        PrinterSnapshot {
            inactivity: self.inactivity,
            options: self.options,
            is_open: self.is_open,
        }
    }

    /// `LoadSnapshot` (:256). The options come from the snapshot except the Pravets encoding
    /// (AppleWin picks it from the machine type): `convert_encoding` keeps this card's choice,
    /// or `Pravets82` if it had none. A job that was open is resumed in append mode through
    /// `CheckPrint`, which also restarts the idle counter.
    pub fn apply_snapshot(&mut self, s: &PrinterSnapshot) {
        let encoding = self
            .options
            .convert_encoding
            .unwrap_or(PrinterEncoding::Pravets82);
        self.options = s.options;
        self.options.convert_encoding = s.options.convert_encoding.map(|_| encoding);
        self.inactivity = s.inactivity;
        // The virtual job belongs to the machine that was replaced (ADR 0008).
        self.close_virtual(CloseReason::StateLoaded);
        if s.is_open {
            self.options.append = true;
            self.check_print();
        }
    }

    /// `CheckPrint` (:56-71): opens the job on first use and restarts the idle counter.
    fn check_print(&mut self) -> bool {
        self.inactivity = 0;
        let Some(sink) = &mut self.sink else {
            return false;
        };
        if !self.is_open {
            match sink.open(self.options.append) {
                Ok(()) => {
                    self.is_open = true;
                    self.last_error = None;
                    crate::diagnostics::resolved("printer");
                }
                Err(e) => {
                    let msg = format!("Printer: {e}");
                    crate::diagnostics::report("printer", msg.clone());
                    self.last_error = Some(msg);
                }
            }
        }
        self.is_open
    }

    /// `ClosePrint` (:74-90).
    fn close_print(&mut self) {
        if self.is_open {
            if let Some(sink) = &mut self.sink {
                sink.close(self.options.dump_to_printer);
            }
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
        if let Some(v) = &mut self.virtual_out {
            v.touch();
        }
        self.check_print();
        0xFF // status: always ready (:125)
    }

    fn io_write(&mut self, lo: u8, value: u8, ctx: &mut CardCtx) {
        if lo == 0 {
            if let Some(v) = &mut self.virtual_out {
                // The card's firmware: its slot ROM page or the shared $C800-$CFFF space.
                let pc = ctx.opcode.pc;
                let from_firmware =
                    (pc >> 8) as u8 == 0xC0 | ctx.slot || (0xC800..=0xCFFF).contains(&pc);
                v.write(value, from_firmware);
            }
        }
        if !self.check_print() || lo != 0 {
            return;
        }
        let mut c = value & 0x7F;
        if let Some(enc) = self.options.convert_encoding {
            c = enc.convert(value);
        }
        // `c > 0x7F` keeps Cyrillic output when conversion is on (:149).
        if !self.options.filter_unprintable || c > 31 || c == 13 || c == 10 {
            let Some(sink) = &mut self.sink else { return };
            if let Err(e) = sink.write(c) {
                let msg = format!("Printer: write failed: {e}");
                crate::diagnostics::report("printer-write", msg.clone());
                self.last_error = Some(msg);
            }
        }
    }

    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        apple2_roms::PARALLEL[offset as usize]
    }

    fn reset(&mut self, _power_cycle: bool, _ctx: &mut CardCtx) {
        self.close_print();
        self.close_virtual(CloseReason::Reset);
    }

    fn update(&mut self, cycles: u64, _ctx: &mut CardCtx) {
        let idle_cycles = self.options.idle_limit as u64 * CYCLES_PER_IDLE_SECOND;
        if let Some(v) = &mut self.virtual_out {
            v.update(cycles, idle_cycles);
        }
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
        self.close_virtual(CloseReason::Shutdown);
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
        let dir =
            std::env::temp_dir().join(format!("apple2_hostio_printer_{}", std::process::id()));
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

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "apple2_hostio_printer_{name}_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn sanitize_rejects_empty_nul_and_directories() {
        let dir = scratch("sanitize_reject");
        for bad in ["", "   ", "a\0b"] {
            let e = sanitize_printer_path(Path::new(bad), Some(&dir)).unwrap_err();
            assert_eq!(e.kind(), io::ErrorKind::InvalidInput, "{bad:?}");
        }
        let sub = dir.join("sub");
        std::fs::create_dir(&sub).unwrap();
        assert!(
            sanitize_printer_path(&sub, None).is_err(),
            "existing directory"
        );
        assert!(sanitize_printer_path(Path::new("newdir/"), Some(&dir)).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn sanitize_resolves_relative_names_against_the_base_directory() {
        let dir = scratch("sanitize_rel");
        let canon = std::fs::canonicalize(&dir).unwrap();
        let got = sanitize_printer_path(Path::new("./a/../out.txt"), Some(&dir)).unwrap();
        assert_eq!(got, canon.join("out.txt"));
        // Without a base the current directory is used.
        let got = sanitize_printer_path(Path::new("x.txt"), None).unwrap();
        assert!(got.is_absolute());
        assert_eq!(got.file_name().unwrap(), "x.txt");
        // An absolute name is kept (AppleWin writes wherever it is told to).
        let got = sanitize_printer_path(&dir.join("abs.txt"), None).unwrap();
        assert_eq!(got, canon.join("abs.txt"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn file_sink_open_errors_name_the_file_and_the_cause() {
        let dir = scratch("open_err");
        let mut s = FileSink::new("no_such_dir/out.txt").with_base_dir(&dir);
        let e = s.open(false).unwrap_err();
        let msg = e.to_string();
        assert!(msg.contains("cannot open printer file"), "{msg}");
        assert!(msg.contains("out.txt"), "{msg}");
        let mut s = FileSink::new(dir.clone());
        assert!(s.open(false).unwrap_err().to_string().contains("directory"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn file_sink_in_a_directory_without_permission_fails_visibly() {
        use std::os::unix::fs::PermissionsExt;
        let dir = scratch("perm");
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        // Root ignores the mode; nothing to check then.
        if std::fs::File::create(dir.join("probe")).is_ok() {
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::fs::remove_dir_all(dir).unwrap();
            return;
        }
        let mut card = ParallelPrinterCard::new(
            Box::new(FileSink::new(dir.join("Printer.txt"))),
            PrinterOptions::default(),
        );
        assert!(
            !card.check_print(),
            "emulation goes on, printer disconnected"
        );
        let err = card
            .last_error()
            .expect("error kept for the frontend")
            .to_string();
        assert!(
            err.contains("Printer.txt") && err.to_lowercase().contains("permission"),
            "{err}"
        );
        assert!(crate::diagnostics::peek().iter().any(|m| m == &err));
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_failure_on_a_full_device_is_reported() {
        let mut s = FileSink::new("/dev/full");
        s.open(true).unwrap();
        s.write(b'x').unwrap(); // buffered
        s.close(false);
        let e = s.last_error().expect("flush failure kept");
        assert!(e.contains("/dev/full") && e.contains("write failed"), "{e}");
    }

    #[test]
    fn card_reports_open_failures_once_and_recovers() {
        let dir = scratch("recover");
        let mut card = ParallelPrinterCard::new(
            Box::new(FileSink::new("missing/Printer.txt").with_base_dir(&dir)),
            PrinterOptions::default(),
        );
        assert!(!card.check_print());
        assert!(!card.check_print());
        let mine = |v: Vec<String>| v.into_iter().filter(|m| m.contains("recover")).count();
        assert_eq!(
            mine(crate::diagnostics::peek()),
            1,
            "same failure, one message"
        );
        std::fs::create_dir(dir.join("missing")).unwrap();
        assert!(card.check_print());
        assert!(card.last_error().is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_custom_file_name_can_be_read_and_replaced() {
        let mut card = ParallelPrinterCard::new(
            Box::new(FileSink::new("mine.txt")),
            PrinterOptions::default(),
        );
        assert_eq!(card.filename(), Some(Path::new("mine.txt")));
        card.set_filename(PathBuf::from("other.txt"));
        assert_eq!(card.filename(), Some(Path::new("other.txt")));
        let m = ParallelPrinterCard::new(Box::new(MemorySink::new()), PrinterOptions::default());
        assert_eq!(m.filename(), None);
    }
}
