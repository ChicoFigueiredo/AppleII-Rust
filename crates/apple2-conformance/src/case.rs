//! Declarative conformance cases (`tests/conformance/cases/*.toml`).

use crate::audio::{AudioSpec, PcmLimitsSpec};
use apple2_core::{CardKind, Model};
use serde::Deserialize;
use std::path::Path;

/// One conformance case. See `tests/conformance/cases/` for examples.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    /// User story (`HU-0101`).
    pub story: String,
    /// Acceptance criterion (`CA` text or id).
    pub criterion: String,
    /// Model name, see [`parse_model`].
    pub model: String,
    /// `"6502"`, `"65C02"` or `"auto"` (the model's CPU, the default).
    #[serde(default)]
    pub cpu: Option<String>,
    /// Cards as `"slot:kind"`, e.g. `"0:language-card-16k"` or `"6:disk2"` (Disk II, drive 1
    /// loaded from `fixture`). When present it replaces the model's default slots; `[]` means no
    /// cards.
    #[serde(default)]
    pub cards: Option<Vec<String>>,
    /// `-memclear N` (`g_nMemoryClearType`, -1..=7); default 2 (`MIP_FF_FF_00_00`). The "random"
    /// bytes are always the deterministic ones (draw = 0), as in the oracle.
    #[serde(default)]
    pub memclear: Option<i32>,
    #[serde(default)]
    pub fixture: Option<FixtureSpec>,
    /// Disk image in drive 2 of the Disk II (`trace-checkpoints` cases).
    #[serde(default)]
    pub fixture2: Option<FixtureSpec>,
    /// Hard disk image(s) in the hard disk controller of slot 7 (`trace-checkpoints` cases).
    #[serde(default)]
    pub hdd: Option<FixtureSpec>,
    #[serde(default)]
    pub hdd2: Option<FixtureSpec>,
    /// "Enhance Disk Speed" off: authentic Disk II timing (`trace-checkpoints` cases).
    #[serde(default)]
    pub authentic: bool,
    /// Roles (`fixture`, `fixture2`, `hdd`, `hdd2`) whose image file is read-only on the host, so
    /// the drive reports it write-protected (`trace-checkpoints` cases).
    #[serde(default)]
    pub readonly: Vec<String>,
    pub run: RunSpec,
    #[serde(default)]
    pub input: Vec<InputEvent>,
    /// Sound card and 6502 program of `audio-events` cases.
    #[serde(default)]
    pub audio: Option<AudioSpec>,
    pub oracle: OracleSpec,
    /// Video options of `framebuffer-hash` cases.
    #[serde(default)]
    pub video: Option<VideoSpec>,
    /// Free text kept in the report.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Rendering options and starting scene of a `framebuffer-hash` case (HU-0201/HU-0202).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoSpec {
    /// `Tipo de vídeo`: `mono-custom`, `color-idealized`, `color-rgb`, `color-monitor`,
    /// `color-tv` (default), `mono-tv`, `mono-amber`, `mono-green` or `mono-white`.
    #[serde(default, rename = "type")]
    pub video_type: Option<String>,
    /// 60 (NTSC, default) or 50 (PAL).
    #[serde(default)]
    pub hz: Option<u32>,
    /// `video7-sl7` (default), `le-chat-mauve-eve` or `le-chat-mauve-feline`.
    #[serde(default)]
    pub rgb_card: Option<String>,
    /// `visible` (560x384, default) or `full` (600x420 with the 20/18-pixel borders).
    #[serde(default)]
    pub region: Option<String>,
    /// Scene file relative to `tests/conformance` (see `scene.rs`); absent: the ROM boots.
    #[serde(default)]
    pub scene: Option<String>,
    /// SHA-256 of the scene file.
    #[serde(default)]
    pub scene_sha256: Option<String>,
    /// Slot (1-7) of a VidHD card: 640x400 framebuffer and Super Hi-Res (HU-0202).
    #[serde(default)]
    pub vidhd_slot: Option<u8>,
}

