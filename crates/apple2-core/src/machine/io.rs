//! $C000-$CFFF dispatch of the [`Board`] (AppleWin Memory.cpp `IORead_C0xx`/`IOWrite_C0xx`
//! tables, :540-870): keyboard, speaker, game port, video/paging switches and, through
//! [`crate::card::Card`], slot cards. The floating bus is the video scanner byte.

use super::Board;
use crate::language_card::is_rmw_abs;
use crate::memory::mf;
use crate::slots::{expansion_offset, ExpansionRom};
use crate::video_scanner::Switches;
use crate::Model;

impl Board {
    /// Cycle of the frame the video scanner is at for an access now: the start of the executing
    /// instruction (AppleWin advances the scanner after each opcode, cpu6502.h:385).
    fn scan_cycle(&self) -> u32 {
        (self.cycles_in_frame + self.cycles_per_frame - self.opcode_ticks) % self.cycles_per_frame
    }

    /// Value read from an address nobody drives: the byte the video generator is fetching from
    /// the CPU read image (`MemReadFloatingBus`, Memory.cpp:2458-2476), which is the auxiliary RAM
    /// while RAMRD is on (AUD-C-050; see [`crate::memory::Memory::peek_floating_image`]).
    pub fn floating_bus(&self) -> u8 {
        let m = self.memory.memmode();
        let sw = Switches {
            text: self.video.text,
            mixed: self.video.mixed,
            hires: m & mf::HIRES != 0,
            page2: m & mf::PAGE2 != 0 && m & mf::STORE80 == 0,
        };
        self.memory
            .peek_floating_image(self.scanner.address(self.scan_cycle(), sw))
    }

    // ---- Keyboard, speaker, game port. ----

    /// Cycle stamped on timed I/O (speaker, paddles): the count when the instruction began.
    /// AppleWin runs the handler before adding the opcode's cycles to `uExecutedCycles`
    /// (cpu6502.h: `ABS STA CYC(4)`), like the video scanner.
    pub(super) fn access_cycle(&self) -> u64 {
        self.cycles - self.opcode_ticks as u64
    }

    /// `SpkrToggle` ($C030-$C03F, read or write, Speaker.cpp:372).
    fn speaker_toggle(&mut self) -> u8 {
        let now = self.access_cycle();
        self.speaker.toggle(now);
        self.floating_bus()
    }

    /// `IORead_C06x` (Memory.cpp:834): $C060 tape/Pravets 8A keyboard, buttons, paddles. Address
    /// bit 3 is ignored (UTAIIe:7-5).
    fn game_io_read(&mut self, addr: u16) -> u8 {
        let fb = self.floating_bus();
        let bit7 = |on: bool| (fb & 0x7F) | if on { 0x80 } else { 0 };
        match addr & 7 {
            // TAPEIN reads high when unconnected (tape is out of scope).
            0 if self.model == Model::Pravets8A => self.keyboard.pravets_c060(fb),
            0 => bit7(true),
            n @ 1..=3 => {
                let n = n as usize - 1;
                let forced = self
                    .game_io
                    .as_ref()
                    .and_then(|d| d.button(n, &self.annunciators));
                bit7(forced.unwrap_or_else(|| self.game.button(n, self.model.is_apple2())))
            }
            n => {
                let n = n as usize - 4;
                let forced = self
                    .game_io
                    .as_ref()
                    .is_some_and(|d| d.paddle(n, &self.annunciators).is_some());
                bit7(self.game.paddle_active_with(n, self.access_cycle(), forced))
            }
        }
    }

