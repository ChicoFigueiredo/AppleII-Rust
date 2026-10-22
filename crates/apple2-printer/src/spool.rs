//! Spool service: consumes the card's `CaptureEvent`s off the emulation thread, writes them to the
//! Arquivo de impressões, feeds one `PrinterEngine` per job incrementally and publishes cheap
//! snapshots for the UI. With the window closed only this service runs: nothing is lost (T-018).

use crate::archive::{
    Archive, ArchiveError, DiagnosticSummary, ExportRecord, JobEntry, JobState, JobVersion, NewJob,
    RecoveryEvent, RetentionWarning,
};
use crate::capture::{CaptureEvent, CloseReason, JobOrigin, Transport};
use crate::document::PageDocument;
use crate::engine::PrinterEngine;
use crate::paper::RenderOptions;
use std::collections::VecDeque;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Creates a fresh engine for each job (and for each reprint).
pub type EngineFactory = Box<dyn Fn() -> Box<dyn PrinterEngine> + Send>;

const MAX_NOTICES: usize = 30;
/// How long the followed job must be silent before another job's bytes take the preview.
const FOCUS_QUIET: Duration = Duration::from_millis(500);
const REPRINT_CHUNK: usize = 8 * 1024;
const TITLE_CHARS: usize = 60;

/// One row of the job list shown by the Central de Impressoras.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobSummary {
    pub id: String,
    pub title: String,
    /// `None` when the job is damaged.
    pub state: Option<JobState>,
    pub damaged: Option<String>,
    pub started_at: String,
    pub total_bytes: u64,
    pub pages: u32,
    pub close_reason: Option<CloseReason>,
    /// Where the job came from (`None` when damaged).
    pub origin: Option<JobOrigin>,
    pub transport: Option<Transport>,
    /// Options frozen when the job started (version 1).
    pub options: RenderOptions,
    pub note: String,
    /// SHA-256 of the raw bytes, once the job is closed.
    pub sha256: Option<String>,
    /// The user cancelled the job (bytes kept).
    pub cancelled: bool,
    pub versions: Vec<JobVersion>,
    /// PDFs and raw copies the user exported (most recent last).
    pub exports: Vec<ExportRecord>,
}

/// The job being printed now.
#[derive(Clone, Debug)]
pub struct ActiveView {
    pub job_id: String,
    pub capture_job: u64,
    /// Options frozen when the job started.
    pub options: RenderOptions,
    /// Human title so far (first text line + date), before the job closes.
    pub title: String,
    /// Bytes received from the card so far.
    pub bytes: u64,
    /// Shared, immutable copy: cloning the view is cheap.
    pub document: Arc<PageDocument>,
    /// Changes only when the document changed, so the preview redraws only then.
    pub doc_revision: u64,
    /// Set when bytes could no longer be saved (disk full, read-only folder).
    pub storage_error: Option<String>,
}

/// Result of a reprint, shown by the Central.
#[derive(Clone, Debug)]
pub struct ReprintView {
    pub job_id: String,
    pub version: u32,
    pub document: Arc<PageDocument>,
}

#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    /// Increases on every publication.
    pub revision: u64,
    pub jobs: Vec<JobSummary>,
    pub active: Option<ActiveView>,
    pub reprint: Option<ReprintView>,
    /// Latest messages (errors, recoveries, ignored events), oldest first. Never silent.
    pub notices: Vec<String>,
    pub retention: Option<RetentionWarning>,
}

/// How long after the last `get` the document keeps being copied for the preview.
const WATCH_WINDOW_MS: u64 = 2_000;

/// Read side of the published snapshots; holds the lock only to clone an `Arc`. Every read stamps
/// the time, so the spool copies the (large) live document only while somebody is looking: with
/// the window closed or the UI stuck, the job costs no copies at all.
#[derive(Clone)]
pub struct SnapshotHandle {
    cur: Arc<Mutex<Arc<Snapshot>>>,
    base: Instant,
    /// Milliseconds since `base` of the last read, plus one (0 = never read).
    last_read: Arc<AtomicU64>,
}

