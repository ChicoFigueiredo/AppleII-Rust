//! Card plumbing through the public seams (AppleWin Card.h, Memory.cpp `IO_Cxxx`): a tiny test
//! card with slot ROM, expansion ROM, I/O registers and events, driven from the CPU bus and by
//! real code run through `Machine::step`.

use apple2_core::card::{Card, CardCtx, EXPANSION_ROM_SIZE, SLOT_ROM_SIZE};
use apple2_core::memory::mf;
use apple2_core::slots::ExpansionRom;
use apple2_core::{CardKind, IrqSource, Machine, MachineConfig, Model};
use apple2_cpu::Bus;

/// Registers: 0-7 plain storage, 1 also arms the IRQ event after `value` cycles, 8 (read)
/// acknowledges the IRQ, 9 = last slot-ROM write, 10 = last expansion write, 11 = reset kind,
/// 12 = periodic event counter, 14 = cycles seen by `update` (low byte).
struct TestCard {
    regs: [u8; 16],
    slot_rom: [u8; SLOT_ROM_SIZE],
    exp: Option<Box<[u8; EXPANSION_ROM_SIZE]>>,
    updated: u64,
}

impl TestCard {
    /// Slot ROM: `LDA #id : RTS`; expansion ROM filled with `0xE0 | id` (if `exp`).
    fn new(id: u8, exp: bool) -> Box<Self> {
        let mut slot_rom = [0xA5u8; SLOT_ROM_SIZE];
        slot_rom[..3].copy_from_slice(&[0xA9, id, 0x60]);
        Box::new(TestCard { regs: [0; 16], slot_rom, exp: exp.then(|| Box::new([0xE0 | id; EXPANSION_ROM_SIZE])), updated: 0 })
    }
}

impl Card for TestCard {
    fn name(&self) -> &'static str {
        "Test"
    }
    fn io_read(&mut self, lo: u8, ctx: &mut CardCtx) -> u8 {
        match lo {
            0..=7 | 9..=12 => self.regs[lo as usize],
            8 => {
                ctx.set_irq(IrqSource::Ssc, false);
                0
            }
            14 => self.updated as u8,
            _ => ctx.floating_bus,
        }
    }
    fn io_write(&mut self, lo: u8, value: u8, ctx: &mut CardCtx) {
        self.regs[lo as usize] = value;
        match lo {
            1 => ctx.schedule(7, value as i32),
            2 => ctx.schedule(8, value as i32),
            3 => ctx.cancel(8),
            _ => {}
        }
    }
    fn slot_rom_read(&mut self, offset: u8, _ctx: &mut CardCtx) -> u8 {
        self.slot_rom[offset as usize]
    }
    fn slot_rom_write(&mut self, _offset: u8, value: u8, _ctx: &mut CardCtx) {
        self.regs[9] = value;
    }
    fn expansion_rom(&self) -> Option<&[u8; EXPANSION_ROM_SIZE]> {
        self.exp.as_deref()
    }
    fn expansion_rom_write(&mut self, _offset: u16, value: u8, _ctx: &mut CardCtx) {
        self.regs[10] = value;
    }
    fn reset(&mut self, power_cycle: bool, ctx: &mut CardCtx) {
        self.regs[11] = if power_cycle { 2 } else { 1 };
        ctx.set_irq(IrqSource::Ssc, false);
    }
    fn update(&mut self, cycles: u64, _ctx: &mut CardCtx) {
        self.updated += cycles;
    }
    fn on_event(&mut self, token: u16, ctx: &mut CardCtx) -> i32 {
        match token {
            7 => {
                ctx.set_irq(IrqSource::Ssc, true);
                0
            }
            _ => {
                self.regs[12] += 1;
                100
            }
        }
    }
    fn save(&self) -> Vec<u8> {
        self.regs.to_vec()
    }
    fn load(&mut self, state: &[u8]) -> bool {
        state.len() == 16 && {
            self.regs.copy_from_slice(state);
            true
        }
    }
}

