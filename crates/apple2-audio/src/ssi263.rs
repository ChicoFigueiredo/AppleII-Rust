//! SSI263 phoneme speech chip and the Votrax SC-01 path of the original Mockingboard
//! (AppleWin SSI263.cpp, phoneme samples from SSI263Phonemes.h).
//!
//! AppleWin plays a phoneme through a DirectSound ring buffer and raises the "phoneme complete"
//! IRQ when the buffer consumed its last sample, with a 1000-cycle `PeriodicUpdate` and a debugger
//! fallback (`UpdateAccurateLength`). Here a phoneme is a pre-rendered wave started at an exact
//! Cycle: the IRQ is a scheduler event at `start + length` and the audio is a pure function of
//! the Cycle, so speech and IRQs agree by construction and need no feedback loop.

use crate::clock::SampleClock;
use std::sync::OnceLock;

/// Native sample rate of the phoneme data (`SAMPLE_RATE_SSI263`).
pub const SSI_RATE: u32 = 22_050;
const DURATION_MODE_MASK: u8 = 0xC0;
const DURATION_MODE_SHIFT: u8 = 6;
pub const PHONEME_MASK: u8 = 0x3F;
const MODE_PHONEME_TRANSITIONED_INFLECTION: u8 = 0xC0;
const CONTROL_MASK: u8 = 0x80;
const AMPLITUDE_MASK: u8 = 0x0F;
const FILTER_FREQ_SILENCE: u8 = 0xFF;
/// `kPhonemeLeadoutFlag`.
const LEADOUT_FLAG: i32 = 0x100;

/// `PHASOR_MODE` values (MockingboardDefs.h).
pub const PH_MOCKINGBOARD: u8 = 0;
pub const PH_PHASOR: u8 = 5;
pub const PH_ECHO_PLUS: u8 = 7;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SsiType {
    Empty,
    /// SSI263P: keeps playing across Ctrl-Reset.
    P,
    /// SSI263AP.
    Ap,
}

/// `m_Votrax2SSI263` (cpp:248): SC-01 phoneme -> nearest SSI263 phoneme.
const VOTRAX_TO_SSI263: [u8; 64] = [
    0x02, 0x0A, 0x0B, 0x00, 0x28, 0x08, 0x08, 0x2F, 0x0E, 0x07, 0x07, 0x07, 0x37, 0x38, 0x24,
    0x33, //
    0x32, 0x32, 0x2F, 0x10, 0x39, 0x0F, 0x13, 0x13, 0x20, 0x29, 0x25, 0x2C, 0x26, 0x34, 0x25,
    0x30, //
    0x08, 0x09, 0x03, 0x1B, 0x0E, 0x27, 0x11, 0x07, 0x16, 0x05, 0x28, 0x1D, 0x01, 0x23, 0x0C,
    0x0D, //
    0x10, 0x1A, 0x19, 0x18, 0x11, 0x11, 0x14, 0x14, 0x35, 0x36, 0x1C, 0x0A, 0x01, 0x10, 0x00, 0x00,
];

struct PhonemeData {
    /// `(offset, length)` of phonemes 2..=63 (index = phoneme - 2).
    info: Vec<(usize, usize)>,
    samples: Vec<i16>,
}

fn phonemes() -> &'static PhonemeData {
    static D: OnceLock<PhonemeData> = OnceLock::new();
    D.get_or_init(|| {
        let b = include_bytes!("../data/ssi263_phonemes.bin");
        let u32_at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().unwrap()) as usize;
        let info = (0..62)
            .map(|i| (u32_at(i * 8), u32_at(i * 8 + 4)))
            .collect();
        let samples = b[62 * 8..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| i16::from_le_bytes(*c))
            .collect();
        PhonemeData { info, samples }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Idle,
    /// Wave playing; the "phoneme complete" event is due at `end_cycle`.
    Playing,
    /// Silent tail (`m_phonemeLeadoutLength`) until the phoneme repeats at `end_cycle`.
    Leadout,
}

