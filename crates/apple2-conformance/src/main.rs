//! `conformance`: run declarative cases, generate and diff reference traces.

use apple2_conformance::audio;
use apple2_conformance::case::{parse_model, InputEvent, RunSpec};
use apple2_conformance::manifest::TestResult;
use apple2_conformance::pcm;
use apple2_conformance::report::{counts, write_report};
use apple2_conformance::runner::{generate_trace, load_cases, read_trace, run_case, Env};
use apple2_conformance::trace::{compare_with, parse_field_list, CompareOptions};
use apple2_conformance::Case;
use std::path::PathBuf;
use std::process::ExitCode;

const USAGE: &str = "\
uso:
  conformance run (--all | --case ID [--case ID ...]) --report DIR [--root DIR]
  conformance trace --model M (--frames N | --cycles N) [--cpu 6502|65C02] [--input frame:N:TEXT | cycle:N:TEXT]... [--meta k=v]... -o FILE
  conformance diff ESPERADO.csv OBTIDO.csv [--ignore cycle,p] [--context N]
  conformance audio-run --case ID --out-dir DIR [--root DIR] [--trace]
  conformance pcm-compare ESPERADO.pcm OBTIDO.pcm [--channels N] [--f-tol X --rms-tol X --peak-tol X]

Saída de `run`: DIR/<caso>.manifest.json, DIR/junit.xml, DIR/summary.md e artefatos de diff.
Código de saída de `run`: 0 se tudo PASS, 1 se algum FAIL, 2 se algum BLOCKED (sem FAIL).
TEXT aceita \\r \\n \\\\ e \\xNN. Modelos: apple2 apple2plus apple2jplus apple2e apple2e-enhanced
pravets82 pravets8m pravets8a tk3000e base64a.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("run") => cmd_run(&args[1..]),
        Some("trace") => cmd_trace(&args[1..]),
        Some("diff") => cmd_diff(&args[1..]),
        Some("audio-run") => cmd_audio_run(&args[1..]),
        Some("pcm-compare") => cmd_pcm_compare(&args[1..]),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(0)
        }
        _ => Err("subcomando ausente ou desconhecido".to_string()),
    };
    match r {
        Ok(code) => ExitCode::from(code),
        Err(e) => {
            eprintln!("erro: {e}\n\n{USAGE}");
            ExitCode::from(64)
        }
    }
}

/// Simple `--flag value` / positional parser.
struct Args<'a> {
    rest: &'a [String],
    i: usize,
}

impl<'a> Args<'a> {
    fn new(rest: &'a [String]) -> Self {
        Args { rest, i: 0 }
    }
    fn next(&mut self) -> Option<&'a str> {
        let r = self.rest.get(self.i)?;
        self.i += 1;
        Some(r)
    }
    fn value(&mut self, flag: &str) -> Result<&'a str, String> {
        self.next()
            .ok_or_else(|| format!("{flag} precisa de um valor"))
    }
}

fn cmd_run(rest: &[String]) -> Result<u8, String> {
    let (mut all, mut ids, mut report, mut root) = (false, Vec::<String>::new(), None, None);
    let mut a = Args::new(rest);
    while let Some(arg) = a.next() {
        match arg {
            "--all" => all = true,
            "--case" => ids.push(a.value(arg)?.to_string()),
            "--report" => report = Some(PathBuf::from(a.value(arg)?)),
            "--root" => root = Some(PathBuf::from(a.value(arg)?)),
            o => return Err(format!("argumento desconhecido `{o}`")),
        }
    }
    if all == !ids.is_empty() {
        return Err("use exatamente um de --all ou --case".into());
    }
    let report = report.ok_or("--report DIR é obrigatório")?;
    let env = Env::detect(root.as_deref());
    let cases = load_cases(&env.root.join("cases"))?;
    let selected: Vec<&Case> = if all {
        cases.iter().collect()
    } else {
        ids.iter()
            .map(|id| {
                cases.iter().find(|c| &c.id == id).ok_or_else(|| {
                    format!(
                        "caso `{id}` não existe em {}",
                        env.root.join("cases").display()
                    )
                })
            })
            .collect::<Result<_, _>>()?
    };
    let mut outcomes = Vec::new();
    for c in selected {
        let o = run_case(&env, c);
        println!(
            "{:<12} {} ({:.2}s)\n             {}",
            o.manifest.result.as_str(),
            c.id,
            o.seconds,
            o.manifest.actual.lines().next().unwrap_or("")
        );
        outcomes.push(o);
    }
    write_report(&report, &outcomes).map_err(|e| format!("{}: {e}", report.display()))?;
    let c = counts(&outcomes);
    let line: Vec<String> = TestResult::ALL
        .iter()
        .filter(|r| c[r] > 0)
        .map(|r| format!("{r}={}", c[r]))
        .collect();
    println!("relatório em {} ({})", report.display(), line.join(" "));
    Ok(if c[&TestResult::Fail] > 0 {
        1
    } else if c[&TestResult::Blocked] > 0 {
        2
    } else {
        0
    })
}

