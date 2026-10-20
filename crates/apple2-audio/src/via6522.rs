//! Rockwell 6522 VIA as AppleWin models it (6522.cpp), as used by the Mockingboard family.
//!
//! Time model. AppleWin keeps the counters lazily (`UpdateCycles` before every access) and
//! corrects for the 4/5/6-cycle opcode by decoding the instruction (`GetOpcodeCyclesForRead/
//! Write`, cpp:412-608). Here every callback gets the exact Cycle count `now` = Cycles ticked
//! before the access, which is the same thing: a read sees the counter as of `now`, a write
//! takes effect at `now + 1` (the end of the write cycle, always the last cycle of the opcode).
//! Callers pass `now = CardCtx::cycles`.
//!
//! Timer IRQs are not raised here: a write to T1CH/T2CH asks the caller to schedule a
//! synchronous event (`SetTimerSyncEvent`, cpp:102) and [`Via::on_timer1_event`] /
//! [`Via::on_timer2_event`] set the IFR bit when it fires. The IFR *read* additionally reports
//! a timer that has underflowed by the end of the reading opcode (cpp:392-398).

/// IFR/IER bits (6522.h:67-71).
pub const IXR_SSI263: u8 = 1 << 1;
pub const IXR_VOTRAX: u8 = 1 << 4;
pub const IXR_TIMER2: u8 = 1 << 5;
pub const IXR_TIMER1: u8 = 1 << 6;
pub const IFR_IRQ: u8 = 1 << 7;
const ACR_FREERUN: u8 = 1 << 6;
/// Rockwell fig. 16: the period is N+2 cycles (`kExtraTimerCycles`).
pub const EXTRA_TIMER_CYCLES: i32 = 2;
const EXTRA_MEGA: i32 = EXTRA_TIMER_CYCLES + 1;

/// Register numbers.
pub mod reg {
    pub const ORB: u8 = 0;
    pub const ORA: u8 = 1;
    pub const DDRB: u8 = 2;
    pub const DDRA: u8 = 3;
    pub const T1CL: u8 = 4;
    pub const T1CH: u8 = 5;
    pub const T1LL: u8 = 6;
    pub const T1LH: u8 = 7;
    pub const T2CL: u8 = 8;
    pub const T2CH: u8 = 9;
    pub const SR: u8 = 10;
    pub const ACR: u8 = 11;
    pub const PCR: u8 = 12;
    pub const IFR: u8 = 13;
    pub const IER: u8 = 14;
    pub const ORA_NO_HS: u8 = 15;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Via {
    pub orb: u8,
    pub ora: u8,
    pub ddrb: u8,
    pub ddra: u8,
    pub t1_counter: u16,
    pub t1_latch: u16,
    pub t2_counter: u16,
    pub t2_latch: u16,
    pub sr: u8,
    pub acr: u8,
    pub pcr: u8,
    pub ifr: u8,
    pub ier: u8,
    pub t1_irq_delay: bool,
    pub t2_irq_delay: bool,
    pub t1_active: bool,
    pub t2_active: bool,
    /// `SyncEvent::m_canAssertIRQ` is cleared (inverted so the default is "can assert"): set by an
    /// IER write that leaves the timer's bit clear (cpp:217-220), kept until the next IER write.
    pub t1_gated: bool,
    pub t2_gated: bool,
    /// Cycle the counters are valid at.
    pub last: u64,
    /// MEGA Audio quirks (cpp:113-120, 264-297, 346-402).
    pub mega: bool,
    /// An AY8913 drives port A (`SetBusBeingDriven`).
    pub bus_driven: bool,
}

/// A timer-event request produced by a register write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimerSched {
    /// 0 = timer 1, 1 = timer 2.
    pub timer: u8,
    /// Cycles from `now` (the write access).
    pub cycles: i32,
    /// `m_canAssertIRQ` for the event: the timer's IER bit (cpp:217-220).
    pub can_assert_irq: bool,
}

impl Via {
    pub fn new(mega: bool) -> Self {
        let mut v = Via {
            orb: 0,
            ora: 0,
            ddrb: 0,
            ddra: 0,
            t1_counter: 0,
            t1_latch: 0xFFFF,
            t2_counter: 0,
            t2_latch: 0,
            sr: 0,
            acr: 0,
            pcr: 0,
            ifr: 0,
            ier: 0,
            t1_irq_delay: false,
            t2_irq_delay: false,
            t1_active: false,
            t2_active: false,
            t1_gated: false,
            t2_gated: false,
            last: 0,
            mega,
            bus_driven: false,
        };
        v.reset(true, 0);
        v
    }

