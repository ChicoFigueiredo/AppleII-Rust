//! 4Play: four Atari-style digital joystick ports on one card (AppleWin FourPlay.cpp, Alex
//! Lukacz 2021). `$C0n0-$C0n3` read joystick 1-4; the host feeds [`Stick`] states with
//! [`FourPlay::set_stick`] (AppleWin polled joysticks 1-2 and the E/S/D/F/Z/X and I/J/K/L/N/M
//! keys for 3-4: that mapping is the frontend's job).

use apple2_core::{Card, CardCtx};
use std::any::Any;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stick {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub trigger1: bool,
    pub trigger2: bool,
}

impl Stick {
    /// `MakeByte` (FourPlay.cpp:44): bit 4 always low, bit 5 always high.
    pub fn byte(&self) -> u8 {
        u8::from(self.up) | u8::from(self.down) << 1 | u8::from(self.left) << 2 | u8::from(self.right) << 3
            | 0x20
            | u8::from(self.trigger2) << 6
            | u8::from(self.trigger1) << 7
    }
}

#[derive(Default)]
pub struct FourPlay {
    sticks: [Stick; 4],
}

impl FourPlay {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_stick(&mut self, port: usize, stick: Stick) {
        self.sticks[port & 3] = stick;
    }
}

impl Card for FourPlay {
    fn name(&self) -> &'static str {
        "4Play"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        match lo {
            0..=3 => self.sticks[lo as usize].byte(),
            _ => ctx.floating_bus,
        }
    }

    // No state in AppleWin's snapshot; the held sticks are host input, so none here either.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_layout_matches_makebyte() {
        assert_eq!(Stick::default().byte(), 0x20);
        let s = Stick { up: true, right: true, trigger1: true, ..Default::default() };
        assert_eq!(s.byte(), 0x01 | 0x08 | 0x20 | 0x80);
        let s = Stick { down: true, left: true, trigger2: true, ..Default::default() };
        assert_eq!(s.byte(), 0x02 | 0x04 | 0x20 | 0x40);
    }
}
