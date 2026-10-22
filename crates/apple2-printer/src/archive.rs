//! Arquivo de impressões (GLOSSARY): the permanent, on-disk record of every Trabalho de impressão.
//!
//! Layout under the root folder (one directory per job, never named after anything the Apple II
//! sent):
//!
//! ```text
//! <root>/20261021-140301-a3f9c2/raw.bin        append-only bytes exactly as the card delivered them
//! <root>/20261021-140301-a3f9c2/manifest.json  versioned metadata (atomic tmp + rename)
//! <root>/20261021-140301-a3f9c2/v2.pdf         optional exports (see `export_path`)
//! ```
//!
//! Rules (ADR 0008): the archive never deletes anything by itself (only [`Archive::delete`] on an
//! explicit call, refusing the active job); `raw.bin` bytes never change after they were written;
//! a crashed job is recovered as `interrupted`, truncated to the last confirmed checkpoint; a
//! hostile or corrupt manifest becomes a "damaged" listing, never a panic, an unbounded allocation
//! or a write outside the root.

use crate::capture::{CaptureEvent, CloseReason, JobOrigin, Transport};
use crate::paper::RenderOptions;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Current manifest schema. v1 had `size` instead of `total_bytes` and no checkpoints, versions,
/// note or exports; v3 added `cancelled` and `options.table` / `options.country` (see
/// [`migrate_manifest_value`]).
pub const SCHEMA_VERSION: u32 = 3;
pub const MANIFEST_FILE: &str = "manifest.json";
pub const RAW_FILE: &str = "raw.bin";
/// Printer profile name stored in every manifest of this crate's LX-310 engine.
pub const PROFILE_LX310: &str = "Epson LX-310";

/// A manifest bigger than this is hostile or corrupt: it is not even read.
pub const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
const MAX_DIAGNOSTICS: usize = 100;
const MAX_VERSIONS: usize = 1000;
const MAX_EXPORTS: usize = 1000;
const MAX_TITLE_CHARS: usize = 200;
const MAX_NOTE_CHARS: usize = 20_000;
const MAX_SCANNED_ENTRIES: usize = 200_000;