impl SnapshotHandle {
    fn new() -> SnapshotHandle {
        SnapshotHandle {
            cur: Arc::new(Mutex::new(Arc::new(Snapshot::default()))),
            base: Instant::now(),
            last_read: Arc::new(AtomicU64::new(0)),
        }
    }
    pub fn get(&self) -> Arc<Snapshot> {
        self.last_read.store(
            self.base.elapsed().as_millis() as u64 + 1,
            Ordering::Relaxed,
        );
        match self.cur.lock() {
            Ok(g) => g.clone(),
            Err(p) => p.into_inner().clone(),
        }
    }
    fn set(&self, s: Arc<Snapshot>) {
        match self.cur.lock() {
            Ok(mut g) => *g = s,
            Err(p) => *p.into_inner() = s,
        }
    }
    /// True when somebody read a snapshot recently (or never yet: the first reader must see the
    /// document).
    fn watched(&self, window_ms: u64) -> bool {
        match self.last_read.load(Ordering::Relaxed) {
            0 => true,
            t => (self.base.elapsed().as_millis() as u64 + 1).saturating_sub(t) <= window_ms,
        }
    }
}

struct ActiveJob {
    id: String,
    capture_job: u64,
    options: RenderOptions,
    started_at: String,
    /// Live title, recomputed on each publication while the job is open (the first line grows).
    title: String,
    engine: Box<dyn PrinterEngine>,
    /// Bytes received (and fed to the engine).
    fed: u64,
    storage_error: Option<String>,
    last_pages: u32,
    last_diags: usize,
    /// Card slot the job came from (one open job per slot).
    slot: u8,
    last_bytes_at: Instant,
}

pub struct Spool {
    archive: Archive,
    factory: EngineFactory,
    options: RenderOptions,
    /// The job the preview follows ("foco de prévia").
    active: Option<ActiveJob>,
    /// Other open jobs, one per card slot (several Generic Printers can print at once).
    background: Vec<ActiveJob>,
    snapshot: SnapshotHandle,
    snap_rev: u64,
    notices: VecDeque<String>,
    doc: Arc<PageDocument>,
    doc_rev: u64,
    doc_dirty: bool,
    reprint: Option<ReprintView>,
    last_doc_publish: Instant,
    doc_interval: Duration,
    /// Cost of the last document copy: the interval grows to keep copying under ~20% of the time.
    last_copy: Duration,
    watch_window: Duration,
    jobs_cache: Vec<JobSummary>,
}

fn summary(e: &JobEntry) -> JobSummary {
    match e {
        JobEntry::Valid(m) => JobSummary {
            id: m.id.clone(),
            title: m.title.clone(),
            state: Some(m.state),
            damaged: None,
            started_at: m.started_at.clone(),
            total_bytes: m.total_bytes,
            pages: m.pages,
            close_reason: m.close_reason,
            origin: Some(m.origin.clone()),
            transport: Some(m.transport),
            options: m.options,
            note: m.note.clone(),
            sha256: m.sha256.clone(),
            cancelled: m.cancelled,
            versions: m.versions.clone(),
            exports: m.exports.clone(),
        },
        JobEntry::Damaged { id, reason } => JobSummary {
            id: id.clone(),
            title: String::new(),
            state: None,
            damaged: Some(reason.clone()),
            started_at: String::new(),
            total_bytes: 0,
            pages: 0,
            close_reason: None,
            origin: None,
            transport: None,
            options: RenderOptions::default(),
            note: String::new(),
            sha256: None,
            cancelled: false,
            versions: Vec::new(),
            exports: Vec::new(),
        },
    }
}

fn diag_summaries(doc: &PageDocument) -> Vec<DiagnosticSummary> {
    doc.diagnostics
        .iter()
        .take(100)
        .map(|d| DiagnosticSummary {
            offset: d.offset,
            message: d.message.chars().take(200).collect(),
        })
        .collect()
}

