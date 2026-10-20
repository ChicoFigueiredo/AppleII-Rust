//! ThunderClock Plus (and the ROM-only TK clock): a NEC uPD1990AC calendar chip behind one I/O port, plus
//! 2K of firmware (slot ROM = its first page, expansion ROM = the whole image).
//!
//! AppleWin ships the firmware (`ThunderClockPlus.rom`, `TKClock.rom`) but its card is a stub
//! (Peripheral_Clock_ThunderClockPlus.cpp is commented out, `CT_GenericClock` is a dummy), so
//! this is a new implementation driven by the real firmware. The chip is modelled from the
//! uPD1990AC datasheet and the firmware's access pattern (`$Cs00`+`$CAB7`/`$CACF` routines):
//!
//! `$C0n0` write: bit 0 DATA IN, bit 1 CLK, bit 2 STB, bits 3-5 command C0-C2.
//! `$C0n0` read: bit 7 DATA OUT.
//!
//! Commands, latched on the falling edge of STB: 1 register shift, 3 time read (loads the 40-bit
//! register from the host clock). While in shift mode every rising CLK edge shifts the register
//! right, the DATA IN bit entering at the top. The time-set command (2) is accepted but ignored:
//! the host clock is authoritative. Register layout, LSB first: seconds and minutes, hours and day
//! of month (two BCD digits each), weekday (0 = Sunday), month (1-12, binary).

use crate::state::{Dec, Enc};
use crate::time::{HostClock, WallTime};
use apple2_core::card::EXPANSION_ROM_SIZE;
use apple2_core::{Card, CardCtx};
use std::any::Any;

const CMD_SHIFT: u8 = 1;
const CMD_TIME_READ: u8 = 3;

fn bcd(v: u8) -> u64 {
    u64::from(((v / 10) << 4) | (v % 10))
}

/// The uPD1990AC 40-bit register for `t`.
pub fn register_for(t: &WallTime) -> u64 {
    bcd(t.second) | bcd(t.minute) << 8 | bcd(t.hour) << 16 | bcd(t.day) << 24 | u64::from(t.weekday & 0xF) << 32 | u64::from(t.month & 0xF) << 36
}

/// Which firmware/card variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClockModel {
    ThunderClockPlus,
    /// The TK clock (`TKClock.rom`, 2304 bytes: a slot page followed by the 2K expansion ROM).
    /// Its firmware drives a MC6821 PIA (DDR/CR writes at `$C080-$C083`), not the uPD1990AC, and
    /// AppleWin has no emulation of it: only the ROMs are present here (reads float).
    TkClock,
}

pub struct ThunderClock {
    model: ClockModel,
    clock: Box<dyn HostClock>,
    slot_rom: [u8; 256],
    expansion: Box<[u8; EXPANSION_ROM_SIZE]>,
    register: u64,
    command: u8,
    stb: bool,
    clk: bool,
}

impl ThunderClock {
    pub fn new(model: ClockModel, clock: Box<dyn HostClock>) -> Self {
        let (rom, exp_at): (&[u8], usize) = match model {
            ClockModel::ThunderClockPlus => (apple2_roms::THUNDERCLOCKPLUS, 0),
            ClockModel::TkClock => (apple2_roms::TKCLOCK, 256),
        };
        let mut slot_rom = [0u8; 256];
        slot_rom.copy_from_slice(&rom[..256]);
        let mut expansion = Box::new([0u8; EXPANSION_ROM_SIZE]);
        expansion.copy_from_slice(&rom[exp_at..exp_at + EXPANSION_ROM_SIZE]);
        ThunderClock { model, clock, slot_rom, expansion, register: 0, command: 0, stb: false, clk: false }
    }

    pub fn model(&self) -> ClockModel {
        self.model
    }

    fn data_out(&self) -> bool {
        self.command == CMD_SHIFT && self.register & 1 != 0
    }
}

impl Card for ThunderClock {
    fn name(&self) -> &'static str {
        match self.model {
            ClockModel::ThunderClockPlus => "ThunderClock Plus",
            ClockModel::TkClock => "TK Clock",
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn io_read(&mut self, _lo: u8, ctx: &mut CardCtx) -> u8 {
        if self.model == ClockModel::TkClock {
            return ctx.floating_bus;
        }
        (ctx.floating_bus & 0x7F) | if self.data_out() { 0x80 } else { 0 }
    }

    fn io_write(&mut self, _lo: u8, value: u8, _ctx: &mut CardCtx) {
        if self.model == ClockModel::TkClock {
            return;
        }
        let (clk, stb) = (value & 2 != 0, value & 4 != 0);
        if self.stb && !stb {
            self.command = (value >> 3) & 7;
            if self.command == CMD_TIME_READ {
                self.register = register_for(&self.clock.now());
            }
        }
        if clk && !self.clk && self.command == CMD_SHIFT {
            self.register = self.register >> 1 | u64::from(value & 1) << 39;
        }
        self.clk = clk;
        self.stb = stb;
    }

    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        self.slot_rom[offset as usize]
    }

    fn expansion_rom(&self) -> Option<&[u8; EXPANSION_ROM_SIZE]> {
        Some(&self.expansion)
    }

    fn reset(&mut self, _power_cycle: bool, _ctx: &mut CardCtx) {
        self.command = 0;
        self.stb = false;
        self.clk = false;
    }

    fn save(&self) -> Vec<u8> {
        let mut e = Enc::default();
        e.u64(self.register).u8(self.command).bool(self.stb).bool(self.clk);
        e.0
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let mut d = Dec::new(state);
        let r = (|| Some((d.u64()?, d.u8()?, d.bool()?, d.bool()?)))();
        match r {
            Some((reg, cmd, stb, clk)) if d.done() && cmd < 8 => {
                (self.register, self.command, self.stb, self.clk) = (reg & ((1 << 40) - 1), cmd, stb, clk);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_packs_bcd_fields_lsb_first() {
        let r = register_for(&WallTime::SAMPLE);
        assert_eq!(r & 0xFF, 0x27, "seconds");
        assert_eq!(r >> 8 & 0xFF, 0x35, "minutes");
        assert_eq!(r >> 16 & 0xFF, 0x14, "hours");
        assert_eq!(r >> 24 & 0xFF, 0x19, "day");
        assert_eq!(r >> 32 & 0xF, 1, "weekday");
        assert_eq!(r >> 36 & 0xF, 10, "month");
    }
}