/// Side effects of an SSI263 operation on the rest of the card, applied by the caller.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SsiOut {
    /// `CpuIrqDeassert(IS_SPEECH)`.
    pub deassert_irq: bool,
    /// `CpuIrqAssert(IS_SPEECH)` (Phasor native mode).
    pub assert_irq: bool,
    /// Bits to clear/set in this unit's 6522 IFR.
    pub ifr_clear: u8,
    pub ifr_set: u8,
    /// (Re)schedule the chip's event this many Cycles from now.
    pub schedule: Option<i32>,
}

#[derive(Clone, Debug)]
pub struct Ssi263 {
    pub ty: SsiType,
    pub has_sc01: bool,
    pub dur_phon: u8,
    pub inflection: u8,
    pub rate_inflection: u8,
    pub ctrl_art_amp: u8,
    pub filter_freq: u8,
    /// `m_currentMode`: D7 pin, A/!R interrupt enable, function (DR1:0).
    pub d7: bool,
    pub enable_ints: bool,
    pub function: u8,
    pub card_mode: u8,
    pub is_votrax: bool,
    pub votrax_phoneme: u8,
    /// -1 = never played; phoneme (| leadout flag) otherwise.
    current_active: i32,
    phase: Phase,
    wave: Vec<i16>,
    wave_phoneme: u8,
    wave_dur: u8,
    leadout: u64,
    play_start: u64,
    end_cycle: u64,
    /// Start/end of phoneme notifications for the conformance log, drained by the card.
    pub(crate) notes: Vec<SsiNote>,
    /// Length of the last phoneme's input wave (`g_nPhonemeInfo[].nLength`).
    input_len: usize,
}

/// What the chip tells the card about phoneme boundaries (conformance event log).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SsiNote {
    /// `Play(phoneme)`; `samples` is the phoneme's length in 22.05 kHz input samples.
    Start { phoneme: u8, samples: u32 },
    /// `UpdateIRQ`: the phoneme is complete.
    End,
}

impl Ssi263 {
    pub fn new(ty: SsiType, has_sc01: bool) -> Self {
        let mut s = Ssi263 {
            ty,
            has_sc01,
            dur_phon: 0,
            inflection: 0,
            rate_inflection: 0,
            ctrl_art_amp: 0,
            filter_freq: 0,
            d7: false,
            enable_ints: false,
            function: 0,
            card_mode: PH_MOCKINGBOARD,
            is_votrax: false,
            votrax_phoneme: 0,
            current_active: -1,
            phase: Phase::Idle,
            wave: Vec::new(),
            wave_phoneme: 0,
            wave_dur: 0,
            leadout: 0,
            play_start: 0,
            end_cycle: 0,
            notes: Vec::new(),
            input_len: 0,
        };
        s.reset_state(true);
        s
    }

    /// `ResetState` (SSI263.h:23).
    fn reset_state(&mut self, power_cycle: bool) {
        if power_cycle {
            self.current_active = -1;
            self.is_votrax = false;
            self.votrax_phoneme = 0;
        }
        self.phase = Phase::Idle;
        self.wave.clear();
        self.leadout = 0;
        // After power-on, CTL=0 gives enableInts=1 and DR1:0 != 0 in practice.
        self.dur_phon = MODE_PHONEME_TRANSITIONED_INFLECTION;
        self.inflection = 0;
        self.rate_inflection = 0;
        self.ctrl_art_amp = if power_cycle || self.ty == SsiType::Ap {
            CONTROL_MASK
        } else {
            0
        };
        self.filter_freq = if power_cycle { FILTER_FREQ_SILENCE } else { 0 };
        self.d7 = false;
        self.function = 0;
        self.enable_ints = false;
    }

    pub fn is_phoneme_active(&self) -> bool {
        self.current_active >= 0
    }

    /// `Reset` (cpp:932). `now` is only used to cancel nothing; returns the effects.
    pub fn reset(&mut self, power_cycle: bool, is_phasor_card: bool) -> SsiOut {
        let mut out = SsiOut::default();
        if self.ty == SsiType::Empty {
            return out;
        }
        if !power_cycle && self.ty == SsiType::P {
            // The phoneme keeps playing; on a Phasor it also does a CTL H->L (cpp:945).
            if is_phasor_card && self.ctrl_art_amp & CONTROL_MASK == 0 {
                self.set_device_mode_and_ints();
            }
            return out;
        }
        self.reset_state(power_cycle);
        out.deassert_irq = true;
        out
    }

