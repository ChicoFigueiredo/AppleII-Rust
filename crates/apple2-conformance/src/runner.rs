//! Runs conformance cases against the Rust machine and fills in manifests.

use crate::case::{Authority, Case, Expr, InputEvent, OracleKind, RunSpec};
use crate::manifest::{now_utc, Manifest, TestResult};
use crate::scene::Scene;
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

impl Env {
    /// Where a reference file lives. Paths into `target/conformance-cache/` are not versioned and
    /// a worktree has its own `target/`: when the file is not in this workspace, the cache of the
    /// main checkout (the parent of the git common dir, where the oracle scripts put it) is tried;
    /// `CONFORMANCE_CACHE` (the cache directory itself) takes precedence over both.
    pub fn resolve_reference(&self, rel: &str) -> PathBuf {
        const CACHE: &str = "target/conformance-cache/";
        let local = self.root.join(rel);
        let Some(at) = rel.find(CACHE) else {
            return local;
        };
        let tail = &rel[at + CACHE.len()..];
        if let Ok(dir) = std::env::var("CONFORMANCE_CACHE") {
            return Path::new(&dir).join(tail);
        }
        if local.exists() {
            return local;
        }
        command_line(Command::new("git").current_dir(&self.workspace).args([
            "rev-parse",
            "--path-format=absolute",
            "--git-common-dir",
        ]))
        .and_then(|common| {
            let p = Path::new(&common).parent()?.join(CACHE).join(tail);
            p.exists().then_some(p)
        })
        .unwrap_or(local)
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
    if let Some(v) = &case.video {
        cfg.video_standard = v.standard()?;
    }
    cfg.ram_init = apple2_core::RamInit::APPLEWIN_DETERMINISTIC;
    if let Some(n) = case.memclear {
        cfg.ram_init.pattern = n;
    }
    if let Some(slots) = case.card_slots()? {
        cfg.slots = slots;
    }
    Ok(cfg)
}

/// A Disk II in `slot`, drive 1 loaded from `image` (opened write-protected: a trace never
/// modifies the file).
#[derive(Clone, Debug)]
pub struct Disk2Setup {
    pub slot: u8,
    pub image: Option<PathBuf>,
}

/// Builds the machine of `cfg` and installs the Disk II, if any.
pub fn build_machine(cfg: MachineConfig, disk2: Option<&Disk2Setup>) -> Result<Machine, String> {
    let mut m = Machine::try_new(cfg).map_err(|e| e.to_string())?;
    if let Some(d) = disk2 {
        let mut card = apple2_storage::disk2::Disk2Card::new();
        if let Some(p) = &d.image {
            card.insert_path(0, p, true)
                .map_err(|e| format!("{}: {e:?}", p.display()))?;
        }
        if !m.insert_card(d.slot, Box::new(card)) {
            return Err(format!("slot {} refused the Disk II", d.slot));
        }
        // The card is installed after power-on: redo it so it sees the reset.
        m.power_cycle();
    }
    Ok(m)
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
    disk2: Option<&Disk2Setup>,
) -> Result<Vec<u8>, String> {
    let m = build_machine(cfg, disk2)?;
    generate_trace_from(m, run, events, meta)
}

/// Like [`generate_trace`] from a machine already built (for example imported from a save-state).
/// `run.cycles` is an absolute cycle count, as in [`drive`].
pub fn generate_trace_from(
    mut m: Machine,
    run: &RunSpec,
    events: &[InputEvent],
    meta: &[(&str, &str)],
) -> Result<Vec<u8>, String> {
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

/// Puts a VidHD in `slot` and power-cycles, so the card starts in its power-on state.
fn install_vidhd(m: &mut Machine, slot: u8) -> Result<(), String> {
    if !m.insert_card(slot, Box::new(apple2_cards::VidHd::new())) {
        return Err(format!("slot {slot} cannot take a VidHD"));
    }
    m.power_cycle();
    Ok(())
}

/// `Renderer::run_frame` for a machine with a VidHD, which the renderer sees through `WithVidHd`.
fn render_frames_vidhd(
    m: &mut Machine,
    r: &mut Renderer,
    slot: u8,
    frames: u64,
) -> Result<(), String> {
    use apple2_cards::VidHd;
    use apple2_video::WithVidHd;
    fn view(m: &Machine, slot: u8) -> Result<WithVidHd<'_>, String> {
        let hd = m.card::<VidHd>(slot).ok_or("the VidHD vanished")?;
        Ok(WithVidHd {
            source: m,
            vidhd: hd,
        })
    }
    r.init(&view(m, slot)?);
    while m.frames() < frames {
        r.resync(m.cycles_in_frame());
        let target = m.frames() + 1;
        while m.frames() < target {
            let c = m.step();
            r.update(&view(m, slot)?, c);
        }
    }
    Ok(())
}

/// The renderer options of a case (`[video]`, defaults: colour TV, NTSC, Video-7 SL7).
pub fn render_config(case: &Case) -> Result<RenderConfig, String> {
    let v = case.video.clone().unwrap_or_default();
    Ok(RenderConfig {
        video_type: v.video_type()?,
        rgb_card: v.rgb_card()?,
        ..RenderConfig::default()
    })
}

/// RGBA8 bytes of the visible area, or of the whole framebuffer with its borders.
pub fn framebuffer_rgba(r: &Renderer, full: bool) -> Vec<u8> {
    if !full {
        return r.rgba8();
    }
    let mut out = Vec::with_capacity(r.pixels().len() * 4);
    for &c in r.pixels() {
        out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]);
    }
    out
}

