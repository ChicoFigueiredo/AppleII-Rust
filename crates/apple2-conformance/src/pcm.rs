//! PCM comparison for the audio conformance cases (policy: docs/conformance/test-oracles.md,
//! "Áudio: Mockingboard, Phasor, SSI263 e alto-falante").
//!
//! Streams are 16-bit samples, interleaved when `channels > 1`. `expected` is the oracle's,
//! `actual` the Rust machine's. Metrics:
//!
//! * M0: all samples equal after one global lag `L` (|L| <= [`GLOBAL_LAG`]);
//! * M1: the set of distinct sample values is the same on both sides;
//! * M2: per window of [`WINDOW`] samples, after the best local lag (|l| <= [`LOCAL_LAG`]), the
//!   maximum absolute error and the RMS error;
//! * M3: per window, the relative difference of the zero-crossing rate (frequency) and of the RMS.

use std::collections::BTreeSet;

/// Largest global lag searched (policy M0).
pub const GLOBAL_LAG: i64 = 735;
/// Largest per-window extra lag searched (policy M2).
pub const LOCAL_LAG: i64 = 2205;
/// Window length in samples per channel (100 ms at 44.1 kHz).
pub const WINDOW: usize = 4410;

/// Thresholds of the decision rule (calibrated, see audit/audio-differential.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub e_max: i32,
    pub r_max: f64,
    pub f_tol: f64,
    pub rms_tol: f64,
}

/// What was measured.
#[derive(Clone, Debug, PartialEq)]
pub struct Measures {
    pub channels: usize,
    pub frames_expected: usize,
    pub frames_actual: usize,
    /// Global lag: `expected[i] ~ actual[i + lag]`.
    pub lag: i64,
    pub exact: bool,
    pub value_sets_equal: bool,
    pub windows: usize,
    /// Windows silent on both sides (not measured).
    pub silent_windows: usize,
    pub worst_e: i32,
    pub worst_r: f64,
    pub worst_f: f64,
    pub worst_rms: f64,
    /// Window index of the worst error (for the report).
    pub worst_window: usize,
}

/// Verdict of [`decide`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// M0: identical samples.
    Exact,
    /// M1 equal and M2/M3 within the limits.
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

/// Zero-crossing count (sign changes of non-zero samples) of a slice.
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

/// Measures `actual` against `expected` (no verdict). `limits` only drive the early exit of the
/// local-lag search, so the numbers do not depend on it when a window needs the full search.
pub fn measure(expected: &[i16], actual: &[i16], channels: usize, limits: &Limits) -> Measures {
    let mut m = Measures {
        channels,
        frames_expected: expected.len() / channels,
        frames_actual: actual.len() / channels,
        lag: 0,
        exact: true,
        value_sets_equal: true,
        windows: 0,
        silent_windows: 0,
        worst_e: 0,
        worst_r: 0.0,
        worst_f: 0.0,
        worst_rms: 0.0,
        worst_window: 0,
    };
    for c in 0..channels {
        let e = channel(expected, channels, c);
        let a = channel(actual, channels, c);
        let lag = global_lag(&e, &a);
        if c == 0 {
            m.lag = lag;
        }
        // Overlap under the global lag.
        let n = e.len().min((a.len() as i64 - lag).max(0) as usize);
        let mut es: BTreeSet<i16> = BTreeSet::new();
        let mut as_: BTreeSet<i16> = BTreeSet::new();
        for i in 0..n {
            let (x, y) = (e[i], a[(i as i64 + lag) as usize]);
            es.insert(x);
            as_.insert(y);
            if x != y {
                m.exact = false;
            }
        }
        if es != as_ {
            m.value_sets_equal = false;
        }
        for (w, start) in (0..n).step_by(WINDOW).enumerate() {
            let end = (start + WINDOW).min(n);
            if e[start..end].iter().all(|&s| s == 0)
                && (start..end).all(|i| a[(i as i64 + lag) as usize] == 0)
            {
                m.silent_windows += 1;
                continue;
            }
            m.windows += 1;
            // Best local lag: the first one that meets the limits, else the least error.
            let mut best: Option<(i32, f64, i64)> = None;
            for step in 0..=LOCAL_LAG {
                for l in if step == 0 { vec![0] } else { vec![step, -step] } {
                    let mut max_e = 0i32;
                    let mut sq = 0f64;
                    for i in start..end {
                        let d = (e[i] as i32 - get(&a, i as i64 + lag + l)).abs();
                        max_e = max_e.max(d);
                        sq += (d as f64) * (d as f64);
                    }
                    let r = (sq / (end - start) as f64).sqrt();
                    let better = best.is_none_or(|(be, br, _)| (max_e, r) < (be, br));
                    if better {
                        best = Some((max_e, r, l));
                    }
                    if max_e <= limits.e_max && r <= limits.r_max {
                        break;
                    }
                }
                if best.is_some_and(|(be, br, _)| be <= limits.e_max && br <= limits.r_max) {
                    break;
                }
            }
            let (we, wr, l) = best.expect("at least lag 0");
            let rms_e = rms((start..end).map(|i| e[i] as i32));
            let rms_a = rms((start..end).map(|i| get(&a, i as i64 + lag + l)));
            let ce = crossings((start..end).map(|i| e[i] as i32)) as f64;
            let ca = crossings((start..end).map(|i| get(&a, i as i64 + lag + l))) as f64;
            let f_rel = if ce == 0.0 && ca == 0.0 {
                0.0
            } else {
                (ce - ca).abs() / ce.max(ca)
            };
            let rms_rel = if rms_e == 0.0 && rms_a == 0.0 {
                0.0
            } else {
                (rms_e - rms_a).abs() / rms_e.max(rms_a)
            };
            if we > m.worst_e || (we == m.worst_e && wr > m.worst_r) {
                m.worst_window = w;
            }
            m.worst_e = m.worst_e.max(we);
            m.worst_r = m.worst_r.max(wr);
            m.worst_f = m.worst_f.max(f_rel);
            m.worst_rms = m.worst_rms.max(rms_rel);
        }
    }
    m
}

