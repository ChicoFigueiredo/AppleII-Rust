//! SNES MAX: two NES/SNES controllers on a serial interface card (AppleWin SNESMAX.cpp, Alex
//! Lukacz 2021). Write `$C0n0` to latch, write `$C0n1` to clock the next button out, read
//! `$C0n0`: bit 7 = controller 1, bit 6 = controller 2, active low. Seventeen bits come out per
//! latch, buttons B Y Select Start Up Down Left Right A X L R (Fl Fr) first, then the
//! "controller plugged in" bit (0 when present).
//!
//! The host supplies controller button masks ([`buttons`]) with [`SnesMax::set_controller`];
//! AppleWin's button remapping files are a frontend concern.

use crate::state::{Dec, Enc};
use apple2_core::{Card, CardCtx};
use std::any::Any;

/// Bit positions of a button mask (`m_controllerXButtons`, SNESMAX.cpp:16-24).
pub mod buttons {
    pub const B: u32 = 1 << 0;
    pub const Y: u32 = 1 << 1;
    pub const SELECT: u32 = 1 << 2;
    pub const START: u32 = 1 << 3;
    pub const UP: u32 = 1 << 4;
    pub const DOWN: u32 = 1 << 5;
    pub const LEFT: u32 = 1 << 6;
    pub const RIGHT: u32 = 1 << 7;
    pub const A: u32 = 1 << 8;
    pub const X: u32 = 1 << 9;
    pub const L: u32 = 1 << 10;
    pub const R: u32 = 1 << 11;
}

const NUM_BUTTONS: u32 = 16;
const PLUGGED: u32 = 0x10000;
const IDLE: u32 = 0x1FFFF;

pub struct SnesMax {
    /// Host state: button mask, or None when the controller is unplugged.
    host: [Option<u32>; 2],
    /// Shift registers (active low, `~mask`), reloaded by the latch.
    shift: [u32; 2],
    index: u32,
}

impl Default for SnesMax {
    fn default() -> Self {
        Self::new()
    }
}

impl SnesMax {
    pub fn new() -> Self {
        SnesMax {
            host: [None; 2],
            shift: [IDLE; 2],
            index: 0,
        }
    }

    /// Controller `n` (0 or 1): the pressed-button mask, or `None` if not connected.
    pub fn set_controller(&mut self, n: usize, mask: Option<u32>) {
        self.host[n & 1] = mask.map(|m| m & 0xFFF);
    }

    /// `m_buttonIndex` (SNESMAX.cpp:41): how many bits were clocked out since the last latch.
    pub fn button_index(&self) -> u32 {
        self.index
    }

    /// Sets `m_buttonIndex` and resets both shift registers to "no buttons"
    /// (`LoadSnapshot`, SNESMAX.cpp:270-274).
    pub fn restore_button_index(&mut self, index: u32) {
        self.index = index;
        self.shift = [IDLE; 2];
    }
}

impl Card for SnesMax {
    fn name(&self) -> &'static str {
        "SNES MAX"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        let fb = ctx.floating_bus;
        match lo {
            0 => (fb & 0x3F) | ((self.shift[1] & 1) as u8) << 6 | ((self.shift[0] & 1) as u8) << 7,
            _ => fb,
        }
    }

    fn io_write(&mut self, lo: u8, _value: u8, _ctx: &mut CardCtx) {
        match lo {
            0 => {
                // Latch: unplugged controllers read all ones (bit 16 set = not present).
                self.index = 0;
                for (s, h) in self.shift.iter_mut().zip(self.host) {
                    *s = !h.map_or(0, |m| m | PLUGGED);
                }
            }
            1 if self.index <= NUM_BUTTONS => {
                self.index += 1;
                for s in &mut self.shift {
                    *s >>= 1;
                }
            }
            _ => {}
        }
    }

    fn save(&self) -> Vec<u8> {
        let mut e = Enc::default();
        e.u32(self.index).u32(self.shift[0]).u32(self.shift[1]);
        e.0
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let mut d = Dec::new(state);
        match (d.u32(), d.u32(), d.u32()) {
            (Some(i), Some(a), Some(b)) if d.done() && i <= NUM_BUTTONS + 1 => {
                (self.index, self.shift) = (i, [a, b]);
                true
            }
            _ => false,
        }
    }
}