impl VideoSpec {
    pub fn video_type(&self) -> Result<apple2_video::VideoType, String> {
        parse_video_type(self.video_type.as_deref().unwrap_or("color-tv"))
    }

    pub fn standard(&self) -> Result<apple2_core::VideoStandard, String> {
        match self.hz.unwrap_or(60) {
            60 => Ok(apple2_core::VideoStandard::Ntsc),
            50 => Ok(apple2_core::VideoStandard::Pal),
            h => Err(format!("video.hz {h}: use 50 or 60")),
        }
    }

    pub fn rgb_card(&self) -> Result<apple2_video::RgbCard, String> {
        match self.rgb_card.as_deref().unwrap_or("video7-sl7") {
            "video7-sl7" => Ok(apple2_video::RgbCard::Video7Sl7),
            "le-chat-mauve-eve" => Ok(apple2_video::RgbCard::LeChatMauveEve),
            "le-chat-mauve-feline" => Ok(apple2_video::RgbCard::LeChatMauveFeline),
            o => Err(format!("video.rgb_card `{o}` unknown")),
        }
    }

    pub fn full_region(&self) -> Result<bool, String> {
        match self.region.as_deref().unwrap_or("visible") {
            "visible" => Ok(false),
            "full" => Ok(true),
            o => Err(format!("video.region `{o}`: use visible or full")),
        }
    }

    fn validate(&self) -> Result<(), String> {
        if self.vidhd_slot.is_some_and(|s| !(1..=7).contains(&s)) {
            return Err("video.vidhd_slot: use 1-7".into());
        }
        self.video_type()?;
        self.standard()?;
        self.rgb_card()?;
        self.full_region()?;
        if let Some(h) = &self.scene_sha256 {
            if !is_sha256_hex(h) {
                return Err("video.scene_sha256 is not 64 lowercase hex digits".into());
            }
        }
        Ok(())
    }
}

/// `VideoType` by kebab-case name or AppleWin number (0-8).
pub fn parse_video_type(s: &str) -> Result<apple2_video::VideoType, String> {
    use apple2_video::VideoType as V;
    Ok(match s {
        "mono-custom" | "0" => V::MonoCustom,
        "color-idealized" | "1" => V::ColorIdealized,
        "color-rgb" | "2" => V::ColorVideocardRgb,
        "color-monitor" | "3" => V::ColorMonitorNtsc,
        "color-tv" | "4" => V::ColorTv,
        "mono-tv" | "5" => V::MonoTv,
        "mono-amber" | "6" => V::MonoAmber,
        "mono-green" | "7" => V::MonoGreen,
        "mono-white" | "8" => V::MonoWhite,
        o => return Err(format!("video.type `{o}` unknown")),
    })
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSpec {
    /// Relative to the workspace root (e.g. `target/conformance-cache/applewin-test/...`).
    pub path: String,
    /// Expected SHA-256 (lowercase hex).
    pub sha256: String,
    /// `true`: the fixture is an AppleWin `.aws.yaml` the machine is imported from; `run.cycles`
    /// then counts from where the state stopped (the oracle's `--state ... --cycles N`).
    #[serde(default)]
    pub save_state: bool,
    /// Directories (relative to the workspace root) searched for the disk images it names, after
    /// the state's own directory and its parents.
    #[serde(default)]
    pub image_dirs: Vec<String>,
}

/// How long to run, from power-on. Exactly one of `frames` and `cycles`.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunSpec {
    #[serde(default)]
    pub frames: Option<u64>,
    #[serde(default)]
    pub cycles: Option<u64>,
    /// Runs the case this many times from power-on; every run must give the same observation.
    #[serde(default = "one")]
    pub repeat: u32,
}

fn one() -> u32 {
    1
}

/// A keyboard event, at a frame or a cycle (exactly one of `at_frame`, `at_cycle`) and with
/// `text` (pasted, one character per strobe clear) or `key` (one `key_press` of a character code).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputEvent {
    #[serde(default)]
    pub at_frame: Option<u64>,
    #[serde(default)]
    pub at_cycle: Option<u64>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub key: Option<u8>,
}

