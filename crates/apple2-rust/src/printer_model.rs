//! Data the Central de Impressoras draws and the requests it raises. Pure data: no egui, no winit,
//! no threads. [`crate::printer_hub`] fills the view model from the spool; [`crate::printer_window`]
//! draws it and turns clicks into [`CenterEvent`]s.

use apple2_printer::document::PageDocument;
use apple2_printer::paper::{InkColor, PaperKind, RenderOptions, Ribbon};
use std::sync::Arc;

/// What the printer is doing now (HU-IMP-031).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// No Generic Printer card feeds the virtual printer.
    #[default]
    Offline,
    /// Ready, no job open.
    Online,
    /// A job is open and bytes are arriving.
    Receiving,
    /// A job is open and nothing arrived for a moment (the program is thinking or finished).
    Waiting,
    /// The last job is complete.
    Finished,
    /// Something could not be saved or a job is damaged.
    Error,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Offline => "desligada",
            Status::Online => "online",
            Status::Receiving => "recebendo",
            Status::Waiting => "aguardando",
            Status::Finished => "concluído",
            Status::Error => "erro",
        }
    }
}

/// State of one job in the lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobState {
    /// Bytes are still arriving (the job is open).
    Receiving,
    Finished,
    /// Interrupted by a crash (recovered up to the last checkpoint) or damaged.
    Failed,
}

impl JobState {
    pub fn label(self) -> &'static str {
        match self {
            JobState::Receiving => "recebendo",
            JobState::Finished => "concluído",
            JobState::Failed => "com erro",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VersionEntry {
    pub id: u32,
    pub options: RenderOptions,
    pub pages: u32,
    pub created: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct JobEntry {
    /// Archive id (directory name).
    pub id: String,
    pub title: String,
    pub note: String,
    /// Apple II model that printed it.
    pub machine: String,
    pub slot: u8,
    /// Transporte of the job: "Automático", "8 bits" or "7 bits (AppleWin)".
    pub transport: String,
    /// Local, human date of the start.
    pub started: String,
    pub pages: u32,
    pub state: JobState,
    pub active: bool,
    pub damaged: Option<String>,
    pub versions: Vec<VersionEntry>,
    /// PDFs exported in this session (path, version).
    pub exports: Vec<(u32, String)>,
}

/// Paper, ribbon, colour, character table and international set chosen for the next jobs (all in
/// the render options: the table and set stand for the printer's DIP switches).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PrintSetup {
    pub options: RenderOptions,
}

/// The pages shown in the preview.
#[derive(Clone, Debug, Default)]
pub struct Shown {
    pub job: Option<String>,
    /// 0 = the job being printed (live); otherwise the archived version number.
    pub version: u32,
    pub label: String,
    pub document: Arc<PageDocument>,
    /// Papel, Fita and colour these pages were printed with (decides the preview's look).
    pub options: RenderOptions,
    /// The pages are being rendered from the archive.
    pub loading: bool,
    pub live: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub text: String,
    pub error: bool,
}

/// Snapshot of the printer subsystem for drawing. The hub replaces it as the spool changes; the
/// window redraws only when [`PrinterViewModel::revision`] moves.
#[derive(Clone, Debug, Default)]
pub struct PrinterViewModel {
    pub status: Status,
    /// Extra words after the status (job title, error text).
    pub detail: String,
    /// Apple II model label and slot of the printer card.
    pub machine: String,
    pub slot: Option<u8>,
    pub profile: String,
    pub setup: PrintSetup,
    /// Newest first.
    pub jobs: Vec<JobEntry>,
    /// Index into `jobs` of the job being printed.
    pub active_job: Option<usize>,
    /// Index into `jobs` of the job whose pages are shown.
    pub selected_job: Option<usize>,
    pub shown: Shown,
    pub messages: Vec<Message>,
    /// Bumped by the producer whenever anything above changed (also keys the texture cache).
    pub revision: u64,
}

impl PrinterViewModel {
    pub fn bump(&mut self) {
        self.revision += 1;
    }

    /// Same visible content (the revision and the identity of equal documents aside).
    pub fn same_content(&self, o: &PrinterViewModel) -> bool {
        self.status == o.status
            && self.detail == o.detail
            && self.machine == o.machine
            && self.slot == o.slot
            && self.setup == o.setup
            && self.jobs == o.jobs
            && self.active_job == o.active_job
            && self.selected_job == o.selected_job
            && self.messages == o.messages
            && self.shown.job == o.shown.job
            && self.shown.version == o.shown.version
            && self.shown.label == o.shown.label
            && self.shown.options == o.shown.options
            && self.shown.loading == o.shown.loading
            && self.shown.live == o.shown.live
            && Arc::ptr_eq(&self.shown.document, &o.shown.document)
    }

    pub fn document(&self) -> &PageDocument {
        &self.shown.document
    }

    pub fn selected(&self) -> Option<&JobEntry> {
        self.selected_job.and_then(|i| self.jobs.get(i))
    }
}

/// How the preview paints the paper and the ink.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look {
    pub paper: PaperKind,
    pub ink: InkColor,
    pub ribbon: Ribbon,
}

impl From<RenderOptions> for Look {
    fn from(o: RenderOptions) -> Look {
        Look {
            paper: o.paper.kind,
            ink: o.ink,
            ribbon: o.ribbon,
        }
    }
}

impl Default for Look {
    fn default() -> Self {
        RenderOptions::default().into()
    }
}

/// Which pages a PDF export covers (1-based, inclusive in the UI; converted at the boundary).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PageRange {
    All,
    /// First and last page, 1-based and inclusive.
    Pages(usize, usize),
}

/// Everything the "Salvar PDF" form collects. The file path is chosen afterwards.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfRequest {
    pub job: String,
    /// 0 = the job being printed (what exists so far).
    pub version: u32,
    pub range: PageRange,
    pub cover: bool,
    pub title: String,
    pub note: String,
}

/// Events the Central raises for the integration layer.
#[derive(Clone, Debug, PartialEq)]
pub enum CenterEvent {
    /// Shows a job (its newest version, or the live one).
    SelectJob(String),
    SelectVersion {
        job: String,
        version: u32,
    },
    /// Next jobs use these (R05: the running job keeps its own).
    SetNextSetup(PrintSetup),
    /// The form was confirmed; the app asks for a path and calls the hub.
    ExportPdf(PdfRequest),
    SaveRaw {
        job: String,
    },
    FinishJob,
    /// After the user confirmed.
    CancelJob,
    Reprint {
        job: String,
        options: RenderOptions,
    },
    SetTitle {
        job: String,
        title: String,
    },
    SetNote {
        job: String,
        note: String,
    },
    /// After the user confirmed.
    DeleteJob(String),
}