fn cmd_trace(rest: &[String]) -> Result<u8, String> {
    let (mut model, mut frames, mut cycles, mut cpu, mut out) = (None, None, None, None, None);
    let mut inputs = Vec::new();
    let mut extra_meta: Vec<(&str, &str)> = Vec::new();
    let mut a = Args::new(rest);
    while let Some(arg) = a.next() {
        match arg {
            "--meta" => extra_meta.push(
                a.value(arg)?
                    .split_once('=')
                    .ok_or("--meta precisa de chave=valor")?,
            ),
            "--model" => model = Some(a.value(arg)?),
            "--frames" => frames = Some(a.value(arg)?.parse::<u64>().map_err(|e| e.to_string())?),
            "--cycles" => cycles = Some(a.value(arg)?.parse::<u64>().map_err(|e| e.to_string())?),
            "--cpu" => cpu = Some(a.value(arg)?),
            "--input" => inputs.push(InputEvent::parse_cli(a.value(arg)?)?),
            "-o" | "--output" => out = Some(PathBuf::from(a.value(arg)?)),
            o => return Err(format!("argumento desconhecido `{o}`")),
        }
    }
    let model_name = model.ok_or("--model é obrigatório")?;
    let out = out.ok_or("-o ARQUIVO é obrigatório")?;
    let run = RunSpec {
        frames,
        cycles,
        repeat: 1,
    };
    if frames.is_some() == cycles.is_some() {
        return Err("use exatamente um de --frames ou --cycles".into());
    }
    let model = parse_model(model_name)?;
    let mut cfg = apple2_core::MachineConfig::for_model(model);
    cfg.cpu_cmos = match cpu {
        None | Some("auto") => None,
        Some("6502") => Some(false),
        Some("65C02") | Some("65c02") => Some(true),
        Some(o) => return Err(format!("--cpu `{o}`: use 6502, 65C02 ou auto")),
    };
    let rom_sha = model
        .system_rom()
        .map(apple2_conformance::sha256::sha256_hex)
        .ok_or("o modelo não tem ROM embutida")?;
    let cpu_name = if cfg.is_cmos() { "65C02" } else { "6502" };
    let input_desc = inputs
        .iter()
        .map(InputEvent::describe)
        .collect::<Vec<_>>()
        .join("; ");
    let mut meta = vec![
        ("source", "apple2-rust"),
        ("model", model_name),
        ("cpu", cpu_name),
        ("rom_sha256", rom_sha.as_str()),
    ];
    if !input_desc.is_empty() {
        meta.push(("input", input_desc.as_str()));
    }
    meta.extend(extra_meta);
    let bytes = generate_trace(cfg, &run, &inputs, &meta)?;
    if let Some(dir) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(&out, &bytes).map_err(|e| format!("{}: {e}", out.display()))?;
    let rows = bytes.iter().filter(|&&b| b == b'\n').count();
    println!("{}: {} bytes, {} linhas", out.display(), bytes.len(), rows);
    Ok(0)
}

fn cmd_diff(rest: &[String]) -> Result<u8, String> {
    let (mut files, mut ignore, mut context) = (Vec::new(), Vec::new(), 8usize);
    let mut a = Args::new(rest);
    while let Some(arg) = a.next() {
        match arg {
            "--ignore" => ignore = parse_field_list(a.value(arg)?)?,
            "--context" => {
                context = a
                    .value(arg)?
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?
            }
            o if o.starts_with("--") => return Err(format!("argumento desconhecido `{o}`")),
            f => files.push(PathBuf::from(f)),
        }
    }
    let [exp, act] = files.as_slice() else {
        return Err("diff precisa de ESPERADO.csv e OBTIDO.csv".into());
    };
    let (e, o) = (read_trace(exp)?, read_trace(act)?);
    match compare_with(
        &e,
        &o,
        &CompareOptions {
            context_lines: context,
            ignore,
        },
    ) {
        None => {
            println!("traces idênticos ({} instruções)", e.rows.len());
            Ok(0)
        }
        Some(d) => {
            print!("{}", d.report());
            Ok(1)
        }
    }
}

