//! No-Slot Clock / Dallas SmartWatch DS1215 under the system ROM (AppleWin NoSlotClock.cpp).
//!
//! A `DS1215` sits between the CPU and a ROM socket. Reads with A2 low are "write cycles" whose
//! A0 carries one bit of the 64-bit unlock pattern `0x5CA33AC55CA33AC5` (LSB first); after a
//! full match, 64 reads with A2 high return the BCD clock register one bit per read in D0.
//! On the //e it watches the internal `$C300-$C3FF`/`$C800-$CFFF` ROM, on the ][/][+ the
//! `$F800-$FFFF` ROM (see [`apple2_core::hooks::RomTap`]).

use crate::state::{Dec, Enc};
use crate::time::HostClock;
use apple2_core::hooks::RomTap;
use std::any::Any;

const INIT_SEQUENCE: u64 = 0x5CA3_3AC5_5CA3_3AC5;

/// `RingRegister64` (NoSlotClock.cpp:222): a 64-bit register read or written one bit at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ring {
    mask: u64,
    reg: u64,
}

impl Ring {
    fn new(reg: u64) -> Self {
        Ring { mask: 1, reg }
    }

    fn write_bits(&mut self, mut data: u8, count: u32) {
        for _ in 0..count {
            self.reg = if data & 1 != 0 { self.reg | self.mask } else { self.reg & !self.mask };
            self.next_bit();
            data >>= 1;
        }
    }

    fn compare(&self, bit: u8) -> bool {
        (self.reg & self.mask != 0) == (bit & 1 != 0)
    }

    /// Advances; true when the ring wrapped.
    fn next_bit(&mut self) -> bool {
        self.mask <<= 1;
        if self.mask == 0 {
            self.mask = 1;
            true
        } else {
            false
        }
    }
}

/// The six values `CNoSlotClock::SaveSnapshot` writes (NoSlotClock.cpp:196).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NscSnapshot {
    pub clock_register_enabled: bool,
    pub write_enabled: bool,
    pub clock_register_mask: u64,
    pub clock_register: u64,
    pub comparison_register_mask: u64,
    pub comparison_register: u64,
}

pub struct NoSlotClock {
    clock: Box<dyn HostClock>,
    clock_reg: Ring,
    cmp: Ring,
    reg_enabled: bool,
    write_enabled: bool,
}

impl NoSlotClock {
    pub fn new(clock: Box<dyn HostClock>) -> Self {
        let mut c = NoSlotClock { clock, clock_reg: Ring::new(0), cmp: Ring::new(INIT_SEQUENCE), reg_enabled: false, write_enabled: true };
        c.reset();
        c
    }

    pub fn snapshot(&self) -> NscSnapshot {
        NscSnapshot {
            clock_register_enabled: self.reg_enabled,
            write_enabled: self.write_enabled,
            clock_register_mask: self.clock_reg.mask,
            clock_register: self.clock_reg.reg,
            comparison_register_mask: self.cmp.mask,
            comparison_register: self.cmp.reg,
        }
    }

    /// `CNoSlotClock::LoadSnapshot` (:208). False (nothing changed) if a mask is not a single
    /// bit, which no run of the clock can produce.
    pub fn apply_snapshot(&mut self, s: &NscSnapshot) -> bool {
        if !s.clock_register_mask.is_power_of_two() || !s.comparison_register_mask.is_power_of_two() {
            return false;
        }
        (self.reg_enabled, self.write_enabled) = (s.clock_register_enabled, s.write_enabled);
        self.clock_reg = Ring { mask: s.clock_register_mask, reg: s.clock_register };
        self.cmp = Ring { mask: s.comparison_register_mask, reg: s.comparison_register };
        true
    }

    /// Decodes the bytes of [`RomTap::save`] into a snapshot; `None` if they are not a No-Slot
    /// Clock's.
    pub fn snapshot_of_saved(state: &[u8]) -> Option<NscSnapshot> {
        let mut c = NoSlotClock::new(Box::new(crate::time::FixedClock(crate::time::WallTime::SAMPLE)));
        c.load(state).then(|| c.snapshot())
    }

    /// `CNoSlotClock::Reset` (:54): a power-cycle, not the RESET signal.
    pub fn reset(&mut self) {
        self.cmp.mask = 1;
        self.reg_enabled = false;
        self.write_enabled = true;
    }

    /// `ClockRead` (:97): A2 high.
    fn clock_read(&mut self, data: &mut u8) -> bool {
        if !self.reg_enabled {
            self.cmp.mask = 1;
            self.write_enabled = true;
            return false;
        }
        *data = if self.clock_reg.reg & self.clock_reg.mask != 0 { *data | 1 } else { *data & !1 };
        if self.clock_reg.next_bit() {
            self.reg_enabled = false;
        }
        true
    }

