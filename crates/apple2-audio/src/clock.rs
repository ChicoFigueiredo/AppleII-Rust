//! Cycle <-> host-sample conversion shared by every audio source.
//!
//! AppleWin ties samples to wall time through DirectSound ring buffers (SoundCore.cpp). Here the
//! only clock is the emulated Cycle counter, so frame `k` of a source always sits at the same
//! Cycle and all sources stay sample-aligned without feedback loops.

use apple2_core::VideoStandard;

/// Host sample rate and CPU clock, in integers so the mapping is exact and deterministic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleClock {
    pub rate_hz: u32,
    /// CPU clock in millihertz (`1020484450` = 1020484.45 Hz).
    pub clk_mhz: u64,
}

impl SampleClock {
    pub fn new(rate_hz: u32, standard: VideoStandard) -> Self {
        Self::with_clock(rate_hz, standard.cpu_clock_hz())
    }

    pub fn with_clock(rate_hz: u32, clk_hz: f64) -> Self {
        SampleClock {
            rate_hz,
            clk_mhz: (clk_hz * 1000.0).round() as u64,
        }
    }

    pub fn clk_hz(&self) -> f64 {
        self.clk_mhz as f64 / 1000.0
    }

    /// Frames whose start (see `cycle_of_frame`) lies strictly before `cycle`: an event at
    /// `cycle` only affects later frames.
    pub fn frames_at(&self, cycle: u64) -> u64 {
        if cycle == 0 {
            return 0;
        }
        ((cycle - 1) as u128 * self.rate_hz as u128 * 1000 / self.clk_mhz as u128) as u64 + 1
    }

    /// Frames that are entirely in the past at `cycle` (their end, the start of the next
    /// frame, is `<= cycle`): what box-filtered sources such as the speaker can deliver.
    pub fn complete_frames(&self, cycle: u64) -> u64 {
        (cycle as u128 * self.rate_hz as u128 * 1000 / self.clk_mhz as u128) as u64
    }

    /// Cycle at which frame `k` starts: `ceil(k * clk / rate)`.
    pub fn cycle_of_frame(&self, k: u64) -> u64 {
        (k as u128 * self.clk_mhz as u128).div_ceil(self.rate_hz as u128 * 1000) as u64
    }

    /// Cycles taken by `samples` samples of a `sample_rate` Hz stream (rounded).
    pub fn cycles_for_samples(&self, samples: u64, sample_rate: u32) -> u64 {
        (samples as u128 * self.clk_mhz as u128 + (sample_rate as u128 * 1000) / 2) as u64
            / (sample_rate as u64 * 1000)
    }

    /// Samples of a `sample_rate` Hz stream elapsed in `cycles` (floor).
    pub fn samples_in(&self, cycles: u64, sample_rate: u32) -> u64 {
        (cycles as u128 * sample_rate as u128 * 1000 / self.clk_mhz as u128) as u64
    }
}

/// AppleWin `NewVolume` (SoundCore.cpp:476): the UI volume is in hundredths of dB,
/// `0 = loudest`, `max = DSBVOLUME_MIN (-100 dB)`. Returns a linear gain.
pub fn volume_gain(volume: u32, volume_max: u32) -> f32 {
    if volume_max == 0 {
        return 1.0;
    }
    let f = (volume.min(volume_max)) as f32 / volume_max as f32;
    10f32.powf(-5.0 * f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_second_of_cycles_is_one_second_of_frames() {
        let c = SampleClock::with_clock(48_000, 1_020_484.45);
        assert!((c.frames_at(1_020_484) as i64 - 48_000).abs() <= 1);
        assert_eq!(c.cycle_of_frame(48_000), 1_020_485);
        assert_eq!(c.frames_at(c.cycle_of_frame(1234)), 1234);
        assert_eq!(c.frames_at(c.cycle_of_frame(1234) + 1), 1235);
    }

    #[test]
    fn volume_maps_to_decibels() {
        assert_eq!(volume_gain(0, 100), 1.0);
        assert!((volume_gain(100, 100) - 1e-5).abs() < 1e-9);
        assert!((volume_gain(20, 100) - 0.1).abs() < 1e-6);
    }
}
