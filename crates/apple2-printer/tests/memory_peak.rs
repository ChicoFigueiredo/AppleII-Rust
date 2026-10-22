//! Worst-case memory of one job (R07/HU-IMP-080): a dense ESC L stream of 16.5 MB, fed through the
//! Spool with a reader polling the snapshot, up to the per-job dot limit and the close. Run with
//! `cargo test --release -p apple2-printer --test memory_peak -- --ignored --nocapture`; the peak
//! resident set (VmHWM) is compared against the ceiling documented in docs/printer/desempenho.md.
#![cfg(all(feature = "fs", target_os = "linux"))]
mod common;
use apple2_printer::archive::Archive;
use apple2_printer::capture::CloseReason;
use apple2_printer::lx310::{Lx310, Lx310Config};
use apple2_printer::paper::RenderOptions;
use apple2_printer::spool::{EngineFactory, Spool};
use common::*;

fn status_kb(key: &str) -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find(|l| l.starts_with(key))
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
}

/// 900 solid columns per line (inside the right margin), one line every 24/216", about 16.5 MB.
fn dense_stream() -> Vec<u8> {
    let mut v = Vec::with_capacity(17_000_000);
    while v.len() < 16_500_000 {
        v.extend_from_slice(&[0x1B, b'L', 0x84, 0x03]);
        v.extend(std::iter::repeat_n(0xFF, 900));
        v.extend_from_slice(&[b'\r', 0x1B, b'J', 24]);
    }
    v
}

#[test]
#[ignore = "memory measurement, ~1 GB of RSS"]
fn peak_rss_of_a_dense_job_stays_under_the_documented_ceiling() {
    let data = dense_stream();
    let dir = tempfile::tempdir().unwrap();
    let f: EngineFactory = Box::new(|| Box::new(Lx310::new(Lx310Config::default())));
    let mut s = Spool::new(
        Archive::open(dir.path()).unwrap(),
        f,
        RenderOptions::default(),
    );
    let h = s.snapshot_handle();
    let base = status_kb("VmRSS:");
    s.handle(open(1));
    for (i, c) in data.chunks(4096).enumerate() {
        s.handle(bytes(1, (i * 4096) as u64, c));
        let _ = h.get(); // a reader that never stops
        if i % 64 == 0 {
            s.tick();
        }
    }
    let before_close = status_kb("VmHWM:");
    s.handle(close(1, CloseReason::User));
    s.publish_now();
    let peak = status_kb("VmHWM:");
    let pages = h.get().active.as_ref().map(|a| a.document.pages.len());
    println!(
        "stream {} bytes, pages {pages:?}, base {} MiB, peak before close {} MiB, peak {} MiB",
        data.len(),
        base / 1024,
        before_close / 1024,
        peak / 1024
    );
    assert!(peak / 1024 <= 850, "peak RSS {} MiB", peak / 1024);
}