impl InputEvent {
    pub fn validate(&self) -> Result<(), String> {
        match (self.at_frame, self.at_cycle) {
            (Some(_), None) | (None, Some(_)) => {}
            _ => return Err("input: exactly one of at_frame/at_cycle".into()),
        }
        match (&self.text, self.key) {
            (Some(_), None) | (None, Some(_)) => Ok(()),
            _ => Err("input: exactly one of text/key".into()),
        }
    }

    /// Human form for manifests: `frame:30:text=PRINT 1\r` / `cycle:5000:key=65`.
    pub fn describe(&self) -> String {
        let at = match (self.at_frame, self.at_cycle) {
            (Some(f), _) => format!("frame:{f}"),
            (_, Some(c)) => format!("cycle:{c}"),
            _ => "?".into(),
        };
        match (&self.text, self.key) {
            (Some(t), _) => format!("{at}:text={}", escape(t)),
            (_, Some(k)) => format!("{at}:key={k}"),
            _ => at,
        }
    }

    /// Parses `frame:N:TEXT` or `cycle:N:TEXT`, with `\r`, `\n`, `\\` and `\xNN` escapes in TEXT.
    pub fn parse_cli(s: &str) -> Result<InputEvent, String> {
        let mut it = s.splitn(3, ':');
        let (kind, n, text) = (it.next(), it.next(), it.next());
        let (Some(kind), Some(n), Some(text)) = (kind, n, text) else {
            return Err(format!(
                "input `{s}`: expected frame:N:TEXT or cycle:N:TEXT"
            ));
        };
        let n: u64 = n.parse().map_err(|_| format!("input `{s}`: bad number"))?;
        let mut ev = InputEvent {
            at_frame: None,
            at_cycle: None,
            text: Some(unescape(text)?),
            key: None,
        };
        match kind {
            "frame" => ev.at_frame = Some(n),
            "cycle" => ev.at_cycle = Some(n),
            _ => return Err(format!("input `{s}`: expected `frame` or `cycle`")),
        }
        Ok(ev)
    }
}

