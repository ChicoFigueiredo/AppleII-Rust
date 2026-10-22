//! HU-IMP-080: the live preview is cheap. The document is copied rarely (more rarely the more it
//! costs) and not at all when nobody looks; the first stroke reaches the snapshot fast.
#![cfg(feature = "fs")]
mod common;
use apple2_printer::archive::Archive;
use apple2_printer::capture::CloseReason;
use apple2_printer::lx310::{Lx310, Lx310Config};
use apple2_printer::paper::RenderOptions;
use apple2_printer::spool::{EngineFactory, Spool, SpoolService};
use common::*;
use std::sync::Arc;
use std::time::{Duration, Instant};

fn lx_spool(dir: &std::path::Path) -> Spool {
    let f: EngineFactory = Box::new(|| Box::new(Lx310::new(Lx310Config::default())));
    Spool::new(Archive::open(dir).unwrap(), f, RenderOptions::default())
}

fn listing(pages: usize) -> Vec<u8> {
    let mut v = Vec::new();
    for p in 0..pages {
        for l in 0..60 {
            let text = format!("{:>5} PRINT \"PAGINA {p} LINHA {l}\":GOTO 10 ", l * 10);
            let line: String = text.chars().cycle().take(72).collect();
            v.extend_from_slice(line.as_bytes());
            v.extend_from_slice(b"\r\n");
        }
        v.push(0x0C);
    }
    v
}

#[test]
fn copies_of_the_document_are_rare_and_an_unwatched_job_makes_none() {
    let data = listing(60);
    // Watched: a reader polls after every event.
    let dir = tempfile::tempdir().unwrap();
    let mut s = lx_spool(dir.path());
    let h = s.snapshot_handle();
    s.handle(open(1));
    let (mut events, mut copies) = (0, 0);
    let mut last = Arc::as_ptr(&h.get().active.as_ref().unwrap().document);
    let t = Instant::now();
    for (i, c) in data.chunks(256).enumerate() {
        s.handle(bytes(1, (i * 256) as u64, c));
        events += 1;
        let p = Arc::as_ptr(&h.get().active.as_ref().unwrap().document);
        if p != last {
            copies += 1;
            last = p;
        }
    }
    let watched_time = t.elapsed();
    s.handle(close(1, CloseReason::Idle));
    println!("watched: {events} events, {copies} copies, {watched_time:?}");
    assert!(copies * 4 < events, "{copies} copies for {events} events");

    // Unwatched: one read, then nobody (window of 0 ms: the read is already "old").
    let dir = tempfile::tempdir().unwrap();
    let mut s = lx_spool(dir.path());
    s.set_watch_window(Duration::ZERO);
    let h = s.snapshot_handle();
    s.handle(open(1));
    std::thread::sleep(Duration::from_millis(5));
    let first = Arc::as_ptr(&h.get().active.as_ref().unwrap().document);
    std::thread::sleep(Duration::from_millis(5));
    let t = Instant::now();
    for (i, c) in data.chunks(256).enumerate() {
        s.handle(bytes(1, (i * 256) as u64, c));
    }
    let unwatched_time = t.elapsed();
    let snap = h.get();
    assert_eq!(Arc::as_ptr(&snap.active.as_ref().unwrap().document), first);
    println!("unwatched: {unwatched_time:?}");
    assert!(unwatched_time <= watched_time + Duration::from_millis(50));
    s.handle(close(1, CloseReason::Idle));
    // Closing publishes the full document for whoever looks next.
    let snap = h.get();
    assert!(snap.jobs[0].pages >= 60);
}

/// P95 of "event reaches the spool thread -> a snapshot carries the first dot".
#[test]
fn first_stroke_latency_p95() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    let svc = SpoolService::spawn(lx_spool(dir.path()), rx);
    let h = svc.handle();
    let mut lat = Vec::new();
    for job in 1..=60u64 {
        let t = Instant::now();
        tx.send(open(job)).unwrap();
        tx.send(bytes(job, 0, b"A")).unwrap();
        loop {
            let snap = h.snapshot();
            let seen = snap.active.as_ref().is_some_and(|a| {
                a.capture_job == job && a.document.pages.iter().any(|p| !p.dots.is_empty())
            });
            if seen {
                break;
            }
            assert!(
                t.elapsed() < Duration::from_secs(5),
                "first stroke never shown"
            );
            std::thread::sleep(Duration::from_micros(200));
        }
        lat.push(t.elapsed().as_secs_f64() * 1000.0);
        tx.send(close(job, CloseReason::Idle)).unwrap();
        std::thread::sleep(Duration::from_millis(15));
    }
    lat.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let (p50, p95) = (
        lat[lat.len() / 2],
        lat[(lat.len() as f64 * 0.95) as usize - 1],
    );
    println!("primeiro traco: P50 {p50:.1} ms, P95 {p95:.1} ms");
    assert!(p95 < 250.0, "P95 {p95:.1} ms");
    drop(tx);
    svc.join();
}