/// The machine a case starts from: power-on, or imported from its save-state fixture. The run is
/// counted from the start state, so a state case's `cycles` become absolute here.
fn start_machine(
    env: &Env,
    case: &Case,
    disk2: Option<&Disk2Setup>,
) -> Result<(Machine, RunSpec), String> {
    let mut run = case.run.clone();
    match case.fixture.as_ref().filter(|f| f.save_state) {
        None => {
            let cfg = machine_config(case)?;
            Ok((build_machine(cfg, disk2)?, run))
        }
        Some(fx) => {
            let dirs = fixture_dirs(env, fx);
            let imp = crate::state::import_file(&env.workspace.join(&fx.path), &dirs)?;
            if !imp.missing.is_empty() {
                return Err(format!("imagens ausentes: {}", imp.missing.join(", ")));
            }
            if run.frames.is_some() {
                return Err("um caso com save-state conta em run.cycles, não em frames".into());
            }
            run.cycles = run.cycles.map(|c| c + imp.machine.cycles());
            Ok((imp.machine, run))
        }
    }
}

fn fixture_dirs(env: &Env, fx: &crate::case::FixtureSpec) -> Vec<PathBuf> {
    fx.image_dirs
        .iter()
        .map(|d| env.workspace.join(d))
        .collect()
}

/// SHA-256 of the data rows of a trace given as text (header and `#` lines skipped).
pub fn trace_rows_sha256(text: &str) -> String {
    let mut rows = String::new();
    for line in text.lines() {
        if line.starts_with('#') || line.starts_with("cycle,") || line.is_empty() {
            continue;
        }
        rows.push_str(line);
        rows.push('\n');
    }
    sha256_hex(rows.as_bytes())
}

