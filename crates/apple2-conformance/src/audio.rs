//! Audio conformance cases (`oracle.kind = "audio-events"`, HU-0501 / HU-0502).
//!
//! A case runs a small 6502 program (or an AppleWin save-state) on the Rust machine with a
//! Mockingboard-family card, records the audio events and the PCM, and compares them with what the
//! AppleWin oracle recorded for the same program (`tools/conformance/oracle`, `audio.cpp`).
//! The comparison policy is in docs/conformance/test-oracles.md ("Áudio").

use crate::case::Case;
use crate::pcm::{self, Limits, Verdict as PcmVerdict};
use crate::runner::Env;
use crate::trace::TraceWriter;
use apple2_audio::ay8910::AyType;
use apple2_audio::ssi263::SsiType;
use apple2_audio::{AudioBus, AudioEvent, Mixer, MockingboardCard, SamCard, Sockets, Variant};
use apple2_core::{Card, Machine, MachineConfig, Model, VideoStandard};
use serde::Deserialize;
use std::path::Path;

/// `[audio]` of a case: what to plug in and which program to start.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioSpec {
    /// `mockingboard`, `phasor`, `megaaudio` or `sdmusic`; absent = no sound card.
    #[serde(default)]
    pub card: Option<String>,
    /// Slot of the card (default 4).
    #[serde(default)]
    pub slot: Option<u8>,
    /// Raw 6502 program, relative to `tests/conformance`.
    #[serde(default)]
    pub program: Option<String>,
    /// Load address and start address of the program (default `$0300`).
    #[serde(default)]
    pub load: Option<u32>,
    #[serde(default)]
    pub pc: Option<u32>,
    /// AY type per socket 0..=3: `empty`, `ay8910`, `ay8912`, `ay8913`, `ym2149`.
    #[serde(default)]
    pub ay_sockets: Option<Vec<String>>,
    /// `6522a-bad`, `6522b-bad`.
    #[serde(default)]
    pub bad_6522: Option<Vec<bool>>,
    /// SSI263 type per socket 0..=1: `empty`, `ssi263p`, `ssi263ap`.
    #[serde(default)]
    pub ssi263: Option<Vec<String>>,
    #[serde(default)]
    pub sc01: Option<bool>,
    /// Which PCM stream the case compares: `mockingboard` (stereo) or `speaker` (mono).
    #[serde(default)]
    pub pcm: Option<String>,
}

/// Thresholds of the PCM decision rule (`oracle.pcm_limits`).
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PcmLimitsSpec {
    pub f_tol: f64,
    pub rms_tol: f64,
    pub peak_tol: f64,
}

impl From<PcmLimitsSpec> for Limits {
    fn from(s: PcmLimitsSpec) -> Limits {
        Limits {
            f_tol: s.f_tol,
            rms_tol: s.rms_tol,
            peak_tol: s.peak_tol,
        }
    }
}

/// Everything a run produced.
pub struct AudioRun {
    /// `(cycle, event)` sorted by cycle (stable), only events before the run's last cycle.
    pub events: Vec<(u64, AudioEvent)>,
    /// Mockingboard-family mix at 44.1 kHz, stereo interleaved.
    pub mb_pcm: Vec<i16>,
    /// Speaker at 44.1 kHz, mono.
    pub spkr_pcm: Vec<i16>,
    pub final_cycle: u64,
    pub warnings: Vec<String>,
}

fn variant(name: &str) -> Result<Variant, String> {
    Ok(match name {
        "mockingboard" => Variant::MockingboardC,
        "phasor" => Variant::Phasor,
        "megaaudio" => Variant::MegaAudio,
        "sdmusic" => Variant::SdMusic,
        o => return Err(format!("audio.card `{o}` unknown")),
    })
}

fn ay_type(name: &str) -> Result<AyType, String> {
    Ok(match name {
        "empty" => AyType::Empty,
        "ay8910" => AyType::Ay3_8910,
        "ay8912" => AyType::Ay3_8912,
        "ay8913" => AyType::Ay3_8913,
        "ym2149" => AyType::Ym2149f,
        o => return Err(format!("audio.ay_sockets: `{o}` unknown")),
    })
}