    /// `Read` (cpp:123): only D7 is driven, the rest is the floating bus.
    pub fn read(&self, floating_bus: u8) -> u8 {
        if self.ty == SsiType::Empty {
            return floating_bus;
        }
        (floating_bus & 0x7F) | (self.d7 as u8) << 7
    }

    fn set_device_mode_and_ints(&mut self) {
        if self.dur_phon & DURATION_MODE_MASK != 0 {
            self.function = (self.dur_phon & DURATION_MODE_MASK) >> DURATION_MODE_SHIFT;
            self.enable_ints = true;
        } else {
            // "Disables A/!R output only; does not change previous A/!R response".
            self.enable_ints = false;
        }
    }

    /// `Write` (cpp:135). `now` = Cycle of the access.
    pub fn write(&mut self, reg: u8, val: u8, now: u64, clk: &SampleClock) -> SsiOut {
        let mut out = SsiOut::default();
        if self.ty == SsiType::Empty {
            return out;
        }
        // Writes to regs 0-2 (and CTL=1 in reg 3) de-assert the IRQ and clear D7 (GH#1197).
        if reg <= 2 {
            out.deassert_irq = true;
            self.d7 = false;
        }
        match reg {
            0 => {
                self.dur_phon = val;
                self.is_votrax = false;
                if self.ctrl_art_amp & CONTROL_MASK == 0 {
                    out.schedule = self.play(self.dur_phon & PHONEME_MASK, now, clk);
                }
            }
            1 => self.inflection = val,
            2 => self.rate_inflection = val,
            3 => {
                if self.ctrl_art_amp & CONTROL_MASK != 0 && val & CONTROL_MASK == 0 {
                    // CTL H->L: out of power-down, so play the phoneme.
                    self.set_device_mode_and_ints();
                    self.is_votrax = false;
                    out.schedule = self.play(self.dur_phon & PHONEME_MASK, now, clk);
                }
                self.ctrl_art_amp = val;
                if val & CONTROL_MASK != 0 {
                    out.deassert_irq = true;
                    self.d7 = false;
                }
            }
            _ => self.filter_freq = val,
        }
        out
    }

    /// `Votrax_Write` (cpp:319): SC-01 phoneme from port B.
    pub fn votrax_write(&mut self, value: u8, now: u64, clk: &SampleClock) -> SsiOut {
        let mut out = SsiOut::default();
        if !self.has_sc01 {
            return out;
        }
        self.is_votrax = true;
        self.votrax_phoneme = value & PHONEME_MASK;
        out.ifr_clear = crate::via6522::IXR_VOTRAX; // !A/R: phoneme data acknowledged
        self.dur_phon = 0; // the SC-01's duration doesn't depend on pitch
        out.schedule = self.play(VOTRAX_TO_SSI263[self.votrax_phoneme as usize], now, clk);
        out
    }

    /// `Play` (cpp:342): pre-renders the phoneme wave and returns when its end event is due.
    fn play(&mut self, phoneme: u8, now: u64, clk: &SampleClock) -> Option<i32> {
        self.current_active = phoneme as i32;
        // Frame-timing function plays at the fastest rate (cpp:613).
        let dur = if self.function == 1 {
            3
        } else {
            self.dur_phon >> DURATION_MODE_SHIFT
        };
        self.build_wave(phoneme, dur);
        self.notes.push(SsiNote::Start {
            phoneme,
            samples: self.input_len as u32,
        });
        self.play_start = now;
        self.end_cycle = now
            + clk
                .cycles_for_samples(self.wave.len() as u64, SSI_RATE)
                .max(1);
        self.phase = Phase::Playing;
        Some((self.end_cycle - now) as i32)
    }