/// Runs an `audio-events` case on the Rust machine and writes `ID.events`, `ID.mb.pcm`,
/// `ID.spkr.pcm` (and `ID.trace.csv` with `--trace`) to DIR, without comparing anything.
fn cmd_audio_run(rest: &[String]) -> Result<u8, String> {
    let (mut id, mut out, mut root, mut with_trace) = (None, None, None, false);
    let mut a = Args::new(rest);
    while let Some(arg) = a.next() {
        match arg {
            "--case" => id = Some(a.value(arg)?.to_string()),
            "--out-dir" => out = Some(PathBuf::from(a.value(arg)?)),
            "--root" => root = Some(PathBuf::from(a.value(arg)?)),
            "--trace" => with_trace = true,
            o => return Err(format!("argumento desconhecido `{o}`")),
        }
    }
    let id = id.ok_or("--case ID é obrigatório")?;
    let out = out.ok_or("--out-dir DIR é obrigatório")?;
    let env = Env::detect(root.as_deref());
    let cases = load_cases(&env.root.join("cases"))?;
    let case = cases
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| format!("caso `{id}` não existe"))?;
    std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
    let mut tw = if with_trace {
        Some(
            apple2_conformance::TraceWriter::new(Vec::new(), &[("source", "apple2-rust")])
                .map_err(|e| e.to_string())?,
        )
    } else {
        None
    };
    let run = audio::run(case, &env, tw.as_mut())?;
    let lines: Vec<String> = run.events.iter().map(|(c, e)| e.line(*c)).collect();
    let w = |name: String, bytes: &[u8]| {
        std::fs::write(out.join(&name), bytes).map_err(|e| format!("{name}: {e}"))
    };
    w(
        format!("{id}.events"),
        audio::render_lines(&lines).as_bytes(),
    )?;
    let le = |v: &[i16]| v.iter().flat_map(|s| s.to_le_bytes()).collect::<Vec<u8>>();
    w(format!("{id}.mb.pcm"), &le(&run.mb_pcm))?;
    w(format!("{id}.spkr.pcm"), &le(&run.spkr_pcm))?;
    if let Some(t) = tw {
        w(
            format!("{id}.trace.csv"),
            &t.finish().map_err(|e| e.to_string())?,
        )?;
    }
    println!(
        "{id}: {} eventos, {} quadros de PCM do Mockingboard, {} do alto-falante, {} ciclos",
        lines.len(),
        run.mb_pcm.len() / 2,
        run.spkr_pcm.len(),
        run.final_cycle
    );
    for wmsg in &run.warnings {
        eprintln!("aviso: {wmsg}");
    }
    Ok(0)
}

/// Prints the PCM metrics of two raw s16le files (used to calibrate the limits with the oracle
/// against itself). The limits only matter for the verdict line.
fn cmd_pcm_compare(rest: &[String]) -> Result<u8, String> {
    let (mut files, mut channels) = (Vec::new(), 2usize);
    let mut l = pcm::Limits {
        f_tol: 0.01,
        rms_tol: 0.02,
        peak_tol: 0.02,
    };
    let mut a = Args::new(rest);
    while let Some(arg) = a.next() {
        let num = |v: &str| v.parse::<f64>().map_err(|e| e.to_string());
        match arg {
            "--channels" => {
                channels = a
                    .value(arg)?
                    .parse()
                    .map_err(|e: std::num::ParseIntError| e.to_string())?
            }
            "--f-tol" => l.f_tol = num(a.value(arg)?)?,
            "--rms-tol" => l.rms_tol = num(a.value(arg)?)?,
            "--peak-tol" => l.peak_tol = num(a.value(arg)?)?,
            f if !f.starts_with("--") => files.push(PathBuf::from(f)),
            o => return Err(format!("argumento desconhecido `{o}`")),
        }
    }
    let [e, o] = files.as_slice() else {
        return Err("pcm-compare precisa de ESPERADO.pcm e OBTIDO.pcm".into());
    };
    let rd = |p: &PathBuf| {
        std::fs::read(p)
            .map(|b| pcm::read_s16le(&b))
            .map_err(|e| format!("{}: {e}", p.display()))
    };
    let m = pcm::measure(&rd(e)?, &rd(o)?, channels);
    println!("{}", audio::describe_measures(&m));
    let v = pcm::decide(&m, &l);
    println!("{v:?}");
    Ok(u8::from(matches!(v, pcm::Verdict::Fail(_))))
}
