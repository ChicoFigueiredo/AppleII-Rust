//! HU-0501 / AUD-P-010: the AY generator against AppleWin's own `AY8913` class.
//!
//! `tests/conformance/audio/ay-core/*.script` drive the fork's `AY8910.cpp` in the oracle
//! (`audetto-oracle --audio-ay-script`, no emulated machine) and the Rust [`Ay`] with the same
//! register writes and frame lengths; the oracle's samples are in
//! `tests/conformance/reference-audio/ay-core/*.pcm`. The port must be sample-exact: this is the
//! "samples identical" expectation of the audio policy (docs/conformance/test-oracles.md), checked
//! where it can hold, i.e. with the same timeline on both sides.
//!
//! Script format: `clk HZ`, `type T`, `reset`, `w REG VAL TSTATES`, `frame N` (see
//! tools/conformance/oracle/audetto-oracle/audio.cpp). A write at `TSTATES` CPU cycles into the
//! frame lands on sample `TSTATES * (44100/50) / (clk/50)` (`sound_ay_overlay`, AY8910.cpp:496).

use apple2_audio::ay8910::{Ay, AyType};
use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/conformance")
}

fn ay_type(s: &str) -> AyType {
    match s {
        "empty" => AyType::Empty,
        "ay8910" => AyType::Ay3_8910,
        "ay8912" => AyType::Ay3_8912,
        "ay8913" => AyType::Ay3_8913,
        "ym2149" => AyType::Ym2149f,
        o => panic!("type {o}"),
    }
}

/// Runs a script on the Rust chip; returns s16 samples, 3 voices per sample.
fn run(script: &str) -> Vec<i16> {
    let (mut clk, mut ty) = (1_020_484.0f64, AyType::Ay3_8913);
    let mut ay: Option<Ay> = None;
    let mut pending: Vec<(u8, u8, u32)> = Vec::new();
    let mut out = Vec::new();
    for line in script.lines() {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w.as_slice() {
            [] => {}
            [c, ..] if c.starts_with('#') => {}
            ["clk", v] => clk = v.parse().unwrap(),
            ["type", t] => ty = ay_type(t),
            rest => {
                // The first real command creates the chip, like AY8910_InitClock + Reset.
                let chip = ay.get_or_insert_with(|| {
                    let mut a = Ay::new(ty);
                    a.set_clock(clk.trunc(), 44_100);
                    a
                });
                match rest {
                    ["reset"] => {
                        chip.reset();
                        pending.clear();
                    }
                    ["w", r, v, t] => {
                        if chip.ty != AyType::Empty {
                            pending.push((
                                r.parse().unwrap(),
                                v.parse().unwrap(),
                                t.parse().unwrap(),
                            ));
                        }
                    }
                    ["frame", n] => {
                        let n: usize = n.parse().unwrap();
                        let (sfreq, cpufreq) = (44_100u32 / 50, (clk / 50.0) as u32);
                        let ofs: Vec<usize> = pending
                            .iter()
                            .map(|&(_, _, ts)| {
                                (((ts as u64 * sfreq as u64) / cpufreq as u64) as u16 as usize)
                                    .min(n - 1)
                            })
                            .collect();
                        let mut next = 0;
                        for f in 0..n {
                            while next < pending.len() && f >= ofs[next] {
                                chip.write(pending[next].0, pending[next].1);
                                next += 1;
                            }
                            for v in chip.sample() {
                                out.push(v as i16);
                            }
                        }
                        pending.clear();
                    }
                    o => panic!("line `{line}` ({o:?})"),
                }
            }
        }
    }
    out
}

fn check(name: &str) {
    let script =
        std::fs::read_to_string(root().join(format!("audio/ay-core/{name}.script"))).unwrap();
    let want = std::fs::read(root().join(format!("reference-audio/ay-core/{name}.pcm"))).unwrap();
    let want: Vec<i16> = want
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| i16::from_le_bytes(*c))
        .collect();
    let got = run(&script);
    assert_eq!(got.len(), want.len(), "{name}: number of samples");
    if let Some(i) = (0..got.len()).find(|&i| got[i] != want[i]) {
        panic!(
            "{name}: first difference at sample {} voice {}: oracle {}, Rust {}",
            i / 3,
            i % 3,
            want[i],
            got[i]
        );
    }
    assert!(got.iter().any(|&s| s != 0), "{name}: silent script");
}

#[test]
fn ntsc_ay8913_frames_are_sample_exact() {
    check("ntsc-ay8913-frames");
}

#[test]
fn ntsc_ay8913_mb_update_chunks_are_sample_exact() {
    check("ntsc-ay8913-chunks");
}

#[test]
fn phasor_double_clock_is_sample_exact() {
    check("phasor-ay8912-2x");
}

#[test]
fn pal_ay8910_with_noise_and_envelopes_is_sample_exact() {
    check("pal-ay8910-mixed");
}

#[test]
fn ym2149f_is_sample_exact() {
    check("ntsc-ym2149");
}