    /// The averaging/decimation loop of `Update` (cpp:624-660) run ahead of time.
    fn build_wave(&mut self, phoneme: u8, dur: u8) {
        self.wave_phoneme = phoneme;
        self.wave_dur = dur;
        let d = phonemes();
        let (pause, mut p) = (phoneme == 0, phoneme);
        if p == 1 {
            p = 2; // phoneme 1 has no sample
        }
        let idx = if pause { 0 } else { p as usize - 2 };
        let (offset, len) = d.info[idx];
        self.input_len = len;
        let input = &d.samples[offset..offset + len];
        let avg = match dur {
            0 | 1 => 1,
            2 => 2,
            _ => 4,
        };
        self.wave.clear();
        let (mut sum, mut num, mut mod4, mut i) = (0i32, 0, 0u8, 0usize);
        while i < len {
            if !pause {
                sum += input[i] as i32;
            }
            num += 1;
            i += 1;
            if num == avg {
                self.wave.push((sum / avg) as i16);
                sum = 0;
                num = 0;
            }
            mod4 = (mod4 + 1) & 3;
            if dur == 1 && mod4 == 3 && i < len {
                i += 1; // DUR=1 consumes 4 input samples per 3 output samples
            }
        }
        self.leadout = (len / 10) as u64; // arbitrary, as in AppleWin
    }

    /// `SetSpeechIRQ` (cpp:806) at the end of a phoneme.
    fn set_speech_irq(&mut self, pcr: u8, out: &mut SsiOut) {
        if !self.is_votrax && self.ctrl_art_amp & CONTROL_MASK == 0 {
            if self.enable_ints {
                if self.card_mode == PH_MOCKINGBOARD {
                    // PCR b0 = 0: CA1 negative active edge latches SSI263's A/!R.
                    if !self.d7 && pcr & 1 == 0 {
                        out.ifr_set |= crate::via6522::IXR_SSI263;
                    }
                } else if self.card_mode == PH_PHASOR {
                    out.assert_irq = true; // A/!R is wired to the 6502 IRQ
                }
                // Echo+: SSI263 not visible, but still runs.
            }
            self.d7 = true; // always, even with ints disabled
        }
        if self.is_votrax && pcr == 0xB0 {
            out.ifr_set |= crate::via6522::IXR_VOTRAX; // !A/R: old phoneme timed out
        }
    }

    /// The chip's scheduler event: phoneme complete (-> IRQ) or leadout done (-> repeat).
    /// `now` = Cycle at the callback; times follow the nominal schedule, not the callback's.
    pub fn on_event(&mut self, now: u64, pcr: u8, clk: &SampleClock) -> (SsiOut, i32) {
        let mut out = SsiOut::default();
        match self.phase {
            Phase::Playing => {
                self.notes.push(SsiNote::End);
                self.current_active |= LEADOUT_FLAG;
                self.set_speech_irq(pcr, &mut out);
                self.phase = Phase::Leadout;
                self.end_cycle += clk.cycles_for_samples(self.leadout, SSI_RATE);
            }
            Phase::Leadout => {
                // RepeatPhoneme: the same phoneme again unless powered down.
                let start = self.end_cycle;
                if !self.is_votrax {
                    if self.ctrl_art_amp & CONTROL_MASK == 0 {
                        self.play(self.dur_phon & PHONEME_MASK, start, clk);
                    } else {
                        self.phase = Phase::Idle;
                    }
                    self.current_active &= !LEADOUT_FLAG;
                } else {
                    self.play(VOTRAX_TO_SSI263[self.votrax_phoneme as usize], start, clk);
                }
            }
            Phase::Idle => return (out, 0),
        }
        let wait = if self.phase == Phase::Idle {
            0
        } else {
            (self.end_cycle.saturating_sub(now)).max(1) as i32
        };
        (out, wait)
    }

    /// `SetCardMode` (cpp:852): the Phasor mode changed.
    pub fn set_card_mode(&mut self, mode: u8, pcr: u8) -> SsiOut {
        let mut out = SsiOut::default();
        let old = self.card_mode;
        self.card_mode = mode;
        if old == mode {
            return out;
        }
        if self.d7 {
            self.d7 = false; // so that the Mockingboard path sets the IFR; post: D7=1
            self.set_speech_irq(pcr, &mut out);
        }
        if mode != PH_PHASOR {
            out.deassert_irq = true;
        }
        out
    }