/// ROM with RESET=$0300, IRQ=$0400; the 16K //e variant has 4K of internal Cx ROM = $CC.
/// `cards`: (slot, id, has expansion ROM).
fn machine(model: Model, cards: &[(u8, u8, bool)]) -> Machine {
    let mut rom = vec![0xEAu8; 0x3000];
    rom[0x2FFC..0x2FFE].copy_from_slice(&[0x00, 0x03]);
    rom[0x2FFE..].copy_from_slice(&[0x00, 0x04]);
    if !model.is_apple2() {
        let mut e = vec![0xCCu8; 0x1000];
        e.extend(rom);
        rom = e;
    }
    let mut cfg = MachineConfig::for_model(model);
    cfg.custom_rom = Some(rom);
    let mut m = Machine::new(cfg);
    load(&mut m, 0x300, &[0x4C, 0x00, 0x03]); // JMP *
    for &(slot, id, exp) in cards {
        assert!(m.insert_card(slot, TestCard::new(id, exp)));
    }
    m
}

fn plus(cards: &[(u8, u8, bool)]) -> Machine {
    machine(Model::Apple2Plus, cards)
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

fn load(m: &mut Machine, at: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke(at + i as u16, *b);
    }
}

#[test]
fn io_registers_reach_only_the_card_in_that_slot() {
    let mut m = plus(&[(5, 5, false)]);
    wr(&mut m, 0xC0D2, 0x77); // $C080 + 5*16 + 2
    assert_eq!(rd(&mut m, 0xC0D2), 0x77);
    let fb = m.board().floating_bus();
    assert_eq!(rd(&mut m, 0xC0E2), fb, "slot 6 is empty");
    assert_eq!(rd(&mut m, 0xC0DF), fb, "unclaimed register of the card reads the floating bus");
}

#[test]
fn language_card_keeps_working_beside_other_cards() {
    let mut m = plus(&[(5, 5, false)]);
    rd(&mut m, 0xC081);
    rd(&mut m, 0xC081);
    m.poke(0xD000, 0x99);
    rd(&mut m, 0xC080);
    assert_eq!(m.peek(0xD000), 0x99);
}

#[test]
fn slot_rom_runs_from_the_cpu_and_empty_slot_is_floating() {
    let mut m = plus(&[(5, 0x42, false)]);
    load(&mut m, 0x300, &[0x20, 0x00, 0xC5, 0x85, 0x10, 0x4C, 0x05, 0x03]); // JSR $C500; STA $10; JMP *
    m.run_cycles(30);
    assert_eq!(m.peek(0x10), 0x42);
    assert_eq!(rd(&mut m, 0xC600), m.board().floating_bus());
    wr(&mut m, 0xC510, 0x5A); // writes reach the card's slot ROM handler
    assert_eq!(rd(&mut m, 0xC0D9), 0x5A);
}

#[test]
fn expansion_rom_needs_a_slot_select_and_is_released_by_cfff() {
    let mut m = plus(&[(5, 5, true), (6, 6, true), (7, 7, false)]);
    let fb = m.board().floating_bus();
    assert_eq!(rd(&mut m, 0xC800), fb, "nothing selected");
    rd(&mut m, 0xC500);
    assert_eq!(rd(&mut m, 0xC800), 0xE5);
    assert_eq!(rd(&mut m, 0xCFFE), 0xE5);
    rd(&mut m, 0xC700); // slot 7 has no expansion ROM: selection unchanged
    assert_eq!(rd(&mut m, 0xC900), 0xE5);
    wr(&mut m, 0xC900, 0x3C); // expansion writes reach the selected card
    assert_eq!(rd(&mut m, 0xC0DA), 0x3C);
    assert_eq!(rd(&mut m, 0xCFFF), fb, "$CFFF releases the space, even for its own read");
    assert_eq!(rd(&mut m, 0xC800), fb);
    rd(&mut m, 0xC600); // a new select brings in slot 6
    assert_eq!(rd(&mut m, 0xC800), 0xE6);
}