fn ssi_type(name: &str) -> Result<SsiType, String> {
    Ok(match name {
        "empty" => SsiType::Empty,
        "ssi263p" => SsiType::P,
        "ssi263ap" => SsiType::Ap,
        o => return Err(format!("audio.ssi263: `{o}` unknown")),
    })
}

impl AudioSpec {
    pub fn validate(&self) -> Result<(), String> {
        if let Some(c) = &self.card {
            variant(c)?;
        }
        if let Some(s) = self.slot {
            if !(1..=7).contains(&s) {
                return Err("audio.slot must be 1..=7".into());
            }
        }
        for s in self.ay_sockets.iter().flatten() {
            ay_type(s)?;
        }
        for s in self.ssi263.iter().flatten() {
            ssi_type(s)?;
        }
        if self.ay_sockets.as_ref().is_some_and(|v| v.len() != 4) {
            return Err("audio.ay_sockets needs 4 entries".into());
        }
        if self.ssi263.as_ref().is_some_and(|v| v.len() != 2) {
            return Err("audio.ssi263 needs 2 entries".into());
        }
        if self.bad_6522.as_ref().is_some_and(|v| v.len() != 2) {
            return Err("audio.bad_6522 needs 2 entries".into());
        }
        match self.pcm.as_deref() {
            None | Some("mockingboard" | "speaker") => Ok(()),
            Some(o) => Err(format!("audio.pcm `{o}`: expected mockingboard or speaker")),
        }
    }

    pub fn sockets(&self) -> Result<Sockets, String> {
        let mut s = Sockets::default();
        if let Some(v) = &self.ay_sockets {
            for (i, n) in v.iter().enumerate() {
                s.ay[i] = ay_type(n)?;
            }
        }
        if let Some(v) = &self.ssi263 {
            for (i, n) in v.iter().enumerate() {
                s.ssi263[i] = ssi_type(n)?;
            }
        }
        if let Some(v) = &self.bad_6522 {
            s.bad_6522 = [v[0], v[1]];
        }
        if let Some(b) = self.sc01 {
            s.sc01 = b;
        }
        Ok(s)
    }
}

/// Power-on registers of the oracle (`audetto-oracle` trace: A=X=Y=$FF, SP=$FC, P=$34).
fn power_on_registers(m: &mut Machine) {
    let cpu = m.cpu_mut();
    cpu.a = 0xFF;
    cpu.x = 0xFF;
    cpu.y = 0xFF;
    cpu.sp = 0xFC;
    cpu.p = 0x34;
}

struct FixtureHost<'a> {
    bus: &'a AudioBus,
}

impl apple2_savestate::LoadHost for FixtureHost<'_> {
    fn open_image(
        &mut self,
        req: &apple2_savestate::ImageRequest,
    ) -> Result<apple2_storage::image::DiskImage, String> {
        Err(format!(
            "{}: images are not loaded by audio cases",
            req.filename
        ))
    }

    fn make_card(&mut self, name: &str, slot: u8, model: Model) -> Option<Box<dyn Card>> {
        let v = match name {
            "Mockingboard C" => Variant::MockingboardC,
            "Phasor" => Variant::Phasor,
            "MEGA Audio" => Variant::MegaAudio,
            "SD Music" => Variant::SdMusic,
            "SAM" => return Some(Box::new(SamCard::new(self.bus))),
            _ => return apple2_savestate::default_card(name, slot, model),
        };
        Some(Box::new(MockingboardCard::new(v, self.bus)))
    }
}