/// First non-empty text line of the document, cleaned for a title.
fn first_text_line(doc: &PageDocument) -> Option<String> {
    for page in &doc.pages {
        for run in &page.text {
            let cleaned: String = run.text.chars().filter(|c| !c.is_control()).collect();
            let line: String = cleaned.trim().chars().take(TITLE_CHARS).collect();
            if !line.is_empty() {
                return Some(line);
            }
        }
    }
    None
}

fn auto_title(doc: &PageDocument, started_at: &str) -> String {
    let date = chrono::DateTime::parse_from_rfc3339(started_at)
        .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|_| started_at.to_string());
    match first_text_line(doc) {
        Some(l) => format!("{l} — {date}"),
        None => format!("Impressão sem texto — {date}"),
    }
}

fn storage_diag(offset: u64, why: &str) -> DiagnosticSummary {
    DiagnosticSummary {
        offset,
        message: format!("armazenamento: bytes seguintes não foram gravados ({why})"),
    }
}

impl Spool {
    pub fn new(archive: Archive, factory: EngineFactory, options: RenderOptions) -> Spool {
        let mut s = Spool {
            archive,
            factory,
            options,
            active: None,
            background: Vec::new(),
            snapshot: SnapshotHandle::new(),
            snap_rev: 0,
            notices: VecDeque::new(),
            doc: Arc::new(PageDocument::default()),
            doc_rev: 0,
            doc_dirty: false,
            reprint: None,
            last_doc_publish: Instant::now(),
            doc_interval: Duration::from_millis(50),
            last_copy: Duration::ZERO,
            watch_window: Duration::from_millis(WATCH_WINDOW_MS),
            jobs_cache: Vec::new(),
        };
        let events: Vec<String> = s
            .archive
            .recovery()
            .iter()
            .map(|r| match r {
                RecoveryEvent::Interrupted {
                    id,
                    found_bytes,
                    kept_bytes,
                } => format!(
                    "trabalho {id} interrompido; mantidos {kept_bytes} de {found_bytes} bytes (último checkpoint)"
                ),
                RecoveryEvent::Damaged { id, reason } => {
                    format!("trabalho {id} danificado: {reason}")
                }
            })
            .collect();
        for e in events {
            s.notice(e);
        }
        s.publish(true);
        s
    }

    pub fn archive(&self) -> &Archive {
        &self.archive
    }

    pub fn archive_mut(&mut self) -> &mut Archive {
        &mut self.archive
    }

    /// Read handle for the UI.
    pub fn snapshot_handle(&self) -> SnapshotHandle {
        self.snapshot.clone()
    }

    /// Options frozen into jobs that start from now on.
    pub fn set_options(&mut self, options: RenderOptions) {
        self.options = options;
    }

    /// Minimum time between document copies while bytes keep arriving (default 50 ms).
    pub fn set_document_interval(&mut self, d: Duration) {
        self.doc_interval = d;
    }

    /// How long after the last snapshot read the live document keeps being copied (default 2 s).
    pub fn set_watch_window(&mut self, d: Duration) {
        self.watch_window = d;
    }

    fn notice(&mut self, msg: String) {
        if self.notices.len() >= MAX_NOTICES {
            self.notices.pop_front();
        }
        self.notices.push_back(msg);
    }