    /// `Reset` (cpp:39). Power-on clears every register and sets T1 latch to $FFFF ("if it's too
    /// small then MB detection routines will fail!").
    pub fn reset(&mut self, power_cycle: bool, now: u64) {
        if power_cycle {
            *self = Via {
                orb: 0,
                ora: 0,
                ddrb: 0,
                ddra: 0,
                t1_counter: 0,
                t1_latch: 0xFFFF,
                t2_counter: 0,
                t2_latch: 0,
                sr: 0,
                acr: 0,
                pcr: 0,
                ifr: 0,
                ier: 0,
                t1_irq_delay: false,
                t2_irq_delay: false,
                t1_active: false,
                t2_active: false,
                t1_gated: self.t1_gated,
                t2_gated: self.t2_gated,
                last: now,
                mega: self.mega,
                bus_driven: false,
            };
        }
        self.ddrb = 0;
        self.ddra = 0;
        self.acr = 0;
        self.pcr = 0;
        self.update_ifr(0x7F, 0);
        self.ier = 0;
        self.update_ifr(0, 0);
        self.t1_active = false;
        self.t2_active = false;
        self.t1_irq_delay = false;
        self.t2_irq_delay = false;
        self.last = now;
    }

    /// `UpdateIFR` (cpp:131).
    pub fn update_ifr(&mut self, clr: u8, set: u8) {
        self.ifr &= !clr;
        self.ifr |= set;
        if self.ifr & self.ier & 0x7F != 0 {
            self.ifr |= IFR_IRQ;
        } else {
            self.ifr &= !IFR_IRQ;
        }
    }

    pub fn irq(&self) -> bool {
        self.ifr & IFR_IRQ != 0
    }

    /// `CheckTimerUnderflow` (cpp:242): true if an IRQ-worthy underflow is reached.
    fn check_underflow(&self, counter: &mut u16, delay: &mut bool, clocks: u16) -> bool {
        if clocks == 0 {
            return false;
        }
        let old = *counter as i32;
        let timer = old - clocks as i32;
        *counter = timer as u16;
        let mut irq = false;
        if *delay {
            *delay = false;
            irq = true;
        }
        if old >= 0 && timer < 0 {
            if self.mega {
                if timer <= -3 {
                    irq = true;
                } else {
                    *delay = true;
                }
            } else if timer <= -2 {
                irq = true;
            } else {
                *delay = true;
            }
        }
        irq
    }

    /// `OnTimer1Underflow` (cpp:284): folds the underflowed cycles back by the period. Returns
    /// the new `t1_irq_delay`.
    fn on_t1_underflow(&self, counter: &mut u16) -> bool {
        let mut timer = *counter as i16 as i32;
        if self.mega {
            let latch = if self.t1_latch != 0 { self.t1_latch as i32 } else { 0xFFFF };
            while timer < -2 {
                timer += latch + EXTRA_MEGA;
            }
        } else {
            while timer < -1 {
                timer += self.t1_latch as i32 + EXTRA_TIMER_CYCLES;
            }
        }
        *counter = timer as u16;
        timer < 0
    }

    /// `UpdateCycles` (Mockingboard.cpp:979) for this VIA: brings the counters to `now`.
    pub fn update_to(&mut self, now: u64) {
        let clocks = now.saturating_sub(self.last) as u16;
        self.last = self.last.max(now);
        let (mut c1, mut d1) = (self.t1_counter, self.t1_irq_delay);
        if self.check_underflow(&mut c1, &mut d1, clocks) {
            d1 = self.on_t1_underflow(&mut c1);
        }
        self.t1_counter = c1;
        self.t1_irq_delay = d1;
        let (mut c2, mut d2) = (self.t2_counter, self.t2_irq_delay);
        self.check_underflow(&mut c2, &mut d2, clocks);
        self.t2_counter = c2;
        self.t2_irq_delay = d2;
    }

