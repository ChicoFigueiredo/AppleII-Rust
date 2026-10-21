//! `apple2-bench` (HU-1001): desempenho determinístico do emulador por cenário.
//!
//! ```text
//! apple2-bench [--reps N] [--only a,b] [--json FILE] [--report FILE]
//!              [--write-baseline FILE] [--check FILE] [--no-latency] [--list]
//! ```
//!
//! Cada repetição roda em um processo filho (`--child NOME`), para que o pico de RSS (`VmHWM`)
//! seja de um cenário só. O pai agrega mediana e p95 e escreve o JSON e o relatório Markdown.
//! Veja `report.rs` para o formato do baseline e o critério de `--check`.

mod breakdown;
mod latency;
mod report;
mod scenarios;
mod stats;
mod sys;

use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Stdio};

struct Args {
    reps: usize,
    only: Option<Vec<String>>,
    json: Option<PathBuf>,
    report: Option<PathBuf>,
    write_baseline: Option<PathBuf>,
    check: Option<PathBuf>,
    latency: bool,
    child: Option<String>,
    root: PathBuf,
    allow_debug: bool,
    list: bool,
    breakdown: bool,
    render: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut a = Args {
        reps: 10,
        only: None,
        json: None,
        report: None,
        write_baseline: None,
        check: None,
        latency: true,
        child: None,
        root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
        allow_debug: false,
        list: false,
        breakdown: false,
        render: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut val = |name: &str| it.next().ok_or(format!("{name} precisa de um valor"));
        match arg.as_str() {
            "--reps" => a.reps = val("--reps")?.parse().map_err(|e| format!("--reps: {e}"))?,
            "--only" => {
                a.only = Some(val("--only")?.split(',').map(str::to_string).collect());
            }
            "--json" => a.json = Some(val("--json")?.into()),
            "--report" => a.report = Some(val("--report")?.into()),
            "--write-baseline" => a.write_baseline = Some(val("--write-baseline")?.into()),
            "--check" => a.check = Some(val("--check")?.into()),
            "--child" => a.child = Some(val("--child")?),
            "--root" => a.root = val("--root")?.into(),
            "--no-latency" => a.latency = false,
            "--allow-debug" => a.allow_debug = true,
            "--list" => a.list = true,
            "--breakdown" => a.breakdown = true,
            "--render" => a.render = Some(val("--render")?.into()),
            "-h" | "--help" => {
                println!("{}", include_str!("usage.txt"));
                std::process::exit(0);
            }
            other => return Err(format!("argumento desconhecido: {other}")),
        }
    }
    if a.reps == 0 {
        return Err("--reps precisa ser pelo menos 1".into());
    }
    Ok(a)
}

fn run_child(name: &str, root: &std::path::Path) -> Result<(), String> {
    let sc = scenarios::SCENARIOS
        .iter()
        .find(|s| s.name == name)
        .ok_or(format!("cenário desconhecido: {name}"))?;
    let s = (sc.run)(root)?;
    let out = json!({
        "wall_s": s.wall_s, "cpu_s": s.cpu_s, "frames": s.frames, "cycles": s.cycles,
        "audio_frames": s.audio_frames, "rss_peak_kib": sys::rss_peak_kb(),
        "fingerprint": {
            "cycles": s.fp_cycles,
            "frame": format!("{:016x}", s.fp_frame),
            "audio": format!("{:016x}", s.fp_audio),
        },
        "audio_peak": s.audio_peak,
    });
    println!("{out}");
    Ok(())
}

fn spawn_child(name: &str, root: &std::path::Path) -> Result<Value, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let out = Command::new(exe)
        .args(["--child", name, "--root"])
        .arg(root)
        .stderr(Stdio::null())
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("cenário {name} falhou (status {})", out.status));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("saída do filho ilegível: {e}"))
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("apple2-bench: {e}");
        std::process::exit(2);
    }
}

