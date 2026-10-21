//! Tabela, relatório Markdown, baseline e o critério de `--check`.
//!
//! # Margem de regressão
//!
//! A margem de cada cenário é `max(15 %, 3 x CV)`, onde CV é o coeficiente de variação do MHz
//! emulado nas repetições que gravaram o baseline (desvio-padrão amostral / média).
//! - 15 % é o piso: abaixo disso, ruído de frequência, escalonador e cache de uma máquina
//!   compartilhada (runner de CI) esconde qualquer sinal; mais apertado só produziria falso positivo.
//! - 3 x CV cobre cenários que já são ruidosos na máquina do baseline (3 sigma: ~0,3 % de falso
//!   positivo se o ruído for normal). O maior dos dois vale.
//!
//! O MHz do `--check` é calculado sobre o tempo de CPU da thread (não o de parede): numa máquina
//! ocupada a thread é preemptada e o relógio de parede mede o vizinho, não o emulador. O CV
//! também vem desse MHz.
//!
//! A regressão é só para baixo: a mediana atual de MHz precisa ficar acima de
//! `baseline * (1 - margem)`. Melhoras nunca falham.
//!
//! Além do tempo, `--check` exige que a impressão digital (ciclos emulados, hash do último
//! quadro, hash do áudio) seja idêntica: otimização não pode mudar timing, vídeo nem áudio.
//!
//! Se a CPU/núcleos do baseline não são os da máquina atual (baseline gravado no desenvolvedor,
//! checado no runner), o MHz absoluto não é comparável. Aí cada cenário é comparado pela razão
//! `MHz / MHz(cpu)`, que cancela a velocidade da máquina; o cenário `cpu` não é checado nesse
//! modo e a saída avisa.

use serde_json::{json, Value};
use std::fmt::Write;

fn f(v: &Value, path: &[&str]) -> f64 {
    path.iter()
        .fold(v, |v, k| &v[*k])
        .as_f64()
        .unwrap_or(f64::NAN)
}

fn scenarios(doc: &Value) -> impl Iterator<Item = (&String, &Value)> {
    doc["scenarios"].as_object().into_iter().flatten()
}

pub fn table(doc: &Value) -> String {
    let mut s = String::new();
    let _ = writeln!(
        s,
        "{:<14} {:>9} {:>9} {:>9} {:>9} {:>8} {:>8} {:>8}",
        "cenario", "fps med", "fps p95", "MHz med", "MHz p95", "CPU %", "RSS MiB", "CV %"
    );
    for (name, v) in scenarios(doc) {
        let _ = writeln!(
            s,
            "{:<14} {:>9.1} {:>9.1} {:>9.2} {:>9.2} {:>8.0} {:>8.1} {:>8.1}",
            name,
            f(v, &["fps", "median"]),
            f(v, &["fps", "p95"]),
            f(v, &["mhz", "median"]),
            f(v, &["mhz", "p95"]),
            f(v, &["cpu_pct", "median"]),
            f(v, &["rss_peak_mib", "p95"]),
            f(v, &["cv_pct"]),
        );
    }
    s
}