    /// Consumes one card event.
    pub fn handle(&mut self, event: CaptureEvent) {
        match event {
            CaptureEvent::Open {
                job,
                origin,
                transport,
            } => {
                // One open job per card: an Open from a slot that still has one means the earlier
                // job never closed.
                let slot = origin.slot;
                if let Some(prev) = self
                    .active
                    .as_ref()
                    .filter(|a| a.slot == slot)
                    .map(|a| a.capture_job)
                {
                    self.notice(format!(
                        "trabalho {prev} sem fechamento; fechado como Reset ao abrir o {job}"
                    ));
                    self.finish_active(CloseReason::Reset);
                }
                if let Some(prev) = self
                    .background
                    .iter()
                    .find(|a| a.slot == slot)
                    .map(|a| a.capture_job)
                {
                    self.notice(format!(
                        "trabalho {prev} sem fechamento; fechado como Reset ao abrir o {job}"
                    ));
                    self.finish_background(prev, CloseReason::Reset);
                }
                // The new job takes the preview; the one it replaces keeps printing in background.
                if let Some(prev) = self.active.take() {
                    self.background.push(prev);
                }
                self.open_job(NewJob {
                    capture_job: job,
                    origin,
                    transport,
                    profile: crate::archive::PROFILE_LX310.to_string(),
                    options: self.options,
                });
                self.publish(true);
            }
            CaptureEvent::Bytes { job, offset, data } => {
                if self.active.as_ref().map(|a| a.capture_job) == Some(job) {
                    self.on_bytes(job, offset, &data);
                } else if let Some(i) = self.background.iter().position(|a| a.capture_job == job) {
                    self.bytes_in_background(i, job, offset, &data);
                } else {
                    self.on_bytes(job, offset, &data);
                }
                self.publish(false);
            }
            CaptureEvent::Close { job, reason } => {
                if self.active.as_ref().map(|a| a.capture_job) == Some(job) {
                    self.finish_active(reason);
                } else if self.background.iter().any(|a| a.capture_job == job) {
                    self.finish_background(job, reason);
                } else {
                    self.notice(format!("Close do trabalho {job} ignorado (não está ativo)"));
                }
                self.publish(true);
            }
        }
    }

    fn open_job(&mut self, new: NewJob) {
        let capture_job = new.capture_job;
        let slot = new.origin.slot;
        let options = new.options;
        let mut engine = (self.factory)();
        engine.begin(options);
        let (id, storage_error) = match self.archive.start_job(new) {
            Ok(id) => (id, None),
            Err(e) => {
                // The preview still works; nothing can be saved for this job.
                let msg = format!("não foi possível criar o trabalho no Arquivo: {e}");
                self.notice(msg.clone());
                (format!("sem-arquivo-{capture_job}"), Some(msg))
            }
        };
        self.doc = Arc::new(PageDocument::default());
        self.doc_rev += 1;
        self.doc_dirty = false;
        let started_at = self
            .archive
            .manifest(&id)
            .map(|m| m.started_at.clone())
            .unwrap_or_default();
        self.active = Some(ActiveJob {
            id,
            capture_job,
            options,
            started_at,
            title: String::new(),
            engine,
            fed: 0,
            storage_error,
            last_pages: 0,
            last_diags: 0,
            slot,
            last_bytes_at: Instant::now(),
        });
    }

    fn on_bytes(&mut self, job: u64, offset: u64, data: &[u8]) {
        let mut msgs = Vec::new();
        let Some(a) = self.active.as_mut().filter(|a| a.capture_job == job) else {
            self.notice(format!(
                "{} bytes do trabalho {job} ignorados (trabalho não está ativo)",
                data.len()
            ));
            return;
        };
        a.last_bytes_at = Instant::now();
        let mut data = data;
        if offset < a.fed {
            let skip = ((a.fed - offset) as usize).min(data.len());
            msgs.push(format!(
                "trabalho {job}: {skip} bytes repetidos descartados (offset {offset}, esperado {})",
                a.fed
            ));
            data = &data[skip..];
        } else if offset > a.fed {
            msgs.push(format!(
                "trabalho {job}: lacuna de {} bytes antes do offset {offset}",
                offset - a.fed
            ));
        }
        if !data.is_empty() {
            if a.storage_error.is_none() {
                if let Err(e) = self.archive.append(&a.id, data) {
                    let msg = format!("falha ao gravar o trabalho {}: {e}", a.id);
                    msgs.push(msg.clone());
                    a.storage_error = Some(msg);
                    // Force the progress record below so the manifest shows the problem.
                    a.last_pages = u32::MAX;
                }
            }
            a.engine.feed(a.fed, data);
            a.fed += data.len() as u64;
            if a.engine.take_revision() > 0 {
                self.doc_rev += 1;
                self.doc_dirty = true;
            }
            let doc = a.engine.document();
            let (pages, diags) = (doc.pages.len() as u32, doc.diagnostics.len());
            if pages != a.last_pages || diags != a.last_diags {
                a.last_pages = pages;
                a.last_diags = diags;
                let mut list = Vec::new();
                if let Some(e) = &a.storage_error {
                    let at = self.archive.manifest(&a.id).map_or(0, |m| m.total_bytes);
                    list.push(storage_diag(at, e));
                }
                list.extend(diag_summaries(doc));
                let total = diags as u64 + u64::from(a.storage_error.is_some());
                let _ = self.archive.set_progress(&a.id, pages, total, &list);
            }
        }
        for m in msgs {
            self.notice(m);
        }
    }