/// Runs the case on the Rust machine. `trace` receives every instruction when given.
pub fn run(
    case: &Case,
    env: &Env,
    mut trace: Option<&mut TraceWriter<Vec<u8>>>,
) -> Result<AudioRun, String> {
    let spec = case.audio.clone().unwrap_or_default();
    spec.validate()?;
    let bus = AudioBus::new(44_100, VideoStandard::Ntsc);
    let mut warnings = Vec::new();
    let mut m = if let Some(fx) = &case.fixture {
        let path = env.workspace.join(&fx.path);
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut host = FixtureHost { bus: &bus };
        let loaded =
            apple2_savestate::load_aws_yaml(&text, &mut host).map_err(|e| e.to_string())?;
        warnings = loaded.warnings;
        loaded.machine
    } else {
        let mut cfg = MachineConfig::for_model(case.model());
        cfg.cpu_cmos = case.cpu_override()?;
        cfg.slots = [None; 8];
        let mut m = Machine::try_new(cfg).map_err(|e| e.to_string())?;
        if let Some(card) = &spec.card {
            let slot = spec.slot.unwrap_or(4);
            let c = MockingboardCard::with_sockets(variant(card)?, &bus, spec.sockets()?);
            if !m.insert_card(slot, Box::new(c)) {
                return Err(format!("slot {slot} refused the card"));
            }
        }
        if let Some(rel) = &spec.program {
            let p = env.root.join(rel);
            let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
            let load = spec.load.unwrap_or(0x0300) as u16;
            for (i, b) in bytes.iter().enumerate() {
                m.poke(load.wrapping_add(i as u16), *b);
            }
            power_on_registers(&mut m);
            m.cpu_mut().pc = spec.pc.map_or(load, |p| p as u16);
        }
        m
    };
    // The audio clock follows the machine's video standard (a save-state may be PAL).
    bus.set_clock(apple2_audio::SampleClock::new(
        44_100,
        m.config().video_standard,
    ));
    let total = match (case.run.cycles, case.run.frames) {
        (Some(c), _) => c,
        (_, Some(f)) => f * (m.video_lines() as u64 * 65),
        _ => return Err("run: cycles or frames".into()),
    };
    let speaker_only = spec.pcm.as_deref() == Some("speaker");
    let start = m.cycles();
    bus.record_events(true);
    let mut mixer = Mixer::new(&bus);
    // The mixer's time base is the machine's absolute Cycle counter; skip what happened before.
    let _ = mixer.produce(start, std::iter::empty());
    let mut toggles: Vec<u64> = Vec::new();
    let (mut mb, mut spk) = (Vec::new(), Vec::new());
    let end = start + total;
    let mut next_mix = start;
    while m.cycles() < end {
        match trace.as_deref_mut() {
            Some(t) => {
                t.step(&mut m).map_err(|e| e.to_string())?;
                m.run_cycles(0); // cards catch up (Card::update)
            }
            None => {
                m.run_cycles(1); // one instruction, then the cards catch up
            }
        }
        if m.cycles() >= next_mix + 17_030 {
            next_mix = m.cycles();
            let t: Vec<u64> = m.drain_speaker_toggles().collect();
            toggles.extend(&t);
            let out = mixer.produce(m.cycles(), t);
            push_pcm(&out, &mut mb, &mut spk, speaker_only);
        }
    }
    let t: Vec<u64> = m.drain_speaker_toggles().collect();
    toggles.extend(&t);
    let out = mixer.produce(m.cycles(), t);
    push_pcm(&out, &mut mb, &mut spk, speaker_only);

    let mut events = bus.take_events();
    events.extend(toggles.iter().map(|&c| (c, AudioEvent::Speaker)));
    events.sort_by_key(|e| e.0);
    for e in &mut events {
        e.0 -= start;
    }
    events.retain(|e| e.0 < total);
    Ok(AudioRun {
        events,
        mb_pcm: mb,
        spkr_pcm: spk,
        final_cycle: m.cycles() - start,
        warnings,
    })
}

fn to_i16(f: f32) -> i16 {
    (f * 32768.0).round().clamp(-32768.0, 32767.0) as i16
}

/// The mixer returns speaker + cards. The programs never use both, so a case compares either the
/// cards' stereo mix or the speaker alone (left channel).
fn push_pcm(out: &[f32], mb: &mut Vec<i16>, spk: &mut Vec<i16>, speaker_only: bool) {
    if !speaker_only {
        mb.extend(out.iter().map(|&f| to_i16(f)));
    } else {
        spk.extend(out.chunks(2).map(|c| to_i16(c[0])));
    }
}

