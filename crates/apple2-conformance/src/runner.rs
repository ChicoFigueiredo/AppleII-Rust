//! Runs conformance cases against the Rust machine and fills in manifests.

use crate::case::{Authority, Case, Expr, InputEvent, OracleKind, RunSpec};
use crate::manifest::{now_utc, Manifest, TestResult};
use crate::sha256::sha256_hex;
use crate::trace::{
    compare_with, parse_field_list, CompareOptions, Trace, TraceReader, TraceWriter,
};
use apple2_core::text_screen::text_screen;
use apple2_core::{CardKind, Machine, MachineConfig};
use apple2_video::{RenderConfig, Renderer};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

/// Where a run happens and what identifies it.
#[derive(Clone, Debug)]
pub struct Env {
    /// `tests/conformance`: cases, manifests and reference traces live here.
    pub root: PathBuf,
    /// Workspace root; fixture paths are relative to it.
    pub workspace: PathBuf,
    pub rust_sha: String,
    pub toolchain: String,
}

impl Env {
    /// `root` defaults to this workspace's `tests/conformance`.
    pub fn detect(root: Option<&Path>) -> Env {
        let default_ws = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let root = root.map_or_else(|| default_ws.join("tests/conformance"), Path::to_path_buf);
        let workspace = root.join("../..");
        Env {
            rust_sha: git_sha(&workspace),
            toolchain: command_line(Command::new("rustc").arg("--version"))
                .unwrap_or_else(|| "unknown".into()),
            root,
            workspace,
        }
    }
}

fn command_line(cmd: &mut Command) -> Option<String> {
    let out = cmd.output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then_some(s)
}

fn git_sha(dir: &Path) -> String {
    if let Ok(v) = std::env::var("CONFORMANCE_RUST_SHA") {
        return v;
    }
    let Some(sha) = command_line(
        Command::new("git")
            .current_dir(dir)
            .args(["rev-parse", "HEAD"]),
    ) else {
        return "unknown".into();
    };
    let dirty = command_line(Command::new("git").current_dir(dir).args([
        "status",
        "--porcelain",
        "--untracked-files=no",
    ]))
    .is_some();
    if dirty {
        format!("{sha}+dirty")
    } else {
        sha
    }
}

/// One finished case.
#[derive(Clone, Debug)]
pub struct Outcome {
    pub manifest: Manifest,
    /// `(file name, bytes)` written next to the manifest (diff artifacts, actual dumps).
    pub artifacts: Vec<(String, Vec<u8>)>,
    pub seconds: f64,
}

/// Builds the machine configuration of a case.
pub fn machine_config(case: &Case) -> Result<MachineConfig, String> {
    let mut cfg = MachineConfig::for_model(case.model());
    cfg.cpu_cmos = case.cpu_override()?;
    if let Some(slots) = case.card_slots()? {
        cfg.slots = slots;
    }
    Ok(cfg)
}

pub fn cards_description(cfg: &MachineConfig) -> String {
    let v: Vec<String> = cfg
        .slots
        .iter()
        .enumerate()
        .filter_map(|(i, c)| {
            c.map(|k| {
                let k = match k {
                    CardKind::LanguageCard16K => "LanguageCard16K",
                    CardKind::Saturn128K => "Saturn128K",
                };
                format!("{i}={k}")
            })
        })
        .collect();
    if v.is_empty() {
        "none".into()
    } else {
        v.join(",")
    }
}

fn fire(m: &mut Machine, ev: &InputEvent) -> Result<(), String> {
    if let Some(t) = &ev.text {
        if !m.paste(t) {
            return Err(format!("paste refused at cycle {}", m.cycles()));
        }
    }
    if let Some(k) = ev.key {
        if !m.key_press(k) {
            return Err(format!("key {k:#04x} refused at cycle {}", m.cycles()));
        }
    }
    Ok(())
}

/// What `drive` must also do while running.
#[derive(Default)]
pub struct Hooks<'a> {
    /// Records every instruction before it runs.
    pub trace: Option<&'a mut TraceWriter<Vec<u8>>>,
    /// Renders every frame (frame-limited runs with frame-timed input only).
    pub renderer: Option<&'a mut Renderer>,
}