    /// Ends the job the preview follows (if any) with `reason`; another open job, if there is
    /// one, takes the preview.
    pub fn finish_active(&mut self, reason: CloseReason) {
        self.finish_focus(reason);
        if let Some(next) = self.background.pop() {
            self.focus(next);
        }
    }

    /// Makes `job` the one the preview follows (the document is copied once).
    fn focus(&mut self, job: ActiveJob) {
        self.doc = Arc::new(job.engine.document().clone());
        self.doc_rev += 1;
        self.doc_dirty = false;
        self.active = Some(job);
    }

    /// Feeds bytes to an open job that is not the preview's. It takes the preview when the
    /// current one has been quiet for half a second (alternating printers do not thrash it).
    fn bytes_in_background(&mut self, i: usize, job: u64, offset: u64, data: &[u8]) {
        let quiet = self
            .active
            .as_ref()
            .is_none_or(|a| a.last_bytes_at.elapsed() >= FOCUS_QUIET);
        let (rev, dirty) = (self.doc_rev, self.doc_dirty);
        let focus = self.active.take();
        self.active = Some(self.background.swap_remove(i));
        self.on_bytes(job, offset, data);
        let fed = self.active.take();
        if quiet {
            if let Some(f) = focus {
                self.background.push(f);
            }
            if let Some(j) = fed {
                self.focus(j);
            }
        } else {
            self.doc_rev = rev;
            self.doc_dirty = dirty;
            self.active = focus;
            if let Some(j) = fed {
                self.background.push(j);
            }
        }
    }

    /// Closes an open job that is not the preview's, leaving the preview as it is.
    fn finish_background(&mut self, job: u64, reason: CloseReason) {
        let Some(i) = self.background.iter().position(|a| a.capture_job == job) else {
            return;
        };
        let (doc, rev, dirty) = (self.doc.clone(), self.doc_rev, self.doc_dirty);
        let focus = self.active.take();
        self.active = Some(self.background.remove(i));
        self.finish_focus(reason);
        self.doc = doc;
        self.doc_rev = rev;
        self.doc_dirty = dirty;
        self.active = focus;
    }

    fn finish_focus(&mut self, reason: CloseReason) {
        let Some(mut a) = self.active.take() else {
            return;
        };
        a.engine.finish();
        a.engine.take_revision();
        let doc = a.engine.document().clone();
        if self.archive.is_active(&a.id) {
            let mut list = Vec::new();
            if let Some(e) = &a.storage_error {
                let at = self.archive.manifest(&a.id).map_or(0, |m| m.total_bytes);
                list.push(storage_diag(at, e));
            }
            list.extend(diag_summaries(&doc));
            let total = doc.diagnostics.len() as u64 + u64::from(a.storage_error.is_some());
            let _ = self
                .archive
                .set_progress(&a.id, doc.pages.len() as u32, total, &list);
            if let Err(e) = self.archive.finish_job(&a.id, reason) {
                self.notice(format!("falha ao fechar o trabalho: {e}"));
            }
            let started = self
                .archive
                .manifest(&a.id)
                .map(|m| m.started_at.clone())
                .unwrap_or_default();
            let title = auto_title(&doc, &started);
            if let Err(e) = self.archive.set_auto_title(&a.id, &title) {
                self.notice(format!("falha ao gravar o título: {e}"));
            }
        }
        self.doc = Arc::new(doc);
        self.doc_dirty = false;
        self.doc_rev += 1;
    }

