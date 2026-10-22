//! HU-IMP-080 / T-017: pressure on the spool with the real LX-310 engine: 1 MiB of bitmap, jobs of
//! 100+ pages, the limits of `Lx310Config` as visible diagnostics, bounded memory, nobody reading
//! the snapshot. Timings are printed (`--nocapture`) and bounded generously.
#![cfg(feature = "fs")]
mod common;
use apple2_printer::archive::{Archive, JobState};
use apple2_printer::capture::CloseReason;
use apple2_printer::lx310::{Lx310, Lx310Config};
use apple2_printer::paper::RenderOptions;
use apple2_printer::spool::{EngineFactory, Spool};
use common::*;
use std::time::Instant;

fn lx_spool(dir: &std::path::Path, cfg: Lx310Config) -> Spool {
    let f: EngineFactory = Box::new(move || Box::new(Lx310::new(cfg)));
    Spool::new(Archive::open(dir).unwrap(), f, RenderOptions::default())
}

/// `lines` graphic lines of `cols` columns each: ESC <cmd> n1 n2 data CR LF.
fn bitmap(cmd: &[u8], cols: usize, fill: u8, total: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(total + 1024);
    while v.len() < total {
        v.extend_from_slice(&[0x1B]);
        v.extend_from_slice(cmd);
        v.extend_from_slice(&[(cols & 0xFF) as u8, (cols >> 8) as u8]);
        v.extend(std::iter::repeat_n(fill, cols));
        v.extend_from_slice(b"\r\n");
    }
    v
}

fn run_job(s: &mut Spool, data: &[u8], chunk: usize) -> std::time::Duration {
    let t = Instant::now();
    s.handle(open(1));
    let mut off = 0u64;
    for c in data.chunks(chunk) {
        s.handle(bytes(1, off, c));
        off += c.len() as u64;
    }
    s.handle(close(1, CloseReason::Idle));
    t.elapsed()
}

fn raw_of(dir: &std::path::Path, s: &Spool) -> Vec<u8> {
    let snap = s.snapshot_handle().get();
    std::fs::read(dir.join(&snap.jobs[0].id).join("raw.bin")).unwrap()
}

/// Diagnostic messages stored in the manifest of the only job.
fn diags(dir: &std::path::Path, s: &Spool) -> Vec<String> {
    let snap = s.snapshot_handle().get();
    let m = s.archive().manifest(&snap.jobs[0].id).unwrap().clone();
    let _ = dir;
    m.diagnostics.iter().map(|d| d.message.clone()).collect()
}

#[test]
fn t017_dense_bitmap_1mib_keeps_every_byte_and_reports_the_dot_limit() {
    for (cmd, cols) in [(&b"K"[..], 480usize), (&b"L"[..], 960), (&b"*\x00"[..], 960)] {
        let dir = tempfile::tempdir().unwrap();
        let mut s = lx_spool(dir.path(), Lx310Config::default());
        let data = bitmap(cmd, cols, 0xFF, 1 << 20);
        let el = run_job(&mut s, &data, 4096);
        println!("bitmap {:?}: {} bytes in {:?}", cmd, data.len(), el);
        // No byte lost, whatever the engine did with it.
        assert_eq!(sha_hex(&raw_of(dir.path(), &s)), sha_hex(&data));
        let snap = s.snapshot_handle().get();
        assert_eq!(snap.jobs[0].state, Some(JobState::Completed));
        assert_eq!(snap.jobs[0].total_bytes, data.len() as u64);
        assert_eq!(snap.jobs[0].sha256.as_deref(), Some(sha_hex(&data).as_str()));
        assert!(el.as_secs() < 60, "too slow: {el:?}");
        // 1 MiB of full columns exceeds the default dot budget: the loss is announced.
        if cmd == b"K" {
            let d = diags(dir.path(), &s);
            assert!(d.iter().any(|m| m.contains("limite de pontos")), "{d:?}");
        }
    }
}

