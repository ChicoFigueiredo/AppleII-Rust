//! Mediana, p95 e dispersão das repetições.

use serde_json::{json, Value};

/// Percentil por interpolação linear (`q` em 0..=1) de valores já ordenados.
pub fn percentile(sorted: &[f64], q: f64) -> f64 {
    if sorted.len() == 1 {
        return sorted[0];
    }
    let pos = q * (sorted.len() - 1) as f64;
    let (lo, hi) = (pos.floor() as usize, pos.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (pos - lo as f64)
}

fn sorted(v: &[f64]) -> Vec<f64> {
    let mut s = v.to_vec();
    s.sort_by(|a, b| a.total_cmp(b));
    s
}

#[cfg(test)]
pub fn median(v: &[f64]) -> f64 {
    percentile(&sorted(v), 0.5)
}

/// Média e desvio-padrão amostral.
pub fn mean_stdev(v: &[f64]) -> (f64, f64) {
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    if v.len() < 2 {
        return (mean, 0.0);
    }
    let var = v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, var.sqrt())
}

fn field(runs: &[Value], k: &str) -> Vec<f64> {
    runs.iter().map(|r| r[k].as_f64().unwrap_or(0.0)).collect()
}

/// Resume as repetições de um cenário.
///
/// "p95" é sempre o pior caso: o percentil 95 do tempo (ou de CPU/memória), e as taxas
/// (quadros/s, MHz) derivadas dele, ou seja, a taxa que 95% das execuções igualaram ou superaram.
pub fn summarize(title: &str, runs: &[Value]) -> Result<Value, String> {
    let fp = &runs[0]["fingerprint"];
    if runs.iter().any(|r| &r["fingerprint"] != fp) {
        return Err(format!(
            "{title}: a impressão digital (ciclos/quadro/áudio) variou entre repetições: não determinístico"
        ));
    }
    let wall = field(runs, "wall_s");
    let cpu = field(runs, "cpu_s");
    let frames = runs[0]["frames"].as_f64().unwrap_or(0.0);
    let cycles = runs[0]["cycles"].as_f64().unwrap_or(0.0);
    let rss: Vec<f64> = field(runs, "rss_peak_kib")
        .iter()
        .map(|k| k / 1024.0)
        .collect();
    let cpu_pct: Vec<f64> = wall
        .iter()
        .zip(&cpu)
        .map(|(w, c)| c / w.max(1e-12) * 100.0)
        .collect();
    let mhz: Vec<f64> = wall.iter().map(|w| cycles / w.max(1e-12) / 1e6).collect();
    // Base do --check: MHz sobre o tempo de CPU da thread (cai para o relógio de parede onde
    // o /proc não existe). Não sofre com a thread ser preemptada numa máquina ocupada.
    let mhz_cpu: Vec<f64> = wall
        .iter()
        .zip(&cpu)
        .map(|(w, c)| cycles / (if *c > 0.0 { *c } else { *w }).max(1e-12) / 1e6)
        .collect();
    let (mhz_mean, mhz_sd) = mean_stdev(&mhz_cpu);
    let mhz_cpu_sorted = sorted(&mhz_cpu);
    let (ws, cs, rs) = (sorted(&wall), sorted(&cpu_pct), sorted(&rss));
    let (w50, w95) = (percentile(&ws, 0.5), percentile(&ws, 0.95));
    Ok(json!({
        "title": title,
        "frames": frames,
        "cycles": cycles,
        "audio_frames": runs[0]["audio_frames"],
        "wall_s": {"median": w50, "p95": w95},
        "fps": {"median": frames / w50, "p95": frames / w95},
        "mhz": {"median": cycles / w50 / 1e6, "p95": cycles / w95 / 1e6},
        "mhz_cpu": {
            "median": percentile(&mhz_cpu_sorted, 0.5), "p5": percentile(&mhz_cpu_sorted, 0.05),
            "best": mhz_cpu_sorted[mhz_cpu_sorted.len() - 1],
            "mean": mhz_mean, "stdev": mhz_sd,
        },
        "cpu_pct": {"median": percentile(&cs, 0.5), "p95": percentile(&cs, 0.95)},
        "rss_peak_mib": {"median": percentile(&rs, 0.5), "p95": percentile(&rs, 0.95)},
        "cv_pct": if mhz_mean > 0.0 { mhz_sd / mhz_mean * 100.0 } else { 0.0 },
        "mhz_runs": mhz,
        "mhz_cpu_runs": mhz_cpu,
        "fingerprint": fp,
        "audio_peak": runs[0]["audio_peak"],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_interpolate() {
        let v = [1.0, 2.0, 3.0, 4.0, 5.0];
        assert_eq!(percentile(&v, 0.5), 3.0);
        assert!((percentile(&v, 0.95) - 4.8).abs() < 1e-9);
        assert_eq!(percentile(&[7.0], 0.95), 7.0);
        assert_eq!(median(&[3.0, 1.0, 2.0]), 2.0);
    }

    #[test]
    fn stdev_is_sample_stdev() {
        let (m, sd) = mean_stdev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert_eq!(m, 5.0);
        assert!((sd - 2.138_089_935).abs() < 1e-6);
    }
}