#[derive(Debug)]
pub enum ArchiveError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    NotFound(String),
    /// The job is being written; the operation needs it closed.
    JobActive(String),
    /// The job's manifest or raw file is damaged.
    Damaged {
        id: String,
        reason: String,
    },
    InvalidName(String),
    Manifest(String),
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArchiveError::Io { path, source } => {
                write!(f, "erro de disco em {}: {source}", path.display())
            }
            ArchiveError::NotFound(id) => write!(f, "trabalho de impressão não encontrado: {id}"),
            ArchiveError::JobActive(id) => {
                write!(f, "o trabalho {id} está ativo; conclua-o antes")
            }
            ArchiveError::Damaged { id, reason } => {
                write!(f, "trabalho {id} danificado: {reason}")
            }
            ArchiveError::InvalidName(n) => write!(f, "nome de trabalho inválido: {n:?}"),
            ArchiveError::Manifest(m) => write!(f, "manifesto inválido: {m}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

fn io_err(path: &Path, source: io::Error) -> ArchiveError {
    ArchiveError::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub type Result<T> = std::result::Result<T, ArchiveError>;

/// Default root: `<platform data dir>/apple2-rust/impressoes` (same app identity as apple2-config).
pub fn default_root() -> Option<PathBuf> {
    directories::ProjectDirs::from("org", "AppleII-Rust", "apple2-rust")
        .map(|d| d.data_dir().join("impressoes"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Active,
    Completed,
    Interrupted,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticSummary {
    pub offset: u64,
    pub message: String,
}

/// One rendering of the job's raw bytes. Version 1 is the original; reprints add more.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobVersion {
    pub id: u32,
    pub created_at: String,
    pub options: RenderOptions,
    pub pages: u32,
    pub diagnostics_total: u64,
}

/// A PDF the user exported. `path` is informational only: the archive never writes to it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportRecord {
    pub version: u32,
    pub path: String,
    pub exported_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema_version: u32,
    /// Directory name; must match the directory the manifest lives in.
    pub id: String,
    /// Job number given by the card (`CaptureEvent::job`).
    pub capture_job: u64,
    pub origin: JobOrigin,
    pub transport: Transport,
    pub profile: String,
    /// Options frozen when the job started (version 1).
    pub options: RenderOptions,
    pub started_at: String,
    #[serde(default)]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub close_reason: Option<CloseReason>,
    pub total_bytes: u64,
    /// Bytes known to be on disk (fsync'd); recovery truncates `raw.bin` to this.
    #[serde(default)]
    pub confirmed_offset: u64,
    /// SHA-256 (hex) of the first `total_bytes` of `raw.bin`; set when the job closes or is recovered.
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub pages: u32,
    #[serde(default)]
    pub diagnostics_total: u64,
    /// First diagnostics only (bounded).
    #[serde(default)]
    pub diagnostics: Vec<DiagnosticSummary>,
    #[serde(default)]
    pub title: String,
    /// True once the user edited the title (the automatic title no longer overwrites it).
    #[serde(default)]
    pub title_edited: bool,
    #[serde(default)]
    pub note: String,
    /// The user cancelled the job in the Central: its bytes are kept, nothing is deleted.
    #[serde(default)]
    pub cancelled: bool,
    pub state: JobState,
    #[serde(default)]
    pub versions: Vec<JobVersion>,
    #[serde(default)]
    pub exports: Vec<ExportRecord>,
}

/// A listed job: readable, or damaged (listed so the user can see and delete it).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JobEntry {
    Valid(Box<Manifest>),
    Damaged { id: String, reason: String },
}

impl JobEntry {
    pub fn id(&self) -> &str {
        match self {
            JobEntry::Valid(m) => &m.id,
            JobEntry::Damaged { id, .. } => id,
        }
    }
    pub fn manifest(&self) -> Option<&Manifest> {
        match self {
            JobEntry::Valid(m) => Some(m),
            JobEntry::Damaged { .. } => None,
        }
    }
}

/// What opening the archive did to a job (shown to the user, never silent).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryEvent {
    Interrupted {
        id: String,
        /// Length `raw.bin` had on disk.
        found_bytes: u64,
        /// Length after truncating to the last checkpoint.
        kept_bytes: u64,
    },
    Damaged {
        id: String,
        reason: String,
    },
}

/// Parameters of a new job.
#[derive(Clone, Debug)]
pub struct NewJob {
    pub capture_job: u64,
    pub origin: JobOrigin,
    pub transport: Transport,
    pub profile: String,
    pub options: RenderOptions,
}

impl NewJob {
    /// Builds the parameters from a card `Open` event.
    pub fn from_open(event: &CaptureEvent, options: RenderOptions) -> Option<NewJob> {
        match event {
            CaptureEvent::Open {
                job,
                origin,
                transport,
            } => Some(NewJob {
                capture_job: *job,
                origin: origin.clone(),
                transport: *transport,
                profile: PROFILE_LX310.to_string(),
                options,
            }),
            _ => None,
        }
    }
}

/// Where `raw.bin` bytes go. Real archives use a file; tests inject faults (disk full).
pub trait RawSink: Send {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()>;
    fn sync(&mut self) -> io::Result<()>;
    /// Drops a partially written tail after a failed write.
    fn truncate(&mut self, len: u64) -> io::Result<()>;
}

struct FileSink(File);

impl RawSink for FileSink {
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        self.0.write_all(data)
    }
    fn sync(&mut self) -> io::Result<()> {
        self.0.sync_data()
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        self.0.set_len(len)
    }
}

type SinkFactory = Box<dyn Fn(File) -> Box<dyn RawSink> + Send>;
type Clock = Box<dyn Fn() -> DateTime<Utc> + Send>;

struct ActiveRaw {
    sink: Box<dyn RawSink>,
    hasher: Sha256,
    written: u64,
    unsynced: u64,
    last_sync: Instant,
}

pub struct Archive {
    root: PathBuf,
    entries: Vec<JobEntry>,
    active: Vec<(String, ActiveRaw)>,
    recovery: Vec<RecoveryEvent>,
    retention_limit: Option<u64>,
    checkpoint_bytes: u64,
    checkpoint_interval: Duration,
    sink_factory: Option<SinkFactory>,
    clock: Clock,
    counter: u64,
}