    /// User action "concluir trabalho".
    pub fn finish_by_user(&mut self) {
        self.finish_active(CloseReason::User);
        self.publish(true);
    }

    /// Periodic work: checkpoints that are due and pending document copies.
    pub fn tick(&mut self) {
        for e in self.archive.checkpoint_if_due() {
            self.notice(format!("checkpoint: {e}"));
        }
        self.publish(false);
    }

    /// Ends the active job as `Shutdown` and publishes (emulator exit).
    pub fn shutdown(&mut self) {
        while self.active.is_some() || !self.background.is_empty() {
            self.finish_active(CloseReason::Shutdown);
        }
        self.publish(true);
    }

    /// Publishes right now (tests, and after user actions).
    pub fn publish_now(&mut self) {
        self.publish(true);
    }

    fn publish(&mut self, force: bool) {
        let interval = self.doc_interval.max(self.last_copy * 4);
        if self.doc_dirty
            && (force || self.last_doc_publish.elapsed() >= interval)
            && (force || self.snapshot.watched(self.watch_window.as_millis() as u64))
        {
            if let Some(a) = &self.active {
                let t = Instant::now();
                self.doc = Arc::new(a.engine.document().clone());
                self.last_copy = t.elapsed();
            }
            self.doc_dirty = false;
            self.last_doc_publish = Instant::now();
        }
        self.snap_rev += 1;
        if let Some(a) = self.active.as_mut() {
            a.title = auto_title(&self.doc, &a.started_at);
        }
        let active = self.active.as_ref().map(|a| ActiveView {
            job_id: a.id.clone(),
            capture_job: a.capture_job,
            options: a.options,
            title: a.title.clone(),
            bytes: a.fed,
            document: self.doc.clone(),
            doc_revision: self.doc_rev,
            storage_error: a.storage_error.clone(),
        });
        // The list of the Arquivo only grows: rebuild it on forced publications (open, close and
        // every user action); between them only the active job's row can have moved.
        if force || self.jobs_cache.is_empty() {
            self.jobs_cache = self.archive.jobs().iter().map(summary).collect();
        } else {
            for a in self.active.iter().chain(self.background.iter()) {
                if let Some(e) = self.archive.get(&a.id) {
                    if let Some(row) = self.jobs_cache.iter_mut().find(|j| j.id == a.id) {
                        let title = std::mem::take(&mut row.title);
                        *row = summary(e);
                        if row.title.is_empty() {
                            row.title = title;
                        }
                    }
                }
            }
        }
        let mut jobs: Vec<JobSummary> = self.jobs_cache.clone();
        if let Some(v) = &active {
            // The title of an open job is only stored when it closes; show the live one.
            if let Some(j) = jobs.iter_mut().find(|j| j.id == v.job_id) {
                if j.title.is_empty() {
                    j.title = v.title.clone();
                }
            }
        }
        let snap = Snapshot {
            revision: self.snap_rev,
            jobs,
            active,
            reprint: self.reprint.clone(),
            notices: self.notices.iter().cloned().collect(),
            retention: self.archive.retention_warning(),
        };
        self.snapshot.set(Arc::new(snap));
    }

