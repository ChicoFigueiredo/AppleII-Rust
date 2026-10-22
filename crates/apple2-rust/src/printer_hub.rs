//! Integration of the virtual printer with the frontend, without any window code: the card's
//! capture queue -> bridge thread -> [`SpoolService`] thread (one `Lx310` per job) -> snapshots ->
//! [`PrinterViewModel`]. The Central de Impressoras ([`crate::printer_window`]) only draws the view
//! model; everything here is testable without a GPU.
//!
//! Threads: the emulation thread only pushes into the capture queue (never waits). The bridge
//! thread forwards events to the spool and counts them for the UI (first byte -> automatic
//! opening, activity light). The spool thread writes the Arquivo de impressões and runs the
//! engine. The UI thread only clones an `Arc<Snapshot>` (a mutex held for one pointer copy) and
//! rebuilds the view model when the snapshot revision moved. Reading archived jobs, exporting
//! PDF and copying raw bytes happen on short-lived worker threads.

use crate::printer_model::{
    CenterEvent, JobEntry, JobState, Message, PageRange, PdfRequest, PrintSetup, PrinterViewModel,
    Shown, Status, VersionEntry,
};
use apple2_config::card::{
    PrinterInk, PrinterPaper, PrinterRibbon, PrinterSpec, PrinterTable, SlotCardSpec,
};
use apple2_config::Settings;
use apple2_hostio::CaptureReceiver;
use apple2_printer::archive::{self, Archive, JobState as ArchiveJobState};
use apple2_printer::capture::CaptureEvent;
use apple2_printer::document::PageDocument;
use apple2_printer::engine::PrinterEngine;
use apple2_printer::font::CharTable;
use apple2_printer::lx310::{Lx310, Lx310Config};
use apple2_printer::paper::{InkColor, Paper, PaperKind, RenderOptions, Ribbon};
use apple2_printer::pdf::{self, PageSelection, PdfDate, PdfMeta};
use apple2_printer::spool::{JobSummary, Snapshot, Spool, SpoolCommand, SpoolHandle, SpoolService};
use std::collections::{HashMap, VecDeque};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Environment variable that moves the Arquivo de impressões (smoke tests, portable installs).
pub const ARCHIVE_ENV: &str = "APPLE2_RUST_PRINTER_ARCHIVE";
/// How long without bytes before an open job shows as "aguardando".
pub const WAITING_AFTER: Duration = Duration::from_millis(1500);
/// How often the UI re-reads the spool while something is happening.
pub const POLL_PERIOD: Duration = Duration::from_millis(33);
/// How long the card has to answer a close request before the spool closes the job itself.
pub const CLOSE_FALLBACK: Duration = Duration::from_millis(1500);
const MAX_MESSAGES: usize = 8;
/// Largest raw job "Salvar RAW" copies (it is held in memory while the file is written).
pub const MAX_RAW_COPY: u64 = 512 * 1024 * 1024;

// ---------------------------------------------------------------------------------------------
// Settings <-> printer crate mapping
// ---------------------------------------------------------------------------------------------

pub fn paper_kind(p: PrinterPaper) -> PaperKind {
    match p {
        PrinterPaper::GreenBar => PaperKind::GreenBar,
        PrinterPaper::BlueBar => PaperKind::BlueBar,
        PrinterPaper::PlainContinuous => PaperKind::PlainContinuous,
        PrinterPaper::A4 => PaperKind::A4,
        PrinterPaper::Letter => PaperKind::Letter,
    }
}

pub fn printer_paper(k: PaperKind) -> PrinterPaper {
    match k {
        PaperKind::GreenBar => PrinterPaper::GreenBar,
        PaperKind::BlueBar => PrinterPaper::BlueBar,
        PaperKind::PlainContinuous => PrinterPaper::PlainContinuous,
        PaperKind::A4 => PrinterPaper::A4,
        PaperKind::Letter => PrinterPaper::Letter,
    }
}

/// Render options frozen at the start of a job, from the card's settings.
pub fn render_options(spec: &PrinterSpec) -> RenderOptions {
    RenderOptions {
        paper: Paper::preset(paper_kind(spec.paper)),
        ribbon: match spec.ribbon {
            PrinterRibbon::New => Ribbon::New,
            PrinterRibbon::Worn => Ribbon::Worn,
        },
        ink: match spec.ink {
            PrinterInk::Black => InkColor::Black,
            PrinterInk::Blue => InkColor::Blue,
            PrinterInk::Purple => InkColor::Purple,
        },
        table: char_table(spec.table),
        country: spec.country.min(12),
    }
}

pub fn char_table(t: PrinterTable) -> CharTable {
    match t {
        PrinterTable::Pc437 => CharTable::Pc437,
        PrinterTable::Pc850 => CharTable::Pc850,
        PrinterTable::Pc860 => CharTable::Pc860,
        PrinterTable::Brascii => CharTable::Brascii,
        PrinterTable::Abicomp => CharTable::Abicomp,
        PrinterTable::Italic => CharTable::Italic,
    }
}

pub fn printer_table(t: CharTable) -> PrinterTable {
    match t {
        CharTable::Pc437 => PrinterTable::Pc437,
        CharTable::Pc850 => PrinterTable::Pc850,
        CharTable::Pc860 => PrinterTable::Pc860,
        CharTable::Brascii => PrinterTable::Brascii,
        CharTable::Abicomp => PrinterTable::Abicomp,
        CharTable::Italic => PrinterTable::Italic,
    }
}