/// Runs `m` from its current state for `run`, firing `events` when their frame or cycle is
/// reached (before the instruction that follows).
pub fn drive(
    m: &mut Machine,
    run: &RunSpec,
    events: &[InputEvent],
    mut hooks: Hooks<'_>,
) -> Result<(), String> {
    let mut pending: Vec<&InputEvent> = events.iter().collect();
    pending.sort_by_key(|e| (e.at_frame.is_none(), e.at_frame, e.at_cycle));
    let mut pending = std::collections::VecDeque::from(pending);
    let due = |m: &Machine, e: &InputEvent| match (e.at_frame, e.at_cycle) {
        (Some(f), _) => m.frames() >= f,
        (_, Some(c)) => m.cycles() >= c,
        _ => true,
    };
    let finished = |m: &Machine| match (run.frames, run.cycles) {
        (Some(f), _) => m.frames() >= f,
        (_, Some(c)) => m.cycles() >= c,
        _ => true,
    };
    // Frame-timed events sort first and cycle-timed ones after; either kind may be due earlier
    // than the head of the queue, so scan the whole queue.
    let mut fire_due = |m: &mut Machine| -> Result<(), String> {
        let mut i = 0;
        while i < pending.len() {
            if due(m, pending[i]) {
                let ev = pending.remove(i).unwrap();
                fire(m, ev)?;
            } else {
                i += 1;
            }
        }
        Ok(())
    };
    let cycle_timed = events.iter().any(|e| e.at_cycle.is_some());
    let by_frame = run.frames.is_some() && !cycle_timed && hooks.trace.is_none();
    if by_frame {
        if let Some(r) = hooks.renderer.as_deref_mut() {
            while !finished(m) {
                fire_due(m)?;
                r.run_frame(m);
            }
        } else {
            while !finished(m) {
                fire_due(m)?;
                m.run_frame();
            }
        }
        fire_due(m)?;
        return Ok(());
    }
    if hooks.renderer.is_some() {
        return Err("framebuffer cases support only run.frames with input at_frame".into());
    }
    loop {
        fire_due(m)?;
        if finished(m) {
            return Ok(());
        }
        match hooks.trace.as_deref_mut() {
            Some(t) => {
                t.step(m).map_err(|e| e.to_string())?;
            }
            None => {
                m.step();
            }
        }
    }
}

/// Runs a fresh machine and returns the trace v1 bytes (what `conformance trace` writes).
pub fn generate_trace(
    cfg: MachineConfig,
    run: &RunSpec,
    events: &[InputEvent],
    meta: &[(&str, &str)],
) -> Result<Vec<u8>, String> {
    let mut m = Machine::try_new(cfg).map_err(|e| e.to_string())?;
    let mut w = TraceWriter::new(Vec::new(), meta).map_err(|e| e.to_string())?;
    drive(
        &mut m,
        run,
        events,
        Hooks {
            trace: Some(&mut w),
            renderer: None,
        },
    )?;
    w.finish().map_err(|e| e.to_string())
}

/// Loads every `*.toml` in `dir`, sorted by file name; duplicate ids are an error.
pub fn load_cases(dir: &Path) -> Result<Vec<Case>, String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();
    let mut cases: Vec<Case> = Vec::new();
    for p in paths {
        let c = Case::load(&p)?;
        if cases.iter().any(|o| o.id == c.id) {
            return Err(format!("{}: duplicate case id `{}`", p.display(), c.id));
        }
        cases.push(c);
    }
    Ok(cases)
}

/// File SHA-256, or the reason it could not be computed.
pub fn file_sha256(path: &Path) -> Result<String, String> {
    std::fs::read(path)
        .map(|b| sha256_hex(&b))
        .map_err(|e| format!("{}: {e}", path.display()))
}

struct Observed {
    /// The value compared with the oracle (hash, or trace hash for trace cases).
    value: String,
    machine: Machine,
    renderer: Option<Renderer>,
    trace: Option<Vec<u8>>,
}