fn escape(s: &str) -> String {
    let mut o = String::new();
    for c in s.chars() {
        match c {
            '\r' => o.push_str("\\r"),
            '\n' => o.push_str("\\n"),
            '\\' => o.push_str("\\\\"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\x{:02X}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn unescape(s: &str) -> Result<String, String> {
    let mut o = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            o.push(c);
            continue;
        }
        match it.next() {
            Some('r') => o.push('\r'),
            Some('n') => o.push('\n'),
            Some('\\') => o.push('\\'),
            Some('x') => {
                let h: String = it.by_ref().take(2).collect();
                let v = u8::from_str_radix(&h, 16).map_err(|_| format!("bad escape \\x{h}"))?;
                o.push(v as char);
            }
            other => return Err(format!("bad escape \\{}", other.unwrap_or(' '))),
        }
    }
    Ok(o)
}

/// Who or what says what the right answer is (GLOSSARY: Oráculo), plus the harness-only
/// `regression` and `determinism` authorities, which never count as AppleWin evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Authority {
    /// AppleWin at the frozen SHA.
    Applewin,
    HardwareSpec,
    /// A validated fixture.
    Fixture,
    KnownResult,
    /// The Rust machine compared with its own earlier output: protects against change, proves
    /// nothing about AppleWin.
    Regression,
    /// Repeated runs agree with each other.
    Determinism,
}

impl Authority {
    pub fn as_str(self) -> &'static str {
        match self {
            Authority::Applewin => "applewin",
            Authority::HardwareSpec => "hardware-spec",
            Authority::Fixture => "fixture",
            Authority::KnownResult => "known-result",
            Authority::Regression => "regression",
            Authority::Determinism => "determinism",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OracleKind {
    /// SHA-256 of the 24 decoded text rows joined with `\n`.
    TextHash,
    /// SHA-256 of main RAM (64K) followed by aux RAM (64K).
    MemoryHash,
    /// SHA-256 of the visible RGBA framebuffer after rendering every frame.
    FramebufferHash,
    /// SHA-256 of the JSON `MachineState`.
    StateHash,
    /// A reference trace file compared instruction by instruction.
    Trace,
    /// Expressions over registers and memory, all of which must hold.
    Expr,
    /// A disk boot compared in lock step with an oracle run through a checkpoint file
    /// (`oracle.trace`, see `disk.rs`); needs a `fixture` holding the disk image.
    TraceCheckpoints,
    /// Audio events (and optionally PCM) recorded by the AppleWin oracle for the same program or
    /// save-state; see `audio.rs` and docs/conformance/test-oracles.md ("Áudio").
    AudioEvents,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleSpec {
    pub kind: OracleKind,
    pub authority: Authority,
    /// What produced the expected value and how to regenerate it.
    pub source: String,
    /// Expected hash for the `*-hash` kinds. Absent with `repeat > 1`: the runs only have to
    /// agree with each other.
    #[serde(default)]
    pub sha256: Option<String>,
    /// Reference trace, relative to `tests/conformance/` (kind `trace`). It may point to the
    /// cache (`../../target/conformance-cache/...`); when absent the result is BLOCKED and the
    /// message carries `source` (the command that regenerates it).
    #[serde(default)]
    pub trace: Option<String>,
    /// Expected SHA-256 of the reference trace file (kind `trace`). Mandatory for traces kept in
    /// the cache, which are not versioned: a different file is BLOCKED, not compared.
    #[serde(default)]
    pub trace_sha256: Option<String>,
    /// Kind `trace` without a stored file: SHA-256 of the trace's data rows (the lines after the
    /// `cycle,pc,...` header, each ended by a newline). A mismatch has no row-level diff;
    /// regenerate the oracle trace to see it.
    #[serde(default)]
    pub trace_rows_sha256: Option<String>,
    /// Trace fields not compared (e.g. `["cycle"]`).
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Rows of context kept in the diff artifact (default 8).
    #[serde(default)]
    pub context_lines: Option<usize>,
    /// Expressions for kind `expr`: `a == 0x00`, `mem[0x0400] >= 0xA0`, `pc != 0`.
    #[serde(default)]
    pub expect: Vec<String>,
    /// Kind `audio-events`: reference event file, relative to `tests/conformance/`. A case needs
    /// `events`, `pcm` or both.
    #[serde(default)]
    pub events: Option<String>,
    /// Kind `audio-events`: `exact` (default) or `ssi` (phoneme end timing, see `ssi_tolerance`).
    #[serde(default)]
    pub timing: Option<String>,
    /// Kind `audio-events`, timing `ssi`: tolerance in cycles of each phoneme duration.
    #[serde(default)]
    pub ssi_tolerance: Option<u64>,
    /// Kind `audio-events`: reference PCM (raw s16le), relative to `tests/conformance/`.
    #[serde(default)]
    pub pcm: Option<String>,
    #[serde(default)]
    pub pcm_limits: Option<PcmLimitsSpec>,
    /// Kind `audio-events`: an event line without its cycle (e.g. `IOW,4,43,80`, SSI263 CTL=1).
    /// After the last such event of the oracle's log the Rust PCM must be exactly silent
    /// (power-down silences the chip, GH#1412).
    #[serde(default)]
    pub silent_after: Option<String>,
    /// A finding (e.g. `AUD-P-020`) that explains why this case is expected to FAIL: the result
    /// stays FAIL in the manifest (never PASS), the id is added to `actual`, and the shipped-cases
    /// test accepts the failure. Remove it when the divergence is fixed.
    #[serde(default)]
    pub known_divergence: Option<String>,
}

impl Case {
    pub fn load(path: &Path) -> Result<Case, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let c: Case = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        c.validate()
            .map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(c)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.id.is_empty()
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
        {
            return Err(format!("id `{}` must be [A-Za-z0-9._-]+", self.id));
        }
        match (self.run.frames, self.run.cycles) {
            (Some(_), None) | (None, Some(_)) => {}
            _ => return Err("run: exactly one of frames/cycles".into()),
        }
        if self.run.repeat == 0 {
            return Err("run.repeat must be >= 1".into());
        }
        for ev in &self.input {
            ev.validate()?;
        }
        parse_model(&self.model)?;
        if let Some(v) = &self.video {
            v.validate()?;
            if self.oracle.kind != OracleKind::FramebufferHash {
                return Err("`video` only applies to framebuffer-hash cases".into());
            }
        }
        self.cpu_override()?;
        self.card_slots()?;
        self.disk2_slot()?;
        if let Some(n) = self.memclear {
            if !(-1..=7).contains(&n) {
                return Err(format!("memclear {n}: expected -1..=7"));
            }
        }
        let o = &self.oracle;
        match o.kind {
            OracleKind::Trace
                if o.trace.is_none()
                    && o.trace_rows_sha256.is_none()
                    && !(o.authority == Authority::Determinism && self.run.repeat >= 2) =>
            {
                return Err(
                    "oracle.trace required (or authority = determinism with run.repeat >= 2 for self-consistency)"
                        .into(),
                );
            }
            OracleKind::TraceCheckpoints if o.trace.is_none() => {
                return Err("oracle.trace required".into())
            }
            OracleKind::TraceCheckpoints if self.fixture.is_none() && self.hdd.is_none() => {
                return Err("fixture (the disk image) or hdd required".into())
            }
            OracleKind::Expr if o.expect.is_empty() => return Err("oracle.expect required".into()),
            OracleKind::Expr => {
                for e in &o.expect {
                    Expr::parse(e)?;
                }
            }
            OracleKind::AudioEvents => {
                if o.events.is_none() && o.pcm.is_none() {
                    return Err("oracle.events or oracle.pcm required".into());
                }
                if let Some(a) = &self.audio {
                    a.validate()?;
                }
                match o.timing.as_deref() {
                    None | Some("exact") => {}
                    Some("ssi") if o.ssi_tolerance.is_some() => {}
                    Some("ssi") => {
                        return Err("oracle.ssi_tolerance required with timing `ssi`".into())
                    }
                    Some(t) => return Err(format!("oracle.timing `{t}`: expected exact or ssi")),
                }
                if o.pcm.is_some() != o.pcm_limits.is_some() {
                    return Err("oracle.pcm and oracle.pcm_limits go together".into());
                }
            }
            OracleKind::Trace
            | OracleKind::TraceCheckpoints
            | OracleKind::TextHash
            | OracleKind::MemoryHash
            | OracleKind::FramebufferHash
            | OracleKind::StateHash => {}
        }
        if matches!(
            o.kind,
            OracleKind::TextHash
                | OracleKind::MemoryHash
                | OracleKind::FramebufferHash
                | OracleKind::StateHash
        ) && o.sha256.is_none()
            && self.run.repeat < 2
        {
            return Err("oracle.sha256 required (or run.repeat >= 2 for self-consistency)".into());
        }
        if let Some(h) = &o.sha256 {
            if !is_sha256_hex(h) {
                return Err(format!(
                    "oracle.sha256 `{h}` is not 64 lowercase hex digits"
                ));
            }
        }
        for (name, f) in self.devices() {
            if !is_sha256_hex(&f.sha256) {
                return Err(format!(
                    "{name}.sha256 `{}` is not 64 lowercase hex digits",
                    f.sha256
                ));
            }
        }
        if let Some(h) = &o.trace_sha256 {
            if !is_sha256_hex(h) {
                return Err(format!(
                    "oracle.trace_sha256 `{h}` is not 64 lowercase hex digits"
                ));
            }
        }
        crate::trace::parse_field_list(&o.ignore.join(","))?;
        Ok(())
    }

    /// Every disk image of the case with its role (`fixture`, `fixture2`, `hdd`, `hdd2`).
    pub fn devices(&self) -> Vec<(&'static str, &FixtureSpec)> {
        [
            ("fixture", &self.fixture),
            ("fixture2", &self.fixture2),
            ("hdd", &self.hdd),
            ("hdd2", &self.hdd2),
        ]
        .into_iter()
        .filter_map(|(n, f)| f.as_ref().map(|f| (n, f)))
        .collect()
    }

    pub fn model(&self) -> Model {
        parse_model(&self.model).expect("validated")
    }

    /// `Some(true)` for 65C02, `Some(false)` for 6502, `None` for the model's own.
    pub fn cpu_override(&self) -> Result<Option<bool>, String> {
        match self.cpu.as_deref().map(str::to_ascii_lowercase).as_deref() {
            None | Some("auto") => Ok(None),
            Some("6502") => Ok(Some(false)),
            Some("65c02") => Ok(Some(true)),
            Some(o) => Err(format!("cpu `{o}`: expected 6502, 65C02 or auto")),
        }
    }

    /// Slot of the Disk II card (`"6:disk2"`), if the case has one.
    pub fn disk2_slot(&self) -> Result<Option<u8>, String> {
        let mut found = None;
        for c in self.cards.iter().flatten() {
            if let Some((n, "disk2")) = c.split_once(':').map(|(n, k)| (n.trim(), k.trim())) {
                let n: u8 = n.parse().map_err(|_| format!("card `{c}`: bad slot"))?;
                if !(1..=7).contains(&n) {
                    return Err(format!("card `{c}`: Disk II slot must be 1-7"));
                }
                if found.replace(n).is_some() {
                    return Err("only one Disk II is supported".into());
                }
            }
        }
        Ok(found)
    }

    /// Parsed `cards`, `None` when the case keeps the model's default slots.
    pub fn card_slots(&self) -> Result<Option<[Option<CardKind>; 8]>, String> {
        let Some(list) = &self.cards else {
            return Ok(None);
        };
        let mut slots = [None; 8];
        for c in list {
            let (n, kind) = c
                .split_once(':')
                .ok_or_else(|| format!("card `{c}`: expected slot:kind"))?;
            let n: usize = n
                .trim()
                .parse()
                .map_err(|_| format!("card `{c}`: bad slot"))?;
            if n > 7 {
                return Err(format!("card `{c}`: slot must be 0-7"));
            }
            let kind = match kind.trim() {
                "disk2" => continue,
                "language-card-16k" => CardKind::LanguageCard16K,
                "saturn-128k" => CardKind::Saturn128K,
                k => return Err(format!("card kind `{k}` unknown")),
            };
            if slots[n].replace(kind).is_some() {
                return Err(format!("slot {n} used twice"));
            }
        }
        Ok(Some(slots))
    }
}

pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Model names used in cases and on the command line.
pub const MODEL_NAMES: [(&str, Model); 10] = [
    ("apple2", Model::Apple2),
    ("apple2plus", Model::Apple2Plus),
    ("apple2jplus", Model::Apple2JPlus),
    ("apple2e", Model::Apple2e),
    ("apple2e-enhanced", Model::Apple2eEnhanced),
    ("pravets82", Model::Pravets82),
    ("pravets8m", Model::Pravets8M),
    ("pravets8a", Model::Pravets8A),
    ("tk3000e", Model::Tk3000e),
    ("base64a", Model::Base64A),
];

pub fn parse_model(s: &str) -> Result<Model, String> {
    MODEL_NAMES
        .iter()
        .find(|(n, _)| *n == s)
        .map(|(_, m)| *m)
        .ok_or_else(|| {
            format!(
                "unknown model `{s}` (one of: {})",
                MODEL_NAMES.map(|(n, _)| n).join(", ")
            )
        })
}

pub fn model_name(m: Model) -> &'static str {
    MODEL_NAMES
        .iter()
        .find(|(_, x)| *x == m)
        .map_or("unknown", |(n, _)| n)
}

/// Value read by an [`Expr`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operand {
    Pc,
    A,
    X,
    Y,
    Sp,
    P,
    Cycle,
    /// `mem[addr]`: CPU view (`Machine::peek`).
    Mem(u16),
    /// `main[addr]`: main RAM, ignoring paging.
    Main(u16),
    /// `aux[addr]`: aux RAM.
    Aux(u16),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cmp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

/// `lhs op number`, e.g. `mem[0x0400] == 0xC1`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub text: String,
    pub lhs: Operand,
    pub cmp: Cmp,
    pub rhs: u64,
}

fn parse_num(s: &str) -> Result<u64, String> {
    let s = s.trim();
    let r = if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix('$')) {
        u64::from_str_radix(h, 16)
    } else {
        s.parse()
    };
    r.map_err(|_| format!("bad number `{s}`"))
}