    /// Output sample (`-1.0..=1.0`) at `cycle`, mono. Pure function of the state.
    pub fn sample_at(&self, cycle: u64, clk: &SampleClock) -> f32 {
        if self.phase == Phase::Idle || cycle < self.play_start || self.wave.is_empty() {
            return 0.0;
        }
        let amplitude = if self.is_votrax {
            1.0
        } else if self.ctrl_art_amp & CONTROL_MASK != 0 || self.filter_freq == FILTER_FREQ_SILENCE {
            0.0
        } else {
            (self.ctrl_art_amp & AMPLITUDE_MASK) as f32 / AMPLITUDE_MASK as f32
        };
        let mut j = clk.samples_in(cycle - self.play_start, SSI_RATE);
        let n = self.wave.len() as u64;
        // The phoneme repeats forever (until changed or powered down).
        let repeating = self.is_votrax || self.ctrl_art_amp & CONTROL_MASK == 0;
        if repeating {
            j %= n + self.leadout;
        }
        if j >= n {
            return 0.0;
        }
        self.wave[j as usize] as f32 / 32768.0 * amplitude
    }

    /// Phoneme length in output samples of the last `Play`.
    pub fn wave_len(&self) -> usize {
        self.wave.len()
    }

    // ---- save-state ----

    pub const SAVED_LEN: usize = 43;

    pub fn save(&self, w: &mut Vec<u8>) {
        w.push(match self.ty {
            SsiType::Empty => 0,
            SsiType::P => 1,
            SsiType::Ap => 2,
        });
        w.extend([
            self.has_sc01 as u8,
            self.dur_phon,
            self.inflection,
            self.rate_inflection,
            self.ctrl_art_amp,
            self.filter_freq,
            self.d7 as u8 | (self.enable_ints as u8) << 1 | (self.is_votrax as u8) << 2,
            self.function,
            self.votrax_phoneme,
            self.card_mode,
            self.phase as u8,
            self.wave_phoneme,
            self.wave_dur,
        ]);
        w.extend(self.current_active.to_le_bytes());
        w.extend(self.play_start.to_le_bytes());
        w.extend(self.end_cycle.to_le_bytes());
        w.extend([0u8; 9]); // reserved
    }

    /// Restores [`save`](Self::save), including the play position: the wave is rebuilt
    /// deterministically and the chip's scheduler event (saved with the machine) stays valid.
    pub fn load(&mut self, b: &[u8]) -> bool {
        if b.len() < Self::SAVED_LEN {
            return false;
        }
        self.ty = match b[0] {
            0 => SsiType::Empty,
            1 => SsiType::P,
            _ => SsiType::Ap,
        };
        self.has_sc01 = b[1] != 0;
        (
            self.dur_phon,
            self.inflection,
            self.rate_inflection,
            self.ctrl_art_amp,
            self.filter_freq,
        ) = (b[2], b[3], b[4], b[5], b[6]);
        (self.d7, self.enable_ints, self.is_votrax) = (b[7] & 1 != 0, b[7] & 2 != 0, b[7] & 4 != 0);
        (self.function, self.votrax_phoneme, self.card_mode) = (b[8], b[9], b[10]);
        self.phase = match b[11] {
            1 => Phase::Playing,
            2 => Phase::Leadout,
            _ => Phase::Idle,
        };
        self.current_active = i32::from_le_bytes(b[14..18].try_into().unwrap());
        self.play_start = u64::from_le_bytes(b[18..26].try_into().unwrap());
        self.end_cycle = u64::from_le_bytes(b[26..34].try_into().unwrap());
        self.wave.clear();
        self.leadout = 0;
        if self.phase != Phase::Idle {
            self.build_wave(b[12], b[13]);
        }
        true
    }
}