#[test]
fn code_in_slot_rom_can_read_its_expansion_rom() {
    // JSR $C600; LDA $C800; STA $10
    let mut m = plus(&[(6, 6, true)]);
    load(&mut m, 0x300, &[0x20, 0x00, 0xC6, 0xAD, 0x00, 0xC8, 0x85, 0x10]);
    m.run_cycles(30);
    assert_eq!(m.peek(0x10), 0xE6);
}

#[test]
fn ii_family_has_no_internal_rom_switches() {
    let mut m = plus(&[(5, 5, false)]);
    wr(&mut m, 0xC007, 0); // INTCXROM on: ignored on the ][
    assert_eq!(rd(&mut m, 0xC500), 0xA9);
    assert_eq!(rd(&mut m, 0xC300), m.board().floating_bus(), "slot 3 empty, no internal ROM");
}

#[test]
fn iie_internal_rom_and_peripheral_rom_follow_intcxrom() {
    let mut m = machine(Model::Apple2e, &[(5, 5, false)]);
    assert_eq!(rd(&mut m, 0xC500), 0xA9, "peripheral ROM by default");
    assert_eq!(rd(&mut m, 0xC600), m.board().floating_bus(), "empty slot");
    wr(&mut m, 0xC007, 0);
    assert_eq!(rd(&mut m, 0xC500), 0xCC, "INTCXROM: internal ROM everywhere");
    assert_eq!(rd(&mut m, 0xC600), 0xCC);
    assert_eq!(rd(&mut m, 0xC800), 0xCC);
    wr(&mut m, 0xC006, 0);
    assert_eq!(rd(&mut m, 0xC500), 0xA9);
}

#[test]
fn iie_slot3_shows_internal_rom_until_slotc3rom_and_claims_c800_until_cfff() {
    let mut m = machine(Model::Apple2e, &[(3, 3, true), (5, 5, true)]);
    assert_eq!(rd(&mut m, 0xC300), 0xCC, "SLOTC3ROM off: internal $C3xx");
    assert_eq!(rd(&mut m, 0xC800), 0xCC, "...and INTC8ROM keeps internal ROM at $C800");
    rd(&mut m, 0xC500); // selecting slot 5 cannot override INTC8ROM
    assert_eq!(rd(&mut m, 0xC800), 0xCC);
    rd(&mut m, 0xCFFF);
    assert_eq!(rd(&mut m, 0xC800), m.board().floating_bus(), "released, and the old selection is gone");
    rd(&mut m, 0xCFFF);
    wr(&mut m, 0xC00B, 0); // SLOTC3ROM on: card ROM at $C300 and its expansion ROM
    assert_eq!(rd(&mut m, 0xC300), 0xA9);
    assert_eq!(rd(&mut m, 0xC800), 0xE3);
    assert_eq!(m.memmode() & mf::SLOTC3ROM, mf::SLOTC3ROM);
}

#[test]
fn iie_intc8rom_is_also_set_by_slot3_access_under_intcxrom() {
    let mut m = machine(Model::Apple2e, &[(5, 5, true)]);
    rd(&mut m, 0xC500);
    wr(&mut m, 0xC007, 0);
    rd(&mut m, 0xC300); // sets INTC8ROM although INTCXROM already forces internal ROM
    wr(&mut m, 0xC006, 0); // INTCXROM off: INTC8ROM keeps $C800 internal (sticky)
    assert!(m.board().slots.intc8rom());
    assert_eq!(rd(&mut m, 0xC800), 0xCC);
    rd(&mut m, 0xCFFF);
    assert!(!m.board().slots.intc8rom());
}

