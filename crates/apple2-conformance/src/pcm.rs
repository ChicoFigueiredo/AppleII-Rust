//! PCM comparison for the audio conformance cases (policy v2: docs/conformance/test-oracles.md,
//! "Áudio: Mockingboard, Phasor, SSI263 e alto-falante").
//!
//! Streams are 16-bit samples, interleaved when `channels > 1`. `expected` is the oracle's,
//! `actual` the Rust machine's. After one global lag `L` (`expected[i] ~ actual[i + L]`, the
//! start-up offset between the two timelines, |L| <= [`GLOBAL_LAG`]):
//!
//! * M0: every sample is equal (exact);
//! * M1: the set of distinct sample values is the same on both sides (informative);
//! * M2: maximum absolute error and RMS error sample by sample (informative: the AppleWin
//!   timeline moves with how the host slices time, so square-wave edges jitter);
//! * M3, per window of [`WINDOW`] samples, the relative difference of the RMS, of the
//!   zero-crossing rate (frequency) and of the peak amplitude. Only M3 decides, with limits
//!   calibrated on the oracle against itself.

use std::collections::BTreeSet;

/// Largest global lag searched.
pub const GLOBAL_LAG: i64 = 735;
/// Window length in samples per channel (100 ms at 44.1 kHz).
pub const WINDOW: usize = 4410;

/// Thresholds of the decision rule (calibrated, see audit/audio-differential.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub f_tol: f64,
    pub rms_tol: f64,
    pub peak_tol: f64,
}

/// What was measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Measures {
    pub channels: usize,
    pub frames_expected: usize,
    pub frames_actual: usize,
    /// Global lag: `expected[i] ~ actual[i + lag]`.
    pub lag: i64,
    /// M0.
    pub exact: bool,
    /// M1.
    pub value_sets_equal: bool,
    /// M2 (informative).
    pub max_abs_err: i32,
    pub rms_err: f64,
    pub windows: usize,
    /// Windows silent on both sides (not measured).
    pub silent_windows: usize,
    /// M3: worst relative differences over the measured windows.
    pub worst_f: f64,
    pub worst_rms: f64,
    pub worst_peak: f64,
    /// Window with the worst difference (for the report).
    pub worst_window: usize,
}

/// Verdict of [`decide`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// M0: identical samples.
    Exact,
    /// M3 within the limits.
    Tolerance,
    Fail(String),
}

fn channel(v: &[i16], channels: usize, c: usize) -> Vec<i16> {
    v.iter().skip(c).step_by(channels).copied().collect()
}

fn get(a: &[i16], i: i64) -> i32 {
    if i < 0 || i as usize >= a.len() {
        0
    } else {
        a[i as usize] as i32
    }
}

/// Zero-crossing count (sign changes of non-zero samples).
fn crossings(a: impl Iterator<Item = i32>) -> usize {
    let (mut last, mut n) = (0i32, 0);
    for s in a.filter(|s| *s != 0) {
        let sign = s.signum();
        if last != 0 && sign != last {
            n += 1;
        }
        last = sign;
    }
    n
}

fn rms(a: impl Iterator<Item = i32> + Clone) -> f64 {
    let n = a.clone().count().max(1) as f64;
    (a.map(|s| (s as f64) * (s as f64)).sum::<f64>() / n).sqrt()
}

fn rel(a: f64, b: f64) -> f64 {
    if a == 0.0 && b == 0.0 {
        0.0
    } else {
        (a - b).abs() / a.max(b)
    }
}

/// Best global lag over the first non-silent stretch of `expected`.
fn global_lag(e: &[i16], a: &[i16]) -> i64 {
    let first = e.iter().position(|&s| s != 0).unwrap_or(0);
    let span = WINDOW.min(e.len().saturating_sub(first));
    let mut best = (u64::MAX, 0i64);
    for lag in -GLOBAL_LAG..=GLOBAL_LAG {
        let err: u64 = (first..first + span)
            .map(|i| (e[i] as i64 - get(a, i as i64 + lag) as i64).unsigned_abs())
            .sum();
        if err < best.0 || (err == best.0 && lag.abs() < best.1.abs()) {
            best = (err, lag);
        }
    }
    best.1
}