impl Ssi263 {
    /// `SaveSnapshot` + `SC01_SaveSnapshot` state (SSI263.cpp:1005, :1122).
    pub fn snapshot(&self) -> crate::snapshot::SsiSnapshot {
        crate::snapshot::SsiSnapshot {
            ty: self.ty,
            duration_phoneme: self.dur_phon,
            inflection: self.inflection,
            rate_inflection: self.rate_inflection,
            ctrl_art_amp: self.ctrl_art_amp,
            filter_freq: self.filter_freq,
            current_mode: self.d7 as u8 | (self.enable_ints as u8) << 5 | self.function << 6,
            has_sc01: self.has_sc01,
            votrax_phoneme: self.votrax_phoneme,
            is_votrax_phoneme: self.is_votrax,
        }
    }

    /// `LoadSnapshot` (SSI263.cpp:1032): registers and mode; the play position is not kept. A
    /// phoneme that was active is left `Idle` for [`restart`](Self::restart) to start again
    /// (AppleWin's `RepeatPhoneme`).
    pub fn apply_snapshot(&mut self, s: &crate::snapshot::SsiSnapshot) {
        self.ty = s.ty;
        self.has_sc01 = s.has_sc01;
        self.votrax_phoneme = s.votrax_phoneme;
        self.is_votrax = s.is_votrax_phoneme;
        self.phase = Phase::Idle;
        self.wave.clear();
        self.leadout = 0;
        self.current_active = -1;
        if s.ty != SsiType::Empty {
            self.dur_phon = s.duration_phoneme;
            self.inflection = s.inflection;
            self.rate_inflection = s.rate_inflection;
            self.ctrl_art_amp = s.ctrl_art_amp;
            self.filter_freq = s.filter_freq;
            self.d7 = s.current_mode & 1 != 0;
            self.enable_ints = s.current_mode & 0x20 != 0;
            self.function = s.current_mode >> 6;
            if self.ctrl_art_amp & CONTROL_MASK == 0 {
                self.current_active = (self.dur_phon & PHONEME_MASK) as i32;
            }
        }
        if self.is_votrax {
            self.current_active = 0; // IsPhonemeActive() and the SC-01 (cpp:1094)
        }
    }

    /// True after [`apply_snapshot`](Self::apply_snapshot) with a phoneme to restart.
    pub fn needs_restart(&self) -> bool {
        self.is_phoneme_active() && self.phase == Phase::Idle
    }

