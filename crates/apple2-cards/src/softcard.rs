//! Microsoft Z80 SoftCard (AppleWin z80emu.cpp, Z80VICE/z80.cpp `z80_mainloop`/`z80_RDMEM`).
//!
//! Any write to the card's `$Cs00-$CsFF` page toggles the CPU between the 6502 and the Z80 (it
//! asserts the 6502's DMA/RDY in hardware). The Z80 runs at twice the 6502 clock and sees the
//! Apple's memory remapped so that CP/M's page 0 lands at `$1000`:
//!
//! | Z80            | 6502           |
//! |----------------|----------------|
//! | `$0000-$AFFF`  | `$1000-$BFFF`  |
//! | `$B000-$DFFF`  | `$D000-$FFFF`  |
//! | `$E000-$EFFF`  | `$C000-$CFFF` (I/O and slot ROM) |
//! | `$F000-$FFFF`  | `$0000-$0FFF`  |
//!
//! The slot ROM reads `$FF` (z80emu.cpp:36). As in AppleWin, Z80 interrupts are not delivered.

use crate::state::{Dec, Enc};
use crate::z80::{Z80Bus, Z80};
use apple2_core::card::AltCpu;
use apple2_core::{Card, CardCtx};
use apple2_cpu::Bus;
use std::any::Any;

/// Z80 clock multiplier over the 6502 (`uZ80ClockMultiplier`, z80.cpp:5514).
const CLOCK_MULTIPLIER: u32 = 2;

/// Z80 address to Apple address (z80_RDMEM/z80_WRMEM).
pub fn z80_to_apple(addr: u16) -> u16 {
    match addr >> 12 {
        0..=0xA => addr + 0x1000,
        0xB..=0xD => addr + 0x2000,
        0xE => addr - 0x2000,
        _ => addr - 0xF000,
    }
}

struct AppleBus<'a>(&'a mut dyn Bus);

impl Z80Bus for AppleBus<'_> {
    fn read(&mut self, addr: u16) -> u8 {
        self.0.read(z80_to_apple(addr))
    }
    fn write(&mut self, addr: u16, value: u8) {
        self.0.write(z80_to_apple(addr), value);
    }
}

/// The Z80 plus the T-state remainder that does not fill a 6502 cycle yet.
pub struct Z80Cpu {
    pub z80: Z80,
    carry: u32,
}

impl AltCpu for Z80Cpu {
    fn step(&mut self, bus: &mut dyn Bus) -> u32 {
        let t = self.z80.step(&mut AppleBus(bus)) + self.carry;
        self.carry = t % CLOCK_MULTIPLIER;
        t / CLOCK_MULTIPLIER
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

pub struct SoftCard {
    cpu: Option<Box<Z80Cpu>>,
    /// This card currently owns the CPU (mirrors the board's owner so the toggle is stateless).
    active: bool,
}

impl Default for SoftCard {
    fn default() -> Self {
        Self::new()
    }
}

impl SoftCard {
    pub fn new() -> Self {
        SoftCard { cpu: Some(Box::new(Z80Cpu { z80: Z80::default(), carry: 0 })), active: false }
    }

    /// The Z80 registers (None only while the machine is executing a Z80 step).
    pub fn z80(&self) -> Option<&Z80> {
        self.cpu.as_ref().map(|c| &c.z80)
    }

    pub fn z80_mut(&mut self) -> Option<&mut Z80> {
        self.cpu.as_mut().map(|c| &mut c.z80)
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Card for SoftCard {
    fn name(&self) -> &'static str {
        "Z80 SoftCard"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn slot_rom_read(&mut self, _offset: u8, _ctx: &mut CardCtx) -> u8 {
        0xFF
    }

    fn slot_rom_write(&mut self, _offset: u8, _value: u8, ctx: &mut CardCtx) {
        self.active = !self.active;
        ctx.set_cpu_owner(self.active);
    }

    /// Ctrl-Reset and power-on: back to the 6502, `z80_reset` (CPU.cpp:816).
    fn reset(&mut self, _power_cycle: bool, ctx: &mut CardCtx) {
        self.active = false;
        ctx.set_cpu_owner(false);
        if let Some(c) = self.cpu.as_mut() {
            c.z80.reset();
        }
    }

    fn take_alt_cpu(&mut self) -> Option<Box<dyn AltCpu>> {
        self.cpu.take().map(|c| c as Box<dyn AltCpu>)
    }

    fn restore_alt_cpu(&mut self, cpu: Box<dyn AltCpu>) {
        self.cpu = cpu.into_any().downcast::<Z80Cpu>().ok();
    }

    fn save(&self) -> Vec<u8> {
        let mut e = Enc::default();
        e.bool(self.active);
        if let Some(c) = &self.cpu {
            e.u8(c.carry as u8);
            c.z80.save(&mut e);
        }
        e.0
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let mut d = Dec::new(state);
        let (Some(active), Some(carry), Some(c)) = (d.bool(), d.u8(), self.cpu.as_mut()) else {
            return false;
        };
        self.active = active;
        c.carry = u32::from(carry) % CLOCK_MULTIPLIER;
        c.z80.load(&mut d).is_some() && d.done()
    }
}
