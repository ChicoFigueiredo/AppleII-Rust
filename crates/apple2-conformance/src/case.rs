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
    /// Cards as `"slot:kind"`, e.g. `"0:language-card-16k"`. When present it replaces the
    /// model's default slots; `[]` means no cards.
    #[serde(default)]
    pub cards: Option<Vec<String>>,
    #[serde(default)]
    pub fixture: Option<FixtureSpec>,
    pub run: RunSpec,
    #[serde(default)]
    pub input: Vec<InputEvent>,
    /// Sound card and 6502 program of `audio-events` cases.
    #[serde(default)]
    pub audio: Option<AudioSpec>,
    pub oracle: OracleSpec,
    /// Free text kept in the report.
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureSpec {
    /// Relative to the workspace root (e.g. `target/conformance-cache/applewin-test/...`).
    pub path: String,
    /// Expected SHA-256 (lowercase hex).
    pub sha256: String,
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
    /// Reference trace, relative to `tests/conformance/` (kind `trace`).
    #[serde(default)]
    pub trace: Option<String>,
    /// Trace fields not compared (e.g. `["cycle"]`).
    #[serde(default)]
    pub ignore: Vec<String>,
    /// Rows of context kept in the diff artifact (default 8).
    #[serde(default)]
    pub context_lines: Option<usize>,
    /// Expressions for kind `expr`: `a == 0x00`, `mem[0x0400] >= 0xA0`, `pc != 0`.
    #[serde(default)]
    pub expect: Vec<String>,
    /// Kind `audio-events`: reference event file, relative to `tests/conformance/`.
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
        self.cpu_override()?;
        self.card_slots()?;
        let o = &self.oracle;
        match o.kind {
            OracleKind::Trace if o.trace.is_none() => return Err("oracle.trace required".into()),
            OracleKind::Expr if o.expect.is_empty() => return Err("oracle.expect required".into()),
            OracleKind::Expr => {
                for e in &o.expect {
                    Expr::parse(e)?;
                }
            }
            OracleKind::AudioEvents => {
                if o.events.is_none() {
                    return Err("oracle.events required".into());
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
        if let Some(f) = &self.fixture {
            if !is_sha256_hex(&f.sha256) {
                return Err(format!(
                    "fixture.sha256 `{}` is not 64 lowercase hex digits",
                    f.sha256
                ));
            }
        }
        crate::trace::parse_field_list(&o.ignore.join(","))?;
        Ok(())
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
