//! Peripheral cards (AppleWin Card.h:36-70, CardManager).
//!
//! A [`Card`] sits in one of the slots 1-7 (slot 0 for ][-family cards) and may expose
//! (a) 16 I/O bytes at `$C080+slot*16`, (b) a 256-byte slot ROM at `$Cs00` and (c) a 2K expansion
//! ROM at `$C800-$CFFF`. AppleWin's `InitializeIO`/`RegisterIoHandler` becomes plain trait
//! methods, so dispatch is a vtable call without allocation. `Destroy` has no counterpart (Rust
//! `Drop`).
//!
//! Cards never see the [`Board`](crate::machine::Board): every callback receives a [`CardCtx`]
//! with what a card legitimately needs (cycle count, floating bus, IRQ/NMI lines, scheduling).
//!
//! # State
//! AppleWin's `Save/LoadSnapshot` is [`Card::save`]/[`Card::load`]: each card encodes its own
//! state as opaque bytes (no extra dependency; the bytes are `serde`-friendly as a `Vec<u8>`).
//! The Language Card / Saturn are not boxed cards: [`LanguageCards`](crate::language_card)
//! manages them jointly (shared region/WRITERAM arbitration) and derives `serde` itself.

use crate::irq::{Interrupts, IrqSource};
use crate::scheduler::EventId;

/// Bytes of a slot ROM (`$Cs00-$CsFF`).
pub const SLOT_ROM_SIZE: usize = 0x100;
/// Bytes of an expansion ROM (`$C800-$CFFF`).
pub const EXPANSION_ROM_SIZE: usize = 0x800;

/// Scheduler id of the event `token` of the card in `slot`. Ids with the top bit set belong to
/// cards, so they never collide with the small ids used by `Machine::schedule` callers.
pub fn card_event_id(slot: u8, token: u16) -> EventId {
    EventId(0x8000_0000 | (slot as u32) << 16 | token as u32)
}

/// Scheduler request queued by a card (applied by the board right after the callback, since the
/// scheduler may be mid-update).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardOp {
    Schedule { slot: u8, token: u16, cycles: i32, can_assert_irq: bool },
    SetCanAssertIrq { slot: u8, token: u16, can_assert_irq: bool },
    Cancel { slot: u8, token: u16 },
    SetCpuOwner { slot: u8, active: bool },
}

/// The instruction in execution when a card is called: lets a card model the 6502 false read of
/// `STA abs,X/Y` and `STA (zp),Y` (Mockingboard.cpp:750-778 peeks `PC-3..PC-1` for this).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OpcodeInfo {
    /// Address of the opcode byte.
    pub pc: u16,
    pub opcode: u8,
    /// The two bytes after the opcode, little endian (side-effect-free peek; 0 for events).
    pub operand: u16,
    /// For `STA abs,X` ($9D), `STA abs,Y` ($99) and `STA (zp),Y` ($91): the address before
    /// indexing (`operand`, or the pointer read from zero page); 0 for other opcodes.
    pub indexed_base: u16,
    /// The CPU is a 65C02.
    pub cmos: bool,
}

/// What a card can do to the machine during a callback.
pub struct CardCtx<'a> {
    /// Slot the card sits in.
    pub slot: u8,
    /// Absolute cycle counter at the access.
    pub cycles: u64,
    /// Value of the floating bus at the access (what an unclaimed read returns).
    pub floating_bus: u8,
    /// Cycle of the video frame at the callback (for events: when the event expired), and the
    /// frame length; enough to derive `NTSC_GetCyclesUntilVBlank` ([`CardCtx::cycles_until_vblank`]).
    pub frame_cycle: u32,
    pub cycles_per_frame: u32,
    /// The instruction that caused the access ([`OpcodeInfo`]).
    pub opcode: OpcodeInfo,
    pub(crate) interrupts: &'a mut Interrupts,
    pub(crate) ops: &'a mut Vec<CardOp>,
}