fn observe_once(case: &Case, env_meta: &[(String, String)]) -> Result<Observed, String> {
    let cfg = machine_config(case)?;
    let mut m = Machine::try_new(cfg).map_err(|e| e.to_string())?;
    let kind = case.oracle.kind;
    match kind {
        OracleKind::Trace => {
            let meta: Vec<(&str, &str)> = env_meta
                .iter()
                .map(|(k, v)| (k.as_str(), v.as_str()))
                .collect();
            let mut w = TraceWriter::new(Vec::new(), &meta).map_err(|e| e.to_string())?;
            drive(
                &mut m,
                &case.run,
                &case.input,
                Hooks {
                    trace: Some(&mut w),
                    renderer: None,
                },
            )?;
            let bytes = w.finish().map_err(|e| e.to_string())?;
            Ok(Observed {
                value: sha256_hex(&bytes),
                machine: m,
                renderer: None,
                trace: Some(bytes),
            })
        }
        OracleKind::FramebufferHash => {
            let mut r = Renderer::new(RenderConfig::default());
            r.resync(m.cycles_in_frame());
            drive(
                &mut m,
                &case.run,
                &case.input,
                Hooks {
                    trace: None,
                    renderer: Some(&mut r),
                },
            )?;
            Ok(Observed {
                value: sha256_hex(&r.rgba8()),
                machine: m,
                renderer: Some(r),
                trace: None,
            })
        }
        _ => {
            drive(&mut m, &case.run, &case.input, Hooks::default())?;
            let value = match kind {
                OracleKind::TextHash => sha256_hex(text_dump(&m).as_bytes()),
                OracleKind::MemoryHash => sha256_hex(&memory_dump(&m)),
                OracleKind::StateHash => sha256_hex(&state_dump(&m)),
                OracleKind::Expr => String::new(),
                OracleKind::Trace | OracleKind::FramebufferHash => unreachable!(),
            };
            Ok(Observed {
                value,
                machine: m,
                renderer: None,
                trace: None,
            })
        }
    }
}

/// The 24 text rows joined with `\n` (what `text-hash` hashes).
pub fn text_dump(m: &Machine) -> String {
    text_screen(m).join("\n")
}

/// Main RAM then aux RAM (what `memory-hash` hashes).
pub fn memory_dump(m: &Machine) -> Vec<u8> {
    (0..=0xFFFFu16)
        .map(|a| m.peek_main(a))
        .chain((0..=0xFFFFu16).map(|a| m.peek_aux(a)))
        .collect()
}

/// JSON of the machine state (what `state-hash` hashes).
pub fn state_dump(m: &Machine) -> Vec<u8> {
    serde_json::to_vec(&m.save_state()).expect("machine state serializes")
}

