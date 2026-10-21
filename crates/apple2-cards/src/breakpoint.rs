//! Breakpoint card: debugger regression-test helper (AppleWin Debugger/BreakpointCard.cpp).
//!
//! Test code writes the expected breakpoint hits into a 32-entry FIFO (6 bytes each: type, start
//! lo/hi, end lo/hi, access) then runs; each time the debugger reports a hit with
//! [`BreakpointCard::report`] (only while the card has taken over interception), one entry is
//! popped one cycle later, compared and an IRQ raised. Reading `$C0n0` returns the status and
//! clears the IRQ.
//!
//! ```text
//! C0n0 R status: b7 match, b6 mismatch, b1 FIFO full, b0 FIFO empty
//! C0n1 R reset (flush FIFO)   C0n2 R intercept by card   C0n3 R intercept by debugger
//! C0nX R ID byte X   C0nX W FIFO byte   (any access clears the IRQ)
//! ```

use crate::state::{Dec, Enc};
use apple2_core::{Card, CardCtx, IrqSource};
use std::any::Any;
use std::collections::VecDeque;

const MATCH: u8 = 1 << 7;
const MISMATCH: u8 = 1 << 6;
const FULL: u8 = 1 << 1;
const EMPTY: u8 = 1 << 0;
const PARAMS: usize = 6;
const FIFO_SIZE: usize = 32;
const DEFER_TOKEN: u16 = 0;

pub const BPTYPE_UNKNOWN: u8 = 0;
pub const BPTYPE_PC: u8 = 1;
pub const BPTYPE_MEM: u8 = 2;
pub const BPTYPE_DMA: u8 = 3;

/// A breakpoint hit as the debugger reports it (`INTERCEPTBREAKPOINT`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Breakpoint {
    pub kind: u8,
    pub addr_start: u16,
    pub addr_end: u16,
    /// 0 read, 1 write, 2 read/write.
    pub access: u8,
}

#[derive(Default)]
pub struct BreakpointCard {
    status: u8,
    intercept_by_card: bool,
    set: [u8; PARAMS],
    set_idx: usize,
    fifo: VecDeque<Breakpoint>,
    deferred: Breakpoint,
}

impl BreakpointCard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the debugger must hand breakpoints to [`BreakpointCard::report`] instead of
    /// stopping (`InterceptBreakpoints(slot, cb)`).
    pub fn intercepting(&self) -> bool {
        self.intercept_by_card
    }

    /// `CbFunction` (:177): the debugger reports a hit. Processing is deferred by one cycle so the
    /// IRQ is not taken before the opcode at the breakpoint executes.
    pub fn report(&mut self, bp: Breakpoint, ctx: &mut CardCtx) {
        self.deferred = bp;
        ctx.schedule(DEFER_TOKEN, 1);
    }

    pub fn status(&self) -> u8 {
        self.status | if self.fifo.is_empty() { EMPTY } else { 0 }
    }

    fn reset_state(&mut self) {
        self.set_idx = 0;
        self.status = 0;
        self.fifo.clear();
    }

    /// `Deferred` (:195).
    fn process(&mut self, ctx: &mut CardCtx) {
        self.status &= !(MATCH | MISMATCH);
        ctx.set_irq(IrqSource::BreakpointCard, true);
        let Some(exp) = self.fifo.pop_front() else {
            self.status |= MISMATCH;
            return;
        };
        self.status &= !FULL;
        let got = self.deferred;
        let ok = exp.kind == got.kind
            && match got.kind {
                BPTYPE_PC | BPTYPE_MEM => {
                    exp.addr_start == got.addr_start && exp.access == got.access
                }
                BPTYPE_DMA => {
                    let overlap = (got.addr_start >= exp.addr_start
                        && got.addr_start < exp.addr_end)
                        || (exp.addr_start >= got.addr_start && exp.addr_start < got.addr_end);
                    overlap && exp.access == got.access
                }
                _ => false,
            };
        self.status |= if ok { MATCH } else { MISMATCH };
    }
}

impl Card for BreakpointCard {
    fn name(&self) -> &'static str {
        "Breakpoint"
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }

    fn as_any(&self) -> Option<&dyn Any> {
        Some(self)
    }

    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        ctx.set_irq(IrqSource::BreakpointCard, false);
        match lo {
            0 => return self.status(),
            1 => self.reset_state(),
            2 => self.intercept_by_card = true,
            3 => self.intercept_by_card = false,
            _ => {}
        }
        lo
    }

    fn io_write(&mut self, _lo: u8, value: u8, ctx: &mut CardCtx) {
        ctx.set_irq(IrqSource::BreakpointCard, false);
        self.set[self.set_idx] = value;
        self.set_idx += 1;
        if self.set_idx == PARAMS {
            self.set_idx = 0;
            if self.status & FULL == 0 {
                let s = &self.set;
                self.fifo.push_back(Breakpoint {
                    kind: s[0],
                    addr_start: u16::from(s[2]) << 8 | u16::from(s[1]),
                    addr_end: u16::from(s[4]) << 8 | u16::from(s[3]),
                    access: s[5],
                });
            }
            if self.fifo.len() == FIFO_SIZE {
                self.status |= FULL;
            }
        }
    }

    /// RESET clears the IRQ, flushes the FIFO and returns interception to the card's default.
    fn reset(&mut self, power_cycle: bool, ctx: &mut CardCtx) {
        self.intercept_by_card = false;
        self.reset_state();
        if !power_cycle {
            ctx.set_irq(IrqSource::BreakpointCard, false);
        }
    }

    fn on_event(&mut self, _token: u16, ctx: &mut CardCtx) -> i32 {
        self.process(ctx);
        0
    }

    fn save(&self) -> Vec<u8> {
        // AppleWin saves no state; the FIFO and flags are kept so a save/load round-trips.
        let mut e = Enc::default();
        e.u8(self.status)
            .bool(self.intercept_by_card)
            .bytes(&self.set)
            .u8(self.set_idx as u8)
            .u8(self.fifo.len() as u8);
        for b in self.fifo.iter().chain([&self.deferred]) {
            e.u8(b.kind).u16(b.addr_start).u16(b.addr_end).u8(b.access);
        }
        e.0
    }

    fn load(&mut self, state: &[u8]) -> bool {
        let mut d = Dec::new(state);
        let r = (|| {
            let (status, icpt) = (d.u8()?, d.bool()?);
            let mut set = [0u8; PARAMS];
            d.bytes(&mut set)?;
            let (idx, n) = (usize::from(d.u8()?), usize::from(d.u8()?));
            if idx >= PARAMS || n > FIFO_SIZE {
                return None;
            }
            let mut bp = || {
                Some(Breakpoint {
                    kind: d.u8()?,
                    addr_start: d.u16()?,
                    addr_end: d.u16()?,
                    access: d.u8()?,
                })
            };
            let fifo: Option<VecDeque<_>> = (0..n).map(|_| bp()).collect();
            let deferred = bp()?;
            Some((status, icpt, set, idx, fifo?, deferred))
        })();
        match r {
            Some((status, icpt, set, idx, fifo, deferred)) if d.done() => {
                (
                    self.status,
                    self.intercept_by_card,
                    self.set,
                    self.set_idx,
                    self.fifo,
                    self.deferred,
                ) = (status, icpt, set, idx, fifo, deferred);
                true
            }
            _ => false,
        }
    }
}