    /// Renders the raw bytes of a closed job with `options` (no version is registered).
    pub fn render(
        &mut self,
        job_id: &str,
        options: RenderOptions,
    ) -> Result<PageDocument, ArchiveError> {
        if self.archive.is_active(job_id) {
            return Err(ArchiveError::JobActive(job_id.to_string()));
        }
        let total = self
            .archive
            .manifest(job_id)
            .ok_or_else(|| ArchiveError::NotFound(job_id.to_string()))?
            .total_bytes;
        if !self.archive.verify(job_id)? {
            return Err(ArchiveError::Damaged {
                id: job_id.to_string(),
                reason: "SHA-256 de raw.bin não confere com o manifesto".into(),
            });
        }
        let mut file = self.archive.open_raw(job_id)?.take(total);
        let mut engine = (self.factory)();
        engine.begin(options);
        let mut buf = vec![0u8; REPRINT_CHUNK];
        let mut offset = 0u64;
        loop {
            let n = file.read(&mut buf).map_err(|e| ArchiveError::Io {
                path: self.archive.root().join(job_id),
                source: e,
            })?;
            if n == 0 {
                break;
            }
            engine.feed(offset, &buf[..n]);
            offset += n as u64;
        }
        engine.finish();
        Ok(engine.document().clone())
    }

    /// Reprint: renders the job's raw bytes with other options and records a new version.
    /// `raw.bin` is only read.
    pub fn reprint(&mut self, job_id: &str, options: RenderOptions) -> Result<u32, ArchiveError> {
        let doc = self.render(job_id, options)?;
        let version = self.archive.add_version(
            job_id,
            options,
            doc.pages.len() as u32,
            doc.diagnostics.len() as u64,
        )?;
        self.reprint = Some(ReprintView {
            job_id: job_id.to_string(),
            version,
            document: Arc::new(doc),
        });
        self.publish(true);
        Ok(version)
    }

    /// Renders an existing version (its frozen options).
    pub fn render_version(
        &mut self,
        job_id: &str,
        version: u32,
    ) -> Result<PageDocument, ArchiveError> {
        let opts = self
            .archive
            .manifest(job_id)
            .and_then(|m| m.versions.iter().find(|v: &&JobVersion| v.id == version))
            .map(|v| v.options)
            .ok_or_else(|| ArchiveError::NotFound(format!("{job_id} v{version}")))?;
        self.render(job_id, opts)
    }

    /// Explicit delete from the user; the active job is refused.
    pub fn delete(&mut self, job_id: &str) -> Result<(), ArchiveError> {
        let r = self.archive.delete(job_id);
        if r.is_ok() && self.reprint.as_ref().is_some_and(|v| v.job_id == job_id) {
            self.reprint = None;
        }
        self.publish(true);
        r
    }

    pub fn set_title(&mut self, job_id: &str, title: &str) -> Result<(), ArchiveError> {
        let r = self.archive.set_title(job_id, title);
        self.publish(true);
        r
    }

    /// User action "cancelar trabalho": marks it in the Arquivo; nothing is deleted.
    pub fn cancel(&mut self, job_id: &str) -> Result<(), ArchiveError> {
        let r = self.archive.set_cancelled(job_id);
        self.publish(true);
        r
    }

    /// Registers an exported file in the manifest (the archive never writes to `path`).
    pub fn record_export(
        &mut self,
        job_id: &str,
        version: u32,
        path: &std::path::Path,
    ) -> Result<(), ArchiveError> {
        let r = self.archive.record_export(job_id, version, path);
        self.publish(true);
        r
    }

    pub fn set_note(&mut self, job_id: &str, note: &str) -> Result<(), ArchiveError> {
        let r = self.archive.set_note(job_id, note);
        self.publish(true);
        r
    }

    /// Runs a command; false means "stop the thread".
    fn run_command(&mut self, cmd: SpoolCommand) -> bool {
        let result = match cmd {
            SpoolCommand::FinishActive => {
                self.finish_by_user();
                Ok(())
            }
            SpoolCommand::Reprint { job_id, options } => self.reprint(&job_id, options).map(|_| ()),
            SpoolCommand::SetTitle { job_id, title } => self.set_title(&job_id, &title),
            SpoolCommand::SetNote { job_id, note } => self.set_note(&job_id, &note),
            SpoolCommand::Delete { job_id } => self.delete(&job_id),
            SpoolCommand::Cancel { job_id } => self.cancel(&job_id),
            SpoolCommand::RecordExport {
                job_id,
                version,
                path,
            } => self.record_export(&job_id, version, &path),
            SpoolCommand::SetOptions(o) => {
                self.set_options(o);
                Ok(())
            }
            SpoolCommand::Shutdown => return false,
        };
        if let Err(e) = result {
            self.notice(e.to_string());
            self.publish(true);
        }
        true
    }
}