    /// `RepeatPhoneme` after a load: starts the active phoneme from the beginning at `now`.
    /// Returns the cycles until its end event.
    pub fn restart(&mut self, now: u64, clk: &SampleClock) -> Option<i32> {
        if self.is_votrax {
            self.play(
                VOTRAX_TO_SSI263[(self.votrax_phoneme & PHONEME_MASK) as usize],
                now,
                clk,
            )
        } else {
            self.play(self.dur_phon & PHONEME_MASK, now, clk)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clk() -> SampleClock {
        SampleClock::with_clock(44_100, 1_020_484.45)
    }

    fn started(phoneme: u8, dur_mode: u8) -> (Ssi263, i32) {
        let mut s = Ssi263::new(SsiType::Ap, false);
        s.write(1, 0, 0, &clk());
        s.write(2, 0, 0, &clk());
        s.write(4, 0, 0, &clk()); // filter frequency off the power-on 'silence' value
        s.write(0, dur_mode << 6 | phoneme, 0, &clk());
        let o = s.write(3, 0x0F, 0, &clk()); // CTL H->L, amplitude 15
        (s, o.schedule.unwrap())
    }

    #[test]
    fn data_tables_are_complete() {
        let d = phonemes();
        assert_eq!(d.info.len(), 62);
        assert_eq!(d.samples.len(), 156_566);
        assert_eq!(d.info[0], (0, 0xA60));
    }

    #[test]
    fn powers_up_in_standby_and_ctl_low_plays_the_phoneme() {
        let mut s = Ssi263::new(SsiType::Ap, false);
        assert_eq!(s.ctrl_art_amp & CONTROL_MASK, CONTROL_MASK);
        assert!(!s.is_phoneme_active());
        // Writing the phoneme in standby doesn't play.
        assert!(s.write(0, 0xC0 | 0x0A, 0, &clk()).schedule.is_none());
        s.filter_freq = 0;
        let o = s.write(3, 0x0F, 100, &clk());
        assert!(o.schedule.is_some());
        assert!(s.is_phoneme_active() && s.enable_ints && s.function == 3);
    }

    #[test]
    fn a_phoneme_makes_non_silent_output_and_ends_with_an_irq() {
        let (mut s, due) = started(0x0A, 3);
        let c = clk();
        let peak = (0..due as u64)
            .step_by(40)
            .map(|t| s.sample_at(t, &c).abs())
            .fold(0.0, f32::max);
        assert!(peak > 0.05, "{peak}");
        // 0xA4F input samples at 22.05 kHz averaged by 4 (DUR=3) => about 0.03 s.
        assert!((20_000..40_000).contains(&due), "{due}");
        let (out, wait) = s.on_event(due as u64, 0, &c);
        assert_eq!(out.ifr_set, crate::via6522::IXR_SSI263);
        assert!(s.d7 && wait > 0);
        // After the silent leadout the phoneme repeats (same length).
        let (out, wait2) = s.on_event(due as u64 + wait as u64, 0, &c);
        assert_eq!(out, SsiOut::default());
        assert!((wait2 - due).abs() < 3);
    }

    #[test]
    fn slower_duration_modes_last_longer() {
        let len = |m| started(0x0A, m).1;
        // Mode 1 is frame timing, which AppleWin plays at the fastest rate (cpp:613).
        assert!(len(0) > len(2) && len(2) > len(3) && len(1) == len(3));
    }

    #[test]
    fn silent_in_power_down_and_with_amplitude_zero() {
        let (mut s, _) = started(0x0A, 3);
        let c = clk();
        s.write(3, 0x80 | 0x0F, 500, &c);
        assert!((0..20_000u64).all(|t| s.sample_at(500 + t, &c) == 0.0));
        s.write(3, 0x00, 600, &c); // amplitude 0, CTL=0 again
        assert!((0..20_000u64).all(|t| s.sample_at(600 + t, &c) == 0.0));
    }

    #[test]
    fn writes_to_regs_0_to_2_acknowledge_the_irq() {
        let (mut s, due) = started(0x0A, 3);
        s.on_event(due as u64, 0, &clk());
        assert!(s.d7);
        let o = s.write(2, 0, due as u64 + 10, &clk());
        assert!(o.deassert_irq && !s.d7);
    }

    #[test]
    fn phasor_native_mode_asserts_cpu_irq_instead_of_ifr() {
        let (mut s, due) = started(0x0A, 3);
        s.set_card_mode(PH_PHASOR, 0);
        let (out, _) = s.on_event(due as u64, 0, &clk());
        assert!(out.assert_irq && out.ifr_set == 0);
    }

    #[test]
    fn votrax_phoneme_plays_without_ssi263_and_acknowledges() {
        let mut s = Ssi263::new(SsiType::Empty, true);
        let o = s.votrax_write(0x2C, 0, &clk());
        assert_eq!(o.ifr_clear, crate::via6522::IXR_VOTRAX);
        assert!(o.schedule.is_some());
        let c = clk();
        assert!((0..30_000u64)
            .step_by(30)
            .any(|t| s.sample_at(t, &c).abs() > 0.05));
        let (out, _) = s.on_event(o.schedule.unwrap() as u64, 0xB0, &c);
        assert_eq!(out.ifr_set, crate::via6522::IXR_VOTRAX);
    }

    #[test]
    fn save_load_keeps_registers_and_the_play_position() {
        let (s, _) = started(0x0A, 2);
        let mut buf = vec![];
        s.save(&mut buf);
        assert_eq!(buf.len(), Ssi263::SAVED_LEN);
        let mut t = Ssi263::new(SsiType::Empty, true);
        assert!(t.load(&buf));
        assert_eq!(
            (t.dur_phon, t.ctrl_art_amp, t.ty),
            (s.dur_phon, s.ctrl_art_amp, s.ty)
        );
        // Same audio from the same Cycle: the wave is rebuilt, not restarted.
        let c = clk();
        assert!((0..20_000u64)
            .step_by(7)
            .all(|x| t.sample_at(x, &c) == s.sample_at(x, &c)));
        assert!(t.sample_at(5_000, &c) != 0.0);
    }
}
