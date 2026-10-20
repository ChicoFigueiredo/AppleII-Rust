//! Video soft-switch state (flags only; rendering is phase 3). AppleWin Video.cpp
//! `VideoSetMode` (:179) and `VideoResetState` (:166). 80STORE, PAGE2 and HIRES live in the
//! memory flags (`mf`), which is their single source of truth here.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoMode {
    pub text: bool,
    pub mixed: bool,
    pub col80: bool,
    pub altcharset: bool,
    pub dhires: bool,
    /// J-Plus only: AN2 on selects the ASCII half of the character ROM, off the Katakana half
    /// (NTSC_CharSet.cpp:189, GH#773). Not reset by RESET (the ][ family keeps its switches).
    pub jplus_ascii: bool,
}

impl Default for VideoMode {
    /// `VideoResetState`: text mode, everything else off.
    fn default() -> Self {
        VideoMode { text: true, mixed: false, col80: false, altcharset: false, dhires: false, jplus_ascii: false }
    }
}

impl VideoMode {
    /// Applies the switch at $C0`lo`. $C00C-$C00F and $C05E/F only exist outside the ][ family.
    pub fn set(&mut self, lo: u8, is_e: bool) {
        let on = lo & 1 != 0;
        match lo {
            0x0C | 0x0D if is_e => self.col80 = on,
            0x0E | 0x0F if is_e => self.altcharset = on,
            0x50 | 0x51 => self.text = on,
            0x52 | 0x53 => self.mixed = on,
            // $C05E sets DHIRES, $C05F clears it (Video.cpp:206).
            0x5E | 0x5F if is_e => self.dhires = !on,
            _ => {}
        }
    }
}