fn png_bytes(r: &Renderer) -> Result<Vec<u8>, String> {
    let (_, _, w, h) = r.visible_rect();
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(
            &r.rgba8(),
            w as u32,
            h as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    Ok(out)
}

use image::ImageEncoder;

/// Runs one case and returns its manifest and artifacts. Never panics on case problems: they
/// become BLOCKED (cannot run) or FAIL (ran and disagreed or errored).
pub fn run_case(env: &Env, case: &Case) -> Outcome {
    let started = Instant::now();
    let cfg = machine_config(case);
    let rom = cfg.as_ref().ok().and_then(|c| match &c.custom_rom {
        Some(r) => Some(sha256_hex(r)),
        None => c.model.system_rom().map(sha256_hex),
    });
    let o = &case.oracle;
    let mut mf = Manifest {
        test_id: case.id.clone(),
        user_story: case.story.clone(),
        acceptance_criterion: case.criterion.clone(),
        applewin_sha: (o.authority == Authority::Applewin)
            .then(|| std::env::var("CONFORMANCE_APPLEWIN_SHA").ok())
            .flatten(),
        rust_sha: env.rust_sha.clone(),
        fixture_sha256: case.fixture.as_ref().map(|f| f.sha256.clone()),
        rom_sha256: rom,
        machine_model: case.model.clone(),
        cpu_model: match &cfg {
            Ok(c) if c.is_cmos() => "65C02".into(),
            Ok(_) => "6502".into(),
            Err(_) => "unknown".into(),
        },
        cards_slots: cfg
            .as_ref()
            .map_or_else(|_| "unknown".into(), cards_description),
        host_os: std::env::consts::OS.into(),
        host_arch: std::env::consts::ARCH.into(),
        toolchain: env.toolchain.clone(),
        input_trace: (!case.input.is_empty()).then(|| {
            case.input
                .iter()
                .map(InputEvent::describe)
                .collect::<Vec<_>>()
                .join("; ")
        }),
        command: format!("conformance run --case {}", case.id),
        expected: expected_text(case),
        actual: String::new(),
        diff_artifact: None,
        result: TestResult::Unverified,
        timestamp_utc: now_utc(),
        reviewer: None,
    };
    let mut artifacts: Vec<(String, Vec<u8>)> = Vec::new();

    let verdict = evaluate(env, case, cfg, &mf, &mut artifacts);
    match verdict {
        Verdict::Pass(actual) => {
            mf.result = TestResult::Pass;
            mf.actual = actual;
        }
        Verdict::Fail(actual, diff) => {
            mf.result = TestResult::Fail;
            mf.actual = actual;
            mf.diff_artifact = diff;
        }
        Verdict::Blocked(cause) => {
            mf.result = TestResult::Blocked;
            mf.actual = format!("BLOCKED: {cause}");
        }
    }
    Outcome {
        manifest: mf,
        artifacts,
        seconds: started.elapsed().as_secs_f64(),
    }
}

fn expected_text(case: &Case) -> String {
    let o = &case.oracle;
    let what = match o.kind {
        OracleKind::Trace => format!(
            "trace {} (ignora: {})",
            o.trace.as_deref().unwrap_or("?"),
            if o.ignore.is_empty() {
                "nada".into()
            } else {
                o.ignore.join(",")
            }
        ),
        OracleKind::Expr => o.expect.join(" && "),
        k => {
            let name = match k {
                OracleKind::TextHash => "text-hash",
                OracleKind::MemoryHash => "memory-hash",
                OracleKind::FramebufferHash => "framebuffer-hash",
                _ => "state-hash",
            };
            match &o.sha256 {
                Some(h) => format!("{name} sha256={h}"),
                None => format!("{name}: {} execuções idênticas", case.run.repeat),
            }
        }
    };
    let warn = match o.authority {
        Authority::Regression => {
            " [REGRESSÃO: saída anterior da própria Máquina Rust, NÃO é oráculo do AppleWin]"
        }
        Authority::Determinism => " [DETERMINISMO: compara execuções entre si]",
        _ => "",
    };
    format!(
        "{what}; oráculo={} ({}){warn}",
        o.authority.as_str(),
        o.source
    )
}

enum Verdict {
    Pass(String),
    /// Actual text and the diff artifact file name, if any.
    Fail(String, Option<String>),
    Blocked(String),
}

fn evaluate(
    env: &Env,
    case: &Case,
    cfg: Result<MachineConfig, String>,
    mf: &Manifest,
    artifacts: &mut Vec<(String, Vec<u8>)>,
) -> Verdict {
    let cfg = match cfg {
        Ok(c) => c,
        Err(e) => return Verdict::Blocked(format!("configuração inválida: {e}")),
    };
    if mf.rom_sha256.is_none() {
        return Verdict::Blocked(format!(
            "o modelo {} não tem ROM de sistema embutida",
            case.model
        ));
    }
    if let Some(fx) = &case.fixture {
        let p = env.workspace.join(&fx.path);
        match file_sha256(&p) {
            Err(_) => {
                return Verdict::Blocked(format!(
                    "fixture ausente: {} (esperado no cache de fixtures; veja docs/conformance)",
                    fx.path
                ))
            }
            Ok(h) if h != fx.sha256 => {
                return Verdict::Blocked(format!(
                    "fixture {} com sha256 {h}, esperado {}",
                    fx.path, fx.sha256
                ))
            }
            Ok(_) => {
                return Verdict::Blocked(format!(
                    "fixture {} presente e íntegra, mas o harness v1 ainda não carrega fixtures na Máquina",
                    fx.path
                ))
            }
        }
    }
    let reference = match (&case.oracle.kind, &case.oracle.trace) {
        (OracleKind::Trace, Some(rel)) => {
            let p = env.root.join(rel);
            match TraceReader::read_file(&p) {
                Ok(t) => Some(t),
                Err(e) if !p.exists() => {
                    return Verdict::Blocked(format!("trace de referência ausente: {rel} ({e})"))
                }
                Err(e) => return Verdict::Blocked(format!("trace de referência ilegível: {e}")),
            }
        }
        _ => None,
    };
    let ignore = match parse_field_list(&case.oracle.ignore.join(",")) {
        Ok(f) => f,
        Err(e) => return Verdict::Blocked(e),
    };
    let meta = vec![
        ("source".to_string(), "apple2-rust".to_string()),
        ("case".to_string(), case.id.clone()),
        ("model".to_string(), case.model.clone()),
        ("cpu".to_string(), mf.cpu_model.clone()),
        (
            "rom_sha256".to_string(),
            mf.rom_sha256.clone().unwrap_or_default(),
        ),
        ("cards".to_string(), mf.cards_slots.clone()),
    ];
    let _ = &cfg;

    let mut first: Option<Observed> = None;
    let mut last: Option<Observed> = None;
    for run in 1..=case.run.repeat {
        let obs = match observe_once(case, &meta) {
            Ok(o) => o,
            Err(e) => return Verdict::Fail(format!("erro na execução {run}: {e}"), None),
        };
        match &first {
            None => first = Some(obs),
            Some(f) => {
                if obs.value != f.value {
                    let msg = format!(
                        "não determinístico: execução {run} deu {} e a primeira {}",
                        obs.value, f.value
                    );
                    let diff = dump_actual(case, &obs, artifacts);
                    return Verdict::Fail(msg, diff);
                }
                last = Some(obs);
            }
        }
    }
    let obs = last.or(first).expect("repeat >= 1");
    let runs = case.run.repeat;
    let rep = if runs > 1 {
        format!(" ({runs} execuções idênticas)")
    } else {
        String::new()
    };

    match case.oracle.kind {
        OracleKind::Expr => {
            let mut bad = Vec::new();
            for e in &case.oracle.expect {
                let ex = Expr::parse(e).expect("validated");
                let (ok, v) = ex.eval(&obs.machine);
                if !ok {
                    bad.push(format!("{e} (valor {v:#x})"));
                }
            }
            if bad.is_empty() {
                Verdict::Pass(format!(
                    "todas as {} expressões valem{rep}",
                    case.oracle.expect.len()
                ))
            } else {
                let diff = dump_actual(case, &obs, artifacts);
                Verdict::Fail(format!("falharam: {}", bad.join("; ")), diff)
            }
        }
        OracleKind::Trace => {
            let reference = reference.expect("loaded");
            let actual_bytes = obs.trace.expect("trace case");
            let actual = TraceReader::read(&actual_bytes[..]).expect("own trace parses");
            let opts = CompareOptions {
                context_lines: case.oracle.context_lines.unwrap_or(8),
                ignore,
            };
            match compare_with(&reference, &actual, &opts) {
                None => Verdict::Pass(format!(
                    "{} instruções idênticas à referência{rep}",
                    actual.rows.len()
                )),
                Some(d) => {
                    let name = format!("{}.trace-diff.txt", case.id);
                    let mut text = d.report();
                    text.push_str(&format!(
                        "\nreferência: {}\nlinhas: esperado {:?}, obtido {:?}\n",
                        case.oracle.trace.as_deref().unwrap_or("?"),
                        d.expected_line,
                        d.actual_line
                    ));
                    artifacts.push((name.clone(), text.into_bytes()));
                    artifacts.push((format!("{}.actual.csv", case.id), actual_bytes));
                    Verdict::Fail(d.summary(), Some(name))
                }
            }
        }
        _ => match &case.oracle.sha256 {
            Some(h) if *h != obs.value => {
                let diff = dump_actual(case, &obs, artifacts);
                Verdict::Fail(format!("sha256 obtido {} (esperado {h})", obs.value), diff)
            }
            _ => Verdict::Pass(format!("sha256={}{rep}", obs.value)),
        },
    }
}

/// Saves the observed state for a failing case; returns the diff artifact name.
fn dump_actual(
    case: &Case,
    obs: &Observed,
    artifacts: &mut Vec<(String, Vec<u8>)>,
) -> Option<String> {
    let (name, bytes) = match case.oracle.kind {
        OracleKind::TextHash => (
            format!("{}.actual.txt", case.id),
            text_dump(&obs.machine).into_bytes(),
        ),
        OracleKind::MemoryHash | OracleKind::Expr => {
            (format!("{}.actual.bin", case.id), memory_dump(&obs.machine))
        }
        OracleKind::StateHash => (format!("{}.actual.json", case.id), state_dump(&obs.machine)),
        OracleKind::FramebufferHash => {
            let r = obs.renderer.as_ref()?;
            (format!("{}.actual.png", case.id), png_bytes(r).ok()?)
        }
        OracleKind::Trace => return None,
    };
    artifacts.push((name.clone(), bytes));
    Some(name)
}

/// Re-exported for the CLI: loads a trace file.
pub fn read_trace(path: &Path) -> Result<Trace, String> {
    TraceReader::read_file(path).map_err(|e| format!("{}: {e}", path.display()))
}
