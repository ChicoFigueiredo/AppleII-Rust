//! Speaker toggle log. AppleWin `SpkrToggle` (Speaker.cpp:372) flips the cone on any access to
//! $C030-$C03F; the core only records *when* (phase 5 turns the log into PCM).

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Most toggles kept if nobody drains the log: the newest ones win. 65536 toggles is over 3 s
/// of a 10 kHz square wave, ample for a frontend that drains every frame.
pub const CAPACITY: usize = 1 << 16;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speaker {
    toggles: VecDeque<u64>,
    dropped: u64,
}

impl Speaker {
    /// Records a toggle at absolute cycle `cycle`; the oldest entry is dropped when full.
    pub fn toggle(&mut self, cycle: u64) {
        if self.toggles.len() == CAPACITY {
            self.toggles.pop_front();
            self.dropped += 1;
        }
        self.toggles.push_back(cycle);
    }

    /// Removes and returns the pending toggles, oldest first.
    pub fn drain(&mut self) -> impl Iterator<Item = u64> + '_ {
        self.toggles.drain(..)
    }

    /// Cycle of the newest logged toggle (AppleWin's `g_nSpkrLastCycle`), if any is pending.
    pub fn last_toggle(&self) -> Option<u64> {
        self.toggles.back().copied()
    }

    /// Toggles lost to the cap since power-on.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Power-on (`SpkrReset`).
    pub fn reset(&mut self) {
        *self = Speaker::default();
    }
}
