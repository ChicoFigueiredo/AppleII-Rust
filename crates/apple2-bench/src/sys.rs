//! Leitura de métricas do sistema (Linux: /proc) e identificação da máquina/compilação.
//! Fora do Linux os campos ficam em 0 ou "desconhecido": o benchmark continua valendo para
//! frames/s e MHz, só perde CPU/memória.

use std::process::Command;

/// Tempo de CPU da thread atual em segundos (`/proc/thread-self/schedstat`, resolução de ns).
pub fn thread_cpu_secs() -> f64 {
    std::fs::read_to_string("/proc/thread-self/schedstat")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
        })
        .map_or(0.0, |ns| ns as f64 / 1e9)
}

fn status_kb(field: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix(field).and_then(|r| {
                    r.trim_start_matches(':')
                        .split_whitespace()
                        .next()?
                        .parse()
                        .ok()
                })
            })
        })
        .unwrap_or(0)
}

/// Pico de RSS do processo (`VmHWM`), em KiB.
pub fn rss_peak_kb() -> u64 {
    status_kb("VmHWM")
}

fn cmd(program: &str, args: &[&str]) -> Option<String> {
    let out = Command::new(program).args(args).output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Máquina, SO, repositório e toolchain.
pub fn machine_info() -> serde_json::Value {
    let cpuinfo = std::fs::read_to_string("/proc/cpuinfo").unwrap_or_default();
    let model = cpuinfo
        .lines()
        .find(|l| l.starts_with("model name"))
        .and_then(|l| l.split(':').nth(1))
        .map_or("desconhecido".to_string(), |s| s.trim().to_string());
    let os = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
        })
        .unwrap_or_else(|| std::env::consts::OS.to_string());
    let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let mem_kb = std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("MemTotal:")?
                    .split_whitespace()
                    .next()?
                    .parse::<u64>()
                    .ok()
            })
        })
        .unwrap_or(0);
    let governor = std::fs::read_to_string("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor")
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "n/d".into());
    let dirty = cmd("git", &["status", "--porcelain", "--untracked-files=no"])
        .is_some_and(|s| !s.is_empty());
    serde_json::json!({
        "cpu": model,
        "logical_cores": std::thread::available_parallelism().map_or(0, |n| n.get()),
        "mem_total_mib": mem_kb / 1024,
        "os": format!("{os} (kernel {kernel}, {})", std::env::consts::ARCH),
        "cpu_governor": governor,
        "git_sha": cmd("git", &["rev-parse", "HEAD"]).unwrap_or_else(|| "desconhecido".into()),
        "git_dirty": dirty,
        "rustc": cmd("rustc", &["-V"]).unwrap_or_else(|| "desconhecido".into()),
        "profile": if cfg!(debug_assertions) { "debug" } else { "release (opt-level 3, lto thin)" },
    })
}