/// Retention notice: the archive only warns, it never removes anything (ADR 0008).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetentionWarning {
    pub used_bytes: u64,
    pub limit_bytes: u64,
}

fn is_safe_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Chronological order: the `YYYYMMDD-HHMMSS` prefix; jobs of the same second keep insertion order.
fn sort_key(id: &str) -> &str {
    id.get(..15).unwrap_or(id)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn fmt_time(t: DateTime<Utc>) -> String {
    t.to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn truncate_chars(s: &mut String, max: usize) {
    if let Some((i, _)) = s.char_indices().nth(max) {
        s.truncate(i);
    }
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Length of a regular file (not a symlink), or an explanation.
fn regular_file_len(path: &Path) -> std::result::Result<u64, String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => {
            Err(format!("{} é um link simbólico", file_label(path)))
        }
        Ok(m) if m.is_file() => Ok(m.len()),
        Ok(_) => Err(format!("{} não é um arquivo comum", file_label(path))),
        Err(e) => Err(format!("{} ilegível: {e}", file_label(path))),
    }
}

fn sha256_prefix(path: &Path, len: u64) -> io::Result<String> {
    let mut f = File::open(path)?.take(len);
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(hex(&h.finalize()))
}

/// Upgrades an older manifest (as parsed JSON) to [`SCHEMA_VERSION`]; unknown future versions are
/// refused instead of guessed.
pub fn migrate_manifest_value(mut v: serde_json::Value) -> Result<serde_json::Value> {
    let obj = v
        .as_object_mut()
        .ok_or_else(|| ArchiveError::Manifest("não é um objeto JSON".into()))?;
    let ver = obj
        .get("schema_version")
        .and_then(|x| x.as_u64())
        .ok_or_else(|| ArchiveError::Manifest("sem schema_version".into()))?;
    if ver > SCHEMA_VERSION as u64 {
        return Err(ArchiveError::Manifest(format!(
            "schema {ver} é mais novo que o suportado ({SCHEMA_VERSION})"
        )));
    }
    if ver == 0 {
        return Err(ArchiveError::Manifest("schema_version 0".into()));
    }
    if ver == 1 {
        if let Some(size) = obj.remove("size") {
            obj.entry("total_bytes").or_insert(size);
        }
        let total = obj.get("total_bytes").cloned().unwrap_or(0.into());
        obj.entry("confirmed_offset").or_insert(total);
    }
    if ver <= 2 {
        // v3 added `cancelled` (absent = false, filled by serde's default).
        obj.entry("cancelled").or_insert(false.into());
    }
    obj.insert("schema_version".into(), SCHEMA_VERSION.into());
    Ok(v)
}

fn normalize(m: &mut Manifest) {
    m.diagnostics.truncate(MAX_DIAGNOSTICS);
    m.versions.truncate(MAX_VERSIONS);
    m.exports.truncate(MAX_EXPORTS);
    truncate_chars(&mut m.title, MAX_TITLE_CHARS);
    truncate_chars(&mut m.note, MAX_NOTE_CHARS);
    truncate_chars(&mut m.profile, MAX_TITLE_CHARS);
    if m.versions.is_empty() {
        m.versions.push(JobVersion {
            id: 1,
            created_at: m.started_at.clone(),
            options: m.options,
            pages: m.pages,
            diagnostics_total: m.diagnostics_total,
        });
    }
}

/// Reads and validates `<dir>/manifest.json`. Never trusts anything inside it.
fn read_manifest(dir: &Path, dir_name: &str) -> std::result::Result<Manifest, String> {
    let path = dir.join(MANIFEST_FILE);
    let len = regular_file_len(&path)?;
    if len > MAX_MANIFEST_BYTES {
        return Err(format!("manifest.json grande demais ({len} bytes)"));
    }
    let mut buf = Vec::new();
    File::open(&path)
        .and_then(|f| f.take(MAX_MANIFEST_BYTES).read_to_end(&mut buf))
        .map_err(|e| format!("manifest.json ilegível: {e}"))?;
    let value: serde_json::Value =
        serde_json::from_slice(&buf).map_err(|e| format!("manifest.json corrompido: {e}"))?;
    let value = migrate_manifest_value(value).map_err(|e| e.to_string())?;
    let mut m: Manifest =
        serde_json::from_value(value).map_err(|e| format!("manifest.json inválido: {e}"))?;
    if m.id != dir_name {
        return Err(format!(
            "id do manifesto ({:?}) difere da pasta",
            m.id.chars().take(40).collect::<String>()
        ));
    }
    normalize(&mut m);
    Ok(m)
}

fn write_manifest(dir: &Path, m: &Manifest) -> Result<()> {
    let tmp = dir.join("manifest.json.tmp");
    let dst = dir.join(MANIFEST_FILE);
    let json = serde_json::to_vec_pretty(m).map_err(|e| ArchiveError::Manifest(e.to_string()))?;
    // A stale (or planted) tmp is replaced, never followed.
    let _ = fs::remove_file(&tmp);
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|e| io_err(&tmp, e))?;
    f.write_all(&json).map_err(|e| io_err(&tmp, e))?;
    f.sync_all().map_err(|e| io_err(&tmp, e))?;
    drop(f);
    if let Err(e) = fs::rename(&tmp, &dst) {
        let _ = fs::remove_file(&tmp);
        return Err(io_err(&dst, e));
    }
    Ok(())
}