impl Expr {
    pub fn parse(text: &str) -> Result<Expr, String> {
        let t = text.trim();
        let (pos, len, cmp) = [
            ("==", Cmp::Eq),
            ("!=", Cmp::Ne),
            ("<=", Cmp::Le),
            (">=", Cmp::Ge),
            ("<", Cmp::Lt),
            (">", Cmp::Gt),
        ]
        .into_iter()
        .filter_map(|(op, c)| t.find(op).map(|p| (p, op.len(), c)))
        .min_by_key(|(p, _, _)| *p)
        .ok_or_else(|| format!("expression `{t}`: no comparison operator"))?;
        let lhs = t[..pos].trim();
        let rhs = parse_num(&t[pos + len..]).map_err(|e| format!("expression `{t}`: {e}"))?;
        let operand = match lhs {
            "pc" => Operand::Pc,
            "a" => Operand::A,
            "x" => Operand::X,
            "y" => Operand::Y,
            "sp" => Operand::Sp,
            "p" => Operand::P,
            "cycle" => Operand::Cycle,
            _ => {
                let (name, rest) = lhs
                    .split_once('[')
                    .ok_or_else(|| format!("expression `{t}`: unknown operand `{lhs}`"))?;
                let addr = rest
                    .strip_suffix(']')
                    .ok_or_else(|| format!("expression `{t}`: missing `]`"))?;
                let addr = parse_num(addr).map_err(|e| format!("expression `{t}`: {e}"))?;
                let addr = u16::try_from(addr)
                    .map_err(|_| format!("expression `{t}`: address > $FFFF"))?;
                match name {
                    "mem" => Operand::Mem(addr),
                    "main" => Operand::Main(addr),
                    "aux" => Operand::Aux(addr),
                    _ => return Err(format!("expression `{t}`: unknown operand `{name}[]`")),
                }
            }
        };
        Ok(Expr {
            text: t.to_string(),
            lhs: operand,
            cmp,
            rhs,
        })
    }

