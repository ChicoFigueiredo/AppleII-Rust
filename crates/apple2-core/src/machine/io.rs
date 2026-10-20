//! $C000-$CFFF dispatch of the [`Board`] (AppleWin Memory.cpp `IORead_C0xx`/`IOWrite_C0xx`
//! tables, :540-870). Keyboard, speaker and game I/O are stubs until slice 6; slot cards and
//! the floating bus value arrives in slice 5. Slot cards are dispatched through [`crate::card::Card`].

use super::Board;
use crate::slots::{expansion_offset, ExpansionRom};
use crate::language_card::is_rmw_abs;
use crate::memory::mf;
use apple2_cpu::Variant;

impl Board {
    /// Value read from an address nobody drives (slice 5 returns the video scanner byte).
    pub fn floating_bus(&self) -> u8 {
        0
    }

    // ---- Slice 6 stubs: keyboard, speaker, game port. ----

    /// `KeybReadData` ($C000-$C00F read).
    fn keyboard_read_data(&mut self) -> u8 {
        0
    }

    /// `KeybReadFlag` ($C010 read on the //e): AKD in bit 7, then clears the strobe.
    fn keyboard_read_flag(&mut self) -> u8 {
        0
    }

    /// `KeybGetKeycode`: bits 0-6 of the $C011-$C01F status reads.
    fn keyboard_keycode(&self) -> u8 {
        0
    }

    fn keyboard_clear_strobe(&mut self) {}

    fn speaker_toggle(&mut self, _addr: u16) -> u8 {
        self.floating_bus()
    }

    fn game_io_read(&mut self, _addr: u16) -> u8 {
        self.floating_bus()
    }

    // ---- Slice 5 stub. ----

    /// `VideoGetVblBar`: true outside the vertical blank.
    fn vbl_bar(&self) -> bool {
        true
    }

    // ---- Slice 3/4 seams. ----

    /// $C080-$C0FF: Language Card first, then the card in the slot.
    fn slot_io_read(&mut self, addr: u16) -> u8 {
        if self.lc_io(addr, false) {
            return self.floating_bus();
        }
        let (slot, lo) = (((addr as u8) >> 4) - 8, addr as u8 & 0xF);
        match self.card_call(slot, |c, x| c.io_read(lo, x)) {
            Some(v) => v,
            None => self.floating_bus(), // IO_Null
        }
    }

    fn slot_io_write(&mut self, addr: u16, value: u8) {
        if !self.lc_io(addr, true) {
            let (slot, lo) = (((addr as u8) >> 4) - 8, addr as u8 & 0xF);
            self.card_call(slot, |c, x| c.io_write(lo, value, x));
        }
    }

    /// Language Card / Saturn access; false if no such card is in the addressed slot.
    fn lc_io(&mut self, addr: u16, write: bool) -> bool {
        let slot = ((addr as u8) >> 4) - 8;
        if !self.lc.has_slot(slot) {
            return false;
        }
        let rmw = write && {
            // GH#700: operand bytes of the executing opcode (the CPU does not expose them).
            let (pc, op) = self.current_opcode();
            let byte = |a: u16| self.memory.read(a).unwrap_or(0) as u16;
            let operand = byte(pc.wrapping_add(1)) | byte(pc.wrapping_add(2)) << 8;
            is_rmw_abs(op, operand, addr, self.model.cpu_variant() == Variant::Cmos65C02)
        };
        self.lc.io(slot, addr as u8, write, rmw, &mut self.memory)
    }

    /// `IO_Cxxx` (Memory.cpp:946-1086) for a read (`write` None) or write of `$C100-$CFFF`:
    /// updates IO_SELECT/INTC8ROM/the expansion-ROM choice, then serves the access from the
    /// internal Cx ROM, the slot card, its expansion ROM or the floating bus.
    pub(super) fn cx_access(&mut self, addr: u16, write: Option<u8>) -> u8 {
        let m = self.memory.memmode();
        let (intcxrom, slotc3rom) = (m & mf::INTCXROM != 0, m & mf::SLOTC3ROM != 0);
        self.slots.select(addr, self.is_e, intcxrom, slotc3rom);
        if addr < 0xC800 {
            let slot = (addr >> 8) as u8 & 7;
            // Peripheral ROM unless the internal one is switched in (UTAIIe:5-28).
            if self.is_e && (intcxrom || (!slotc3rom && slot == 3)) {
                return self.memory.internal_cx(addr).unwrap_or_else(|| self.floating_bus());
            }
            if self.slots.card(slot).is_none() && !self.lc.has_slot(slot) {
                return self.floating_bus(); // GH#149/GH#164: empty slot
            }
            return match write {
                None => self.card_call(slot, |c, x| c.slot_rom_read(addr as u8, x)).unwrap_or(0),
                Some(v) => {
                    self.card_call(slot, |c, x| c.slot_rom_write(addr as u8, v, x));
                    0
                }
            };
        }
        if self.is_e && (intcxrom || self.slots.intc8rom()) {
            return self.memory.internal_cx(addr).unwrap_or_else(|| self.floating_bus());
        }
        match self.slots.expansion() {
            ExpansionRom::Slot(slot) => match write {
                None => self
                    .card_call(slot, |c, x| c.expansion_rom().map_or(x.floating_bus, |r| r[expansion_offset(addr)]))
                    .unwrap_or_else(|| self.floating_bus()),
                Some(v) => {
                    self.card_call(slot, |c, x| c.expansion_rom_write(addr - 0xC800, v, x));
                    0
                }
            },
            _ => self.floating_bus(),
        }
    }