    /// `VideoGetVblBar` (Video.cpp:483): true outside the vertical blank.
    fn vbl_bar(&self) -> bool {
        self.scanner.vbl_bar(self.scan_cycle())
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
            is_rmw_abs(op, operand, addr, self.cmos)
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
        if self.is_e && self.rom_tap.is_some() {
            // IsPotentialNoSlotClockAccess (Memory.cpp:903, UTAIIe:5-28).
            let hi = (addr >> 8) as u8;
            if ((intcxrom || !slotc3rom) && hi == 0xC3) || (intcxrom && hi == 0xC8) {
                let mut data = write.unwrap_or(0);
                if let Some(tap) = self.rom_tap.as_mut() {
                    if tap.access(addr, write.is_some(), &mut data) && write.is_none() {
                        return data;
                    }
                }
            }
        }
        if addr < 0xC800 {
            let slot = (addr >> 8) as u8 & 7;
            // Peripheral ROM unless the internal one is switched in (UTAIIe:5-28).
            if self.is_e && (intcxrom || (!slotc3rom && slot == 3)) {
                return self
                    .memory
                    .internal_cx(addr)
                    .unwrap_or_else(|| self.floating_bus());
            }
            if self.slots.card(slot).is_none() && !self.lc.has_slot(slot) {
                return self.floating_bus(); // GH#149/GH#164: empty slot
            }
            return match write {
                None => self
                    .card_call(slot, |c, x| c.slot_rom_read(addr as u8, x))
                    .unwrap_or(0),
                Some(v) => {
                    self.card_call(slot, |c, x| c.slot_rom_write(addr as u8, v, x));
                    0
                }
            };
        }
        if self.is_e && (intcxrom || self.slots.intc8rom()) {
            return self
                .memory
                .internal_cx(addr)
                .unwrap_or_else(|| self.floating_bus());
        }
        match self.slots.expansion() {
            ExpansionRom::Slot(slot) => match write {
                None => self
                    .card_call(slot, |c, x| {
                        c.expansion_rom()
                            .map_or(x.floating_bus, |r| r[expansion_offset(addr)])
                    })
                    .unwrap_or_else(|| self.floating_bus()),
                Some(v) => {
                    self.card_call(slot, |c, x| c.expansion_rom_write(addr - 0xC800, v, x));
                    0
                }
            },
            _ => self.floating_bus(),
        }
    }

    /// `IO_F8xx` (Memory.cpp:1089, GH#827): the No-Slot Clock under the ][/][+ ROM sees accesses
    /// to `$F800-$FFFF` while the Language Card is not mapped. Returns the replaced read value.
    #[inline]
    pub(super) fn f8_tap(&mut self, addr: u16, write: Option<u8>) -> Option<u8> {
        if addr < 0xF800 || self.is_e || self.rom_tap.is_none() {
            return None;
        }
        if self.memory.memmode() & (mf::HIGHRAM | mf::WRITERAM) != 0 {
            return None;
        }
        let mut data = write.unwrap_or(0);
        let replaced = self
            .rom_tap
            .as_mut()?
            .access(addr, write.is_some(), &mut data);
        (replaced && write.is_none()).then_some(data)
    }

    /// Annunciator/dongle-aware paddle strobe ($C070-$C07F).
    fn trigger_paddles(&mut self) {
        let forced = match &self.game_io {
            Some(d) => std::array::from_fn(|n| d.paddle(n, &self.annunciators)),
            None => [None; 4],
        };
        self.game.trigger_with(self.access_cycle(), forced);
    }

    /// `$C000-$C07F` bus snoop for cards that asked for it.
    fn snoop(&mut self, addr: u16, write: bool, value: u8) {
        for slot in 0..8u8 {
            if self.snoop_mask & (1 << slot) != 0 {
                self.card_call(slot, |c, x| c.io_snoop(addr, write, value, x));
            }
        }
    }

    // ---- Dispatch. ----

    /// Read of $C000-$C0FF.
    pub(super) fn io_read(&mut self, addr: u16) -> u8 {
        let lo = addr as u8;
        if self.snoop_mask != 0 && lo < 0x80 {
            self.snoop(addr, false, 0);
        }
        match lo >> 4 {
            0 => self.keyboard.read_data(),
            1 => self.read_c01x(lo, addr),
            3 => self.speaker_toggle(),
            7 => self.c07x_read(lo),
            5 => self.c05x(lo),
            6 => self.game_io_read(addr),
            0x8.. => self.slot_io_read(addr),
            _ => self.floating_bus(), // C02x, C04x, C07x (IO_Null)
        }
    }