impl Archive {
    /// Opens (creating the root if needed), scans every job and recovers interrupted ones.
    pub fn open(root: impl Into<PathBuf>) -> Result<Archive> {
        let root = root.into();
        fs::create_dir_all(&root).map_err(|e| io_err(&root, e))?;
        let mut a = Archive {
            root,
            entries: Vec::new(),
            active: Vec::new(),
            recovery: Vec::new(),
            retention_limit: None,
            checkpoint_bytes: 32 * 1024,
            checkpoint_interval: Duration::from_secs(2),
            sink_factory: None,
            clock: Box::new(Utc::now),
            counter: 0,
        };
        a.rescan()?;
        Ok(a)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// What the last scan recovered or found damaged.
    pub fn recovery(&self) -> &[RecoveryEvent] {
        &self.recovery
    }

    pub fn set_retention_limit(&mut self, bytes: Option<u64>) {
        self.retention_limit = bytes;
    }

    pub fn set_checkpoint_policy(&mut self, bytes: u64, interval: Duration) {
        self.checkpoint_bytes = bytes.max(1);
        self.checkpoint_interval = interval;
    }

    /// Replaces the clock (tests).
    pub fn set_clock(&mut self, clock: impl Fn() -> DateTime<Utc> + Send + 'static) {
        self.clock = Box::new(clock);
    }

    /// Wraps the `raw.bin` file of jobs created from now on (fault injection in tests).
    pub fn set_sink_factory(&mut self, f: impl Fn(File) -> Box<dyn RawSink> + Send + 'static) {
        self.sink_factory = Some(Box::new(f));
    }

    pub fn now(&self) -> DateTime<Utc> {
        (self.clock)()
    }

    pub fn jobs(&self) -> &[JobEntry] {
        &self.entries
    }

    pub fn get(&self, id: &str) -> Option<&JobEntry> {
        self.entries.iter().find(|e| e.id() == id)
    }

    pub fn manifest(&self, id: &str) -> Option<&Manifest> {
        self.get(id).and_then(JobEntry::manifest)
    }

    pub fn is_active(&self, id: &str) -> bool {
        self.active.iter().any(|(a, _)| a == id)
    }

    fn job_dir(&self, id: &str) -> Result<PathBuf> {
        if !is_safe_id(id) {
            return Err(ArchiveError::InvalidName(id.chars().take(80).collect()));
        }
        Ok(self.root.join(id))
    }

    fn manifest_mut(&mut self, id: &str) -> Result<&mut Manifest> {
        match self.entries.iter_mut().find(|e| e.id() == id) {
            Some(JobEntry::Valid(m)) => Ok(m),
            Some(JobEntry::Damaged { id, reason }) => Err(ArchiveError::Damaged {
                id: id.clone(),
                reason: reason.clone(),
            }),
            None => Err(ArchiveError::NotFound(id.to_string())),
        }
    }

    /// Re-reads the root. Active jobs of this process are kept as they are in memory.
    pub fn rescan(&mut self) -> Result<()> {
        self.recovery.clear();
        let mut found: Vec<JobEntry> = Vec::new();
        let rd = fs::read_dir(&self.root).map_err(|e| io_err(&self.root, e))?;
        for ent in rd.take(MAX_SCANNED_ENTRIES).flatten() {
            let name = ent.file_name().to_string_lossy().into_owned();
            let Ok(ft) = ent.file_type() else { continue };
            if ft.is_symlink() {
                found.push(JobEntry::Damaged {
                    id: name,
                    reason: "link simbólico dentro do Arquivo (ignorado)".into(),
                });
                continue;
            }
            if !ft.is_dir() {
                continue;
            }
            if !is_safe_id(&name) {
                found.push(JobEntry::Damaged {
                    id: name,
                    reason: "nome de pasta fora do padrão do Arquivo".into(),
                });
                continue;
            }
            if self.is_active(&name) {
                if let Some(e) = self.get(&name) {
                    found.push(e.clone());
                    continue;
                }
            }
            found.push(self.load_job(&name));
        }
        found.sort_by(|a, b| sort_key(a.id()).cmp(sort_key(b.id())));
        for e in &found {
            if let JobEntry::Damaged { id, reason } = e {
                self.recovery.push(RecoveryEvent::Damaged {
                    id: id.clone(),
                    reason: reason.clone(),
                });
            }
        }
        self.entries = found;
        Ok(())
    }

    fn load_job(&mut self, name: &str) -> JobEntry {
        let dir = self.root.join(name);
        let damaged = |reason: String| JobEntry::Damaged {
            id: name.to_string(),
            reason,
        };
        let mut m = match read_manifest(&dir, name) {
            Ok(m) => m,
            Err(r) => return damaged(r),
        };
        let raw = dir.join(RAW_FILE);
        let raw_len = match regular_file_len(&raw) {
            Ok(l) => l,
            Err(r) => return damaged(r),
        };
        if m.state == JobState::Active {
            let kept = m.confirmed_offset.min(raw_len);
            let recovered = (|| -> io::Result<()> {
                if raw_len > kept {
                    OpenOptions::new().write(true).open(&raw)?.set_len(kept)?;
                }
                m.sha256 = Some(sha256_prefix(&raw, kept)?);
                Ok(())
            })();
            if let Err(e) = recovered {
                return damaged(format!("recuperação falhou: {e}"));
            }
            m.total_bytes = kept;
            m.confirmed_offset = kept;
            m.state = JobState::Interrupted;
            if let Err(e) = write_manifest(&dir, &m) {
                return damaged(format!("não foi possível gravar a recuperação: {e}"));
            }
            self.recovery.push(RecoveryEvent::Interrupted {
                id: name.to_string(),
                found_bytes: raw_len,
                kept_bytes: kept,
            });
        } else if raw_len != m.total_bytes {
            return damaged(format!(
                "raw.bin tem {raw_len} bytes, o manifesto diz {}",
                m.total_bytes
            ));
        }
        JobEntry::Valid(Box::new(m))
    }

    fn new_id(&mut self, now: DateTime<Utc>) -> String {
        self.counter += 1;
        let mut h = Sha256::new();
        h.update(now.timestamp().to_le_bytes());
        h.update(std::process::id().to_le_bytes());
        h.update(self.counter.to_le_bytes());
        h.update(Utc::now().timestamp_subsec_nanos().to_le_bytes());
        format!(
            "{}-{}",
            now.format("%Y%m%d-%H%M%S"),
            hex(&h.finalize()[..3])
        )
    }

    /// Creates the job directory and files. Never reuses an existing directory.
    pub fn start_job(&mut self, new: NewJob) -> Result<String> {
        let now = self.now();
        let started = fmt_time(now);
        let mut last_err = None;
        for _ in 0..32 {
            let id = self.new_id(now);
            let dir = self.root.join(&id);
            match fs::create_dir(&dir) {
                Ok(()) => return self.init_job(id, dir, new, started),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(e) => {
                    last_err = Some(io_err(&dir, e));
                    break;
                }
            }
        }
        Err(last_err.unwrap_or_else(|| ArchiveError::InvalidName("sem id livre".into())))
    }

    fn init_job(
        &mut self,
        id: String,
        dir: PathBuf,
        new: NewJob,
        started: String,
    ) -> Result<String> {
        let raw_path = dir.join(RAW_FILE);
        let opened = OpenOptions::new()
            .append(true)
            .create_new(true)
            .open(&raw_path)
            .map_err(|e| io_err(&raw_path, e));
        let file = match opened {
            Ok(f) => f,
            Err(e) => {
                // Nothing was written yet: remove only the directory we just created.
                let _ = fs::remove_dir(&dir);
                return Err(e);
            }
        };
        let sink: Box<dyn RawSink> = match &self.sink_factory {
            Some(f) => f(file),
            None => Box::new(FileSink(file)),
        };
        let raw = ActiveRaw {
            sink,
            hasher: Sha256::new(),
            written: 0,
            unsynced: 0,
            last_sync: Instant::now(),
        };
        let mut m = Manifest {
            schema_version: SCHEMA_VERSION,
            id: id.clone(),
            capture_job: new.capture_job,
            origin: new.origin,
            transport: new.transport,
            profile: new.profile,
            options: new.options,
            started_at: started.clone(),
            completed_at: None,
            close_reason: None,
            total_bytes: 0,
            confirmed_offset: 0,
            sha256: None,
            pages: 0,
            diagnostics_total: 0,
            diagnostics: Vec::new(),
            title: String::new(),
            title_edited: false,
            note: String::new(),
            cancelled: false,
            state: JobState::Active,
            versions: vec![JobVersion {
                id: 1,
                created_at: started,
                options: new.options,
                pages: 0,
                diagnostics_total: 0,
            }],
            exports: Vec::new(),
        };
        normalize(&mut m);
        if let Err(e) = write_manifest(&dir, &m) {
            drop(raw);
            let _ = fs::remove_file(&raw_path);
            let _ = fs::remove_dir(&dir);
            return Err(e);
        }
        self.entries.push(JobEntry::Valid(Box::new(m)));
        self.entries
            .sort_by(|a, b| sort_key(a.id()).cmp(sort_key(b.id())));
        self.active.push((id.clone(), raw));
        Ok(id)
    }

    fn active_index(&self, id: &str) -> Result<usize> {
        self.active
            .iter()
            .position(|(a, _)| a == id)
            .ok_or_else(|| ArchiveError::NotFound(id.to_string()))
    }

    /// Appends bytes of an active job. On a write error the partial tail is dropped, so `raw.bin`
    /// always holds a whole prefix of the stream; the caller decides what to do next.
    pub fn append(&mut self, id: &str, data: &[u8]) -> Result<()> {
        let dir = self.job_dir(id)?;
        let idx = self.active_index(id)?;
        let (checkpoint_bytes, interval) = (self.checkpoint_bytes, self.checkpoint_interval);
        let raw = &mut self.active[idx].1;
        if let Err(e) = raw.sink.write_all(data) {
            let _ = raw.sink.truncate(raw.written);
            return Err(io_err(&dir.join(RAW_FILE), e));
        }
        raw.hasher.update(data);
        raw.written += data.len() as u64;
        raw.unsynced += data.len() as u64;
        let written = raw.written;
        let due = raw.unsynced >= checkpoint_bytes
            || (raw.unsynced > 0 && raw.last_sync.elapsed() >= interval);
        if let Ok(m) = self.manifest_mut(id) {
            m.total_bytes = written;
        }
        if due {
            self.checkpoint(id)?;
        }
        Ok(())
    }

    /// fsync's the raw file and records the confirmed offset in the manifest.
    pub fn checkpoint(&mut self, id: &str) -> Result<()> {
        let dir = self.job_dir(id)?;
        let idx = self.active_index(id)?;
        let raw = &mut self.active[idx].1;
        raw.sink
            .sync()
            .map_err(|e| io_err(&dir.join(RAW_FILE), e))?;
        raw.unsynced = 0;
        raw.last_sync = Instant::now();
        let written = raw.written;
        let m = self.manifest_mut(id)?;
        m.confirmed_offset = written;
        m.total_bytes = written;
        let snapshot = m.clone();
        write_manifest(&dir, &snapshot)
    }

    /// Checkpoints every active job whose interval elapsed (called by the spool's tick).
    pub fn checkpoint_if_due(&mut self) -> Vec<ArchiveError> {
        let due: Vec<String> = self
            .active
            .iter()
            .filter(|(_, r)| r.unsynced > 0 && r.last_sync.elapsed() >= self.checkpoint_interval)
            .map(|(id, _)| id.clone())
            .collect();
        due.iter()
            .filter_map(|id| self.checkpoint(id).err())
            .collect()
    }

    /// In-memory progress (written to disk by the next checkpoint or close).
    pub fn set_progress(
        &mut self,
        id: &str,
        pages: u32,
        diagnostics_total: u64,
        diagnostics: &[DiagnosticSummary],
    ) -> Result<()> {
        let m = self.manifest_mut(id)?;
        m.pages = pages;
        m.diagnostics_total = diagnostics_total;
        m.diagnostics = diagnostics.iter().take(MAX_DIAGNOSTICS).cloned().collect();
        if let Some(v) = m.versions.first_mut() {
            v.pages = pages;
            v.diagnostics_total = diagnostics_total;
        }
        Ok(())
    }

    /// Closes an active job: syncs, stores the SHA-256 and marks it completed. The writer is
    /// released even when the final manifest write fails (the error is returned).
    pub fn finish_job(&mut self, id: &str, reason: CloseReason) -> Result<()> {
        let dir = self.job_dir(id)?;
        let idx = self.active_index(id)?;
        let (_, mut raw) = self.active.remove(idx);
        let sync = raw.sink.sync().map_err(|e| io_err(&dir.join(RAW_FILE), e));
        let sha = hex(&raw.hasher.finalize());
        let completed = fmt_time(self.now());
        let m = self.manifest_mut(id)?;
        m.total_bytes = raw.written;
        m.confirmed_offset = raw.written;
        m.sha256 = Some(sha);
        m.completed_at = Some(completed);
        m.close_reason = Some(reason);
        m.state = JobState::Completed;
        let snapshot = m.clone();
        write_manifest(&dir, &snapshot)?;
        sync
    }

    /// Sets the automatic title unless the user edited it.
    pub fn set_auto_title(&mut self, id: &str, title: &str) -> Result<()> {
        let dir = self.job_dir(id)?;
        let m = self.manifest_mut(id)?;
        if m.title_edited {
            return Ok(());
        }
        m.title = title.to_string();
        truncate_chars(&mut m.title, MAX_TITLE_CHARS);
        let snap = m.clone();
        write_manifest(&dir, &snap)
    }

    pub fn set_title(&mut self, id: &str, title: &str) -> Result<()> {
        let dir = self.job_dir(id)?;
        let m = self.manifest_mut(id)?;
        m.title = title.to_string();
        truncate_chars(&mut m.title, MAX_TITLE_CHARS);
        m.title_edited = true;
        let snap = m.clone();
        write_manifest(&dir, &snap)
    }

    /// Marks the job as cancelled by the user (bytes and versions stay; nothing is deleted).
    pub fn set_cancelled(&mut self, id: &str) -> Result<()> {
        let dir = self.job_dir(id)?;
        let m = self.manifest_mut(id)?;
        m.cancelled = true;
        let snap = m.clone();
        write_manifest(&dir, &snap)
    }

    pub fn set_note(&mut self, id: &str, note: &str) -> Result<()> {
        let dir = self.job_dir(id)?;
        let m = self.manifest_mut(id)?;
        m.note = note.to_string();
        truncate_chars(&mut m.note, MAX_NOTE_CHARS);
        let snap = m.clone();
        write_manifest(&dir, &snap)
    }

    /// Registers a reprint with other options. The raw bytes are untouched. Returns the version id.
    pub fn add_version(
        &mut self,
        id: &str,
        options: RenderOptions,
        pages: u32,
        diagnostics_total: u64,
    ) -> Result<u32> {
        let dir = self.job_dir(id)?;
        let now = fmt_time(self.now());
        let m = self.manifest_mut(id)?;
        if m.versions.len() >= MAX_VERSIONS {
            return Err(ArchiveError::Manifest(
                "versões demais neste trabalho".into(),
            ));
        }
        let vid = m.versions.iter().map(|v| v.id).max().unwrap_or(0) + 1;
        m.versions.push(JobVersion {
            id: vid,
            created_at: now,
            options,
            pages,
            diagnostics_total,
        });
        let snap = m.clone();
        write_manifest(&dir, &snap)?;
        Ok(vid)
    }

    /// Where a PDF of `version` should be written inside the job directory (a fixed, safe name).
    pub fn export_path(&self, id: &str, version: u32) -> Result<PathBuf> {
        Ok(self.job_dir(id)?.join(format!("v{version}.pdf")))
    }

    /// Records an exported PDF (anywhere on disk; the path is only displayed).
    pub fn record_export(&mut self, id: &str, version: u32, path: &Path) -> Result<()> {
        let dir = self.job_dir(id)?;
        let now = fmt_time(self.now());
        let m = self.manifest_mut(id)?;
        if m.exports.len() >= MAX_EXPORTS {
            m.exports.remove(0);
        }
        m.exports.push(ExportRecord {
            version,
            path: path.display().to_string(),
            exported_at: now,
        });
        let snap = m.clone();
        write_manifest(&dir, &snap)
    }

    /// Opens `raw.bin` for reading (refusing symlinks).
    pub fn open_raw(&self, id: &str) -> Result<File> {
        let path = self.job_dir(id)?.join(RAW_FILE);
        regular_file_len(&path).map_err(|reason| ArchiveError::Damaged {
            id: id.to_string(),
            reason,
        })?;
        File::open(&path).map_err(|e| io_err(&path, e))
    }

    /// Recomputes the SHA-256 of the raw bytes and compares with the manifest.
    pub fn verify(&self, id: &str) -> Result<bool> {
        let m = self
            .manifest(id)
            .ok_or_else(|| ArchiveError::NotFound(id.to_string()))?;
        let path = self.job_dir(id)?.join(RAW_FILE);
        let sha = sha256_prefix(&path, m.total_bytes).map_err(|e| io_err(&path, e))?;
        Ok(m.sha256.as_deref() == Some(sha.as_str()))
    }

    /// Bytes used on disk by every job (files directly inside each job directory).
    pub fn total_size(&self) -> u64 {
        let mut total = 0;
        for e in &self.entries {
            let Ok(dir) = self.job_dir(e.id()) else {
                continue;
            };
            let Ok(rd) = fs::read_dir(dir) else { continue };
            for f in rd.flatten() {
                if let Ok(md) = fs::symlink_metadata(f.path()) {
                    if md.is_file() {
                        total += md.len();
                    }
                }
            }
        }
        total
    }

    /// Warns when the archive is above the configured limit. Never removes anything.
    pub fn retention_warning(&self) -> Option<RetentionWarning> {
        let limit = self.retention_limit?;
        let used = self.total_size();
        (used > limit).then_some(RetentionWarning {
            used_bytes: used,
            limit_bytes: limit,
        })
    }

    /// Permanently deletes one job. Only on explicit request; the active job is refused.
    pub fn delete(&mut self, id: &str) -> Result<()> {
        let dir = self.job_dir(id)?;
        if self.is_active(id) {
            return Err(ArchiveError::JobActive(id.to_string()));
        }
        if self.get(id).is_none() {
            return Err(ArchiveError::NotFound(id.to_string()));
        }
        match fs::symlink_metadata(&dir) {
            Ok(m) if m.is_dir() && !m.file_type().is_symlink() => {
                fs::remove_dir_all(&dir).map_err(|e| io_err(&dir, e))?;
            }
            Ok(_) => {
                return Err(ArchiveError::Damaged {
                    id: id.to_string(),
                    reason: "não é uma pasta comum; remova manualmente".into(),
                })
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_err(&dir, e)),
        }
        self.entries.retain(|e| e.id() != id);
        Ok(())
    }
}
