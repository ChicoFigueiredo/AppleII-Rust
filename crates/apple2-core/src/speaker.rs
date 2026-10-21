//! Speaker toggle log. AppleWin `SpkrToggle` (Speaker.cpp:372) flips the cone on any access to
//! $C030-$C03F; the core only records *when* (phase 5 turns the log into PCM).

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Most toggles kept if nobody drains the log: the newest ones win. 65536 toggles is over 3 s
/// of a 10 kHz square wave, ample for a frontend that drains every frame.
pub const CAPACITY: usize = 1 << 16;

/// Cycle of the newest toggle ever made, kept after the log is drained. Derived, not machine
/// state: it is not serialized and never makes two states differ (so goldens and state hashes
/// stay put); after a state load it starts empty (speaker "inactive").
#[derive(Clone, Copy, Debug, Default)]
struct LastToggle(Option<u64>);

impl PartialEq for LastToggle {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}
impl Eq for LastToggle {}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Speaker {
    toggles: VecDeque<u64>,
    dropped: u64,
    #[serde(skip)]
    last_any: LastToggle,
}

impl Speaker {
    /// Records a toggle at absolute cycle `cycle`; the oldest entry is dropped when full.
    pub fn toggle(&mut self, cycle: u64) {
        self.last_any = LastToggle(Some(cycle));
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

    /// AppleWin `Spkr_IsActive` (Speaker.cpp:828): the speaker "recently active" flag is set by a
    /// toggle and cleared by `SpkrUpdate` after 0.2 s of Apple time without toggles
    /// (`g_fCurrentCLK6502 / 5` cycles, Speaker.cpp:451). Derived from the newest toggle, drained
    /// or not.
    pub fn is_active(&self, now: u64, clock_hz: f64) -> bool {
        self.last_any
            .0
            .is_some_and(|t| now.saturating_sub(t) <= (clock_hz / 5.0) as u64)
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