fn real_main() -> Result<(), String> {
    let a = parse_args()?;
    if let Some(name) = &a.child {
        return run_child(name, &a.root);
    }
    if let Some(p) = &a.render {
        // Regenera relatório/baseline a partir de um JSON já medido (sem rodar nada).
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let doc: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
        if let Some(r) = &a.report {
            write_file(r, &report::markdown(&doc))?;
        }
        if let Some(b) = &a.write_baseline {
            let out =
                serde_json::to_string_pretty(&report::baseline(&doc)).map_err(|e| e.to_string())?;
            write_file(b, &(out + "\n"))?;
        }
        print!("{}", report::table(&doc));
        return Ok(());
    }
    if a.list {
        for s in scenarios::SCENARIOS {
            println!("{:14} {}", s.name, s.title);
        }
        return Ok(());
    }
    if cfg!(debug_assertions) && !a.allow_debug {
        return Err("compile com --release (ou passe --allow-debug: os números não valem)".into());
    }
    let selected: Vec<_> = scenarios::SCENARIOS
        .iter()
        .filter(|s| {
            a.only
                .as_ref()
                .is_none_or(|o| o.iter().any(|n| n == s.name))
        })
        .collect();
    if selected.is_empty() {
        return Err("nenhum cenário selecionado (veja --list)".into());
    }

    let mut results = serde_json::Map::new();
    for sc in &selected {
        results.insert(sc.name.to_string(), measure(sc, a.reps, &a.root)?);
    }

    // --check: um cenário reprovado é medido de novo (com o dobro de repetições) antes de a
    // regressão valer. Numa máquina compartilhada, um vizinho no mesmo núcleo físico (SMT) pode
    // custar ~30 % durante minutos; regressão de verdade reprova de novo.
    let mut check_out: Option<(bool, Vec<String>)> = None;
    if let Some(p) = &a.check {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        let base: Value = serde_json::from_str(&text).map_err(|e| format!("baseline: {e}"))?;
        let machine = sys::machine_info();
        let mut doc = json!({"machine": machine, "scenarios": Value::Object(results.clone())});
        let (mut ok, mut lines) = report::check(&doc, &base);
        if !ok {
            let failed: Vec<&str> = selected
                .iter()
                .map(|s| s.name)
                .filter(|n| {
                    lines
                        .iter()
                        .any(|l| l.starts_with(&format!("FALHA {n}: mediana")))
                })
                .collect();
            if !failed.is_empty() {
                eprintln!("repetindo os cenários reprovados: {failed:?}");
                for sc in selected.iter().filter(|s| failed.contains(&s.name)) {
                    results.insert(sc.name.to_string(), measure(sc, a.reps * 2, &a.root)?);
                }
                doc = json!({"machine": machine, "scenarios": Value::Object(results.clone())});
                (ok, lines) = report::check(&doc, &base);
            }
        }
        check_out = Some((ok, lines));
    }

    let latency = if a.latency && a.check.is_none() {
        eprintln!("latência de entrada...");
        Some(latency::measure()?)
    } else {
        None
    };

    let breakdown = if a.breakdown {
        eprintln!("decomposição de custo por quadro...");
        Some(breakdown::run(&a.root)?)
    } else {
        None
    };

    let doc = json!({
        "schema": 1,
        "machine": sys::machine_info(),
        "reps": a.reps,
        "scenarios": Value::Object(results),
        "input_latency": latency,
        "breakdown": breakdown,
    });
    if let Some(p) = &a.json {
        write_file(
            p,
            &(serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())? + "\n"),
        )?;
    }
    if let Some(p) = &a.report {
        write_file(p, &report::markdown(&doc))?;
    }
    if let Some(p) = &a.write_baseline {
        let b = report::baseline(&doc);
        write_file(
            p,
            &(serde_json::to_string_pretty(&b).map_err(|e| e.to_string())? + "\n"),
        )?;
        eprintln!("baseline gravado em {}", p.display());
    }
    print!("{}", report::table(&doc));
    if let Some((ok, lines)) = check_out {
        for l in lines {
            println!("{l}");
        }
        if !ok {
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Uma execução de aquecimento descartada (cache de arquivos, frequência) e `reps` medidas, cada
/// uma num processo filho.
fn measure(sc: &scenarios::Scenario, reps: usize, root: &std::path::Path) -> Result<Value, String> {
    eprint!("{:14} ", sc.name);
    spawn_child(sc.name, root)?;
    let mut runs = Vec::new();
    for _ in 0..reps {
        runs.push(spawn_child(sc.name, root)?);
        eprint!(".");
    }
    let summary = stats::summarize(sc.title, &runs)?;
    eprintln!(
        " {:.0} fps, {:.2} MHz (mediana)",
        summary["fps"]["median"].as_f64().unwrap_or(0.0),
        summary["mhz"]["median"].as_f64().unwrap_or(0.0)
    );
    Ok(summary)
}

fn write_file(p: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(dir) = p.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    std::fs::write(p, text).map_err(|e| format!("{}: {e}", p.display()))
}
