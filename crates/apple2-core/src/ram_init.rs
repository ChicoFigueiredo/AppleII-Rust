//! Power-on RAM pattern of the AppleWin (`MemReset`, Memory.cpp:2308; `g_nMemoryClearType`,
//! Core.cpp:79; `-memclear N`, CmdLine.cpp:456).
//!
//! AppleWin fills main RAM `$0000-$BFFF` according to a "Memory Initialization Pattern" (MIP) and
//! then pokes a few bytes with `rand() ^ timeGetTime()`. The core is deterministic (ADR 0001), so
//! the source of those "random" values is part of the configuration: [`RandomSource`].

use serde::{Deserialize, Serialize};

/// `MIP_ZERO`
pub const MIP_ZERO: i32 = 0;
/// `MIP_RANDOM`
pub const MIP_RANDOM: i32 = 1;
/// `MIP_FF_FF_00_00`: the AppleWin default.
pub const MIP_FF_FF_00_00: i32 = 2;
/// `MIP_FF_00_FULL_PAGE`
pub const MIP_FF_00_FULL_PAGE: i32 = 3;
/// `MIP_00_FF_HALF_PAGE`
pub const MIP_00_FF_HALF_PAGE: i32 = 4;
/// `MIP_FF_00_HALF_PAGE`
pub const MIP_FF_00_HALF_PAGE: i32 = 5;
/// `MIP_PAGE_ADDRESS_LOW`
pub const MIP_PAGE_ADDRESS_LOW: i32 = 6;
/// `MIP_PAGE_ADDRESS_HIGH`
pub const MIP_PAGE_ADDRESS_HIGH: i32 = 7;
/// `NUM_MIP`
pub const NUM_MIP: i32 = 8;

/// What `getRandomTime()` (`rand() ^ timeGetTime()`) returns.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RandomSource {
    /// Always 0: the deterministic mode of the conformance oracle (`g_bOracleDeterministic`).
    /// The "random" bytes at `xx28/xx29/xx68/xx69` become `00` and `$4E`/`$4F` become `20`.
    Zero,
    /// Pseudo-random sequence from this seed (the frontend picks it from the clock).
    Seed(u32),
}

/// Power-on RAM configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RamInit {
    /// `g_nMemoryClearType`: `-1` = random pattern, `0..=7` = `MIP_*`.
    pub pattern: i32,
    pub random: RandomSource,
}

impl RamInit {
    /// Blank RAM (`MIP_ZERO`, plus the `$4E/$4F` = `$20` workaround). What `MachineConfig::for_model`
    /// uses: frontends and the conformance harness ask for AppleWin's pattern explicitly.
    pub const ZERO: RamInit = RamInit {
        pattern: MIP_ZERO,
        random: RandomSource::Zero,
    };

    /// AppleWin's default pattern (`MIP_FF_FF_00_00`) with the oracle's deterministic draw.
    pub const APPLEWIN_DETERMINISTIC: RamInit = RamInit {
        pattern: MIP_FF_FF_00_00,
        random: RandomSource::Zero,
    };
}

impl Default for RamInit {
    /// AppleWin's pattern with the oracle's deterministic draw.
    fn default() -> Self {
        RamInit {
            pattern: MIP_FF_FF_00_00,
            random: RandomSource::Zero,
        }
    }
}

/// Stateful generator behind [`RandomSource`].
#[derive(Clone, Debug)]
pub struct Draw {
    state: Option<u32>,
}

impl Draw {
    pub fn new(src: RandomSource) -> Self {
        Draw {
            state: match src {
                RandomSource::Zero => None,
                RandomSource::Seed(s) => Some(s),
            },
        }
    }

    /// `getRandomTime()`.
    pub fn draw(&mut self) -> u32 {
        match &mut self.state {
            None => 0,
            Some(s) => {
                // xorshift32 (state 0 is a fixed point; nudge it).
                let mut x = if *s == 0 { 0x9E37_79B9 } else { *s };
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                *s = x;
                x.wrapping_mul(0x2545_F491)
            }
        }
    }
}

