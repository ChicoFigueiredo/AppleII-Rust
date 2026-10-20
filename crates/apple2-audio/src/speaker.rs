//! Speaker toggle log -> PCM (AppleWin Speaker.cpp).
//!
//! AppleWin's `SpkrToggle` (cpp:402) flips `g_nSpeakerData` between two 16-bit levels, averages
//! the level over each output sample (`UpdateRemainderBuffer`, cpp:347) and runs the result
//! through the DC filter (cpp:126-180) that fades a stuck cone to silence 0.2 s after the last
//! toggle. The SAM / 8-bit DAC path (SAM.cpp:45) overwrites the level and makes the speaker
//! quieter until it has been idle for 0.2 s (cpp:451, 824).

use crate::clock::SampleClock;

/// `SPKR_DATA_INIT`: the speaker's initial (low) level.
const SPKR_DATA_INIT: i32 = -0x8000;
/// DC-filter countdown after a toggle: 32768 (full gain threshold) + 10000 samples (cpp:165).
const DC_FULL: f64 = 32768.0 + 10000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpeakerEvent {
    /// Access to $C030-$C03F.
    Toggle,
    /// 8-bit DAC write (SAM card).
    Dac(u8),
}

/// Speaker converter: feed time-ordered events, get mono samples at the host rate.
#[derive(Clone, Debug)]
pub struct SpeakerPcm {
    level: i32,
    quieter: bool,
    /// DC-filter counter in units of 44.1 kHz samples (`g_uDCFilterState`).
    dc: f64,
    frames_done: u64,
    last_activity: Option<u64>,
}

impl Default for SpeakerPcm {
    fn default() -> Self {
        SpeakerPcm { level: SPKR_DATA_INIT, quieter: false, dc: 0.0, frames_done: 0, last_activity: None }
    }
}

impl SpeakerPcm {
    pub fn new() -> Self {
        Self::default()
    }

    /// `SpkrReset`: level back to idle, filter closed.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn apply(&mut self, ev: SpeakerEvent, cycle: u64) {
        self.last_activity = Some(cycle);
        self.dc = DC_FULL;
        match ev {
            SpeakerEvent::Toggle => {
                let drive = if self.quieter { SPKR_DATA_INIT / 4 } else { SPKR_DATA_INIT };
                self.level = if self.level == drive { !drive } else { drive };
            }
            SpeakerEvent::Dac(d) => {
                self.level = (((d ^ 0x80) as u16) << 8) as i16 as i32;
                self.quieter = true;
            }
        }
    }

    /// Appends samples (`-1.0..=1.0`) for every output frame that is complete at `until`.
    /// `events` are `(cycle, event)` in time order; the consumed ones are removed. An event at
    /// cycle `c` affects the frame containing `c` (box average, like AppleWin's remainder buffer).
    pub fn render(
        &mut self,
        clock: &SampleClock,
        events: &mut Vec<(u64, SpeakerEvent)>,
        until: u64,
        out: &mut Vec<f32>,
    ) {
        let target = clock.complete_frames(until);
        let step = 44_100.0 / clock.rate_hz as f64;
        let mut next = 0;
        while self.frames_done < target {
            let b0 = clock.cycle_of_frame(self.frames_done);
            let b1 = clock.cycle_of_frame(self.frames_done + 1);
            // 0.2 s of Apple time without activity: undo the DAC muting (Spkr_SetActive(false)).
            if self.quieter && self.last_activity.is_some_and(|a| b0.saturating_sub(a) > clock.clk_mhz / 5000) {
                self.quieter = false;
            }
            let mut cur = b0;
            let mut acc = 0f64;
            while next < events.len() && events[next].0 < b1 {
                let (c, ev) = events[next];
                let c = c.max(cur);
                acc += self.level as f64 * (c - cur) as f64;
                cur = c;
                self.apply(ev, c);
                next += 1;
            }
            acc += self.level as f64 * (b1 - cur) as f64;
            let mean = acc / (b1 - b0).max(1) as f64;
            let gain = if self.dc <= 0.0 {
                0.0
            } else if self.dc >= 32768.0 {
                1.0
            } else {
                self.dc / 32768.0
            };
            self.dc = (self.dc - step).max(0.0);
            out.push((mean * gain / 32768.0) as f32);
            self.frames_done += 1;
        }
        events.drain(..next);
    }

    /// Jumps to frame `k` without producing output (clock change, long gap, state load).
    pub fn skip_to(&mut self, k: u64) {
        self.frames_done = k;
    }

    /// Frames produced since the last [`reset`](Self::reset).
    pub fn frames_done(&self) -> u64 {
        self.frames_done
    }

    /// True while the DAC has muted the cone (`g_bQuieterSpeaker`).
    pub fn is_quieter(&self) -> bool {
        self.quieter
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clock() -> SampleClock {
        SampleClock::with_clock(44_100, 1_000_000.0)
    }

    fn render(toggles: &[u64], until: u64) -> Vec<f32> {
        let mut s = SpeakerPcm::new();
        let mut ev: Vec<_> = toggles.iter().map(|&c| (c, SpeakerEvent::Toggle)).collect();
        let mut out = vec![];
        s.render(&clock(), &mut ev, until, &mut out);
        out
    }

    #[test]
    fn idle_speaker_is_silent() {
        assert!(render(&[], 100_000).iter().all(|&s| s == 0.0));
    }

    #[test]
    fn toggles_make_a_square_wave_of_the_toggle_frequency() {
        // A toggle every 500 cycles at 1 MHz = 1 kHz square wave (period 1000 cycles).
        let toggles: Vec<u64> = (1..=400).map(|i| i * 500).collect();
        let out = render(&toggles, 200_000);
        let crossings = out.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count();
        // 0.2 s * 1 kHz * 2 edges per period = 400 edges.
        assert!((395..=405).contains(&crossings), "{crossings}");
        let peak = out.iter().cloned().fold(0.0, f32::max);
        assert!(peak > 0.9);
    }

    #[test]
    fn dc_filter_fades_a_stuck_cone_to_zero() {
        // One toggle, then silence: ~0.23 s of full gain, ~0.74 s of fade, then exact zero.
        let out = render(&[1000], 2_000_000);
        let at = |t: f64| out[(t * 44_100.0) as usize].abs();
        assert!(at(0.1) > 0.99);
        assert!(at(0.6) > 0.1 && at(0.6) < 0.9);
        assert_eq!(at(1.5), 0.0);
    }

    #[test]
    fn sam_dac_sets_level_and_quietens_speaker_until_idle() {
        let mut s = SpeakerPcm::new();
        let mut ev = vec![(10, SpeakerEvent::Dac(0xFF)), (20_000, SpeakerEvent::Toggle)];
        let mut out = vec![];
        s.render(&clock(), &mut ev, 50_000, &mut out);
        assert!(s.is_quieter());
        // DAC 0xFF = +127/128 of full scale; the quiet toggle swings between -1/4 and +1/4.
        assert!((out[5] - 0.9921875).abs() < 0.01, "{}", out[5]);
        assert!(out[1000].abs() < 0.3 && out[1000] != 0.0);
        let mut ev = vec![];
        s.render(&clock(), &mut ev, 400_000, &mut out);
        assert!(!s.is_quieter());
    }
}
