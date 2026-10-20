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
    Schedule { slot: u8, token: u16, cycles: i32 },
    Cancel { slot: u8, token: u16 },
}

/// What a card can do to the machine during a callback.
pub struct CardCtx<'a> {
    /// Slot the card sits in.
    pub slot: u8,
    /// Absolute cycle counter at the access.
    pub cycles: u64,
    /// Value of the floating bus at the access (what an unclaimed read returns).
    pub floating_bus: u8,
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
        self.ops.push(CardOp::Schedule { slot: self.slot, token, cycles });
    }

    pub fn cancel(&mut self, token: u16) {
        self.ops.push(CardOp::Cancel { slot: self.slot, token });
    }
}

pub trait Card {
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