    // ---- Dispatch. ----

    /// Read of $C000-$C0FF.
    pub(super) fn io_read(&mut self, addr: u16) -> u8 {
        let lo = addr as u8;
        match lo >> 4 {
            0 => self.keyboard_read_data(),
            1 => self.read_c01x(lo, addr),
            3 => self.speaker_toggle(addr),
            5 => self.c05x(lo),
            6 => self.game_io_read(addr),
            0x8.. => self.slot_io_read(addr),
            _ => self.floating_bus(), // C02x, C04x, C07x (IO_Null)
        }
    }

    /// Write to $C000-$C0FF.
    pub(super) fn io_write(&mut self, addr: u16, value: u8) {
        let lo = addr as u8;
        match lo >> 4 {
            0 => {
                if lo <= 0x0B {
                    let before = self.memory.memmode() & (mf::INTCXROM | mf::SLOTC3ROM);
                    self.memory.set_paging(lo, value);
                    let after = self.memory.memmode();
                    if before != after & (mf::INTCXROM | mf::SLOTC3ROM) {
                        self.slots.cx_switches_changed(after & mf::INTCXROM != 0);
                    }
                } else {
                    self.video.set(lo, self.is_e);
                }
            }
            1 => self.keyboard_clear_strobe(),
            3 => {
                self.speaker_toggle(addr);
            }
            5 => {
                self.c05x(lo);
            }
            7 => match lo & 0xF {
                // RamWorks bank select (the //e only; `set_paging` checks).
                1 | 3 => self.memory.set_paging(lo, value),
                // //c: IOUDIS on/off (Memory.cpp:808-818).
                0xE | 0xF if self.model.is_apple2c() => {
                    let m = self.memory.memmode();
                    self.memory.set_memmode(if lo & 1 == 0 { m | mf::IOUDIS } else { m & !mf::IOUDIS });
                }
                _ => {}
            },
            0x8.. => self.slot_io_write(addr, value),
            _ => {} // C02x tape, C04x, C06x (slice 6)
        }
    }

    /// $C011-$C01F reads (Memory.cpp:579): bit 7 = the flag, bits 0-6 = last key code. On the
    /// ][ family the read only clears the strobe and returns the floating bus.
    fn read_c01x(&mut self, lo: u8, _addr: u16) -> u8 {
        if !self.is_e {
            self.keyboard_clear_strobe();
            return self.floating_bus();
        }
        let m = self.memory.memmode();
        let flag = |f: u32| m & f != 0;
        let v = &self.video;
        let set = match lo & 0xF {
            0x0 => return self.keyboard_read_flag(),
            0x1 => flag(mf::BANK2),
            0x2 => flag(mf::HIGHRAM),
            0x3 => flag(mf::AUXREAD),
            0x4 => flag(mf::AUXWRITE),
            0x5 => flag(mf::INTCXROM),
            0x6 => flag(mf::ALTZP),
            0x7 => flag(mf::SLOTC3ROM),
            0x8 => flag(mf::STORE80),
            0x9 => self.vbl_bar(),
            0xA => v.text,
            0xB => v.mixed,
            0xC => flag(mf::PAGE2),
            0xD => flag(mf::HIRES),
            0xE => v.altcharset,
            _ => v.col80,
        };
        self.keyboard_keycode() | if set { 0x80 } else { 0 }
    }

    /// $C050-$C05F, same for reads and writes: video switches, PAGE2/HIRES, annunciators.
    fn c05x(&mut self, lo: u8) -> u8 {
        match lo & 0xF {
            0..=3 => self.video.set(lo, self.is_e),
            4..=7 => self.memory.set_paging(lo, 0),
            _ => {
                // IOUDIS gates AN3/DHIRES on the //c (Memory.cpp:786).
                if self.is_e && (!self.model.is_apple2c() || self.memory.memmode() & mf::IOUDIS != 0) {
                    self.video.set(lo, true);
                }
                if self.model.is_apple2c() {
                    return 0;
                }
                self.annunciators[(lo as usize >> 1) & 3] = lo & 1 != 0;
                self.memory.set_paging(lo, 0); // Base64A ALTROM0/1
            }
        }
        self.floating_bus()
    }
}