    /// Would the timer have underflowed by the end of the reading opcode (`now + 1`)?
    fn underflowed_next(&self, counter: u16, delay: bool) -> bool {
        let (mut c, mut d) = (counter, delay);
        self.check_underflow(&mut c, &mut d, 1)
    }

    /// Register read at `now` (cpp:338).
    pub fn read(&mut self, r: u8, now: u64) -> u8 {
        self.update_to(now);
        match r & 0xF {
            reg::ORB => {
                if self.mega {
                    0
                } else {
                    self.orb | (self.ddrb ^ 0xFF) // inputs read back as 1 (GH#1260)
                }
            }
            reg::ORA | reg::ORA_NO_HS => {
                if self.mega {
                    0
                } else {
                    self.ora | if self.bus_driven { 0 } else { self.ddra ^ 0xFF }
                }
            }
            reg::DDRB => self.ddrb,
            reg::DDRA => self.ddra,
            reg::T1CL => {
                self.update_ifr(IXR_TIMER1, 0);
                self.t1_counter as u8
            }
            reg::T1CH => (self.t1_counter >> 8) as u8,
            reg::T1LL => self.t1_latch as u8,
            reg::T1LH => (self.t1_latch >> 8) as u8,
            reg::T2CL => {
                self.update_ifr(IXR_TIMER2, 0);
                if self.mega {
                    0xFF
                } else {
                    self.t2_counter as u8
                }
            }
            reg::T2CH => {
                if self.mega {
                    0
                } else {
                    (self.t2_counter >> 8) as u8
                }
            }
            reg::SR => 0,
            reg::ACR => self.acr,
            reg::PCR => self.pcr,
            reg::IFR => {
                let mut v = self.ifr;
                if self.t1_active && self.underflowed_next(self.t1_counter, self.t1_irq_delay) {
                    v |= IXR_TIMER1;
                }
                if self.t2_active && self.underflowed_next(self.t2_counter, self.t2_irq_delay) {
                    v |= IXR_TIMER2;
                }
                v
            }
            _ => {
                if self.mega {
                    self.ier & 0x7F
                } else {
                    0x80 | self.ier
                }
            }
        }
    }

    /// Register write at `now` (cpp:147). Returns the timer event to schedule, if any.
    pub fn write(&mut self, r: u8, v: u8, now: u64) -> Option<TimerSched> {
        self.update_to(now);
        match r & 0xF {
            reg::ORB => self.orb = v & self.ddrb,
            reg::ORA | reg::ORA_NO_HS => self.ora = v & self.ddra,
            reg::DDRB => self.ddrb = v,
            reg::DDRA => self.ddra = v,
            reg::T1CL | reg::T1LL => self.t1_latch = self.t1_latch & 0xFF00 | v as u16,
            reg::T1CH => {
                self.update_ifr(IXR_TIMER1, 0);
                self.t1_latch = self.t1_latch & 0x00FF | (v as u16) << 8;
                self.t1_counter = self.t1_latch;
                self.last = now + 1; // the write cycle itself is not counted
                self.t1_active = true;
                let latch = if self.mega && self.t1_latch == 0 { 0xFFFF } else { self.t1_latch as i32 };
                let extra = if self.mega { EXTRA_MEGA } else { EXTRA_TIMER_CYCLES };
                return Some(TimerSched { timer: 0, cycles: latch + extra + 1, can_assert_irq: !self.t1_gated });
            }
            reg::T1LH => {
                self.update_ifr(IXR_TIMER1, 0);
                self.t1_latch = self.t1_latch & 0x00FF | (v as u16) << 8;
            }
            reg::T2CL => self.t2_latch = self.t2_latch & 0xFF00 | v as u16,
            reg::T2CH => {
                self.update_ifr(IXR_TIMER2, 0);
                self.t2_latch = self.t2_latch & 0x00FF | (v as u16) << 8;
                self.t2_counter = self.t2_latch;
                self.last = now + 1;
                self.t2_active = true;
                let extra = if self.mega { EXTRA_MEGA } else { EXTRA_TIMER_CYCLES };
                return Some(TimerSched { timer: 1, cycles: self.t2_latch as i32 + extra + 1, can_assert_irq: !self.t2_gated });
            }
            reg::SR => {}
            reg::ACR => self.acr = v,
            reg::PCR => self.pcr = v,
            reg::IFR => self.update_ifr(v, 0), // bit 7 cannot be cleared directly
            _ => {
                if v & 0x80 == 0 {
                    self.ier &= !(v & 0x7F);
                } else {
                    self.ier |= v & 0x7F;
                }
                self.t1_gated = self.ier & IXR_TIMER1 == 0;
                self.t2_gated = self.ier & IXR_TIMER2 == 0;
                self.update_ifr(0, 0);
            }
        }
        None
    }