/// User actions sent to a spool running on its own thread.
#[derive(Clone, Debug)]
pub enum SpoolCommand {
    FinishActive,
    Reprint {
        job_id: String,
        options: RenderOptions,
    },
    SetTitle {
        job_id: String,
        title: String,
    },
    SetNote {
        job_id: String,
        note: String,
    },
    Delete {
        job_id: String,
    },
    /// Marks a job as cancelled by the user (bytes are kept; the Central also closes it on the
    /// card with `CloseReason::User`).
    Cancel {
        job_id: String,
    },
    /// A PDF or RAW copy was written to `path`: registers it in the manifest right away.
    RecordExport {
        job_id: String,
        version: u32,
        path: PathBuf,
    },
    SetOptions(RenderOptions),
    /// Closes the active job as `Shutdown` and stops the thread.
    Shutdown,
}

/// UI-side handle of a spool thread.
#[derive(Clone)]
pub struct SpoolHandle {
    snapshot: SnapshotHandle,
    commands: Sender<SpoolCommand>,
}

impl SpoolHandle {
    pub fn snapshot(&self) -> Arc<Snapshot> {
        self.snapshot.get()
    }
    /// False when the spool thread is gone.
    pub fn send(&self, cmd: SpoolCommand) -> bool {
        self.commands.send(cmd).is_ok()
    }
}

/// The spool running on its own thread, fed by the card's channel.
pub struct SpoolService {
    handle: SpoolHandle,
    consumed: Arc<AtomicU64>,
    join: JoinHandle<Spool>,
}

impl SpoolService {
    /// Starts the thread. It stops on `SpoolCommand::Shutdown` or when `events` disconnects, closing
    /// the active job as `Shutdown` either way.
    pub fn spawn(mut spool: Spool, events: Receiver<CaptureEvent>) -> SpoolService {
        let (tx, rx) = std::sync::mpsc::channel::<SpoolCommand>();
        let handle = SpoolHandle {
            snapshot: spool.snapshot_handle(),
            commands: tx,
        };
        let consumed = Arc::new(AtomicU64::new(0));
        let counter = consumed.clone();
        let join = std::thread::Builder::new()
            .name("apple2-printer-spool".into())
            .spawn(move || {
                'run: loop {
                    while let Ok(cmd) = rx.try_recv() {
                        if !spool.run_command(cmd) {
                            break 'run;
                        }
                    }
                    match events.recv_timeout(Duration::from_millis(20)) {
                        Ok(ev) => {
                            let n = match &ev {
                                CaptureEvent::Bytes { data, .. } => data.len() as u64,
                                _ => 0,
                            };
                            spool.handle(ev);
                            counter.fetch_add(n, Ordering::Relaxed);
                        }
                        Err(RecvTimeoutError::Timeout) => spool.tick(),
                        Err(RecvTimeoutError::Disconnected) => break,
                    }
                }
                // Commands sent just before the stop (an export record, a cancel) still count.
                while let Ok(cmd) = rx.try_recv() {
                    if !spool.run_command(cmd) {
                        break;
                    }
                }
                spool.shutdown();
                spool
            })
            .expect("spawn spool thread");
        SpoolService {
            handle,
            consumed,
            join,
        }
    }

    pub fn handle(&self) -> SpoolHandle {
        self.handle.clone()
    }

    /// Payload bytes the thread has finished with (feeds the bridge's back-pressure).
    pub fn consumed_bytes(&self) -> Arc<AtomicU64> {
        self.consumed.clone()
    }

    /// Waits for the thread to stop (send `Shutdown` or drop the event sender first).
    pub fn join(self) -> Option<Spool> {
        self.join.join().ok()
    }
}