/// Measures `actual` against `expected` (no verdict).
pub fn measure(expected: &[i16], actual: &[i16], channels: usize) -> Measures {
    let mut m = Measures {
        channels,
        frames_expected: expected.len() / channels,
        frames_actual: actual.len() / channels,
        lag: 0,
        exact: true,
        value_sets_equal: true,
        max_abs_err: 0,
        rms_err: 0.0,
        windows: 0,
        silent_windows: 0,
        worst_f: 0.0,
        worst_rms: 0.0,
        worst_peak: 0.0,
        worst_window: 0,
    };
    let (mut sq, mut count) = (0f64, 0usize);
    for c in 0..channels {
        let e = channel(expected, channels, c);
        let a = channel(actual, channels, c);
        let lag = global_lag(&e, &a);
        if c == 0 {
            m.lag = lag;
        }
        // Overlap under the global lag; with a negative lag the first `-lag` expected samples
        // have no counterpart.
        let n = e.len().min((a.len() as i64 - lag).max(0) as usize);
        let lo = (-lag).max(0) as usize;
        let (mut es, mut as_) = (BTreeSet::new(), BTreeSet::new());
        for (i, &x) in e.iter().enumerate().take(n).skip(lo) {
            let y = a[(i as i64 + lag) as usize];
            es.insert(x);
            as_.insert(y);
            let d = (x as i32 - y as i32).abs();
            m.exact &= d == 0;
            m.max_abs_err = m.max_abs_err.max(d);
            sq += (d as f64) * (d as f64);
            count += 1;
        }
        m.value_sets_equal &= es == as_;
        for (w, start) in (lo..n).step_by(WINDOW).enumerate() {
            // Only whole windows count (a short tail would make the rates noisy); a stream
            // shorter than one window is measured as a single window.
            let end = if start + WINDOW <= n {
                start + WINDOW
            } else if w == 0 {
                n
            } else {
                break;
            };
            let ev = || (start..end).map(|i| e[i] as i32);
            let av = || (start..end).map(|i| get(&a, i as i64 + lag));
            if ev().all(|s| s == 0) && av().all(|s| s == 0) {
                m.silent_windows += 1;
                continue;
            }
            m.windows += 1;
            let f = rel(crossings(ev()) as f64, crossings(av()) as f64);
            let r = rel(rms(ev()), rms(av()));
            let p = rel(
                ev().map(|s| s.abs()).max().unwrap_or(0) as f64,
                av().map(|s| s.abs()).max().unwrap_or(0) as f64,
            );
            let worst = f.max(r).max(p);
            if worst > m.worst_f.max(m.worst_rms).max(m.worst_peak) {
                m.worst_window = w;
            }
            m.worst_f = m.worst_f.max(f);
            m.worst_rms = m.worst_rms.max(r);
            m.worst_peak = m.worst_peak.max(p);
        }
    }
    m.rms_err = (sq / count.max(1) as f64).sqrt();
    m
}

/// The decision rule: M0 -> exact; else M3 within `limits` -> tolerance; else fail.
pub fn decide(m: &Measures, limits: &Limits) -> Verdict {
    if m.exact && m.frames_actual.abs_diff(m.frames_expected) <= GLOBAL_LAG as usize + 1 {
        return Verdict::Exact;
    }
    let mut why = Vec::new();
    if m.worst_f > limits.f_tol {
        why.push(format!(
            "frequência {:.2} % > {:.2} %",
            m.worst_f * 100.0,
            limits.f_tol * 100.0
        ));
    }
    if m.worst_rms > limits.rms_tol {
        why.push(format!(
            "RMS {:.2} % > {:.2} %",
            m.worst_rms * 100.0,
            limits.rms_tol * 100.0
        ));
    }
    if m.worst_peak > limits.peak_tol {
        why.push(format!(
            "pico {:.2} % > {:.2} %",
            m.worst_peak * 100.0,
            limits.peak_tol * 100.0
        ));
    }
    if why.is_empty() {
        Verdict::Tolerance
    } else {
        Verdict::Fail(why.join("; "))
    }
}

/// Reads raw little-endian 16-bit samples.
pub fn read_s16le(bytes: &[u8]) -> Vec<i16> {
    bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: Limits = Limits {
        f_tol: 0.01,
        rms_tol: 0.02,
        peak_tol: 0.02,
    };

    fn square(n: usize, half: usize, amp: i16) -> Vec<i16> {
        (0..n)
            .map(|i| {
                if (i / half).is_multiple_of(2) {
                    amp
                } else {
                    -amp
                }
            })
            .collect()
    }

    #[test]
    fn identical_streams_are_exact_and_a_shift_is_found_as_the_global_lag() {
        let e = square(20_000, 50, 8000);
        let m = measure(&e, &e, 1);
        assert!(m.exact && m.lag == 0 && m.value_sets_equal);
        assert_eq!(decide(&m, &L), Verdict::Exact);
        let mut a = vec![0i16; 17];
        a.extend_from_slice(&e);
        let m = measure(&e, &a, 1);
        assert_eq!(m.lag, 17);
        assert!(m.exact);
    }

    #[test]
    fn edge_jitter_is_a_tolerance_pass_and_a_wrong_pitch_or_level_fails() {
        let e = square(20_000, 50, 8000);
        // One sample of jitter at some edges: not exact, but the same tone.
        let mut jitter = e.clone();
        for i in (50..jitter.len() - 1).step_by(400) {
            jitter[i] = -jitter[i];
        }
        let m = measure(&e, &jitter, 1);
        assert!(!m.exact && m.value_sets_equal);
        assert_eq!(decide(&m, &L), Verdict::Tolerance, "{m:?}");
        // A tone an octave up: the crossing rate doubles.
        let up = square(20_000, 25, 8000);
        match decide(&measure(&e, &up, 1), &L) {
            Verdict::Fail(why) => assert!(why.contains("frequência"), "{why}"),
            v => panic!("{v:?}"),
        }
        // Half the level: RMS and peak differ.
        let quiet = square(20_000, 50, 4000);
        match decide(&measure(&e, &quiet, 1), &L) {
            Verdict::Fail(why) => assert!(why.contains("RMS") && why.contains("pico"), "{why}"),
            v => panic!("{v:?}"),
        }
    }

    #[test]
    fn silence_on_both_sides_is_exact_and_a_missing_tone_fails() {
        let z = vec![0i16; 9000];
        let m = measure(&z, &z, 2);
        assert!(m.exact);
        assert_eq!(m.windows, 0);
        assert_eq!(decide(&m, &L), Verdict::Exact);
        let e = square(9000, 50, 8000);
        assert!(matches!(decide(&measure(&e, &z, 1), &L), Verdict::Fail(_)));
    }

    #[test]
    fn s16le_round_trip() {
        assert_eq!(read_s16le(&[0x01, 0x00, 0xFF, 0xFF, 0x80]), vec![1, -1]);
    }
}
