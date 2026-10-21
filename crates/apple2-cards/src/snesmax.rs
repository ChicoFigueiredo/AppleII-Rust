//! SNES MAX: two NES/SNES controllers on a serial interface card (AppleWin SNESMAX.cpp, Alex
//! Lukacz 2021). Write `$C0n0` to latch, write `$C0n1` to clock the next button out, read
//! `$C0n0`: bit 7 = controller 1, bit 6 = controller 2, active low. Seventeen bits come out per
//! latch, buttons B Y Select Start Up Down Left Right A X L R (Fl Fr) first, then the
//! "controller plugged in" bit (0 when present).
//!
//! The host supplies controller button masks ([`buttons`]) with [`SnesMax::set_controller`], or a
//! raw pad state ([`HostPad`], the Windows `JOYINFOEX` view) with [`SnesMax::set_host_pad`], which
//! applies `GetControllerButtons` (SNESMAX.cpp:150-178) with the main (DualShock/DualSense) or the
//! alternative ("8BitDo NES30 PRO", user-remappable) button table of each controller.

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

/// Host buttons per pad (`SNESMAXCard::NUM_BUTTONS`): bit `i` of [`HostPad::buttons`].
pub const HOST_BUTTONS: usize = 16;

/// Which card button (bit index into the [`buttons`] mask) each host button bit presses;
/// `None` is `UNUSED`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ButtonMap(pub [Option<u8>; HOST_BUTTONS]);

const fn card_bit(mask: u32) -> Option<u8> {
    Some(mask.trailing_zeros() as u8)
}

impl ButtonMap {
    /// `m_mainControllerButtons`: Sony DS4 / DualSense.
    pub const MAIN: ButtonMap = {
        use buttons::*;
        let n = None;
        ButtonMap([
            card_bit(Y),
            card_bit(B),
            card_bit(A),
            card_bit(X),
            card_bit(L),
            card_bit(R),
            n,
            n,
            card_bit(SELECT),
            card_bit(START),
            n,
            n,
            n,
            n,
            n,
            n,
        ])
    };

    /// Default of `m_altControllerButtons`: "8BitDo NES30 PRO".
    pub const ALT_DEFAULT: ButtonMap = {
        use buttons::*;
        let n = None;
        ButtonMap([
            card_bit(A),
            card_bit(B),
            n,
            card_bit(X),
            card_bit(Y),
            n,
            n,
            n,
            card_bit(L),
            card_bit(R),
            card_bit(SELECT),
            card_bit(START),
            n,
            n,
            n,
            n,
        ])
    };

    /// A button name of the AppleWin mapping file (`ParseControllerMappingFile`, SNESMAX.cpp:
    /// `A B X Y LB RB SELECT START`, empty = unused).
    pub fn parse_name(name: &str) -> Result<Option<u8>, String> {
        use buttons::*;
        Ok(match name {
            "A" => card_bit(A),
            "B" => card_bit(B),
            "X" => card_bit(X),
            "Y" => card_bit(Y),
            "LB" => card_bit(L),
            "RB" => card_bit(R),
            "SELECT" => card_bit(SELECT),
            "START" => card_bit(START),
            "" => None,
            other => return Err(format!("Controller mapping file: Unknown button: {other}")),
        })
    }
}

/// A pad as `joyGetPosEx` reports it (JOY_RETURNPOV | JOY_RETURNBUTTONS).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostPad {
    /// `dwXpos` / `dwYpos`, 0..=65535 with 32768 centred (only the high byte is used).
    pub x: u32,
    pub y: u32,
    /// `dwPOV` in hundredths of a degree (0 = up, 9000 = right...), or 65535 when centred.
    pub pov: u32,
    /// `dwButtons`.
    pub buttons: u32,
}

impl HostPad {
    /// A pad with the sticks and the POV hat centred and no button pressed.
    pub const CENTRED: HostPad = HostPad {
        x: 0x8000,
        y: 0x8000,
        pov: 0xFFFF,
        buttons: 0,
    };
}

/// `SNESMAXCard::GetControllerButtons`: direction from the stick/POV, buttons through `map`.
pub fn controller_buttons(pad: &HostPad, map: &ButtonMap) -> u32 {
    let (x, y, pov) = ((pad.x >> 8) & 0xFF, (pad.y >> 8) & 0xFF, pad.pov);
    let mut m = 0;
    if y < 103 || matches!(pov, 0 | 4500 | 31500) {
        m |= buttons::UP;
    }
    if y > 153 || (13500..=22500).contains(&pov) {
        m |= buttons::DOWN;
    }
    if x < 103 || (22500..=31500).contains(&pov) {
        m |= buttons::LEFT;
    }
    if x > 153 || (4500..=13500).contains(&pov) {
        m |= buttons::RIGHT;
    }
    for (i, bit) in map.0.iter().enumerate() {
        if let (true, Some(bit)) = (pad.buttons & (1 << i) != 0, bit) {
            m |= 1 << bit;
        }
    }
    m
}
const PLUGGED: u32 = 0x10000;
const IDLE: u32 = 0x1FFFF;

pub struct SnesMax {
    /// Host state: button mask, or None when the controller is unplugged.
    host: [Option<u32>; 2],
    /// Shift registers (active low, `~mask`), reloaded by the latch.
    shift: [u32; 2],
    index: u32,
    /// `m_altControllerType` (`-snes-max-alt-joy1/2`, or a valid `-snes-max-user-joy1/2` file).
    alt: [bool; 2],
    /// `m_altControllerButtons`.
    alt_map: [ButtonMap; 2],
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
            alt: [false; 2],
            alt_map: [ButtonMap::ALT_DEFAULT; 2],
        }
    }

    /// Selects the alternative button table for controller `n`.
    pub fn set_alt_controller(&mut self, n: usize, alt: bool) {
        self.alt[n & 1] = alt;
    }

    pub fn alt_controller(&self, n: usize) -> bool {
        self.alt[n & 1]
    }

    /// Replaces the alternative table of controller `n` (a user mapping file).
    pub fn set_alt_map(&mut self, n: usize, map: ButtonMap) {
        self.alt_map[n & 1] = map;
    }

    pub fn alt_map(&self, n: usize) -> &ButtonMap {
        &self.alt_map[n & 1]
    }

    /// Controller `n` from a raw host pad state (`None` = not connected), through the main or
    /// alternative button table.
    pub fn set_host_pad(&mut self, n: usize, pad: Option<HostPad>) {
        let n = n & 1;
        let map = if self.alt[n] {
            &self.alt_map[n]
        } else {
            &ButtonMap::MAIN
        };
        self.host[n] = pad.map(|p| controller_buttons(&p, map) & 0xFFF);
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