/// How the events of a case are compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timing {
    /// Every line, cycle included.
    Exact,
    /// Exact until the first line that depends on the end of a phoneme; after it the sequence
    /// must match without cycles and each phoneme's duration must agree within a tolerance.
    Ssi { tolerance: u64 },
}

/// Event lines of a file or of a run, headers (`#`) dropped.
pub fn lines_of(text: &str) -> Vec<String> {
    text.lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(str::to_string)
        .collect()
}

fn kind(line: &str) -> &str {
    line.split(',').nth(1).unwrap_or("")
}

fn cycle_of(line: &str) -> u64 {
    line.split(',')
        .next()
        .and_then(|c| c.parse().ok())
        .unwrap_or(0)
}

fn strip_cycle(line: &str) -> &str {
    line.split_once(',').map_or(line, |x| x.1)
}

fn depends_on_phoneme_end(line: &str) -> bool {
    matches!(kind(line), "PHEND" | "IRQ")
}

/// Chip a phoneme line belongs to (`slot,device`), or `None` for any other line.
fn chip_of(line: &str) -> Option<String> {
    let f: Vec<&str> = line.split(',').collect();
    match f.get(1) {
        Some(&("PHON" | "PHEND")) if f.len() > 3 => Some(format!("{},{}", f[2], f[3])),
        _ => None,
    }
}

/// `PHEND - PHON` of each completed phoneme of one chip; a `PHEND` with no `PHON` before it (the
/// phoneme restarted by a save-state load) counts from cycle 0, the load.
fn durations(lines: &[String], chip: &str) -> Vec<u64> {
    let mut start = 0u64;
    let mut out = Vec::new();
    for l in lines {
        if chip_of(l).as_deref() != Some(chip) {
            continue;
        }
        if kind(l) == "PHON" {
            start = cycle_of(l);
        } else {
            out.push(cycle_of(l).saturating_sub(start));
        }
    }
    out
}

