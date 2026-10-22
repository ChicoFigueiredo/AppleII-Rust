//! End-to-end latency probe (HU-IMP-032): from the byte the Apple II writes to the printer card to
//! the Central frame that shows its dot, presented by the window's GPU surface.
//!
//! Three stamps travel with a mark `(job, end offset)` instead of the event itself (the capture
//! contract stays untouched): the card stamps the first byte of each block it holds back, the
//! spool stamps the snapshot whose document covers that offset, and the Central stamps the present
//! of the first frame drawn from such a snapshot. Off by default (one relaxed atomic load on the
//! hot path); `APPLE2_RUST_PRINTER_LATENCY=1` turns it on and prints P50/P95 at exit.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Environment variable that enables the probe in the application.
pub const ENV: &str = "APPLE2_RUST_PRINTER_LATENCY";
/// Marks kept waiting for a present (oldest dropped first; a closed Central never consumes them).
const MAX_PENDING: usize = 4096;
const MAX_SAMPLES: usize = 100_000;

#[derive(Clone, Copy, Debug)]
struct Mark {
    job: u64,
    end: u64,
    written: Instant,
    published: Option<Instant>,
}

/// One finished measurement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    /// Written to the card until the snapshot with the dot was published by the spool.
    pub to_snapshot: Duration,
    /// Written to the card until the present of the frame that shows it.
    pub to_screen: Duration,
}

/// Percentiles of the samples taken so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Report {
    pub count: usize,
    pub snapshot_p50: Duration,
    pub snapshot_p95: Duration,
    pub screen_p50: Duration,
    pub screen_p95: Duration,
    pub screen_max: Duration,
}

impl Report {
    pub fn text(&self) -> String {
        if self.count == 0 {
            return "printer latency: no samples (nothing was presented in the Central)".into();
        }
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        format!(
            "printer latency ({} samples): byte on the card -> snapshot P50 {:.1} ms, P95 {:.1} ms; byte -> frame presented P50 {:.1} ms, P95 {:.1} ms, max {:.1} ms",
            self.count,
            ms(self.snapshot_p50),
            ms(self.snapshot_p95),
            ms(self.screen_p50),
            ms(self.screen_p95),
            ms(self.screen_max)
        )
    }
}

/// The probe's state; the process-wide instance is behind [`written`], [`published`], [`presented`].
#[derive(Default)]
pub struct Probe {
    pending: VecDeque<Mark>,
    samples: Vec<Sample>,
}

impl Probe {
    pub fn new() -> Probe {
        Probe::default()
    }

    /// The card holds a block whose first byte ends at offset `end` of `job`.
    pub fn written(&mut self, job: u64, end: u64, now: Instant) {
        if self.pending.len() >= MAX_PENDING {
            self.pending.pop_front();
        }
        self.pending.push_back(Mark {
            job,
            end,
            written: now,
            published: None,
        });
    }

    /// A snapshot whose document covers the first `bytes` of `job` was published.
    pub fn published(&mut self, job: u64, bytes: u64, now: Instant) {
        for m in self.pending.iter_mut().filter(|m| m.job == job) {
            if m.end <= bytes && m.published.is_none() {
                m.published = Some(now);
            }
        }
    }

    /// A frame drawn from a document covering the first `bytes` of `job` reached the screen.
    pub fn presented(&mut self, job: u64, bytes: u64, now: Instant) {
        let samples = &mut self.samples;
        self.pending.retain(|m| {
            if m.job != job || m.end > bytes {
                return true;
            }
            if samples.len() < MAX_SAMPLES {
                samples.push(Sample {
                    to_snapshot: m
                        .published
                        .unwrap_or(now)
                        .saturating_duration_since(m.written),
                    to_screen: now.saturating_duration_since(m.written),
                });
            }
            false
        });
    }

    pub fn samples(&self) -> &[Sample] {
        &self.samples
    }

