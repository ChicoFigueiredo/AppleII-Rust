//! Emulation pacing (ADR 0003): how many cycles the emulation thread owes the wall clock.
//!
//! Pure logic, no clocks of its own: callers pass the elapsed time. Two strategies:
//! [`TimerPacer`] (high-resolution timer, any speed multiplier) and [`audio_frames_due`]
//! (1.0x speed, paced by the fill level of the audio ring, so the audio device is the clock).

use std::time::Duration;

/// Emulation speed. AppleWin's slider goes 0.1x..10x plus "max" (Scroll Lock).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Speed {
    Multiplier(f64),
    Max,
}

impl Speed {
    pub const NORMAL: Speed = Speed::Multiplier(1.0);

    pub fn is_normal(self) -> bool {
        self == Speed::NORMAL
    }
}

/// Wall-clock pacer. `due(now, done)` says how many cycles to run so that `done` (total emulated
/// cycles) tracks `elapsed * clk * speed`.
#[derive(Clone, Debug)]
pub struct TimerPacer {
    clk_hz: f64,
    mult: f64,
    /// Emulated cycles that were due at `anchor`.
    base_cycles: f64,
    anchor: Duration,
    /// Never owe more than this much wall time (avoids the spiral of death after a stall).
    max_lag: Duration,
}

impl TimerPacer {
    pub fn new(clk_hz: f64) -> Self {
        TimerPacer { clk_hz, mult: 1.0, base_cycles: 0.0, anchor: Duration::ZERO, max_lag: Duration::from_millis(100) }
    }

    pub fn set_clock(&mut self, now: Duration, done: u64, clk_hz: f64) {
        self.clk_hz = clk_hz;
        self.rebase(now, done);
    }

    pub fn set_multiplier(&mut self, now: Duration, mult: f64) {
        self.base_cycles = self.target(now);
        self.anchor = now;
        self.mult = mult.max(0.0);
    }

    /// Forget any debt/credit: "right now we are exactly on time".
    pub fn rebase(&mut self, now: Duration, done: u64) {
        self.base_cycles = done as f64;
        self.anchor = now;
    }

    fn target(&self, now: Duration) -> f64 {
        self.base_cycles + now.saturating_sub(self.anchor).as_secs_f64() * self.clk_hz * self.mult
    }

    /// Cycles to execute now (0 when ahead of the clock).
    pub fn due(&mut self, now: Duration, done: u64) -> u64 {
        let max_lag_cycles = self.max_lag.as_secs_f64() * self.clk_hz * self.mult;
        let behind = self.target(now) - done as f64;
        if behind > max_lag_cycles {
            self.rebase(now, done);
            return max_lag_cycles as u64;
        }
        behind.max(0.0) as u64
    }
}

/// Machine frames to run so that the audio ring (`fill` sample frames queued) climbs back to
/// `target`. `per_frame` is the sample frames one machine frame produces; `max_burst` bounds
/// the catch-up after a hiccup.
pub fn audio_frames_due(fill: usize, target: usize, per_frame: usize, max_burst: u32) -> u32 {
    if fill >= target || per_frame == 0 {
        return 0;
    }
    (((target - fill) / per_frame.max(1)) as u32 + 1).min(max_burst)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLK: f64 = 1_000_000.0;
    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn real_time_owes_clock_times_elapsed() {
        let mut p = TimerPacer::new(CLK);
        assert_eq!(p.due(ms(10), 0), 10_000);
        assert_eq!(p.due(ms(20), 10_000), 10_000);
    }

    #[test]
    fn ahead_of_the_clock_owes_nothing() {
        let mut p = TimerPacer::new(CLK);
        assert_eq!(p.due(ms(10), 12_000), 0);
    }

    #[test]
    fn multiplier_scales_and_changes_apply_from_now_on() {
        let mut p = TimerPacer::new(CLK);
        p.set_multiplier(ms(0), 2.0);
        assert_eq!(p.due(ms(10), 0), 20_000);
        p.set_multiplier(ms(10), 0.5);
        assert_eq!(p.due(ms(20), 20_000), 5_000);
    }

    #[test]
    fn long_stall_is_capped_instead_of_catching_up() {
        let mut p = TimerPacer::new(CLK);
        assert_eq!(p.due(ms(5_000), 0), 100_000);
        // after the cap the pacer is back on time
        assert_eq!(p.due(ms(5_010), 100_000), 10_000);
    }

    #[test]
    fn audio_pacing_refills_to_target() {
        assert_eq!(audio_frames_due(5000, 4000, 800, 4), 0);
        assert_eq!(audio_frames_due(3500, 4000, 800, 4), 1);
        assert_eq!(audio_frames_due(0, 4000, 800, 4), 4); // capped burst
    }
}
