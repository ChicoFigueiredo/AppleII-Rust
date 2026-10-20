//! AppleWin `TestCPU6502.cpp::SyncEvents_test` (the delta-list bookkeeping of
//! `SynchronousEventManager`) and the CPU.cpp `IRQ()` timing rules seen through [`Machine`]:
//! 7-cycle entry, the "IRQ on the opcode's last cycle is deferred by one opcode" rule, masking.
use apple2_core::machine::EventAction;
use apple2_core::{EventId, IrqSource, Machine, MachineConfig, Model, Scheduler, SyncEvent};

const SRC: IrqSource = IrqSource::Mouse;

fn deltas(s: &Scheduler) -> Vec<(u32, i32)> {
    s.events().iter().map(|e| (e.id.0, e.cycles_remaining)).collect()
}

#[test]
fn sync_events_test_ascending_insert_then_removals() {
    let mut s = Scheduler::new();
    for (id, cyc) in [(0, 0x10), (1, 0x20), (2, 0x30), (3, 0x40)] {
        s.insert(SyncEvent::new(EventId(id), cyc));
    }
    // id0 -> id1 -> id2 -> id3, every delta 0x10
    assert_eq!(deltas(&s), [(0, 0x10), (1, 0x10), (2, 0x10), (3, 0x10)]);

    s.remove(EventId(1));
    s.remove(EventId(3));
    s.remove(EventId(0));
    assert_eq!(deltas(&s), [(2, 0x30)]);
    s.remove(EventId(2));
    assert!(s.is_empty());
}

#[test]
fn sync_events_test_descending_insert_then_removals() {
    let mut s = Scheduler::new();
    for (id, cyc) in [(0, 0x40), (1, 0x30), (2, 0x20), (3, 0x10)] {
        s.insert(SyncEvent::new(EventId(id), cyc));
    }
    // id3 -> id2 -> id1 -> id0
    assert_eq!(deltas(&s), [(3, 0x10), (2, 0x10), (1, 0x10), (0, 0x10)]);

    s.remove(EventId(3));
    s.remove(EventId(0));
    s.remove(EventId(1));
    assert_eq!(deltas(&s), [(2, 0x20)]);
    s.remove(EventId(2));
    assert!(s.is_empty());
}

/// Machine with a 12K custom ROM: RESET=$0300, IRQ=$0400, NMI=$0500; $0300 = CLI then NOPs;
/// the IRQ handler is `INC $10 : JMP $0402`.
fn machine(model: Model) -> Machine {
    let mut rom = vec![0xEAu8; 0x3000];
    for (vec, target) in [(0xFFFAu16, 0x0500u16), (0xFFFC, 0x0300), (0xFFFE, 0x0400)] {
        let o = vec as usize - 0xD000;
        rom[o] = target as u8;
        rom[o + 1] = (target >> 8) as u8;
    }
    let mut cfg = MachineConfig::for_model(model);
    cfg.custom_rom = Some(rom);
    let mut m = Machine::new(cfg);
    m.poke(0x300, 0x58);
    for a in 0x301..0x320 {
        m.poke(a, 0xEA);
    }
    for (i, b) in [0xE6, 0x10, 0x4C, 0x02, 0x04].into_iter().enumerate() {
        m.poke(0x400 + i as u16, b);
    }
    m
}

#[test]
fn irq_entry_takes_seven_cycles_and_pushes_status_with_b_clear() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model);
        m.step(); // CLI
        m.board_mut().interrupts.set_irq(SRC, true);
        let sp = m.cpu().sp;
        // the line was already asserted when the NOP at $301 started: not on a last cycle, taken now
        assert_eq!(m.step(), 7, "{model:?}");
        assert_eq!(m.cpu().pc, 0x400);
        assert_eq!(m.cpu().sp, sp.wrapping_sub(3));
        let stack = |off: u8| m.peek(0x100 + sp.wrapping_sub(off) as u16);
        assert_eq!((stack(0), stack(1)), (0x03, 0x01), "return address $0301");
        assert_eq!(stack(2) & 0x30, 0x20, "B clear, bit 5 set");
        assert!(m.cpu().p & 0x04 != 0, "I set");
    }
}

#[test]
fn irq_that_appears_on_the_last_cycle_of_an_opcode_is_deferred_one_opcode_only() {
    let mut m = machine(Model::Apple2Plus);
    m.schedule(EventId(1), 4, EventAction::RaiseIrq(SRC));
    m.step(); // CLI: 2 cycles
    m.step(); // NOP: the event expires exactly on its last cycle
    assert_eq!(m.cpu().pc, 0x302);
    m.step(); // deferred: one more NOP runs although the line is asserted and I is clear
    assert_eq!(m.cpu().pc, 0x303);
    assert_eq!(m.step(), 7, "taken after exactly one opcode");
    assert_eq!(m.cpu().pc, 0x400);
}

#[test]
fn irq_expiring_mid_opcode_is_taken_before_the_next_opcode() {
    let mut m = machine(Model::Apple2Plus);
    m.schedule(EventId(1), 3, EventAction::RaiseIrq(SRC));
    m.step(); // CLI
    m.step(); // NOP: expires on its first cycle
    assert_eq!(m.cpu().pc, 0x302);
    assert_eq!(m.step(), 7);
    assert_eq!(m.cpu().pc, 0x400);
}

#[test]
fn irq_stays_pending_while_i_is_set_and_fires_after_cli() {
    let mut m = machine(Model::Apple2Plus);
    m.board_mut().interrupts.set_irq(SRC, true);
    m.poke(0x300, 0xEA); // NOP
    m.poke(0x301, 0x58); // CLI
    assert_eq!(m.step(), 2);
    assert_eq!(m.step(), 2); // CLI
    assert_eq!(m.cpu().pc, 0x302);
    assert_eq!(m.step(), 7);
    assert_eq!(m.cpu().pc, 0x400);
}