    /// `ClockWrite` (:115): A2 low, data in on A0.
    fn clock_write(&mut self, addr: u16) {
        if !self.write_enabled {
            return;
        }
        if !self.reg_enabled {
            if self.cmp.compare(addr as u8) {
                if self.cmp.next_bit() {
                    self.reg_enabled = true;
                    self.populate();
                }
            } else {
                self.write_enabled = false; // a mismatch ignores further writes
            }
        } else if self.clock_reg.next_bit() {
            self.reg_enabled = false; // the register is read-only here
        }
    }

    /// `PopulateClockRegister` (:144): packed BCD, least significant digit first.
    fn populate(&mut self) {
        let t = self.clock.now();
        self.clock_reg = Ring::new(0);
        let r = &mut self.clock_reg;
        for v in [t.centisecond, t.second, t.minute, t.hour, t.weekday + 1, t.day, t.month, (t.year % 100) as u8] {
            r.write_bits(v % 10, 4);
            r.write_bits(v / 10, 4);
        }
    }
}

impl RomTap for NoSlotClock {
    fn access(&mut self, addr: u16, write: bool, data: &mut u8) -> bool {
        if addr & 4 != 0 {
            let mut d = *data;
            let r = self.clock_read(&mut d);
            if !write {
                *data = d;
            }
            r && !write
        } else {
            self.clock_write(addr);
            false
        }
    }

    fn reset(&mut self) {
        NoSlotClock::reset(self);
    }

    fn save(&self) -> Vec<u8> {
        let mut e = Enc::default();
        e.bool(self.reg_enabled).bool(self.write_enabled);
        e.u64(self.clock_reg.mask).u64(self.clock_reg.reg).u64(self.cmp.mask).u64(self.cmp.reg);
        e.0
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let mut d = Dec::new(state);
        let r = (|| Some((d.bool()?, d.bool()?, d.u64()?, d.u64()?, d.u64()?, d.u64()?)))();
        match r {
            Some((re, we, cm, cr, pm, pr)) if d.done() && cm.is_power_of_two() && pm.is_power_of_two() => {
                (self.reg_enabled, self.write_enabled) = (re, we);
                self.clock_reg = Ring { mask: cm, reg: cr };
                self.cmp = Ring { mask: pm, reg: pr };
                true
            }
            _ => false,
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::{FixedClock, WallTime};

    fn nsc() -> NoSlotClock {
        NoSlotClock::new(Box::new(FixedClock(WallTime::SAMPLE)))
    }

    fn unlock(c: &mut NoSlotClock, base: u16) {
        let mut d = 0;
        c.access(base | 4, false, &mut d); // A2 high resets the comparison register
        for i in 0..64 {
            let mut d = 0;
            c.access(base | ((INIT_SEQUENCE >> i) & 1) as u16, false, &mut d);
        }
    }

    fn read_register(c: &mut NoSlotClock, base: u16) -> [u8; 8] {
        let mut out = [0u8; 8];
        for bit in 0..64 {
            let mut d = 0;
            assert!(c.access(base | 4, false, &mut d));
            out[bit / 8] |= (d & 1) << (bit % 8);
        }
        out
    }

    #[test]
    fn unlock_pattern_then_64_reads_give_bcd_time() {
        let mut c = nsc();
        unlock(&mut c, 0xC300);
        assert_eq!(read_register(&mut c, 0xC300), [0x12, 0x27, 0x35, 0x14, 0x02, 0x19, 0x10, 0x26]);
    }

    #[test]
    fn a_wrong_bit_locks_it_out_until_a_read_resets() {
        let mut c = nsc();
        let mut d = 0;
        c.access(0xC304, false, &mut d);
        c.access(0xC301, false, &mut d); // first pattern bit is 1 (0xC5 = ...0101): fine
        c.access(0xC301, false, &mut d); // second is 0: mismatch
        assert!(!c.write_enabled);
        unlock(&mut c, 0xC300); // the A2-high read re-arms everything
        assert!(c.reg_enabled);
    }

    #[test]
    fn locked_clock_does_not_answer_reads() {
        let mut c = nsc();
        let mut d = 0x80;
        assert!(!c.access(0xC304, false, &mut d));
        assert_eq!(d, 0x80);
    }

    #[test]
    fn save_load_midway_through_a_read() {
        let mut c = nsc();
        unlock(&mut c, 0xC300);
        for _ in 0..20 {
            let mut d = 0;
            c.access(0xC304, false, &mut d);
        }
        let st = c.save();
        let mut c2 = nsc();
        assert!(c2.load(&st));
        assert_eq!(c.clock_reg, c2.clock_reg);
        assert!(!c2.load(&st[..5]));
    }
}
