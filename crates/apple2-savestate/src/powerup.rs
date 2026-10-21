//! Power-on RAM pattern (`MemReset`, Memory.cpp:2297-2440).
//!
//! `Snapshot_LoadState_v2` calls `MemReset()` before it reads the `Main Memory` map, and
//! `YamlHelper::LoadMemory` only writes the lines the file has. A save-state that carries a
//! partial dump (the AppleWin-Test stubs hold a single `0000: FF` line) therefore keeps the
//! power-on pattern everywhere else. This module reproduces that pattern for `$0000-$BFFF`.

/// `MIP_FF_FF_00_00`, the default of `g_nMemoryClearType` (Core.cpp:79).
pub const DEFAULT_PATTERN: u32 = 2;

/// Builds the 48K pattern. `rand` stands for `getRandomTime()`; the conformance oracle returns 0.
pub fn power_on_ram(pattern: u32, rand: &mut dyn FnMut() -> u32) -> Vec<u8> {
    let mut m = vec![0u8; 0xC000];
    match pattern {
        // MIP_RANDOM
        1 => {
            for page in m.chunks_mut(256) {
                let mut r = [0u8; 256];
                for i in 0..256usize {
                    let t = rand();
                    r[i & 0xFF] = t as u8;
                    r[(i + 1) & 0xFF] = (t >> 11) as u8;
                }
                page.copy_from_slice(&r);
            }
        }
        // MIP_FF_FF_00_00
        2 => {
            for i in (0..0xC000).step_by(4) {
                m[i] = 0xFF;
                m[i + 1] = 0xFF;
            }
            // Exceptions: xx28 xx29 xx68 xx69 (Apple //e)
            for base in (0..0xC000).step_by(512) {
                let t = rand();
                m[base + 0x28] = t as u8;
                m[base + 0x29] = (t >> 8) as u8;
                let t = rand();
                m[base + 0x68] = t as u8;
                m[base + 0x69] = (t >> 8) as u8;
            }
        }
        // MIP_FF_00_FULL_PAGE
        3 => {
            for base in (0..0xC000).step_by(512) {
                m[base..base + 256].fill(0xFF);
                m[base + 0x28] = 0;
                m[base + 0x68] = 0;
            }
        }
        // MIP_00_FF_HALF_PAGE
        4 => {
            for base in (0x80..0xC000).step_by(256) {
                m[base..base + 128].fill(0xFF);
            }
        }
        // MIP_FF_00_HALF_PAGE
        5 => {
            for base in (0..0xC000).step_by(256) {
                m[base..base + 128].fill(0xFF);
            }
        }
        // MIP_PAGE_ADDRESS_LOW
        6 => {
            for (i, b) in m.iter_mut().enumerate() {
                *b = i as u8;
            }
        }
        // MIP_PAGE_ADDRESS_HIGH
        7 => {
            for (i, b) in m.iter_mut().enumerate() {
                *b = (i >> 8) as u8;
            }
        }
        _ => {} // MIP_ZERO
    }
    // GH#206: "Pooyan" expects RNDL and RNDH to be non-zero.
    let t = rand();
    m[0x4E] = 0x20 | (t as u8);
    m[0x4F] = 0x20 | ((t >> 8) as u8);
    // GH#222
    m[0x620B] = 0;
    m[0xBFFD] = 0;
    m[0xBFFE] = 0;
    m[0xBFFF] = 0;
    m
}