    pub fn report(&self) -> Report {
        let pick = |f: fn(&Sample) -> Duration, q: f64| -> Duration {
            let mut v: Vec<Duration> = self.samples.iter().map(f).collect();
            v.sort();
            if v.is_empty() {
                return Duration::ZERO;
            }
            let i = ((v.len() as f64 * q).ceil() as usize).clamp(1, v.len()) - 1;
            v[i]
        };
        Report {
            count: self.samples.len(),
            snapshot_p50: pick(|s| s.to_snapshot, 0.50),
            snapshot_p95: pick(|s| s.to_snapshot, 0.95),
            screen_p50: pick(|s| s.to_screen, 0.50),
            screen_p95: pick(|s| s.to_screen, 0.95),
            screen_max: pick(|s| s.to_screen, 1.0),
        }
    }
}

static ENABLED: AtomicBool = AtomicBool::new(false);
static PROBE: OnceLock<Mutex<Probe>> = OnceLock::new();

fn with(f: impl FnOnce(&mut Probe)) {
    let m = PROBE.get_or_init(|| Mutex::new(Probe::new()));
    f(&mut m.lock().unwrap_or_else(|e| e.into_inner()));
}

/// Turns the process-wide probe on.
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

#[inline]
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Card side: the first byte of a block, ending at `end`, was written.
pub fn written(job: u64, end: u64) {
    if enabled() {
        let now = Instant::now();
        with(|p| p.written(job, end, now));
    }
}

/// Spool side: a snapshot covering `bytes` of `job` was published.
pub fn published(job: u64, bytes: u64) {
    if enabled() {
        let now = Instant::now();
        with(|p| p.published(job, bytes, now));
    }
}

/// Window side: call right after the surface's `present()`.
pub fn presented(job: u64, bytes: u64) {
    if enabled() {
        let now = Instant::now();
        with(|p| p.presented(job, bytes, now));
    }
}

/// Percentiles so far (all zero when the probe never ran).
pub fn report() -> Report {
    let mut r = None;
    with(|p| r = Some(p.report()));
    r.expect("the closure ran")
}

/// Prints the report to stderr, once per process, when the probe is on (call on exit).
pub fn print_report_once() {
    static DONE: AtomicBool = AtomicBool::new(false);
    if enabled() && !DONE.swap(true, Ordering::Relaxed) {
        eprintln!("{}", report().text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(base: Instant, ms: u64) -> Instant {
        base + Duration::from_millis(ms)
    }

    #[test]
    fn a_mark_becomes_a_sample_when_a_frame_covering_it_is_presented() {
        let t = Instant::now();
        let mut p = Probe::new();
        p.written(1, 10, t);
        p.published(1, 10, at(t, 30));
        // A frame drawn from an older document does not complete it.
        p.presented(1, 9, at(t, 40));
        assert!(p.samples().is_empty());
        // Another job's frame neither.
        p.presented(2, 99, at(t, 41));
        assert!(p.samples().is_empty());
        p.presented(1, 10, at(t, 55));
        assert_eq!(
            p.samples(),
            &[Sample {
                to_snapshot: Duration::from_millis(30),
                to_screen: Duration::from_millis(55)
            }]
        );
        // Consumed once.
        p.presented(1, 10, at(t, 99));
        assert_eq!(p.samples().len(), 1);
    }

    #[test]
    fn percentiles_use_the_nearest_rank() {
        let t = Instant::now();
        let mut p = Probe::new();
        for i in 1..=100u64 {
            p.written(1, i, t);
            p.presented(1, i, at(t, i));
        }
        let r = p.report();
        assert_eq!(r.count, 100);
        assert_eq!(r.screen_p50, Duration::from_millis(50));
        assert_eq!(r.screen_p95, Duration::from_millis(95));
        assert_eq!(r.screen_max, Duration::from_millis(100));
        assert!(r.text().contains("P95 95.0 ms"));
    }

    #[test]
    fn pending_marks_are_bounded() {
        let t = Instant::now();
        let mut p = Probe::new();
        for i in 0..(MAX_PENDING as u64 + 10) {
            p.written(1, i + 1, t);
        }
        p.presented(1, u64::MAX, t);
        assert_eq!(p.samples().len(), MAX_PENDING);
    }
}