fn context(lines: &[String], at: usize, n: usize) -> String {
    let from = at.saturating_sub(n);
    let to = (at + 1).min(lines.len());
    lines[from..to]
        .iter()
        .enumerate()
        .map(|(i, l)| format!("  {:>6}: {l}", from + i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Compares the event lines; `Err` carries the report of the first difference.
pub fn compare_events(
    expected: &[String],
    actual: &[String],
    timing: Timing,
    end_cycle: u64,
) -> Result<String, String> {
    let exact_until = match timing {
        Timing::Exact => usize::MAX,
        Timing::Ssi { .. } => expected
            .iter()
            .position(|l| depends_on_phoneme_end(l))
            .into_iter()
            .chain(actual.iter().position(|l| depends_on_phoneme_end(l)))
            .min()
            .unwrap_or(usize::MAX),
    };
    let n = expected.len().min(actual.len()).min(exact_until);
    for i in 0..n {
        if expected[i] != actual[i] {
            return Err(format!(
                "primeira diferença na linha {}:\n  esperado (oráculo): {}\n  obtido (Rust):      {}\ncontexto do oráculo:\n{}\ncontexto do Rust:\n{}",
                i + 1,
                expected[i],
                actual[i],
                context(expected, i, 6),
                context(actual, i, 6)
            ));
        }
    }
    match timing {
        Timing::Exact => {
            if expected.len() != actual.len() {
                let (longer, name) = if expected.len() > actual.len() {
                    (expected, "o oráculo")
                } else {
                    (actual, "o Rust")
                };
                let at = expected.len().min(actual.len());
                return Err(format!(
                    "os {} primeiros eventos coincidem, mas {name} tem mais ({} contra {}); primeira linha a mais: {}",
                    at,
                    expected.len(),
                    actual.len(),
                    longer[at]
                ));
            }
            Ok(format!(
                "{} eventos idênticos (ciclo incluído)",
                expected.len()
            ))
        }
        Timing::Ssi { tolerance } => {
            let cut = exact_until.min(expected.len()).min(actual.len());
            // After the first phoneme end the program reacts to IRQs the two sides raise at
            // slightly different cycles, and chips drift against each other, so the order
            // between different chips is not an invariant: each chip (and "everything else")
            // is compared as its own sequence without cycles, plus its phoneme durations.
            let (e_tail, a_tail) = (&expected[cut..], &actual[cut..]);
            let mut chips: Vec<String> = e_tail
                .iter()
                .chain(a_tail)
                .filter_map(|l| chip_of(l))
                .collect();
            chips.sort();
            chips.dedup();
            let split = |lines: &[String], chip: Option<&str>| -> Vec<String> {
                lines
                    .iter()
                    .filter(|l| match chip {
                        Some(c) => chip_of(l).as_deref() == Some(c),
                        None => chip_of(l).is_none(),
                    })
                    .cloned()
                    .collect()
            };
            // A phoneme that ends within the tolerance of the last cycle may fall on either side.
            let edge = end_cycle.saturating_sub(2 * tolerance);
            let mut groups: Vec<Option<&str>> = chips.iter().map(|c| Some(c.as_str())).collect();
            groups.push(None);
            let mut worst = 0u64;
            let mut done = 0usize;
            let mut report = Vec::new();
            for g in groups {
                let (e, a) = (split(e_tail, g), split(a_tail, g));
                let n = e.len().min(a.len());
                if let Some(i) = (0..n).find(|&i| strip_cycle(&e[i]) != strip_cycle(&a[i])) {
                    return Err(format!(
                        "sequência de eventos diverge (sem ciclos) em {} no evento {}:\n  esperado (oráculo): {}\n  obtido (Rust):      {}",
                        g.map_or("eventos fora dos chips".to_string(), |c| format!("chip {c}")),
                        i + 1,
                        e[i],
                        a[i]
                    ));
                }
                for extra in e[n..].iter().chain(&a[n..]) {
                    if cycle_of(extra) < edge {
                        return Err(format!(
                            "um lado tem um evento a mais longe do fim da execução (ciclo {} < {edge}): {extra}",
                            cycle_of(extra)
                        ));
                    }
                }
                done += n;
                if let Some(c) = g {
                    let (de, da) = (durations(&e, c), durations(&a, c));
                    let m = de.len().min(da.len());
                    let w = (0..m).map(|i| de[i].abs_diff(da[i])).max().unwrap_or(0);
                    worst = worst.max(w);
                    report.push(format!("chip {c}: pior {w}; oráculo {de:?}; Rust {da:?}"));
                }
            }
            if worst > tolerance {
                return Err(format!(
                    "duração de fonema fora da tolerância: pior diferença {worst} ciclos > {tolerance}\n  {}",
                    report.join("\n  ")
                ));
            }
            Ok(format!(
                "{} eventos do oráculo e {} do Rust; {} exatos (ciclo incluído) e {done} só em sequência por chip; pior diferença de duração de fonema {worst} ciclos (tolerância {tolerance}); {}",
                expected.len(),
                actual.len(),
                cut,
                report.join(" | ")
            ))
        }
    }
}

/// Compares the case's PCM with the oracle's.
pub fn compare_pcm(
    expected_bytes: &[u8],
    run: &AudioRun,
    spec: &AudioSpec,
    limits: &Limits,
) -> (PcmVerdict, pcm::Measures) {
    let (actual, channels) = match spec.pcm.as_deref() {
        Some("speaker") => (&run.spkr_pcm, 1),
        _ => (&run.mb_pcm, 2),
    };
    let expected = pcm::read_s16le(expected_bytes);
    let m = pcm::measure(&expected, actual, channels);
    (pcm::decide(&m, limits), m)
}

pub fn describe_measures(m: &pcm::Measures) -> String {
    format!(
        "{} quadros no oráculo, {} no Rust; desvio global {}; idêntico={}; conjuntos de valores iguais={}; erro por amostra máx {} / RMS {:.1}; {} janelas medidas ({} silenciosas); pior diferença: frequência {:.2} %, RMS {:.2} %, pico {:.2} % (janela {})",
        m.frames_expected,
        m.frames_actual,
        m.lag,
        m.exact,
        m.value_sets_equal,
        m.max_abs_err,
        m.rms_err,
        m.windows,
        m.silent_windows,
        m.worst_f * 100.0,
        m.worst_rms * 100.0,
        m.worst_peak * 100.0,
        m.worst_window
    )
}

/// Reads a reference file under the conformance root.
pub fn read_reference(root: &Path, rel: &str) -> Result<Vec<u8>, String> {
    std::fs::read(root.join(rel)).map_err(|e| format!("{rel}: {e}"))
}

/// Event lines as an `audio-events v1` file body.
pub fn render_lines(lines: &[String]) -> String {
    let mut s = String::from("#audio_events=v1\n#oracle=apple2-rust\n");
    for l in lines {
        s.push_str(l);
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(s: &str) -> Vec<String> {
        lines_of(s)
    }

    #[test]
    fn exact_timing_reports_the_first_different_line_and_length_mismatches() {
        let a = lines("10,IOW,4,01,07\n20,AYW,4,0,0,7,38\n30,SPK\n");
        assert!(compare_events(&a, &a, Timing::Exact, 100).is_ok());
        let b = lines("10,IOW,4,01,07\n21,AYW,4,0,0,7,38\n30,SPK\n");
        let e = compare_events(&a, &b, Timing::Exact, 100).unwrap_err();
        assert!(
            e.contains("linha 2") && e.contains("20,AYW") && e.contains("21,AYW"),
            "{e}"
        );
        let short = lines("10,IOW,4,01,07\n");
        let e = compare_events(&a, &short, Timing::Exact, 100).unwrap_err();
        assert!(e.contains("oráculo tem mais"), "{e}");
    }

    #[test]
    fn ssi_timing_accepts_shifted_phoneme_ends_within_the_tolerance_per_chip() {
        let oracle =
            lines("5,PHON,4,1,0E,100\n1000,PHEND,4,1\n1010,PHON,4,1,0C,100\n2000,PHEND,4,1\n");
        // Same sequence, ends 300 cycles late.
        let rust =
            lines("5,PHON,4,1,0E,100\n1300,PHEND,4,1\n1310,PHON,4,1,0C,100\n2300,PHEND,4,1\n");
        assert!(compare_events(&oracle, &rust, Timing::Ssi { tolerance: 400 }, 3000).is_ok());
        let e = compare_events(&oracle, &rust, Timing::Ssi { tolerance: 200 }, 3000).unwrap_err();
        assert!(e.contains("fora da tolerância"), "{e}");
        // A different phoneme is a sequence error whatever the tolerance.
        let wrong =
            lines("5,PHON,4,1,0E,100\n1000,PHEND,4,1\n1010,PHON,4,1,0D,100\n2000,PHEND,4,1\n");
        let e = compare_events(&oracle, &wrong, Timing::Ssi { tolerance: 5000 }, 3000).unwrap_err();
        assert!(e.contains("diverge"), "{e}");
    }

    #[test]
    fn a_phoneme_end_without_a_start_counts_from_the_load() {
        // Save-state: the phoneme was restarted at cycle 0 by the load.
        let d = durations(
            &lines("120,PHEND,5,0\n130,PHON,5,0,04,9\n250,PHEND,5,0\n"),
            "5,0",
        );
        assert_eq!(d, vec![120, 120]);
    }

    #[test]
    fn audio_spec_rejects_bad_names_and_sizes() {
        let ok = AudioSpec {
            card: Some("phasor".into()),
            ay_sockets: Some(vec!["ay8913".into(); 4]),
            ..Default::default()
        };
        assert!(ok.validate().is_ok());
        assert!(ok
            .sockets()
            .unwrap()
            .ay
            .iter()
            .all(|&t| t == AyType::Ay3_8913));
        let bad = AudioSpec {
            card: Some("sound blaster".into()),
            ..Default::default()
        };
        assert!(bad.validate().is_err());
        let short = AudioSpec {
            ay_sockets: Some(vec!["ay8910".into()]),
            ..Default::default()
        };
        assert!(short.validate().is_err());
    }
}