#[test]
fn t017_dot_limit_is_a_visible_diagnostic_not_silent_loss() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Lx310Config {
        max_dots: 20_000,
        ..Lx310Config::default()
    };
    let mut s = lx_spool(dir.path(), cfg);
    let data = bitmap(b"K", 480, 0xFF, 1 << 20);
    run_job(&mut s, &data, 4096);
    assert_eq!(sha_hex(&raw_of(dir.path(), &s)), sha_hex(&data));
    let d = diags(dir.path(), &s);
    assert!(
        d.iter().any(|m| m.contains("limite de pontos")),
        "diagnostics: {d:?}"
    );
    // Bounded memory: the kept document never exceeds the configured dots.
    let mut e = Lx310::new(cfg);
    apple2_printer::engine::PrinterEngine::begin(&mut e, RenderOptions::default());
    apple2_printer::engine::PrinterEngine::feed(&mut e, 0, &data);
    let dots: usize = apple2_printer::engine::PrinterEngine::document(&e)
        .pages
        .iter()
        .map(|p| p.dots.len())
        .sum();
    assert!(dots <= 20_000, "{dots}");
}

#[test]
fn t017_sparse_bitmap_1mib_within_limits_has_no_limit_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = lx_spool(dir.path(), Lx310Config::default());
    let data = bitmap(b"K", 480, 0x01, 1 << 20);
    let el = run_job(&mut s, &data, 4096);
    println!("sparse bitmap: {} bytes in {:?}", data.len(), el);
    assert_eq!(sha_hex(&raw_of(dir.path(), &s)), sha_hex(&data));
    let d = diags(dir.path(), &s);
    assert!(!d.iter().any(|m| m.contains("limite")), "{d:?}");
}

fn text_pages(n: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for p in 0..n {
        for l in 0..60 {
            v.extend_from_slice(format!("pagina {p} linha {l} ção\r\n").as_bytes());
        }
        v.push(0x0C);
    }
    v
}

#[test]
fn t017_job_of_120_pages_is_complete_and_timed() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = lx_spool(dir.path(), Lx310Config::default());
    let data = text_pages(120);
    let el = run_job(&mut s, &data, 512);
    println!("120 pages: {} bytes in {:?}", data.len(), el);
    assert_eq!(sha_hex(&raw_of(dir.path(), &s)), sha_hex(&data));
    let snap = s.snapshot_handle().get();
    assert!(snap.jobs[0].pages >= 120, "{}", snap.jobs[0].pages);
    assert!(el.as_secs() < 60, "too slow: {el:?}");
}

#[test]
fn t017_page_limit_is_a_visible_diagnostic_and_raw_stays_whole() {
    let dir = tempfile::tempdir().unwrap();
    let cfg = Lx310Config {
        max_pages: 50,
        ..Lx310Config::default()
    };
    let mut s = lx_spool(dir.path(), cfg);
    let data = text_pages(120);
    run_job(&mut s, &data, 512);
    assert_eq!(sha_hex(&raw_of(dir.path(), &s)), sha_hex(&data));
    let snap = s.snapshot_handle().get();
    assert!(snap.jobs[0].pages <= 50);
    let d = diags(dir.path(), &s);
    assert!(
        d.iter().any(|m| m.contains("limite de páginas")),
        "{d:?}"
    );
}

/// Nobody reads the snapshot (UI blocked): the spool keeps going and the notices stay bounded.
#[test]
fn t017_nobody_reading_the_snapshot_does_not_stall_or_grow_notices() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = lx_spool(dir.path(), Lx310Config::default());
    // Out-of-order and duplicate events produce notices; they must stay capped.
    s.handle(open(1));
    for i in 0..500u64 {
        s.handle(bytes(7, i, b"x")); // unknown job
    }
    let snap = s.snapshot_handle().get();
    assert!(snap.notices.len() <= 30, "{}", snap.notices.len());
    s.handle(bytes(1, 0, b"ok\r\n"));
    s.handle(close(1, CloseReason::Idle));
    assert_eq!(raw_of(dir.path(), &s), b"ok\r\n");
}