    /// Timer-1 sync event (`MB_SyncEventCallbackInternal`, Mockingboard.cpp:1016). `now` is the
    /// Cycle count at the callback. Returns the cycles until it fires again (0 = never).
    pub fn on_timer1_event(&mut self, now: u64) -> i32 {
        self.update_to(now);
        self.update_ifr(0, IXR_TIMER1);
        if self.acr & ACR_FREERUN != 0 {
            self.t1_active = true;
            if self.t1_irq_delay {
                return 1; // T1C is really -1: one cycle to underflow
            }
            return self.t1_counter as i32 + EXTRA_TIMER_CYCLES;
        }
        self.t1_active = false; // one-shot
        0
    }

    /// Timer-2 sync event: one-shot only.
    pub fn on_timer2_event(&mut self, now: u64) -> i32 {
        self.update_to(now);
        self.update_ifr(0, IXR_TIMER2);
        self.t2_active = false;
        0
    }

    /// Port-B bits the AY sees (`GetBusViewOfORB`: not OR'd with !DDRB).
    pub fn orb_bus(&self) -> u8 {
        self.orb & self.ddrb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn via_with_t1(latch: u16, at: u64) -> Via {
        let mut v = Via::new(false);
        v.write(reg::T1LL, latch as u8, at);
        v.write(reg::T1CH, (latch >> 8) as u8, at);
        v
    }

    #[test]
    fn power_on_latch_is_ffff_and_irqs_are_off() {
        let mut v = Via::new(false);
        assert_eq!(v.read(reg::T1LL, 0), 0xFF);
        assert_eq!(v.read(reg::T1LH, 0), 0xFF);
        assert_eq!(v.read(reg::IER, 0), 0x80);
        assert_eq!(v.read(reg::IFR, 0), 0);
    }

    #[test]
    fn t1_write_schedules_latch_plus_two_after_the_write_cycle() {
        let mut v = Via::new(false);
        v.write(reg::T1LL, 0x10, 100);
        let s = v.write(reg::T1CH, 0x00, 100).unwrap();
        // Write cycle ends at 101; underflow IRQ latch+2 cycles later = 119 = 100 + 19.
        assert_eq!(s, TimerSched { timer: 0, cycles: 19, can_assert_irq: true });
    }

    #[test]
    fn counter_read_matches_appleworks_opcode_adjustment() {
        // STA T1CH (access at 100, ends 101), then LDA T1CL 4 cycles later: access at 104+... the
        // read cycle is the 4th of its opcode, so the counter is latch - (R_end - W_end - 1).
        let mut v = via_with_t1(0x0100, 100);
        // LDA abs starting at 101 reads at ctx.cycles 104 (R_end 105).
        assert_eq!(v.read(reg::T1CL, 104), (0x100u16 - 3) as u8);
        assert_eq!(v.read(reg::T1CH, 104), 0x00); // 0x00FD: high byte already 0
        let mut v = via_with_t1(0x0200, 100);
        assert_eq!(v.read(reg::T1CH, 104), 0x01); // 0x01FD
    }

    #[test]
    fn ifr_read_sees_underflow_before_the_event_fires() {
        let mut v = via_with_t1(0x10, 100);
        v.write(reg::IER, 0xC0, 105);
        // IRQ is latch+2 = 18 cycles after W_end (101) = R_end 119 => ctx.cycles 118.
        assert_eq!(v.read(reg::IFR, 117) & IXR_TIMER1, 0);
        assert_eq!(v.read(reg::IFR, 118) & IXR_TIMER1, IXR_TIMER1);
        // The event then sets the flag for real and the IRQ summary bit.
        assert_eq!(v.on_timer1_event(119), 0);
        assert_eq!(v.read(reg::IFR, 130), IFR_IRQ | IXR_TIMER1);
        assert!(v.irq());
    }

    #[test]
    fn reading_t1cl_or_writing_t1ch_clears_the_flag() {
        let mut v = via_with_t1(0x10, 0);
        v.write(reg::IER, 0xC0, 1);
        v.on_timer1_event(19);
        assert!(v.irq());
        v.read(reg::T1CL, 30);
        assert!(!v.irq());
        v.on_timer1_event(40);
        assert!(v.irq());
        v.write(reg::T1CH, 0x10, 50);
        assert!(!v.irq());
    }

    #[test]
    fn one_shot_stops_and_free_run_repeats_with_period_n_plus_two() {
        let mut v = via_with_t1(0x20, 0);
        assert_eq!(v.on_timer1_event(35), 0);
        assert!(!v.t1_active);

        let mut v = Via::new(false);
        v.write(reg::ACR, 0x40, 0);
        v.write(reg::T1LL, 0x20, 0);
        let first = v.write(reg::T1CH, 0x00, 0).unwrap().cycles as u64; // 0x20 + 3
        assert_eq!(first, 35);
        // The callback runs at the end of the opcode in which the underflow falls.
        let next = v.on_timer1_event(first) as u64;
        // Rearm so that the next IRQ is exactly one period (N+2 = 34) after the previous one.
        assert_eq!(next, 34);
        let mut at = first;
        for _ in 0..5 {
            at += next;
            assert_eq!(v.on_timer1_event(at) as u64, 34);
        }
    }

    #[test]
    fn free_run_recovers_cycles_when_the_event_is_late() {
        let mut v = Via::new(false);
        v.write(reg::ACR, 0x40, 0);
        v.write(reg::T1LL, 0x20, 0);
        v.write(reg::T1CH, 0x00, 0);
        // Event due at 35 but delivered 3 cycles late (opcode boundary): next one is still due at
        // 35 + 34 = 69, i.e. 31 cycles after the late callback.
        let next = v.on_timer1_event(38);
        assert_eq!(38 + next as u64, 69);
    }

    #[test]
    fn t2_is_one_shot_and_keeps_counting_after_timeout() {
        let mut v = Via::new(false);
        v.write(reg::T2CL, 0x08, 10);
        let s = v.write(reg::T2CH, 0x00, 10).unwrap();
        assert_eq!(s, TimerSched { timer: 1, cycles: 11, can_assert_irq: true });
        assert_eq!(v.on_timer2_event(21), 0);
        assert!(!v.t2_active);
        v.update_to(41);
        assert_eq!(v.t2_counter, (8i32 - 30) as u16); // wrapped: "continues to decrement"
    }

    #[test]
    fn ier_set_clear_and_summary_bit() {
        let mut v = Via::new(false);
        v.write(reg::IER, 0xC0, 0);
        assert_eq!(v.read(reg::IER, 1), 0xC0);
        v.update_ifr(0, IXR_TIMER1);
        assert!(v.irq());
        v.write(reg::IER, 0x40, 2); // bit7=0: clear
        assert!(!v.irq());
        assert_eq!(v.read(reg::IER, 3), 0x80);
        // IFR bits are cleared by writing 1s; bit 7 can't be cleared directly.
        v.write(reg::IER, 0xC0, 4);
        v.write(reg::IFR, 0x80, 5);
        assert!(v.irq());
        v.write(reg::IFR, 0x40, 6);
        assert!(!v.irq());
    }

    #[test]
    fn port_b_inputs_read_back_high_and_outputs_come_from_orb() {
        let mut v = Via::new(false);
        v.write(reg::DDRB, 0x0F, 0);
        v.write(reg::ORB, 0xA5, 0);
        assert_eq!(v.read(reg::ORB, 1), 0xF5);
        assert_eq!(v.orb_bus(), 0x05);
    }

    #[test]
    fn mega_audio_irq_is_one_cycle_late_and_ports_read_zero() {
        let mut v = Via::new(true);
        v.write(reg::ORA, 0x55, 0);
        assert_eq!(v.read(reg::ORA, 1), 0);
        v.write(reg::T1LL, 0x00, 4);
        let s = v.write(reg::T1CH, 0x00, 5).unwrap(); // latch 0 => $FFFF
        assert_eq!(s.cycles, 0xFFFF + 3 + 1);
        v.write(reg::IER, 0xC0, 6);
        assert_eq!(v.read(reg::IER, 7), 0x40); // bit 7 reads 0
    }
}
