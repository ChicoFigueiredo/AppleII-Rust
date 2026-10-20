//! NTSC/PAL timing (AppleWin Common.h:3-6).

use serde::{Deserialize, Serialize};

const CLK_14M_NTSC: f64 = 157_500_000.0 / 11.0;
const CLK_14M_PAL: f64 = 14.25045e6; // UTAIIe:3-17

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum VideoStandard {
    #[default]
    Ntsc,
    Pal,
}

impl VideoStandard {
    /// CPU clock in Hz: `14M * 65 / (65*14+2)` (65 cycles per 912 14M clocks).
    pub fn cpu_clock_hz(self) -> f64 {
        let m = match self {
            VideoStandard::Ntsc => CLK_14M_NTSC,
            VideoStandard::Pal => CLK_14M_PAL,
        };
        m * 65.0 / (65.0 * 14.0 + 2.0)
    }

    /// 65 cycles per scanline; 262 (NTSC) or 312 (PAL) lines.
    pub fn cycles_per_frame(self) -> u32 {
        match self {
            VideoStandard::Ntsc => 65 * 262,
            VideoStandard::Pal => 65 * 312,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_and_frames() {
        assert!((VideoStandard::Ntsc.cpu_clock_hz() - 1_020_484.4).abs() < 0.1);
        assert!((VideoStandard::Pal.cpu_clock_hz() - 1_015_657.0).abs() < 1.0);
        assert_eq!(VideoStandard::Ntsc.cycles_per_frame(), 17030);
        assert_eq!(VideoStandard::Pal.cycles_per_frame(), 20280);
    }
}