pub fn markdown(doc: &Value) -> String {
    let m = &doc["machine"];
    let mut s = String::new();
    let g = |k: &str| m[k].as_str().unwrap_or("?").to_string();
    s.push_str("# Desempenho determinístico (HU-1001)\n\n");
    s.push_str("Gerado por `apple2-bench` (`cargo run --release -p apple2-bench -- --report tests/conformance/reports/performance.md`). Os números valem para a máquina abaixo; em outra máquina compare só tendências e use o `--check` do baseline.\n\n");
    s.push_str("## Ambiente\n\n| Item | Valor |\n|---|---|\n");
    let _ = writeln!(s, "| CPU | {} |", g("cpu"));
    let _ = writeln!(
        s,
        "| Núcleos lógicos | {} (a emulação usa uma thread) |",
        m["logical_cores"]
    );
    let _ = writeln!(s, "| Memória | {} MiB |", m["mem_total_mib"]);
    let _ = writeln!(s, "| SO | {} |", g("os"));
    let _ = writeln!(s, "| Governador de CPU | {} |", g("cpu_governor"));
    let _ = writeln!(
        s,
        "| SHA | `{}`{} |",
        g("git_sha"),
        if m["git_dirty"].as_bool() == Some(true) {
            " (+ alterações locais)"
        } else {
            ""
        }
    );
    let _ = writeln!(s, "| Toolchain | {} |", g("rustc"));
    let _ = writeln!(s, "| Perfil | {} |", g("profile"));
    let _ = writeln!(
        s,
        "| Repetições | {} (+1 de aquecimento descartada) |",
        doc["reps"]
    );

    s.push_str("\n## Cenários\n\n");
    s.push_str("Cada repetição roda num processo novo (o RSS de pico é só do cenário). O trecho cronometrado exclui a criação da máquina, mas inclui, em todo quadro, a execução da CPU e dos cartões, o renderer, a cópia do quadro e a mixagem do áudio (o trabalho da thread de emulação, sem o pacing). \"p95\" é o pior caso: o percentil 95 do tempo, e as taxas derivadas dele (a taxa que 95 % das execuções igualaram ou superaram); para CPU e RSS é o percentil 95 direto. \"MHz sobre CPU\" divide os ciclos pelo tempo de CPU da thread (é a base do `--check`, imune a preempção); o CV % é o dele. Referência: 1,0205 MHz emulado = tempo real (NTSC), 60 quadros/s.\n\n");
    s.push_str("| Cenário | Quadros | fps mediana | fps p95 | MHz mediana | MHz p95 | x tempo real | MHz sobre CPU (mediana) | CPU % (med) | RSS pico MiB (p95) | CV % |\n|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|\n");
    for (name, v) in scenarios(doc) {
        let _ = writeln!(
            s,
            "| `{}` | {:.0} | {:.1} | {:.1} | {:.2} | {:.2} | {:.0}x | {:.2} | {:.0} | {:.1} | {:.1} |",
            name,
            f(v, &["frames"]),
            f(v, &["fps", "median"]),
            f(v, &["fps", "p95"]),
            f(v, &["mhz", "median"]),
            f(v, &["mhz", "p95"]),
            f(v, &["mhz", "median"]) / 1.0205,
            f(v, &["mhz_cpu", "median"]),
            f(v, &["cpu_pct", "median"]),
            f(v, &["rss_peak_mib", "p95"]),
            f(v, &["cv_pct"]),
        );
    }
    s.push_str("\n| Cenário | Descrição |\n|---|---|\n");
    for (name, v) in scenarios(doc) {
        let _ = writeln!(s, "| `{}` | {} |", name, v["title"].as_str().unwrap_or(""));
    }

    if let Some(l) = doc["input_latency"].as_object() {
        s.push_str("\n## Latência de entrada\n\n");
        let im = &l["in_machine"];
        let row = |label: &str, v: &Value, scale: f64| {
            format!(
                "| {label} | {:.2} | {:.2} | {:.2} | {:.2} |\n",
                f(v, &["min"]) / scale,
                f(v, &["median"]) / scale,
                f(v, &["p95"]) / scale,
                f(v, &["max"]) / scale
            )
        };
        s.push_str("| Medida | mín | mediana | p95 | máx |\n|---|---:|---:|---:|---:|\n");
        s.push_str(&row(
            "Tecla até o fim do `LDA $C000` (ciclos emulados)",
            &im["cycles"],
            1.0,
        ));
        s.push_str(&row(
            "O mesmo, em microssegundos de Apple II",
            &im["emulated_microseconds"],
            1.0,
        ));
        s.push_str(&row(
            "Tempo real na thread, `Machine::key_press` (µs)",
            &im["host_ns_machine_only"],
            1e3,
        ));
        s.push_str(&row(
            "Tempo real na thread, via `Emulator::apply(Command::Key)` (µs)",
            &im["host_ns_via_emulator_apply"],
            1e3,
        ));
        s.push_str(&row(
            "`Command::Key` enviado à thread até um quadro publicado mostrar a tecla (ms)",
            &l["key_to_published_frame_ms"],
            1.0,
        ));
        s.push_str("\nAs três primeiras linhas medem o caminho do núcleo: o programa de teste espera a tecla num laço `LDA $C000 / BPL` de 7 ciclos, então a latência em ciclos é o resto do laço (fase aleatória) mais o `LDA`. Com o firmware real o laço de `KEYIN` é parecido. A última linha é ponta a ponta no processo, sem janela nem áudio: a thread de emulação (`emu::spawn`, ritmo por timer a 1,0x, ADR 0003) aplica o comando entre quadros, roda o quadro de 16,7 ms, e só publica depois de renderizar; por isso o piso é o resto do quadro corrente (até ~17 ms) mais o intervalo mínimo de publicação (8 ms). Com áudio o anel de ~80 ms (ADR 0003) soma-se a isso.\n\n");
        s.push_str("**Não dá para medir sem GUI:**\n\n");
        for item in l["not_measurable_without_gui"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let _ = writeln!(s, "- {}", item.as_str().unwrap_or(""));
        }
    }

    if let Some(b) = doc["breakdown"].as_object() {
        s.push_str("\n## Decomposição do custo por quadro\n\nTempo de CPU da thread por quadro (mínimo de 3 execuções), acrescentando uma etapa por vez (veja `breakdown.rs`). 16,7 ms por quadro é o orçamento de tempo real a 60 Hz.\n\n| Cenário | Total ms/quadro | Máquina (CPU, scanner, cartões) | Renderer a cada instrução | Conversão para RGBA | Mixagem de áudio |\n|---|---:|---:|---:|---:|---:|\n");
        for (name, v) in b {
            let t = f(v, &["ms_per_frame_total"]);
            let cell = |k: &str| format!("{:.3} ({:.0} %)", f(v, &[k]), f(v, &[k]) / t * 100.0);
            let _ = writeln!(
                s,
                "| `{}` | {:.3} | {} | {} | {} | {} |",
                name,
                t,
                cell("machine_ms"),
                cell("renderer_update_ms"),
                cell("render_to_rgba_ms"),
                cell("audio_mix_ms")
            );
        }
    }
    s.push_str(include_str!("bottlenecks.md"));
    s
}