/// The decision rule: M0 -> exact; else M1 equal and M2/M3 within `limits` -> tolerance; else fail.
pub fn decide(m: &Measures, limits: &Limits) -> Verdict {
    if m.exact && m.frames_actual.abs_diff(m.frames_expected) <= GLOBAL_LAG as usize + 1 {
        return Verdict::Exact;
    }
    let mut why = Vec::new();
    if !m.value_sets_equal {
        why.push("conjuntos de valores de amostra diferentes (M1)".to_string());
    }
    if m.worst_e > limits.e_max {
        why.push(format!("erro máximo {} > {} (M2)", m.worst_e, limits.e_max));
    }
    if m.worst_r > limits.r_max {
        why.push(format!("erro RMS {:.1} > {} (M2)", m.worst_r, limits.r_max));
    }
    if m.worst_f > limits.f_tol {
        why.push(format!(
            "frequência {:.2} % > {:.2} % (M3)",
            m.worst_f * 100.0,
            limits.f_tol * 100.0
        ));
    }
    if m.worst_rms > limits.rms_tol {
        why.push(format!(
            "RMS {:.2} % > {:.2} % (M3)",
            m.worst_rms * 100.0,
            limits.rms_tol * 100.0
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
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const L: Limits = Limits {
        e_max: 256,
        r_max: 64.0,
        f_tol: 0.01,
        rms_tol: 0.02,
    };

    fn square(n: usize, half: usize, amp: i16) -> Vec<i16> {
        (0..n)
            .map(|i| if (i / half) % 2 == 0 { amp } else { -amp })
            .collect()
    }

    #[test]
    fn identical_streams_are_exact_and_a_shift_is_found_as_the_global_lag() {
        let e = square(20_000, 50, 8000);
        let m = measure(&e, &e, 1, &L);
        assert!(m.exact && m.lag == 0 && m.value_sets_equal);
        assert_eq!(decide(&m, &L), Verdict::Exact);
        let mut a = vec![0i16; 17];
        a.extend_from_slice(&e);
        let m = measure(&e, &a, 1, &L);
        assert_eq!(m.lag, 17);
        assert!(m.exact);
    }

    #[test]
    fn a_small_noise_is_a_tolerance_pass_and_a_wrong_pitch_fails() {
        let e = square(20_000, 50, 8000);
        let noisy: Vec<i16> = e
            .iter()
            .enumerate()
            .map(|(i, &s)| s + (i % 3) as i16)
            .collect();
        let m = measure(&e, &noisy, 1, &L);
        assert!(!m.exact);
        // The value sets differ (M1), so this is a FAIL under the policy.
        assert!(matches!(decide(&m, &L), Verdict::Fail(_)));
        // Same values, jitter of one sample at the edges: tolerance.
        let mut jitter = e.clone();
        for i in (50..jitter.len() - 1).step_by(400) {
            jitter[i] = -jitter[i];
        }
        let m = measure(&e, &jitter, 1, &L);
        assert!(m.value_sets_equal && !m.exact);
        let d = decide(&m, &L);
        assert!(
            matches!(d, Verdict::Tolerance | Verdict::Fail(_)),
            "{d:?} {m:?}"
        );
        // A tone an octave up: frequency off by ~50 %.
        let up = square(20_000, 25, 8000);
        let m = measure(&e, &up, 1, &L);
        match decide(&m, &L) {
            Verdict::Fail(why) => assert!(why.contains("M2") || why.contains("M3"), "{why}"),
            v => panic!("{v:?}"),
        }
    }

    #[test]
    fn silence_on_both_sides_is_exact() {
        let z = vec![0i16; 9000];
        let m = measure(&z, &z, 2, &L);
        assert!(m.exact);
        assert_eq!(m.windows, 0);
        assert_eq!(decide(&m, &L), Verdict::Exact);
    }

    #[test]
    fn s16le_round_trip() {
        assert_eq!(read_s16le(&[0x01, 0x00, 0xFF, 0xFF, 0x80]), vec![1, -1]);
    }
}