    /// Returns `(holds, actual value)`.
    pub fn eval(&self, m: &apple2_core::Machine) -> (bool, u64) {
        let c = m.cpu();
        let v = match self.lhs {
            Operand::Pc => c.pc as u64,
            Operand::A => c.a as u64,
            Operand::X => c.x as u64,
            Operand::Y => c.y as u64,
            Operand::Sp => c.sp as u64,
            Operand::P => c.p as u64,
            Operand::Cycle => m.cycles(),
            Operand::Mem(a) => m.peek(a) as u64,
            Operand::Main(a) => m.peek_main(a) as u64,
            Operand::Aux(a) => m.peek_aux(a) as u64,
        };
        let ok = match self.cmp {
            Cmp::Eq => v == self.rhs,
            Cmp::Ne => v != self.rhs,
            Cmp::Lt => v < self.rhs,
            Cmp::Le => v <= self.rhs,
            Cmp::Gt => v > self.rhs,
            Cmp::Ge => v >= self.rhs,
        };
        (ok, v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"
id = "x-1"
story = "HU-0101"
criterion = "CA-1"
model = "apple2plus"
cpu = "6502"
cards = ["0:language-card-16k"]

[run]
frames = 3

[[input]]
at_frame = 1
text = "A\r"

[oracle]
kind = "text-hash"
authority = "regression"
source = "test"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
"#;

    #[test]
    fn parses_a_full_case() {
        let c: Case = toml::from_str(GOOD).unwrap();
        c.validate().unwrap();
        assert_eq!(c.model(), Model::Apple2Plus);
        assert_eq!(c.cpu_override().unwrap(), Some(false));
        assert_eq!(
            c.card_slots().unwrap().unwrap()[0],
            Some(CardKind::LanguageCard16K)
        );
        assert_eq!(c.run.repeat, 1);
        assert_eq!(c.oracle.authority, Authority::Regression);
    }

    #[test]
    fn rejects_bad_cases() {
        let bad = |from: &str, to: &str| {
            let c: Case = toml::from_str(&GOOD.replace(from, to)).unwrap();
            c.validate().unwrap_err()
        };
        assert!(bad("frames = 3", "frames = 3\ncycles = 4").contains("exactly one"));
        assert!(bad("apple2plus", "nope").contains("unknown model"));
        assert!(bad("6502", "z80").contains("cpu"));
        assert!(bad("0:language-card-16k", "9:language-card-16k").contains("0-7"));
        assert!(bad("at_frame = 1", "at_frame = 1\nat_cycle = 1").contains("at_frame"));
        assert!(bad("x-1", "x 1").contains("id"));
        assert!(bad(
            "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"",
            ""
        )
        .contains("sha256 required"));
        assert!(toml::from_str::<Case>(&GOOD.replace("[run]", "[run]\nbogus = 1")).is_err());
    }

    #[test]
    fn trace_without_reference_needs_a_determinism_case_with_repeats() {
        let trace =
            |authority: &str, repeat: u32| {
                let t = GOOD
                .replace("kind = \"text-hash\"", "kind = \"trace\"")
                .replace("authority = \"regression\"", &format!("authority = \"{authority}\""))
                .replace(
                    "sha256 = \"0000000000000000000000000000000000000000000000000000000000000000\"",
                    "",
                )
                .replace("frames = 3", &format!("frames = 3\nrepeat = {repeat}"));
                toml::from_str::<Case>(&t).unwrap().validate()
            };
        assert!(trace("determinism", 10).is_ok());
        assert!(trace("determinism", 1)
            .unwrap_err()
            .contains("oracle.trace"));
        assert!(trace("regression", 10)
            .unwrap_err()
            .contains("oracle.trace"));
    }

    #[test]
    fn expression_parsing() {
        let e = Expr::parse("mem[0x0400] >= $A0").unwrap();
        assert_eq!(e.lhs, Operand::Mem(0x400));
        assert_eq!(e.cmp, Cmp::Ge);
        assert_eq!(e.rhs, 0xA0);
        assert_eq!(Expr::parse("pc == 64098").unwrap().rhs, 64098);
        assert!(Expr::parse("pc").is_err());
        assert!(Expr::parse("zz == 1").is_err());
        assert!(Expr::parse("mem[0x10000] == 1").is_err());
        assert!(Expr::parse("mem[1 == 1").is_err());
    }

    #[test]
    fn expression_evaluation_on_a_machine() {
        use apple2_core::MachineConfig;
        let m = apple2_core::Machine::new(MachineConfig::for_model(Model::Apple2Plus));
        let reset = m.cpu().pc as u64;
        assert!(Expr::parse(&format!("pc == {reset}")).unwrap().eval(&m).0);
        let (ok, v) = Expr::parse("pc == 1").unwrap().eval(&m);
        assert!(!ok);
        assert_eq!(v, reset);
        assert!(Expr::parse("cycle == 0").unwrap().eval(&m).0);
    }

    #[test]
    fn cli_input_events() {
        let e = InputEvent::parse_cli("frame:30:PRINT 1\\r").unwrap();
        assert_eq!(e.at_frame, Some(30));
        assert_eq!(e.text.as_deref(), Some("PRINT 1\r"));
        assert_eq!(e.describe(), "frame:30:text=PRINT 1\\r");
        let c = InputEvent::parse_cli("cycle:99:\\x41:b").unwrap();
        assert_eq!(c.at_cycle, Some(99));
        assert_eq!(c.text.as_deref(), Some("A:b"));
        assert!(InputEvent::parse_cli("frame:x:a").is_err());
        assert!(InputEvent::parse_cli("tick:1:a").is_err());
        assert!(InputEvent::parse_cli("frame:1").is_err());
    }

    #[test]
    fn model_names_round_trip() {
        for (n, m) in MODEL_NAMES {
            assert_eq!(parse_model(n).unwrap(), m);
            assert_eq!(model_name(m), n);
        }
    }
}