/// Margem de regressão em %, ver o módulo.
pub fn margin_pct(cv_pct: f64) -> f64 {
    (3.0 * cv_pct).max(15.0)
}

pub fn baseline(doc: &Value) -> Value {
    let m = &doc["machine"];
    let mut sc = serde_json::Map::new();
    for (name, v) in scenarios(doc) {
        sc.insert(
            name.clone(),
            json!({
                "mhz_cpu_median": round3(f(v, &["mhz_cpu", "median"])),
                "cv_pct": round3(f(v, &["cv_pct"])),
                "margin_pct": round3(margin_pct(f(v, &["cv_pct"]))),
                "fingerprint": v["fingerprint"],
            }),
        );
    }
    json!({
        "schema": 1,
        "note": "Gerado por `apple2-bench --write-baseline`. margin_pct = max(15, 3 x CV) sobre as repetições que o geraram. Regrave só ao mudar de propósito timing, vídeo ou áudio (impressão digital) ou a máquina de referência.",
        "machine": {
            "cpu": m["cpu"], "logical_cores": m["logical_cores"], "os": m["os"],
            "rustc": m["rustc"], "git_sha": m["git_sha"],
        },
        "reps": doc["reps"],
        "scenarios": Value::Object(sc),
    })
}

fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

/// Compara `doc` (execução atual) com `base`. Devolve (passou, linhas de saída).
pub fn check(doc: &Value, base: &Value) -> (bool, Vec<String>) {
    let mut ok = true;
    let mut out = Vec::new();
    let same_machine = base["machine"]["cpu"] == doc["machine"]["cpu"]
        && base["machine"]["logical_cores"] == doc["machine"]["logical_cores"];
    if same_machine {
        out.push("máquina igual à do baseline: comparando MHz absoluto".to_string());
    } else {
        out.push(format!(
            "AVISO: baseline de outra máquina ({} x {}): comparando MHz relativo ao cenário `cpu`; o próprio `cpu` não é checado",
            base["machine"]["cpu"].as_str().unwrap_or("?"),
            doc["machine"]["cpu"].as_str().unwrap_or("?"),
        ));
    }
    let cur_cpu = f(&doc["scenarios"]["cpu"], &["mhz_cpu", "median"]);
    let base_cpu = f(&base["scenarios"]["cpu"], &["mhz_cpu_median"]);
    for (name, b) in base["scenarios"].as_object().into_iter().flatten() {
        let Some(cur) = doc["scenarios"].get(name) else {
            out.push(format!("{name}: ausente da execução atual (use --only?)"));
            continue;
        };
        if cur["fingerprint"] != b["fingerprint"] {
            ok = false;
            out.push(format!(
                "FALHA {name}: a impressão digital mudou (ciclos/quadro/áudio): atual {} x baseline {}. Timing, vídeo ou áudio não podem mudar numa otimização",
                cur["fingerprint"], b["fingerprint"]
            ));
            continue;
        }
        let margin = f(b, &["margin_pct"]);
        let (cur_v, base_v, unit) = if same_machine {
            (
                f(cur, &["mhz_cpu", "median"]),
                f(b, &["mhz_cpu_median"]),
                "MHz",
            )
        } else if name == "cpu" {
            out.push(format!(
                "pulado {name}: sem referência comum entre máquinas"
            ));
            continue;
        } else {
            (
                f(cur, &["mhz_cpu", "median"]) / cur_cpu,
                f(b, &["mhz_cpu_median"]) / base_cpu,
                "x cpu",
            )
        };
        let limit = base_v * (1.0 - margin / 100.0);
        let delta = (cur_v / base_v - 1.0) * 100.0;
        if cur_v < limit {
            ok = false;
            out.push(format!(
                "FALHA {name}: {cur_v:.3} {unit} < limite {limit:.3} (baseline {base_v:.3}, margem {margin:.1} %, variação {delta:+.1} %)"
            ));
        } else {
            out.push(format!(
                "ok    {name}: {cur_v:.3} {unit} (baseline {base_v:.3}, margem {margin:.1} %, variação {delta:+.1} %)"
            ));
        }
    }
    out.push(if ok {
        "RESULTADO: sem regressão".into()
    } else {
        "RESULTADO: REGRESSÃO".into()
    });
    (ok, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(mhz: f64, cpu_mhz: f64, fp: &str, cpu: &str) -> Value {
        json!({
            "machine": {"cpu": cpu, "logical_cores": 4},
            "scenarios": {
                "cpu": {"mhz_cpu": {"median": cpu_mhz}, "fingerprint": "c"},
                "hgr": {"mhz_cpu": {"median": mhz}, "fingerprint": fp, "cv_pct": 2.0},
            }
        })
    }

    #[test]
    fn margin_has_a_15_percent_floor_and_follows_3_sigma() {
        assert_eq!(margin_pct(1.0), 15.0);
        assert_eq!(margin_pct(10.0), 30.0);
    }

    #[test]
    fn check_passes_within_margin_and_fails_beyond() {
        let base = baseline(&doc(100.0, 400.0, "a", "X"));
        assert!(check(&doc(90.0, 400.0, "a", "X"), &base).0);
        assert!(
            check(&doc(200.0, 400.0, "a", "X"), &base).0,
            "melhora nunca falha"
        );
        assert!(!check(&doc(80.0, 400.0, "a", "X"), &base).0);
    }

    #[test]
    fn check_fails_when_fingerprint_changes() {
        let base = baseline(&doc(100.0, 400.0, "a", "X"));
        assert!(!check(&doc(100.0, 400.0, "b", "X"), &base).0);
    }

    #[test]
    fn other_machine_compares_ratios() {
        let base = baseline(&doc(100.0, 400.0, "a", "X"));
        // máquina 2x mais lenta, mesma razão: passa; razão pior: falha
        assert!(check(&doc(50.0, 200.0, "a", "Y"), &base).0);
        assert!(!check(&doc(30.0, 200.0, "a", "Y"), &base).0);
    }
}