fn observe_once(
    env: &Env,
    case: &Case,
    env_meta: &[(String, String)],
    disk2: Option<&Disk2Setup>,
    scene: Option<&Scene>,
) -> Result<Observed, String> {
    let (mut m, run) = start_machine(env, case, disk2)?;
    let vidhd = case.video.as_ref().and_then(|v| v.vidhd_slot);
    if let Some(slot) = vidhd {
        install_vidhd(&mut m, slot)?;
    }
    if let Some(sc) = scene {
        sc.apply(&mut m);
    }
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
                &run,
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
            let mut r = Renderer::new(render_config(case)?);
            r.resync(m.cycles_in_frame());
            if let Some(slot) = vidhd {
                let frames = case.run.frames.ok_or("vidhd cases need run.frames")?;
                if !case.input.is_empty() {
                    return Err("vidhd cases take no input events".into());
                }
                render_frames_vidhd(&mut m, &mut r, slot, frames)?;
            } else {
                drive(
                    &mut m,
                    &run,
                    &case.input,
                    Hooks {
                        trace: None,
                        renderer: Some(&mut r),
                    },
                )?;
            }
            let full = case.video.as_ref().map_or(Ok(false), |v| v.full_region())?;
            Ok(Observed {
                value: sha256_hex(&framebuffer_rgba(&r, full)),
                machine: m,
                renderer: Some(r),
                trace: None,
            })
        }
        _ => {
            drive(&mut m, &run, &case.input, Hooks::default())?;
            let value = match kind {
                OracleKind::TextHash => sha256_hex(text_dump(&m).as_bytes()),
                OracleKind::MemoryHash => sha256_hex(&memory_dump(&m)),
                OracleKind::StateHash => sha256_hex(&state_dump(&m)),
                OracleKind::Expr => String::new(),
                OracleKind::Trace
                | OracleKind::TraceCheckpoints
                | OracleKind::FramebufferHash
                | OracleKind::AudioEvents => {
                    unreachable!()
                }
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

fn png_bytes(r: &Renderer, full: bool) -> Result<Vec<u8>, String> {
    let (w, h) = if full {
        r.size()
    } else {
        let (_, _, w, h) = r.visible_rect();
        (w, h)
    };
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(
            &framebuffer_rgba(r, full),
            w as u32,
            h as u32,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| e.to_string())?;
    Ok(out)
}

use image::ImageEncoder;

/// Reads and checks the scene of a `framebuffer-hash` case.
pub fn load_scene(env: &Env, case: &Case) -> Result<Option<Scene>, String> {
    let Some(rel) = case.video.as_ref().and_then(|v| v.scene.as_ref()) else {
        return Ok(None);
    };
    let p = env.root.join(rel);
    let bytes = std::fs::read(&p).map_err(|e| format!("cena ausente: {rel} ({e})"))?;
    if let Some(want) = case.video.as_ref().and_then(|v| v.scene_sha256.as_ref()) {
        let got = sha256_hex(&bytes);
        if &got != want {
            return Err(format!("cena {rel} com sha256 {got}, esperado {want}"));
        }
    }
    Scene::parse(&String::from_utf8_lossy(&bytes)).map(Some)
}

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
            .then(|| reference_applewin_sha(env, case))
            .flatten(),
        rust_sha: env.rust_sha.clone(),
        fixture_sha256: case
            .fixture
            .as_ref()
            .map(|f| f.sha256.clone())
            .or_else(|| case.video.as_ref().and_then(|v| v.scene_sha256.clone())),
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
            mf.actual = match &case.oracle.known_divergence {
                Some(id) => format!("{actual} [divergência conhecida {id}]"),
                None => actual,
            };
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

/// `applewin_sha` of the manifest: the `#applewin_sha` of the reference trace header, else
/// `CONFORMANCE_APPLEWIN_SHA`.
fn reference_applewin_sha(env: &Env, case: &Case) -> Option<String> {
    use std::io::BufRead;
    let from_header = case.oracle.trace.as_ref().and_then(|rel| {
        let f = std::fs::File::open(env.resolve_reference(rel)).ok()?;
        std::io::BufReader::new(f)
            .lines()
            .map_while(Result::ok)
            .take_while(|l| l.starts_with('#'))
            .find_map(|l| {
                let (k, v) = l.trim_start_matches('#').trim_start().split_once('=')?;
                (k.trim() == "applewin_sha").then(|| v.trim().to_string())
            })
    });
    from_header.or_else(|| std::env::var("CONFORMANCE_APPLEWIN_SHA").ok())
}

/// The Disk II of the case, with the fixture as drive 1.
fn disk2_setup(env: &Env, case: &Case) -> Result<Option<Disk2Setup>, String> {
    let Some(slot) = case.disk2_slot()? else {
        return Ok(None);
    };
    Ok(Some(Disk2Setup {
        slot,
        image: case.fixture.as_ref().map(|f| env.workspace.join(&f.path)),
    }))
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
        OracleKind::TraceCheckpoints => format!(
            "checkpoints {} (boot do disco em passo sincronizado com o oráculo)",
            o.trace.as_deref().unwrap_or("?")
        ),
        OracleKind::AudioEvents => format!(
            "audio-events {} (timing {}){}",
            o.events.as_deref().unwrap_or("?"),
            o.timing.as_deref().unwrap_or("exact"),
            o.pcm
                .as_deref()
                .map_or(String::new(), |p| format!(", PCM {p}"))
        ),
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
    if case.oracle.kind == OracleKind::TraceCheckpoints {
        return evaluate_disk(env, case);
    }
    if case.oracle.kind == OracleKind::AudioEvents {
        return audio_verdict(env, case, artifacts);
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
            Ok(_) if matches!(case.disk2_slot(), Ok(Some(_))) => {}
            Ok(_) if !fx.save_state => {
                return Verdict::Blocked(format!(
                    "fixture {} presente e íntegra, mas o harness só a carrega como disco de um cartão `N:disk2` ou como save-state",
                    fx.path
                ))
            }
            Ok(_) => {
                // A save-state: the images it names must exist, or the case cannot run.
                let dirs = fixture_dirs(env, fx);
                match crate::state::import_file(&env.workspace.join(&fx.path), &dirs) {
                    Err(e) => return Verdict::Blocked(format!("save-state não importa: {e}")),
                    Ok(i) if !i.missing.is_empty() => {
                        return Verdict::Blocked(format!(
                            "imagem de disco ausente: {}",
                            i.missing.join(", ")
                        ))
                    }
                    Ok(_) => {}
                }
            }
        }
    }
    let reference = match (&case.oracle.kind, &case.oracle.trace) {
        (OracleKind::Trace, Some(rel)) => {
            let p = env.resolve_reference(rel);
            if !p.exists() {
                return Verdict::Blocked(format!(
                    "trace de referência ausente: {rel}. Para gerá-lo: {}",
                    case.oracle.source
                ));
            }
            if let Some(want) = &case.oracle.trace_sha256 {
                match file_sha256(&p) {
                    Ok(h) if h == *want => {}
                    Ok(h) => {
                        return Verdict::Blocked(format!(
                            "trace de referência {rel} com sha256 {h}, esperado {want}. Para regenerar: {}",
                            case.oracle.source
                        ))
                    }
                    Err(e) => return Verdict::Blocked(e),
                }
            }
            match TraceReader::read_file(&p) {
                Ok(t) => Some(t),
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
    let disk2 = match disk2_setup(env, case) {
        Ok(d) => d,
        Err(e) => return Verdict::Blocked(e),
    };

    let scene = match load_scene(env, case) {
        Ok(s) => s,
        Err(e) => return Verdict::Blocked(e),
    };

    let mut first: Option<Observed> = None;
    let mut last: Option<Observed> = None;
    for run in 1..=case.run.repeat {
        let obs = match observe_once(env, case, &meta, disk2.as_ref(), scene.as_ref()) {
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
        OracleKind::Trace if reference.is_none() => {
            // Determinism case: no reference, the runs only had to agree (checked above).
            Verdict::Pass(format!("trace sha256={}{rep}", obs.value))
        }
        OracleKind::Trace => {
            let actual_bytes = obs.trace.expect("trace case");
            let Some(reference) = reference else {
                let want = case.oracle.trace_rows_sha256.clone().unwrap_or_default();
                let got = trace_rows_sha256(&String::from_utf8_lossy(&actual_bytes));
                return if got == want {
                    Verdict::Pass(format!("linhas do trace com sha256 {got}{rep}"))
                } else {
                    let name = format!("{}.actual.csv", case.id);
                    artifacts.push((name.clone(), actual_bytes));
                    Verdict::Fail(
                        format!("sha256 das linhas do trace {got}, esperado {want}"),
                        Some(name),
                    )
                };
            };
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

/// `trace-checkpoints` cases: a Disk II boot in lock step with the oracle (see `disk.rs`).
fn evaluate_disk(env: &Env, case: &Case) -> Verdict {
    use crate::disk::{self, Checkpoints, Failure};
    for (role, fx) in case.devices() {
        let path = env.workspace.join(&fx.path);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => {
                return Verdict::Blocked(format!(
                    "{role} ausente: {} (imagem de terceiros fora do repositório; veja docs/conformance)",
                    fx.path
                ))
            }
        };
        let h = sha256_hex(&bytes);
        if h != fx.sha256 {
            return Verdict::Blocked(format!(
                "{role} {} com sha256 {h}, esperado {}",
                fx.path, fx.sha256
            ));
        }
    }
    let rel = case.oracle.trace.as_deref().expect("validated");
    let ck = match std::fs::read_to_string(env.root.join(rel))
        .map_err(|e| e.to_string())
        .and_then(|t| Checkpoints::parse(&t))
    {
        Ok(c) => c,
        Err(e) => return Verdict::Blocked(format!("checkpoints {rel}: {e}")),
    };
    let mut rig = match disk::build(env, case) {
        Ok(r) => r,
        Err(e) => return Verdict::Fail(format!("erro ao montar a máquina: {e}"), None),
    };
    match disk::check(&mut rig, &ck) {
        Ok(s) => Verdict::Pass(s),
        Err(Failure::Blocked(c)) => Verdict::Blocked(c),
        Err(Failure::Fail(msg, _)) => Verdict::Fail(msg, None),
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
            let full = case
                .video
                .as_ref()
                .is_some_and(|v| v.full_region() == Ok(true));
            (format!("{}.actual.png", case.id), png_bytes(r, full).ok()?)
        }
        OracleKind::Trace | OracleKind::TraceCheckpoints | OracleKind::AudioEvents => return None,
    };
    artifacts.push((name.clone(), bytes));
    Some(name)
}

/// Re-exported for the CLI: loads a trace file.
pub fn read_trace(path: &Path) -> Result<Trace, String> {
    TraceReader::read_file(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// What `conformance scene-fb` prints and writes: the same fields as the oracle's framebuffer
/// mode, so the two outputs can be compared line by line.
pub struct SceneFramebuffer {
    pub width: usize,
    pub height: usize,
    pub visible_sha256: String,
    pub full_sha256: String,
    pub rgba_full: Vec<u8>,
    pub cycles: u64,
}

/// Runs `scene` for `frames` frames, rendering after every instruction, under the given options.
pub fn render_scene(
    model: &str,
    scene: Option<&Scene>,
    cfg: RenderConfig,
    standard: apple2_core::VideoStandard,
    frames: u64,
    vidhd_slot: Option<u8>,
    memclear: Option<i32>,
) -> Result<SceneFramebuffer, String> {
    let model = crate::case::parse_model(model)?;
    let mut mc = MachineConfig::for_model(model);
    mc.video_standard = standard;
    // Same power-on RAM as the oracle (AppleWin's pattern, deterministic draw).
    mc.ram_init = apple2_core::RamInit::APPLEWIN_DETERMINISTIC;
    if let Some(n) = memclear {
        mc.ram_init.pattern = n;
    }
    let mut m = Machine::try_new(mc).map_err(|e| e.to_string())?;
    if let Some(slot) = vidhd_slot {
        install_vidhd(&mut m, slot)?;
    }
    if let Some(scene) = scene {
        scene.apply(&mut m);
    }
    let mut r = Renderer::new(cfg);
    r.resync(m.cycles_in_frame());
    if let Some(slot) = vidhd_slot {
        render_frames_vidhd(&mut m, &mut r, slot, frames)?;
    } else {
        while m.frames() < frames {
            r.run_frame(&mut m);
        }
    }
    let (w, h) = r.size();
    Ok(SceneFramebuffer {
        width: w,
        height: h,
        visible_sha256: sha256_hex(&framebuffer_rgba(&r, false)),
        full_sha256: sha256_hex(&framebuffer_rgba(&r, true)),
        rgba_full: framebuffer_rgba(&r, true),
        cycles: m.cycles(),
    })
}

/// `audio-events` cases: run the program (or save-state) and compare events and PCM with the
/// oracle's recording. A missing fixture or reference is BLOCKED, never PASS.
fn audio_verdict(env: &Env, case: &Case, artifacts: &mut Vec<(String, Vec<u8>)>) -> Verdict {
    use crate::audio::{self, Timing};
    if let Some(fx) = &case.fixture {
        let p = env.workspace.join(&fx.path);
        match file_sha256(&p) {
            Err(_) => {
                return Verdict::Blocked(format!(
                    "fixture ausente: {} (esperada no cache de fixtures; veja docs/conformance)",
                    fx.path
                ))
            }
            Ok(h) if h != fx.sha256 => {
                return Verdict::Blocked(format!(
                    "fixture {} com sha256 {h}, esperado {}",
                    fx.path, fx.sha256
                ))
            }
            Ok(_) => {}
        }
    }
    let o = &case.oracle;
    let expected = match o.events.as_deref() {
        None => String::new(),
        Some(ev_rel) => match audio::read_reference(&env.root, ev_rel) {
            Ok(b) => String::from_utf8_lossy(&b).into_owned(),
            Err(e) => {
                return Verdict::Blocked(format!(
                    "referência do oráculo ausente: {e} (gere com tools/conformance/audio/run-oracle-case.py)"
                ))
            }
        },
    };
    let run = match audio::run(case, env, None) {
        Ok(r) => r,
        Err(e) => return Verdict::Fail(format!("erro na execução: {e}"), None),
    };
    let actual_lines: Vec<String> = run.events.iter().map(|(c, e)| e.line(*c)).collect();
    artifacts.push((
        format!("{}.actual.events", case.id),
        audio::render_lines(&actual_lines).into_bytes(),
    ));
    let timing = match o.timing.as_deref() {
        Some("ssi") => Timing::Ssi {
            tolerance: o.ssi_tolerance.expect("validated"),
        },
        _ => Timing::Exact,
    };
    let mut summary = String::new();
    if o.events.is_some() {
        summary = match audio::compare_events(
            &audio::lines_of(&expected),
            &actual_lines,
            timing,
            run.final_cycle,
        ) {
            Ok(s) => s,
            Err(report) => {
                let name = format!("{}.events-diff.txt", case.id);
                artifacts.push((name.clone(), report.clone().into_bytes()));
                return Verdict::Fail(
                    format!("eventos: {}", report.lines().next().unwrap_or("")),
                    Some(name),
                );
            }
        };
    }
    if let (Some(rel), Some(l)) = (&o.pcm, o.pcm_limits) {
        let limits: crate::pcm::Limits = l.into();
        let bytes = match audio::read_reference(&env.root, rel) {
            Ok(b) => b,
            Err(e) => return Verdict::Blocked(format!("PCM de referência ausente: {e}")),
        };
        let spec = case.audio.clone().unwrap_or_default();
        let (verdict, m) = audio::compare_pcm(&bytes, &run, &spec, &limits);
        let detail = audio::describe_measures(&m);
        match verdict {
            crate::pcm::Verdict::Exact => {
                summary.push_str(&format!("; PCM idêntico ({detail})"));
            }
            crate::pcm::Verdict::Tolerance => {
                summary.push_str(&format!("; PCM dentro da tolerância ({detail})"));
            }
            crate::pcm::Verdict::Fail(why) => {
                let name = format!("{}.pcm-diff.txt", case.id);
                artifacts.push((name.clone(), format!("{why}\n{detail}\n").into_bytes()));
                return Verdict::Fail(format!("PCM fora do limiar: {why} ({detail})"), Some(name));
            }
        }
    }
    if let Some(pat) = &o.silent_after {
        let exp = audio::lines_of(&expected);
        let Some(line) = exp
            .iter()
            .rev()
            .find(|l| l.split_once(',').is_some_and(|x| x.1 == pat))
        else {
            return Verdict::Fail(
                format!("silent_after `{pat}`: o oráculo não tem esse evento"),
                None,
            );
        };
        let c: u64 = line
            .split(',')
            .next()
            .and_then(|c| c.parse().ok())
            .unwrap_or(0);
        let clock = apple2_audio::SampleClock::new(44_100, apple2_core::VideoStandard::Ntsc);
        let from = clock.frames_at(c) as usize + 2;
        let tail = run.mb_pcm.get(from * 2..).unwrap_or(&[]);
        let loud = tail.iter().filter(|&&s| s != 0).count();
        if loud > 0 {
            return Verdict::Fail(
                format!("PCM não é silêncio depois de `{pat}` (ciclo {c}): {loud} amostras não nulas de {}", tail.len()),
                None,
            );
        }
        summary.push_str(&format!(
            "; silêncio exato depois de `{pat}` (ciclo {c}, {} amostras)",
            tail.len()
        ));
    }
    for w in &run.warnings {
        summary.push_str(&format!("; aviso: {w}"));
    }
    Verdict::Pass(summary)
}