/// Fills `ram` (at least 64K) as `MemReset` does for `memmain`. The caller has zeroed it.
pub fn fill_main_ram(ram: &mut [u8], init: &RamInit, draw: &mut Draw) {
    let mut rand_time = draw.draw();
    let mut pattern = init.pattern;
    if pattern < 0 {
        pattern = (rand_time % NUM_MIP as u32) as i32;
        if pattern == MIP_PAGE_ADDRESS_LOW {
            pattern = MIP_FF_FF_00_00;
        }
    }
    match pattern {
        MIP_FF_FF_00_00 => {
            for i in (0..0xC000).step_by(4) {
                ram[i] = 0xFF;
                ram[i + 1] = 0xFF;
            }
            // Exceptions: xx28 xx29 xx68 xx69 (Apple //e)
            for i in (0..0xC000).step_by(512) {
                rand_time = draw.draw();
                ram[i + 0x28] = rand_time as u8;
                ram[i + 0x29] = (rand_time >> 8) as u8;
                rand_time = draw.draw();
                ram[i + 0x68] = rand_time as u8;
                ram[i + 0x69] = (rand_time >> 8) as u8;
            }
        }
        MIP_FF_00_FULL_PAGE => {
            for i in (0..0xC000).step_by(512) {
                ram[i..i + 256].fill(0xFF);
                ram[i + 0x28] = 0;
                ram[i + 0x68] = 0;
            }
        }
        MIP_00_FF_HALF_PAGE => {
            for i in (0x80..0xC000).step_by(256) {
                ram[i..i + 128].fill(0xFF);
            }
        }
        MIP_FF_00_HALF_PAGE => {
            for i in (0..0xC000).step_by(256) {
                ram[i..i + 128].fill(0xFF);
            }
        }
        MIP_RANDOM => {
            let mut random = [0u8; 256];
            for i in (0..0xC000).step_by(256) {
                for j in 0..256usize {
                    rand_time = draw.draw();
                    random[j] = rand_time as u8;
                    random[(j + 1) & 0xFF] = (rand_time >> 11) as u8;
                }
                ram[i..i + 256].copy_from_slice(&random);
            }
        }
        MIP_PAGE_ADDRESS_LOW => {
            for (i, b) in ram[..0xC000].iter_mut().enumerate() {
                *b = i as u8;
            }
        }
        MIP_PAGE_ADDRESS_HIGH => {
            for i in (0..0xC000).step_by(256) {
                ram[i..i + 256].fill((i >> 8) as u8);
            }
        }
        _ => {} // MIP_ZERO
    }
    // GH#206 ("Pooyan" expects RNDL/RNDH non-zero).
    rand_time = draw.draw();
    ram[0x4E] = 0x20 | rand_time as u8;
    ram[0x4F] = 0x20 | (rand_time >> 8) as u8;
    // GH#222
    ram[0x620B] = 0;
    ram[0xBFFD] = 0;
    ram[0xBFFE] = 0;
    ram[0xBFFF] = 0;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ram(pattern: i32, random: RandomSource) -> Vec<u8> {
        let mut r = vec![0u8; 0x10000];
        fill_main_ram(&mut r, &RamInit { pattern, random }, &mut Draw::new(random));
        r
    }

    #[test]
    fn default_is_ff_ff_00_00_deterministic() {
        let r = ram(MIP_FF_FF_00_00, RandomSource::Zero);
        assert_eq!(&r[0x100..0x108], &[0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]);
        assert_eq!(&r[0x28..0x2A], &[0, 0]);
        assert_eq!(&r[0x68..0x6A], &[0, 0]);
        assert_eq!((r[0x4E], r[0x4F]), (0x20, 0x20));
        assert_eq!(r[0x620B], 0);
        assert_eq!(&r[0xBFFC..0xC000], &[0xFF, 0, 0, 0]);
        assert!(r[0xC000..].iter().all(|&b| b == 0));
    }

    #[test]
    fn seeded_draw_changes_only_the_random_bytes() {
        let z = ram(MIP_FF_FF_00_00, RandomSource::Zero);
        let s = ram(MIP_FF_FF_00_00, RandomSource::Seed(12345));
        assert_ne!(z, s);
        for (i, (a, b)) in z.iter().zip(&s).enumerate() {
            if a != b {
                let lo = i & 0x1FF;
                assert!(
                    matches!(lo, 0x28 | 0x29 | 0x68 | 0x69) || i == 0x4E || i == 0x4F,
                    "{i:04X}"
                );
            }
        }
        assert!(s[0x4E] >= 0x20 && s[0x4F] >= 0x20);
        assert_eq!(s, ram(MIP_FF_FF_00_00, RandomSource::Seed(12345)));
    }

    #[test]
    fn other_patterns() {
        let r = ram(MIP_ZERO, RandomSource::Zero);
        assert!(r[..0x4E].iter().all(|&b| b == 0) && r[0x4E] == 0x20);
        let r = ram(MIP_FF_00_FULL_PAGE, RandomSource::Zero);
        assert_eq!(
            (r[0], r[0xFF], r[0x100], r[0x200], r[0x228]),
            (0xFF, 0xFF, 0, 0xFF, 0)
        );
        let r = ram(MIP_00_FF_HALF_PAGE, RandomSource::Zero);
        assert_eq!((r[0x7F], r[0x80], r[0xFF], r[0x100]), (0, 0xFF, 0xFF, 0));
        let r = ram(MIP_FF_00_HALF_PAGE, RandomSource::Zero);
        assert_eq!((r[0x7F], r[0x80], r[0x100]), (0xFF, 0, 0xFF));
        let r = ram(MIP_PAGE_ADDRESS_LOW, RandomSource::Zero);
        assert_eq!((r[0x1234], r[0xBFFC]), (0x34, 0xFC));
        let r = ram(MIP_PAGE_ADDRESS_HIGH, RandomSource::Zero);
        assert_eq!((r[0x1234], r[0x620A]), (0x12, 0x62));
        // random patterns are reproducible from the seed
        let a = ram(MIP_RANDOM, RandomSource::Seed(7));
        assert_eq!(a, ram(MIP_RANDOM, RandomSource::Seed(7)));
        assert_ne!(a, ram(MIP_RANDOM, RandomSource::Seed(8)));
        let _ = ram(-1, RandomSource::Seed(7));
    }
}
