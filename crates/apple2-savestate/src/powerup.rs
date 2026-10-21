//! Power-on RAM pattern (`MemReset`, Memory.cpp:2297-2440) for a save-state load.
//!
//! `Snapshot_LoadState_v2` calls `MemReset()` before it reads the `Main Memory` map, and
//! `YamlHelper::LoadMemory` only writes the lines the file has. A save-state that carries a
//! partial dump (the AppleWin-Test stubs hold a single `0000: FF` line) therefore keeps the
//! power-on pattern everywhere else. The pattern itself is `apple2_core::ram_init`.

use apple2_core::ram_init::{fill_main_ram, Draw};
use apple2_core::RamInit;

/// The 48K pattern `MemReset` leaves in `memmain` for `init`.
pub fn power_on_ram(init: &RamInit) -> Vec<u8> {
    let mut ram = vec![0u8; 0x10000];
    fill_main_ram(&mut ram, init, &mut Draw::new(init.random));
    ram.truncate(0xC000);
    ram
}