    /// Write to $C000-$C0FF.
    pub(super) fn io_write(&mut self, addr: u16, value: u8) {
        let lo = addr as u8;
        if self.snoop_mask != 0 && lo < 0x80 {
            self.snoop(addr, true, value);
        }
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
            1 => {
                self.keyboard.clear_strobe();
            }
            3 => {
                self.speaker_toggle();
            }
            5 => {
                self.c05x(lo);
            }
            7 => {
                self.trigger_paddles();
                match lo & 0xF {
                    // RamWorks bank select (the //e only; `set_paging` checks).
                    1 | 3 => self.memory.set_paging(lo, value),
                    // //c: IOUDIS on/off (Memory.cpp:808-818).
                    0xE | 0xF if self.model.is_apple2c() => {
                        let m = self.memory.memmode();
                        self.memory.set_memmode(if lo & 1 == 0 {
                            m | mf::IOUDIS
                        } else {
                            m & !mf::IOUDIS
                        });
                    }
                    _ => {}
                }
            }
            0x8.. => self.slot_io_write(addr, value),
            // $C060 on the Pravets 8A selects 8-bit keyboard mode (Memory.cpp:719).
            6 if lo == 0x60 && self.model == Model::Pravets8A => {
                self.keyboard.set_pravets_8bit(value)
            }
            _ => {} // C02x tape, C04x, other C06x
        }
    }

    /// $C011-$C01F reads (Memory.cpp:579): bit 7 = the flag, bits 0-6 = last key code. On the
    /// ][ family the read only clears the strobe and returns the floating bus.
    fn read_c01x(&mut self, lo: u8, _addr: u16) -> u8 {
        if !self.is_e {
            self.keyboard.clear_strobe();
            return self.floating_bus();
        }
        let m = self.memory.memmode();
        let flag = |f: u32| m & f != 0;
        let v = &self.video;
        let set = match lo & 0xF {
            0x0 => return self.keyboard.read_flag(),
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
        self.keyboard.keycode() | if set { 0x80 } else { 0 }
    }

    /// $C070-$C07F read: triggers the paddle timers (Memory.cpp:745); $C07E/$C07F expose IOUDIS
    /// and DHIRES in bit 7 on the //c and Enhanced //e (GH#636).
    fn c07x_read(&mut self, lo: u8) -> u8 {
        self.trigger_paddles();
        let fb = self.floating_bus();
        let bit7 = |on: bool| (fb & 0x7F) | if on { 0x80 } else { 0 };
        let enhanced = matches!(self.model, Model::Apple2eEnhanced | Model::Tk3000e);
        match lo & 0xF {
            0xE if self.model.is_apple2c() => bit7(self.memory.memmode() & mf::IOUDIS != 0),
            0xF if enhanced || self.model.is_apple2c() => bit7(self.video.dhires),
            _ => fb,
        }
    }

    /// $C050-$C05F, same for reads and writes: video switches, PAGE2/HIRES, annunciators.
    fn c05x(&mut self, lo: u8) -> u8 {
        match lo & 0xF {
            0..=3 => self.video.set(lo, self.is_e),
            4..=7 => self.memory.set_paging(lo, 0),
            _ => {
                // IOUDIS gates AN3/DHIRES on the //c (Memory.cpp:786).
                if self.is_e
                    && (!self.model.is_apple2c() || self.memory.memmode() & mf::IOUDIS != 0)
                {
                    self.video.set(lo, true);
                }
                if self.model.is_apple2c() {
                    return 0;
                }
                let n = (lo as usize >> 1) & 3;
                if let Some(d) = self.game_io.as_mut() {
                    d.annunciator(n, lo & 1 != 0, &self.annunciators); // before the update (Memory.cpp:884)
                }
                self.annunciators[n] = lo & 1 != 0;
                if self.model == Model::Apple2JPlus && lo >> 1 == 0x2E {
                    self.video.jplus_ascii = lo & 1 != 0; // AN2 picks the Katakana/ASCII ROM half
                }
                self.memory.set_paging(lo, 0); // Base64A ALTROM0/1
            }
        }
        self.floating_bus()
    }
}