impl CardCtx<'_> {
    /// Asserts or releases the IRQ line for `src`.
    pub fn set_irq(&mut self, src: IrqSource, asserted: bool) {
        self.interrupts.set_irq(src, asserted);
    }

    pub fn set_nmi(&mut self, src: IrqSource, asserted: bool) {
        self.interrupts.set_nmi(src, asserted);
    }

    /// Schedules [`Card::on_event`]`(token)` `cycles` from now, replacing a pending event with
    /// the same token.
    pub fn schedule(&mut self, token: u16, cycles: i32) {
        self.schedule_gated(token, cycles, true);
    }

    /// [`CardCtx::schedule`] with `SyncEvent::m_canAssertIRQ`: when false, expiring exactly on
    /// the last cycle of an opcode does not defer the IRQ by one opcode (6522, IER bit clear).
    pub fn schedule_gated(&mut self, token: u16, cycles: i32, can_assert_irq: bool) {
        self.ops.push(CardOp::Schedule { slot: self.slot, token, cycles, can_assert_irq });
    }

    /// Changes `can_assert_irq` of the pending event `token`, if any (6522.cpp:217-220).
    pub fn set_can_assert_irq(&mut self, token: u16, can_assert_irq: bool) {
        self.ops.push(CardOp::SetCanAssertIrq { slot: self.slot, token, can_assert_irq });
    }

    pub fn cancel(&mut self, token: u16) {
        self.ops.push(CardOp::Cancel { slot: self.slot, token });
    }

    /// Hands the CPU to this card's [`AltCpu`] (`true`, e.g. Z80 SoftCard, AppleWin
    /// `SetActiveCpu(CPU_Z80)`) or back to the 6502 (`false`).
    pub fn set_cpu_owner(&mut self, active: bool) {
        self.ops.push(CardOp::SetCpuOwner { slot: self.slot, active });
    }

    /// `NTSC_GetCyclesUntilVBlank(0)` (NTSC.cpp:2761): cycles from now to line 192.
    pub fn cycles_until_vblank(&self) -> u32 {
        let vbl = 192 * 65;
        let pos = self.frame_cycle % self.cycles_per_frame;
        if pos < vbl { vbl - pos } else { self.cycles_per_frame - pos + vbl }
    }
}

/// A processor that replaces the 6502 while a card owns the CPU (see [`CardCtx::set_cpu_owner`]).
/// The machine ticks the bus for the returned cycles after each call.
pub trait AltCpu {
    /// Executes one instruction against `bus` (without ticking it); returns 6502 cycles used.
    fn step(&mut self, bus: &mut dyn apple2_cpu::Bus) -> u32;
    /// For getting the concrete CPU back in [`Card::restore_alt_cpu`].
    fn into_any(self: Box<Self>) -> Box<dyn std::any::Any>;
}

pub trait Card {
    /// For downcasting from [`Machine::with_card`](crate::Machine::with_card): `Some(self)`.
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }

    /// The card wants [`Card::io_snoop`] for every `$C000-$C0FF` access outside its own slot
    /// (checked once, when inserted).
    fn wants_io_snoop(&self) -> bool {
        false
    }

    /// Bus snoop of `$C000-$C07F` accesses (VidHD mirrors the //e switches, Memory.cpp:625-650).
    fn io_snoop(&mut self, _addr: u16, _write: bool, _value: u8, _ctx: &mut CardCtx) {}

    /// Lends the card's CPU for one machine step while it owns the CPU; the machine hands it
    /// back with [`Card::restore_alt_cpu`] (the card stays in its slot to serve I/O meanwhile).
    fn take_alt_cpu(&mut self) -> Option<Box<dyn AltCpu>> {
        None
    }

    fn restore_alt_cpu(&mut self, _cpu: Box<dyn AltCpu>) {}

    /// Card name (AppleWin `Card::GetCardName`).
    fn name(&self) -> &'static str;

    /// Read of `$C080+slot*16+lo` (`lo` 0-15); unclaimed reads return the floating bus.
    fn io_read(&mut self, _lo: u8, ctx: &mut CardCtx) -> u8 {
        ctx.floating_bus
    }

    fn io_write(&mut self, _lo: u8, _value: u8, _ctx: &mut CardCtx) {}

    /// Read of `$Cs00+offset`. A slot without ROM reads 0, like AppleWin's zeroed
    /// `pCxRomPeripheral`.
    fn slot_rom_read(&mut self, _offset: u8, _ctx: &mut CardCtx) -> u8 {
        0
    }

    fn slot_rom_write(&mut self, _offset: u8, _value: u8, _ctx: &mut CardCtx) {}

    /// The 2K expansion ROM mapped at `$C800-$CFFF` while the slot is selected (the
    /// `pExpansionRom` of `RegisterIoHandler`, Memory.cpp:1151); `None` if the card has none.
    fn expansion_rom(&self) -> Option<&[u8; EXPANSION_ROM_SIZE]> {
        None
    }

    /// Write to `$C800+offset` while this card's expansion ROM is selected.
    fn expansion_rom_write(&mut self, _offset: u16, _value: u8, _ctx: &mut CardCtx) {}

    /// Ctrl-Reset (`power_cycle` false) or power-on (true).
    fn reset(&mut self, _power_cycle: bool, _ctx: &mut CardCtx) {}

    /// Called after each `Machine::run_cycles`/`run_frame` with the cycles just executed
    /// (AppleWin `Card::Update`, called once per emulated slice, not per opcode).
    fn update(&mut self, _cycles: u64, _ctx: &mut CardCtx) {}

    /// A synchronous event scheduled with [`CardCtx::schedule`] expired. Returns the cycles until
    /// it re-arms (0 = not again), as `SynchronousEventManager` handlers do.
    fn on_event(&mut self, _token: u16, _ctx: &mut CardCtx) -> i32 {
        0
    }

    /// Snapshot of the card's state (see the module docs).
    fn save(&self) -> Vec<u8> {
        Vec::new()
    }

    /// Restores [`Card::save`] output; false if the bytes are not this card's.
    fn load(&mut self, _state: &[u8]) -> bool {
        true
    }
}