/// Writes the setup back into the card's settings.
pub fn store_setup(spec: &mut PrinterSpec, s: &PrintSetup) {
    spec.paper = printer_paper(s.options.paper.kind);
    spec.ribbon = match s.options.ribbon {
        Ribbon::New => PrinterRibbon::New,
        Ribbon::Worn => PrinterRibbon::Worn,
    };
    spec.ink = match s.options.ink {
        InkColor::Black => PrinterInk::Black,
        InkColor::Blue => PrinterInk::Blue,
        InkColor::Purple => PrinterInk::Purple,
    };
    spec.table = printer_table(s.options.table);
    spec.country = s.options.country.min(12);
}

/// What the hub needs from the settings: the printer card, its look and its behaviour.
#[derive(Clone, Debug, PartialEq)]
pub struct PrinterConfig {
    pub slot: u8,
    /// Apple II model as shown to the user.
    pub machine: String,
    pub setup: PrintSetup,
    pub auto_open: bool,
}

impl PrinterConfig {
    /// The first Generic Printer that sends to the virtual printer, if any.
    pub fn from_settings(settings: &Settings) -> Option<PrinterConfig> {
        settings
            .slots
            .0
            .iter()
            .enumerate()
            .find_map(|(i, c)| match c {
                SlotCardSpec::GenericPrinter(p) if p.destination.to_virtual() => Some((i, p)),
                _ => None,
            })
            .map(|(slot, p)| PrinterConfig {
                slot: slot as u8,
                machine: crate::factory::machine_label(settings.machine.model),
                setup: PrintSetup {
                    options: render_options(p),
                },
                auto_open: p.auto_open_window,
            })
    }
}

/// The folder of the Arquivo de impressões.
pub fn archive_root() -> Option<PathBuf> {
    match std::env::var_os(ARCHIVE_ENV) {
        Some(p) if !p.is_empty() => Some(PathBuf::from(p)),
        _ => archive::default_root(),
    }
}

fn engine_factory() -> apple2_printer::spool::EngineFactory {
    Box::new(|| Box::new(Lx310::new(Lx310Config::default())))
}

// ---------------------------------------------------------------------------------------------
// Bridge: capture queue -> spool, counting what the UI needs to know at once
// ---------------------------------------------------------------------------------------------

/// Bytes the bridge lets sit between the capture queue and the spool thread. Beyond it the bridge
/// waits (never the CPU), the capture queue absorbs up to its own limit and only then drops, counted.
pub const MAX_BRIDGE_BACKLOG: u64 = 32 * 1024 * 1024;

#[derive(Default)]
struct Counters {
    opens: AtomicU64,
    bytes_events: AtomicU64,
    stop: AtomicBool,
}

struct Bridge {
    counters: Arc<Counters>,
    join: Option<JoinHandle<()>>,
}

type Wake = Arc<dyn Fn() + Send + Sync>;

impl Bridge {
    fn spawn(
        rx: CaptureReceiver,
        tx: Sender<CaptureEvent>,
        wake: Wake,
        consumed: Arc<AtomicU64>,
        backlog_limit: u64,
    ) -> Bridge {
        let counters = Arc::new(Counters::default());
        let c = counters.clone();
        let join = std::thread::Builder::new()
            .name("apple2-printer-bridge".into())
            .spawn(move || {
                let mut last_wake = Instant::now() - Duration::from_secs(1);
                let mut sent = 0u64;
                let mut forward = |ev: CaptureEvent, last_wake: &mut Instant| -> bool {
                    if let CaptureEvent::Bytes { data, .. } = &ev {
                        // Back-pressure: the spool is behind (a long reprint on its thread).
                        while sent.saturating_sub(consumed.load(Ordering::Relaxed)) > backlog_limit
                            && !c.stop.load(Ordering::SeqCst)
                        {
                            std::thread::sleep(Duration::from_millis(5));
                        }
                        sent += data.len() as u64;
                    }
                    let urgent = match &ev {
                        CaptureEvent::Open { .. } => {
                            c.opens.fetch_add(1, Ordering::SeqCst);
                            true
                        }
                        CaptureEvent::Bytes { .. } => {
                            c.bytes_events.fetch_add(1, Ordering::SeqCst);
                            false
                        }
                        CaptureEvent::Close { .. } => true,
                    };
                    if tx.send(ev).is_err() {
                        return false;
                    }
                    if urgent || last_wake.elapsed() >= Duration::from_millis(16) {
                        *last_wake = Instant::now();
                        wake();
                    }
                    true
                };
                loop {
                    if c.stop.load(Ordering::SeqCst) {
                        // Everything the card queued before the emulator stopped still goes to
                        // the spool (no loss on exit).
                        for ev in rx.drain() {
                            if !forward(ev, &mut last_wake) {
                                break;
                            }
                        }
                        break;
                    }
                    if let Some(ev) = rx.recv_timeout(Duration::from_millis(20)) {
                        if !forward(ev, &mut last_wake) {
                            break;
                        }
                    }
                }
            })
            .expect("spawn printer bridge thread");
        Bridge {
            counters,
            join: Some(join),
        }
    }