#[test]
fn iie_leaving_intcxrom_drops_the_stale_expansion_rom() {
    let mut m = machine(Model::Apple2e, &[(5, 5, true)]);
    rd(&mut m, 0xC500);
    assert_eq!(rd(&mut m, 0xC800), 0xE5);
    assert_eq!(m.board().slots.expansion(), ExpansionRom::Slot(5));
    wr(&mut m, 0xC007, 0);
    assert_eq!(m.board().slots.expansion(), ExpansionRom::Internal);
    wr(&mut m, 0xC006, 0);
    assert_eq!(m.board().slots.expansion(), ExpansionRom::Null, "Memory.cpp:1838");
    assert_eq!(rd(&mut m, 0xC800), 0xE5, "IO_SELECT is sticky: the next $C800 access reselects");
}

#[test]
fn power_cycle_clears_the_rom_selection_but_reset_does_not() {
    let mut m = plus(&[(5, 5, true)]);
    rd(&mut m, 0xC500);
    m.reset();
    assert_eq!(rd(&mut m, 0xC800), 0xE5);
    m.power_cycle();
    assert_eq!(rd(&mut m, 0xC800), m.board().floating_bus());
}

#[test]
fn event_raises_irq_delivered_to_the_cpu_vector() {
    let mut m = plus(&[(5, 5, false)]);
    // CLI; LDA #200; STA $C0D1 (arm in 200 cycles); JMP *   -- handler at $0400: INC $10; LDA $C0D8; RTI
    load(&mut m, 0x300, &[0x58, 0xA9, 200, 0x8D, 0xD1, 0xC0, 0x4C, 0x06, 0x03]);
    load(&mut m, 0x400, &[0xE6, 0x10, 0xAD, 0xD8, 0xC0, 0x40]);
    m.run_cycles(150);
    assert_eq!(m.peek(0x10), 0, "not yet");
    m.run_cycles(100);
    assert_eq!(m.peek(0x10), 1, "IRQ taken once; the handler acknowledged it");
    m.run_cycles(300);
    assert_eq!(m.peek(0x10), 1);
}

#[test]
fn periodic_event_rearms_and_can_be_cancelled() {
    let mut m = plus(&[(5, 5, false)]);
    wr(&mut m, 0xC0D2, 50); // token 8 in 50 cycles, then every 100
    m.run_cycles(260);
    assert_eq!(rd(&mut m, 0xC0DC), 3, "fires at ~50, ~150, ~250");
    wr(&mut m, 0xC0D3, 0); // cancel
    m.run_cycles(500);
    assert_eq!(rd(&mut m, 0xC0DC), 3);
}

#[test]
fn update_receives_executed_cycles_after_each_run() {
    let mut m = plus(&[(5, 5, false)]);
    let a = m.run_cycles(100);
    let b = m.run_frame();
    assert_eq!(rd(&mut m, 0xC0DE), (a + b) as u8);
}

#[test]
fn reset_and_power_cycle_reach_the_cards() {
    let mut m = plus(&[(5, 5, false)]);
    m.reset();
    assert_eq!(rd(&mut m, 0xC0DB), 1);
    m.power_cycle();
    assert_eq!(rd(&mut m, 0xC0DB), 2);
}

#[test]
fn slot_state_round_trips() {
    let mut m = plus(&[(5, 5, true)]);
    wr(&mut m, 0xC0D4, 0x66);
    rd(&mut m, 0xC500);
    let saved = m.board().slots.save();
    wr(&mut m, 0xC0D4, 0);
    m.power_cycle();
    assert!(m.board_mut().slots.load(&saved));
    assert_eq!(rd(&mut m, 0xC0D4), 0x66);
    assert_eq!(rd(&mut m, 0xC800), 0xE5);
}

#[test]
fn config_slots_still_build_the_ram_cards() {
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.slots[4] = Some(CardKind::Saturn128K);
    let m = Machine::new(cfg);
    assert!(m.board().lc.flags(4).is_some());
    assert!(m.board().slots.card(4).is_none(), "RAM cards are managed by LanguageCards");
}
