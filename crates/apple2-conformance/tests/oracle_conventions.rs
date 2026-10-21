//! Trace v1 conventions that make the Rust trace comparable with the AppleWin oracle (HU-0101,
//! HU-0103). Each test is the regression test of a divergence recorded in
//! `docs/conformance/audit/cpu-trace-differential.md`.

use apple2_conformance::runner::{build_machine, Disk2Setup};
use apple2_conformance::trace::{TraceReader, TraceRow, TraceWriter, BATCH_CYCLES};
use apple2_core::{Card, CardCtx, IrqSource, Machine, MachineConfig, Model};
use std::cell::Cell;
use std::rc::Rc;

/// Custom 12K ROM: RESET=$0300, NMI=$0500, IRQ=$0400.
fn machine() -> Machine {
    let mut rom = vec![0xEAu8; 0x3000];
    for (vec, target) in [(0xFFFAu16, 0x0500u16), (0xFFFC, 0x0300), (0xFFFE, 0x0400)] {
        let o = vec as usize - 0xD000;
        rom[o] = target as u8;
        rom[o + 1] = (target >> 8) as u8;
    }
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.custom_rom = Some(rom);
    Machine::new(cfg)
}

#[test]
fn p_is_recorded_with_bits_4_and_5_set() {
    // AppleWin keeps B and the reserved bit set in regs.ps; the Rust CPU register does not.
    let m = machine();
    assert_eq!(
        m.cpu().p & 0x30,
        0x20,
        "register unchanged: P=$24 after reset"
    );
    let row = TraceRow::from_machine(&m);
    assert_eq!(row.p, m.cpu().p | 0x30);
    assert_eq!(row.p, 0x34, "the oracle's P after the power-on reset");
    assert!(row.to_csv().ends_with(",34"));
}

#[test]
fn interrupt_entry_writes_no_row() {
    // The oracle's hook runs after the opcode fetch; an IRQ/NMI taken instead of a fetch makes
    // no row, and the handler's first instruction is the next row.
    let mut m = machine();
    m.poke(0x300, 0x58); // CLI
    m.poke(0x301, 0xEA); // NOP
    m.poke(0x400, 0xE8); // handler: INX
    let mut w = TraceWriter::new(Vec::new(), &[]).unwrap();
    w.step(&mut m).unwrap(); // CLI
    w.step(&mut m).unwrap(); // NOP
    assert_eq!(w.rows_written(), 2);
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, true);
    assert!(m.next_step_is_interrupt_entry());
    let pc_before = m.cpu().pc;
    w.step(&mut m).unwrap();
    assert_eq!(w.rows_written(), 2, "no row for the interrupt entry");
    assert_eq!(m.cpu().pc, 0x400);
    assert_ne!(pc_before, 0x400);
    assert!(!m.next_step_is_interrupt_entry());
    w.step(&mut m).unwrap(); // INX of the handler
    let t = TraceReader::read(&w.finish().unwrap()[..]).unwrap();
    assert_eq!(t.rows.len(), 3);
    assert_eq!((t.rows[2].pc, t.rows[2].op), (0x400, 0xE8));
}

#[test]
fn nmi_entry_writes_no_row_and_masked_irq_is_not_an_entry() {
    let mut m = machine();
    m.poke(0x300, 0xEA);
    m.poke(0x500, 0xE8);
    // IRQ asserted but I flag set (after reset): the next step is an ordinary instruction.
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, true);
    assert!(!m.next_step_is_interrupt_entry());
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, false);
    m.board_mut().interrupts.set_nmi(IrqSource::Mouse, true);
    assert!(m.next_step_is_interrupt_entry());
    let mut w = TraceWriter::new(Vec::new(), &[]).unwrap();
    w.step(&mut m).unwrap();
    assert_eq!((w.rows_written(), m.cpu().pc), (0, 0x500));
    // The line stays high: an edge-triggered NMI is not entered again.
    assert!(!m.next_step_is_interrupt_entry());
}

#[test]
fn opcode_in_slot_rom_comes_from_the_fetch_not_from_a_peek() {
    // `Machine::peek` cannot read card ROM without side effects; the first Disk II boot opcode
    // is LDX #$20 ($A2).
    let mut m = build_machine(
        MachineConfig::for_model(Model::Apple2Plus),
        Some(&Disk2Setup {
            slot: 6,
            image: None,
        }),
    )
    .unwrap();
    m.cpu_mut().pc = 0xC600;
    let mut w = TraceWriter::new(Vec::new(), &[]).unwrap();
    w.step(&mut m).unwrap();
    let t = TraceReader::read(&w.finish().unwrap()[..]).unwrap();
    assert_eq!((t.rows[0].pc, t.rows[0].op), (0xC600, 0xA2));
}

struct Counter(Rc<Cell<u64>>, Rc<Cell<u32>>);

impl Card for Counter {
    fn name(&self) -> &'static str {
        "Counter"
    }
    fn update(&mut self, cycles: u64, _ctx: &mut CardCtx) {
        self.0.set(self.0.get() + cycles);
        self.1.set(self.1.get() + 1);
    }
}

#[test]
fn cards_are_updated_at_the_end_of_each_1ms_batch_like_the_oracle() {
    // The Disk II counts its motor-off delay in Card::update; the oracle calls it once per
    // 1020-cycle CpuExecute batch. A tracer driving `step` must do the same.
    let (total, calls) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
    let mut m = machine();
    assert!(m.insert_card(5, Box::new(Counter(total.clone(), calls.clone()))));
    let mut w = TraceWriter::new(Vec::new(), &[]).unwrap();
    let start = m.cycles();
    while m.cycles() - start < 10 * BATCH_CYCLES {
        w.step(&mut m).unwrap();
    }
    assert_eq!(BATCH_CYCLES, 1020);
    // Every update covered at least one whole batch (an instruction may overshoot it).
    assert!(calls.get() >= 9 && calls.get() <= 10, "{}", calls.get());
    assert!(total.get() >= calls.get() as u64 * BATCH_CYCLES);
    assert!(total.get() <= m.cycles() - start);
}