    fn stop(&mut self) {
        self.counters.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Workers
// ---------------------------------------------------------------------------------------------

fn safe_id(id: &str) -> bool {
    !id.is_empty() && !id.contains(['/', '\\']) && id != "." && id != ".."
}

/// Renders the first `total` raw bytes of an archived job with `options` (a worker thread job).
pub fn render_job(
    root: &Path,
    id: &str,
    total: u64,
    options: RenderOptions,
) -> Result<PageDocument, String> {
    if !safe_id(id) {
        return Err(format!("identificador de trabalho inválido: {id}"));
    }
    let path = root.join(id).join(archive::RAW_FILE);
    let file = std::fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut file = file.take(total);
    let mut engine = Lx310::new(Lx310Config::default());
    engine.begin(options);
    let mut buf = vec![0u8; 16 * 1024];
    let mut offset = 0u64;
    loop {
        let n = file
            .read(&mut buf)
            .map_err(|e| format!("{}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        engine.feed(offset, &buf[..n]);
        offset += n as u64;
    }
    engine.finish();
    Ok(engine.document().clone())
}

fn now_pdf_date() -> PdfDate {
    use chrono::{Datelike, Timelike};
    let t = chrono::Utc::now();
    PdfDate {
        year: t.year().clamp(0, 9999) as u16,
        month: t.month() as u8,
        day: t.day() as u8,
        hour: t.hour() as u8,
        minute: t.minute() as u8,
        second: t.second() as u8,
    }
}

/// Seed of the worn ribbon: the start of the raw bytes' SHA-256, so a job always looks the same.
fn ribbon_seed(m: Option<&JobSummary>) -> Option<u64> {
    let sha = m?.sha256.as_deref()?;
    u64::from_str_radix(sha.get(..16)?, 16).ok()
}

/// Selection for a 1-based inclusive range over `count` pages.
pub fn page_selection(range: &PageRange, count: usize) -> PageSelection {
    match range {
        PageRange::All => PageSelection::All,
        PageRange::Pages(a, b) => {
            let a = (*a).max(1).min(count.max(1));
            let b = (*b).max(a).min(count.max(1));
            PageSelection::Range {
                start: a - 1,
                end: b,
            }
        }
    }
}

/// Everything a PDF export worker needs.
pub struct ExportJob {
    pub doc: Arc<PageDocument>,
    pub options: RenderOptions,
    pub meta: PdfMeta,
    pub range: PageRange,
    pub path: PathBuf,
}

/// Writes the PDF (atomically). Returns the number of pages written (cover excluded).
pub fn export_pdf_blocking(job: &ExportJob) -> Result<usize, String> {
    let selection = page_selection(&job.range, job.doc.pages.len());
    pdf::export_to_path(
        &job.path,
        &job.doc,
        &job.options,
        &job.meta,
        &selection,
        true,
    )
    .map(|r| r.pages)
    .map_err(|e| format!("{}: {e}", job.path.display()))
}

enum Done {
    Rendered {
        job: String,
        version: u32,
        result: Result<Arc<PageDocument>, String>,
    },
    Exported {
        job: String,
        version: u32,
        path: PathBuf,
        result: Result<usize, String>,
    },
    Raw {
        job: String,
        path: PathBuf,
        result: Result<u64, String>,
    },
}

// ---------------------------------------------------------------------------------------------
// The hub
// ---------------------------------------------------------------------------------------------

/// What changed since the last [`PrinterHub::poll`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Poll {
    /// The view model changed: redraw the Central.
    pub changed: bool,
    /// A job started and the settings ask for the Central to open (once per job).
    pub auto_open: bool,
    /// Bytes arrived since the last poll: light the toolbar LED.
    pub activity: bool,
}

pub struct PrinterHub {
    root: PathBuf,
    spool: SpoolHandle,
    service: Option<SpoolService>,
    bridge: Bridge,
    /// Asks the card to close a job ("Concluir" / "Cancelar").
    closer: CaptureReceiver,
    /// Job the user asked to close and when: if the card does not answer (emulation paused),
    /// the spool closes it itself after [`CLOSE_FALLBACK`].
    pending_close: Option<(u64, Instant)>,
    config: Option<PrinterConfig>,
    seen_opens: u64,
    seen_bytes: u64,
    seen_dropped: u64,
    last_byte: Option<Instant>,
    prev_snap: Option<Arc<Snapshot>>,
    /// Newest live document of the active job.
    live: Option<(String, Arc<PageDocument>)>,
    /// Pages of archived versions already rendered.
    rendered: HashMap<(String, u32), Arc<PageDocument>>,
    rendering: Option<(String, u32)>,
    last_reprint: Option<(String, u32)>,
    pick: Option<Pick>,
    last_shown: Option<(String, Arc<PageDocument>)>,
    seen_notice: Option<String>,
    messages: VecDeque<Message>,
    done_tx: Sender<Done>,
    done_rx: Receiver<Done>,
    vm: PrinterViewModel,
    closed: bool,
    dirty: bool,
}

/// What the user chose to look at.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Pick {
    job: String,
    /// `None` = the newest version.
    version: Option<u32>,
}

impl PrinterHub {
    /// Starts the bridge and spool threads on the archive in `root`. `wake` is called (from the
    /// bridge thread) when something happened that the UI should look at.
    pub fn start(
        root: PathBuf,
        events: CaptureReceiver,
        config: Option<PrinterConfig>,
        wake: Wake,
    ) -> Result<PrinterHub, String> {
        let archive = Archive::open(&root).map_err(|e| format!("{}: {e}", root.display()))?;
        let initial = config.as_ref().map(|c| c.setup).unwrap_or_default();
        let spool = Spool::new(archive, engine_factory(), initial.options);
        let (tx, rx) = channel::<CaptureEvent>();
        let service = SpoolService::spawn(spool, rx);
        let closer = events.clone();
        let bridge = Bridge::spawn(
            events,
            tx,
            wake,
            service.consumed_bytes(),
            MAX_BRIDGE_BACKLOG,
        );
        let (done_tx, done_rx) = channel();
        let mut hub = PrinterHub {
            root,
            spool: service.handle(),
            service: Some(service),
            bridge,
            closer,
            pending_close: None,
            config: None,
            seen_opens: 0,
            seen_bytes: 0,
            seen_dropped: 0,
            last_byte: None,
            prev_snap: None,
            live: None,
            rendered: HashMap::new(),
            rendering: None,
            last_reprint: None,
            pick: None,
            last_shown: None,
            seen_notice: None,
            messages: VecDeque::new(),
            done_tx,
            done_rx,
            vm: PrinterViewModel::default(),
            closed: false,
            dirty: true,
        };
        hub.configure(config);
        Ok(hub)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Settings changed (or the first configuration). `None`: no virtual printer card.
    pub fn configure(&mut self, config: Option<PrinterConfig>) {
        if let Some(c) = &config {
            self.spool.send(SpoolCommand::SetOptions(c.setup.options));
        }
        if self.config != config {
            self.config = config;
            self.dirty = true;
        }
    }

    /// True when a Generic Printer feeds the virtual printer (the toolbar shows the button).
    pub fn available(&self) -> bool {
        self.config.is_some()
    }

    pub fn view_model(&self) -> &PrinterViewModel {
        &self.vm
    }

    pub fn message(&mut self, text: impl Into<String>, error: bool) {
        if self.messages.len() >= MAX_MESSAGES {
            self.messages.pop_front();
        }
        self.messages.push_back(Message {
            text: text.into(),
            error,
        });
        self.dirty = true;
    }

    /// When the UI should poll again (`None`: nothing is happening, wait for a wake-up).
    pub fn next_wake(&self, now: Instant) -> Option<Instant> {
        let busy = self.prev_snap.as_ref().is_some_and(|s| s.active.is_some())
            || self.rendering.is_some()
            || self
                .last_byte
                .is_some_and(|t| now.saturating_duration_since(t) < Duration::from_millis(600));
        busy.then_some(now + POLL_PERIOD)
    }

    // ---- polling ------------------------------------------------------------------------

    /// Reads what the other threads produced and updates the view model. Cheap when nothing moved.
    pub fn poll(&mut self, now: Instant) -> Poll {
        let mut out = Poll::default();
        let opens = self.bridge.counters.opens.load(Ordering::SeqCst);
        let bytes = self.bridge.counters.bytes_events.load(Ordering::SeqCst);
        if opens != self.seen_opens {
            out.auto_open = self.config.as_ref().is_some_and(|c| c.auto_open);
            self.seen_opens = opens;
            self.last_byte = Some(now);
        }
        if bytes != self.seen_bytes {
            out.activity = true;
            self.seen_bytes = bytes;
            self.last_byte = Some(now);
        }
        let dropped = self.closer.dropped_bytes();
        if dropped != self.seen_dropped {
            self.seen_dropped = dropped;
            self.message(
                format!(
                    "A fila de impressão encheu: {dropped} byte(s) do Apple II não foram guardados (o Trabalho em andamento fica incompleto)"
                ),
                true,
            );
        }
        let snap = self.spool.snapshot();
        if let Some((job, since)) = self.pending_close {
            match snap.active.as_ref() {
                Some(a) if a.capture_job == job => {
                    if now.saturating_duration_since(since) >= CLOSE_FALLBACK {
                        // The card did not answer (emulation paused): close at the spool.
                        self.spool.send(SpoolCommand::FinishActive);
                        self.pending_close = None;
                    }
                }
                _ => self.pending_close = None,
            }
        }
        let snap_moved = self
            .prev_snap
            .as_ref()
            .is_none_or(|p| p.revision != snap.revision);
        let mut done = false;
        while let Ok(d) = self.done_rx.try_recv() {
            self.apply_done(d);
            done = true;
        }
        let receiving = snap.active.is_some()
            && self
                .last_byte
                .is_some_and(|t| now.saturating_duration_since(t) < WAITING_AFTER);
        let status_moved = self.vm.status != self.status_for(&snap, receiving);
        if snap_moved || done || self.dirty || status_moved {
            self.absorb(&snap);
            let vm = self.build(&snap, receiving);
            if !self.vm.same_content(&vm) {
                let rev = self.vm.revision + 1;
                self.vm = vm;
                self.vm.revision = rev;
                out.changed = true;
            }
            self.prev_snap = Some(snap);
            self.dirty = false;
        }
        out
    }

    fn apply_done(&mut self, d: Done) {
        match d {
            Done::Rendered {
                job,
                version,
                result,
            } => {
                if self.rendering.as_ref() == Some(&(job.clone(), version)) {
                    self.rendering = None;
                }
                match result {
                    Ok(doc) => {
                        self.rendered.insert((job, version), doc);
                    }
                    Err(e) => {
                        self.message(format!("Não foi possível ler o trabalho {job}: {e}"), true)
                    }
                }
            }
            Done::Exported {
                job,
                version,
                path,
                result,
            } => match result {
                Ok(pages) => {
                    self.message(
                        format!("PDF salvo em {} ({pages} página(s))", path.display()),
                        false,
                    );
                    // Registered in the manifest right away (not only at shutdown).
                    self.spool.send(SpoolCommand::RecordExport {
                        job_id: job,
                        version: version.max(1),
                        path,
                    });
                }
                Err(e) => self.message(format!("Falha ao salvar o PDF: {e}"), true),
            },
            Done::Raw { job, path, result } => match result {
                Ok(n) => {
                    self.message(
                        format!("Bytes brutos salvos em {} ({n} bytes)", path.display()),
                        false,
                    );
                    // Version 0 stands for the raw bytes (PDFs are versions 1 and up).
                    self.spool.send(SpoolCommand::RecordExport {
                        job_id: job,
                        version: 0,
                        path,
                    });
                }
                Err(e) => self.message(format!("Falha ao salvar os bytes brutos: {e}"), true),
            },
        }
        self.dirty = true;
    }

    /// Takes in what a new snapshot carries: live document, reprint result, notices.
    fn absorb(&mut self, snap: &Arc<Snapshot>) {
        if let Some(a) = &snap.active {
            self.live = Some((a.job_id.clone(), a.document.clone()));
            if let Some(e) = &a.storage_error {
                let text = format!("Armazenamento: {e}");
                if !self.messages.iter().any(|m| m.text == text) {
                    self.message(text, true);
                }
            }
        }
        if let Some(r) = &snap.reprint {
            let key = (r.job_id.clone(), r.version);
            if self.last_reprint.as_ref() != Some(&key) {
                self.last_reprint = Some(key.clone());
                self.rendered.insert(key.clone(), r.document.clone());
                self.pick = Some(Pick {
                    job: key.0,
                    version: Some(key.1),
                });
                self.message(
                    format!(
                        "Reimpressão pronta: versão {} do trabalho {}",
                        r.version, r.job_id
                    ),
                    false,
                );
            }
        }
        // New spool notices, oldest first.
        let start = match &self.seen_notice {
            Some(last) => snap
                .notices
                .iter()
                .rposition(|n| n == last)
                .map_or(0, |i| i + 1),
            None => snap.notices.len().saturating_sub(MAX_MESSAGES),
        };
        let fresh: Vec<String> = snap.notices[start.min(snap.notices.len())..].to_vec();
        if let Some(l) = snap.notices.last() {
            self.seen_notice = Some(l.clone());
        }
        for n in fresh {
            self.message(n, false);
        }
    }

    fn status_for(&self, snap: &Snapshot, receiving: bool) -> Status {
        if self.config.is_none() {
            return Status::Offline;
        }
        if snap
            .active
            .as_ref()
            .is_some_and(|a| a.storage_error.is_some())
        {
            return Status::Error;
        }
        if snap.active.is_some() {
            return if receiving {
                Status::Receiving
            } else {
                Status::Waiting
            };
        }
        match newest(&snap.jobs) {
            Some(j) if j.damaged.is_some() => Status::Error,
            Some(j) if j.state == Some(ArchiveJobState::Interrupted) => Status::Error,
            Some(j) if j.state == Some(ArchiveJobState::Completed) && self.job_seen() => {
                Status::Finished
            }
            _ => Status::Online,
        }
    }

    /// "Concluído" is only shown for a job printed in this session (a fresh start is "online").
    fn job_seen(&self) -> bool {
        self.seen_opens > 0
    }

    fn build(&mut self, snap: &Arc<Snapshot>, receiving: bool) -> PrinterViewModel {
        let status = self.status_for(snap, receiving);
        let mut sorted: Vec<&JobSummary> = snap.jobs.iter().collect();
        sorted.sort_by(|a, b| {
            b.started_at
                .cmp(&a.started_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        let jobs: Vec<JobEntry> = sorted.into_iter().map(|j| self.entry(j)).collect();
        let active_job = snap
            .active
            .as_ref()
            .and_then(|a| jobs.iter().position(|j| j.id == a.job_id));
        let selected_job = self
            .pick
            .as_ref()
            .and_then(|p| jobs.iter().position(|j| j.id == p.job))
            .or(active_job)
            .or(if jobs.is_empty() { None } else { Some(0) });
        let setup = self.config.as_ref().map(|c| c.setup).unwrap_or_default();
        let shown = self.shown_for(snap, &jobs, selected_job, active_job);
        let detail = match status {
            Status::Receiving | Status::Waiting | Status::Finished => active_job
                .or(selected_job)
                .and_then(|i| jobs.get(i))
                .map(|j| j.title.clone())
                .unwrap_or_default(),
            Status::Error => snap
                .active
                .as_ref()
                .and_then(|a| a.storage_error.clone())
                .or_else(|| newest(&snap.jobs).and_then(|j| j.damaged.clone()))
                .unwrap_or_else(|| "trabalho interrompido".into()),
            _ => String::new(),
        };
        PrinterViewModel {
            status,
            detail,
            machine: self
                .config
                .as_ref()
                .map(|c| c.machine.clone())
                .unwrap_or_default(),
            slot: self.config.as_ref().map(|c| c.slot),
            profile: "Epson LX-310".into(),
            setup,
            jobs,
            active_job,
            selected_job,
            shown,
            messages: self.messages.iter().cloned().collect(),
            revision: 0,
        }
    }

    fn entry(&self, j: &JobSummary) -> JobEntry {
        let state = match (j.state, &j.damaged) {
            (_, Some(_)) => JobState::Failed,
            (Some(ArchiveJobState::Active), _) => JobState::Receiving,
            (Some(ArchiveJobState::Completed), _) if j.cancelled => JobState::Cancelled,
            (Some(ArchiveJobState::Completed), _) => JobState::Finished,
            _ => JobState::Failed,
        };
        let title = if j.title.is_empty() {
            j.damaged
                .as_ref()
                .map_or_else(|| j.id.clone(), |_| format!("{} (danificado)", j.id))
        } else {
            j.title.clone()
        };
        JobEntry {
            id: j.id.clone(),
            title,
            note: j.note.clone(),
            machine: j
                .origin
                .as_ref()
                .map(|o| o.machine.clone())
                .unwrap_or_default(),
            slot: j.origin.as_ref().map_or(0, |o| o.slot),
            transport: j
                .transport
                .map(|t| transport_label(t).to_string())
                .unwrap_or_default(),
            started: local_time(&j.started_at),
            pages: j.pages,
            state,
            active: j.state == Some(ArchiveJobState::Active),
            damaged: j.damaged.clone(),
            versions: j
                .versions
                .iter()
                .map(|v| VersionEntry {
                    id: v.id,
                    options: v.options,
                    pages: v.pages,
                    created: local_time(&v.created_at),
                })
                .collect(),
            exports: j
                .exports
                .iter()
                .map(|e| (e.version, e.path.clone()))
                .collect(),
        }
    }

    fn shown_for(
        &mut self,
        snap: &Arc<Snapshot>,
        jobs: &[JobEntry],
        selected: Option<usize>,
        active: Option<usize>,
    ) -> Shown {
        let Some(sel) = selected.and_then(|i| jobs.get(i)) else {
            return Shown::default();
        };
        let manifest = snap.jobs.iter().find(|j| j.id == sel.id).cloned();
        let default_opts = self
            .config
            .as_ref()
            .map(|c| c.setup.options)
            .unwrap_or_default();
        let job_opts = manifest.as_ref().map_or(default_opts, |m| m.options);
        let is_active = active.is_some_and(|i| jobs[i].id == sel.id);
        if is_active {
            let doc = snap
                .active
                .as_ref()
                .map(|a| a.document.clone())
                .unwrap_or_default();
            self.last_shown = Some((sel.id.clone(), doc.clone()));
            return Shown {
                job: Some(sel.id.clone()),
                version: 0,
                label: sel.title.clone(),
                document: doc,
                options: job_opts,
                loading: false,
                live: true,
            };
        }
        let version = self
            .pick
            .as_ref()
            .filter(|p| p.job == sel.id)
            .and_then(|p| p.version)
            .or_else(|| sel.versions.last().map(|v| v.id))
            .unwrap_or(1);
        let options = sel
            .versions
            .iter()
            .find(|v| v.id == version)
            .map_or(job_opts, |v| v.options);
        let key = (sel.id.clone(), version);
        let (document, loading) = match self.rendered.get(&key) {
            Some(d) => (d.clone(), false),
            None => {
                // Keep showing what was on screen for this job while the archive is read.
                let stale = self
                    .last_shown
                    .as_ref()
                    .filter(|(id, _)| *id == sel.id)
                    .map(|(_, d)| d.clone());
                if sel.damaged.is_none() && sel.state != JobState::Receiving {
                    self.start_render(&key, manifest.as_ref(), options);
                }
                (stale.unwrap_or_default(), sel.damaged.is_none())
            }
        };
        if !loading {
            self.last_shown = Some((sel.id.clone(), document.clone()));
        }
        Shown {
            job: Some(sel.id.clone()),
            version,
            label: format!("{} (versão {version})", sel.title),
            document,
            options,
            loading,
            live: false,
        }
    }

    fn start_render(
        &mut self,
        key: &(String, u32),
        manifest: Option<&JobSummary>,
        options: RenderOptions,
    ) {
        if self.rendering.as_ref() == Some(key) {
            return;
        }
        let Some(m) = manifest else { return };
        if m.state != Some(ArchiveJobState::Completed) {
            return;
        }
        self.rendering = Some(key.clone());
        let (root, id, total) = (self.root.clone(), key.0.clone(), m.total_bytes);
        let version = key.1;
        let tx = self.done_tx.clone();
        std::thread::Builder::new()
            .name("apple2-printer-render".into())
            .spawn(move || {
                let result = render_job(&root, &id, total, options).map(Arc::new);
                let _ = tx.send(Done::Rendered {
                    job: id,
                    version,
                    result,
                });
            })
            .ok();
    }

    // ---- user requests --------------------------------------------------------------------

    /// Handles a Central event that needs no file dialog.
    pub fn handle(&mut self, ev: CenterEvent) {
        match ev {
            CenterEvent::SelectJob(job) => {
                self.pick = Some(Pick { job, version: None });
            }
            CenterEvent::SelectVersion { job, version } => {
                self.pick = Some(Pick {
                    job,
                    version: Some(version),
                });
            }
            CenterEvent::SetNextSetup(s) => {
                if let Some(c) = &mut self.config {
                    c.setup = s;
                }
                self.spool.send(SpoolCommand::SetOptions(s.options));
            }
            CenterEvent::FinishJob => {
                self.close_on_card();
            }
            CenterEvent::CancelJob => {
                // Cancel = close on the card like "Concluir" and mark the job as cancelled in the
                // Arquivo. The bytes stay; nothing is deleted (use "Apagar trabalho" for that).
                if let Some(id) = self.close_on_card() {
                    self.spool.send(SpoolCommand::Cancel { job_id: id });
                    self.message(
                        "Trabalho cancelado: fechado na impressora e marcado como cancelado; \
                         os bytes continuam no Arquivo de impressões.",
                        false,
                    );
                }
            }
            CenterEvent::Reprint { job, options } => {
                self.spool.send(SpoolCommand::Reprint {
                    job_id: job,
                    options,
                });
            }
            CenterEvent::SetTitle { job, title } => {
                self.spool
                    .send(SpoolCommand::SetTitle { job_id: job, title });
            }
            CenterEvent::SetNote { job, note } => {
                self.spool.send(SpoolCommand::SetNote { job_id: job, note });
            }
            CenterEvent::DeleteJob(job) => {
                self.rendered.retain(|(j, _), _| *j != job);
                if self.pick.as_ref().is_some_and(|p| p.job == job) {
                    self.pick = None;
                }
                self.last_shown = None;
                self.spool.send(SpoolCommand::Delete { job_id: job });
            }
            CenterEvent::ExportPdf(_) | CenterEvent::SaveRaw { .. } => {}
        }
        self.dirty = true;
    }

    /// Asks the card to close the active job as `User` (the next byte opens a new job). Returns the
    /// Arquivo id of the job it applies to.
    fn close_on_card(&mut self) -> Option<String> {
        let (id, capture_job) = self
            .prev_snap
            .as_ref()
            .and_then(|s| s.active.as_ref())
            .map(|a| (a.job_id.clone(), a.capture_job))?;
        self.closer.request_close(capture_job);
        self.pending_close = Some((capture_job, Instant::now()));
        Some(id)
    }

    /// The summary of a job in the latest snapshot the hub absorbed.
    fn job_summary(&self, id: &str) -> Option<&JobSummary> {
        self.prev_snap.as_ref()?.jobs.iter().find(|j| j.id == id)
    }

    /// Saves a PDF of `req` to `path` on a worker thread. Title and note go to the manifest too.
    pub fn export_pdf(&mut self, req: PdfRequest, path: PathBuf) {
        let manifest = self.job_summary(&req.job).cloned();
        if let Some(why) = manifest.as_ref().and_then(|m| m.damaged.as_ref()) {
            self.message(
                format!("Falha ao salvar o PDF: o trabalho está danificado ({why})"),
                true,
            );
            return;
        }
        let current_title = self
            .vm
            .jobs
            .iter()
            .find(|j| j.id == req.job)
            .map(|j| j.title.clone());
        if !req.title.is_empty() && current_title.as_deref() != Some(req.title.as_str()) {
            self.handle(CenterEvent::SetTitle {
                job: req.job.clone(),
                title: req.title.clone(),
            });
        }
        if manifest.as_ref().is_none_or(|m| m.note != req.note) {
            self.handle(CenterEvent::SetNote {
                job: req.job.clone(),
                note: req.note.clone(),
            });
        }
        let live = self
            .live
            .as_ref()
            .filter(|(id, _)| *id == req.job && req.version == 0)
            .map(|(_, d)| d.clone());
        let default_opts = self
            .config
            .as_ref()
            .map(|c| c.setup.options)
            .unwrap_or_default();
        let version = if req.version == 0 && live.is_none() {
            manifest
                .as_ref()
                .and_then(|m| m.versions.last().map(|v| v.id))
                .unwrap_or(1)
        } else {
            req.version
        };
        let options = manifest
            .as_ref()
            .and_then(|m| {
                m.versions
                    .iter()
                    .find(|v| v.id == version)
                    .map(|v| v.options)
            })
            .or(manifest.as_ref().map(|m| m.options))
            .unwrap_or(default_opts);
        let meta = PdfMeta {
            title: if req.title.is_empty() {
                PdfMeta::default().title
            } else {
                req.title.clone()
            },
            author: String::new(),
            date: Some(now_pdf_date()),
            apple2_model: manifest
                .as_ref()
                .and_then(|m| m.origin.as_ref().map(|o| o.machine.clone()))
                .unwrap_or_default(),
            printer_profile: "Epson LX-310".into(),
            note: req.note.clone(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            cover: req.cover,
            ribbon_seed: ribbon_seed(manifest.as_ref()),
        };
        let cached = self.rendered.get(&(req.job.clone(), version)).cloned();
        let (root, id, total) = (
            self.root.clone(),
            req.job.clone(),
            manifest.as_ref().map_or(0, |m| m.total_bytes),
        );
        let tx = self.done_tx.clone();
        let range = req.range.clone();
        let report_version = req.version;
        std::thread::Builder::new()
            .name("apple2-printer-pdf".into())
            .spawn(move || {
                let doc = match live.or(cached) {
                    Some(d) => Ok(d),
                    None => render_job(&root, &id, total, options).map(Arc::new),
                };
                let result = doc.and_then(|doc| {
                    export_pdf_blocking(&ExportJob {
                        doc,
                        options,
                        meta,
                        range,
                        path: path.clone(),
                    })
                });
                let _ = tx.send(Done::Exported {
                    job: id,
                    version: report_version,
                    path,
                    result,
                });
            })
            .ok();
    }

    /// Copies the job's raw printer bytes to `path` (worker thread).
    pub fn save_raw(&mut self, job: &str, path: PathBuf) {
        if !safe_id(job) {
            self.message(
                format!(
                    "Falha ao salvar os bytes brutos: identificador de trabalho inválido: {job}"
                ),
                true,
            );
            return;
        }
        if let Some(why) = self.job_summary(job).and_then(|m| m.damaged.clone()) {
            self.message(
                format!("Falha ao salvar os bytes brutos: o trabalho está danificado ({why})"),
                true,
            );
            return;
        }
        let total = self
            .prev_snap
            .as_ref()
            .and_then(|s| {
                s.active
                    .as_ref()
                    .filter(|a| a.job_id == job)
                    .map(|a| a.bytes)
            })
            .or_else(|| self.job_summary(job).map(|j| j.total_bytes))
            .unwrap_or(u64::MAX);
        let job_id = job.to_string();
        let src = self.root.join(job).join(archive::RAW_FILE);
        let tx = self.done_tx.clone();
        std::thread::Builder::new()
            .name("apple2-printer-raw".into())
            .spawn(move || {
                let result = (|| -> Result<u64, String> {
                    let f =
                        std::fs::File::open(&src).map_err(|e| format!("{}: {e}", src.display()))?;
                    // The copy is built in memory: refuse a raw file no real job reaches.
                    let size = f.metadata().map_or(0, |m| m.len().min(total));
                    if size > MAX_RAW_COPY {
                        return Err(format!(
                            "{}: {size} bytes passam do limite de {} MiB para cópia",
                            src.display(),
                            MAX_RAW_COPY / (1024 * 1024)
                        ));
                    }
                    let mut buf = Vec::new();
                    f.take(total)
                        .read_to_end(&mut buf)
                        .map_err(|e| format!("{}: {e}", src.display()))?;
                    pdf::write_atomic(&path, &buf, true)
                        .map_err(|e| format!("{}: {e}", path.display()))?;
                    Ok(buf.len() as u64)
                })();
                let _ = tx.send(Done::Raw {
                    job: job_id,
                    path,
                    result,
                });
            })
            .ok();
    }

    // ---- shutdown ---------------------------------------------------------------------------

    /// Ends the pipeline in order: everything the card queued reaches the spool, the open job is
    /// closed as `Shutdown` and the spool thread stops (exports were recorded in the manifest when made).
    /// Call after the emulation thread stopped.
    pub fn shutdown(mut self) {
        self.finish();
    }

    fn finish(&mut self) {
        if std::mem::replace(&mut self.closed, true) {
            return;
        }
        self.bridge.stop();
        if let Some(service) = self.service.take() {
            let _ = service.join();
        }
    }
}

impl Drop for PrinterHub {
    fn drop(&mut self) {
        self.finish();
    }
}

fn newest(jobs: &[JobSummary]) -> Option<&JobSummary> {
    jobs.iter().max_by(|a, b| {
        a.started_at
            .cmp(&b.started_at)
            .then_with(|| a.id.cmp(&b.id))
    })
}

pub fn transport_label(t: apple2_printer::capture::Transport) -> &'static str {
    use apple2_printer::capture::Transport;
    match t {
        Transport::Auto => "Automático",
        Transport::EightBit => "8 bits",
        Transport::SevenBitAppleWin => "7 bits (AppleWin)",
    }
}

/// "dd/mm/aaaa hh:mm" in the local time zone.
pub fn local_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|d| {
            d.with_timezone(&chrono::Local)
                .format("%d/%m/%Y %H:%M")
                .to_string()
        })
        .unwrap_or_else(|_| rfc3339.to_string())
}

#[cfg(test)]
mod bridge_tests {
    use super::*;
    use apple2_hostio::capture_channel;
    use apple2_printer::capture::{CloseReason, JobOrigin, Transport};

    /// A spool that stops consuming makes the bridge stop forwarding (bounded memory); the bytes
    /// wait in the capture queue and flow again once the spool catches up. Nothing is lost.
    #[test]
    fn a_stalled_spool_holds_the_bridge_and_nothing_is_lost() {
        let (txq, rx) = capture_channel();
        let (tx, spool_rx) = channel::<CaptureEvent>();
        let consumed = Arc::new(AtomicU64::new(0));
        let mut bridge = Bridge::spawn(rx.clone(), tx, Arc::new(|| {}), consumed.clone(), 1000);
        let origin = JobOrigin {
            machine: "t".into(),
            slot: 1,
        };
        let job = txq.open(origin, Transport::EightBit);
        // Distinct events (each flush is separate: other jobs' bytes cannot merge).
        let mut off = 0;
        for _ in 0..50 {
            txq.bytes(job, off, vec![b'a'; 400]);
            off += 400;
            std::thread::sleep(Duration::from_millis(2));
        }
        std::thread::sleep(Duration::from_millis(100));
        let mut got = 0u64;
        while let Ok(ev) = spool_rx.try_recv() {
            if let CaptureEvent::Bytes { data, .. } = ev {
                got += data.len() as u64;
            }
        }
        assert!(
            got <= 2_000,
            "bridge forwarded {got} bytes to a stalled spool"
        );
        assert!(
            rx.pending_bytes() > 0,
            "the rest waits in the capture queue"
        );
        // The spool catches up: the rest flows, in order, complete.
        let t = Instant::now();
        while got < 20_000 && t.elapsed() < Duration::from_secs(10) {
            consumed.store(got, Ordering::Relaxed);
            while let Ok(ev) = spool_rx.try_recv() {
                if let CaptureEvent::Bytes { data, .. } = ev {
                    got += data.len() as u64;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(got, 20_000);
        assert_eq!(rx.dropped_bytes(), 0);
        txq.close(job, CloseReason::Idle);
        bridge.stop();
    }
}
