//! Language Card / Saturn 128K through the public seams: the CPU bus of a [`Machine`] and real
//! code executed by `Machine::step` (AppleWin LanguageCard.cpp).

use apple2_core::memory::mf;
use apple2_core::{CardKind, Machine, MachineConfig, Model};
use apple2_cpu::Bus;

/// 12K ][ ROM: $D000=$D0, $E000=$E0, $F800=$F8; RESET=$0300.
fn rom_ii() -> Vec<u8> {
    let mut r = vec![0xEAu8; 0x3000];
    r[0] = 0xD0;
    r[0x1000] = 0xE0;
    r[0x2800] = 0xF8;
    r[0x2FFC] = 0x00;
    r[0x2FFD] = 0x03;
    r
}

/// 16K //e ROM (4K internal Cx ROM + the 12K above).
fn rom_e() -> Vec<u8> {
    let mut r = vec![0xCCu8; 0x1000];
    r.extend(rom_ii());
    r
}

fn machine(model: Model, slots: &[(usize, CardKind)]) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    for &(s, k) in slots {
        cfg.slots[s] = Some(k);
    }
    cfg.custom_rom = Some(if model.is_apple2() { rom_ii() } else { rom_e() });
    Machine::new(cfg)
}

fn plus() -> Machine {
    machine(Model::Apple2Plus, &[])
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

#[test]
fn two_odd_reads_enable_ram_writes_but_one_does_not() {
    let mut m = plus();
    rd(&mut m, 0xC080); // even: RAM read, write protect
    assert_eq!(m.memmode() & mf::WRITERAM, 0);
    rd(&mut m, 0xC081); // first odd read: still protected
    assert_eq!(m.memmode() & mf::WRITERAM, 0);
    rd(&mut m, 0xC081); // second: enabled
    assert_ne!(m.memmode() & mf::WRITERAM, 0);
}

fn saturn_only(model: Model, slot: usize) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    cfg.slots = [None; 8];
    cfg.slots[slot] = Some(CardKind::Saturn128K);
    cfg.custom_rom = Some(if model.is_apple2() { rom_ii() } else { rom_e() });
    Machine::new(cfg)
}

fn flags(m: &Machine) -> u32 {
    m.memmode() & mf::LANGCARD_MASK
}

#[test]
fn writes_and_even_accesses_break_the_double_read() {
    let mut m = plus();
    rd(&mut m, 0xC080);
    wr(&mut m, 0xC081, 0); // a write is not a read: arms nothing
    rd(&mut m, 0xC081);
    assert_eq!(flags(&m) & mf::WRITERAM, 0, "write then read");

    rd(&mut m, 0xC081);
    assert_ne!(flags(&m) & mf::WRITERAM, 0);
    rd(&mut m, 0xC080); // even access clears
    assert_eq!(flags(&m) & mf::WRITERAM, 0);

    rd(&mut m, 0xC081);
    rd(&mut m, 0xC080);
    rd(&mut m, 0xC081);
    assert_eq!(flags(&m) & mf::WRITERAM, 0, "read, even, read");

    rd(&mut m, 0xC080);
    rd(&mut m, 0xC081);
    wr(&mut m, 0xC081, 0); // odd write after odd read: plain STA does not enable
    assert_eq!(flags(&m) & mf::WRITERAM, 0);
}

#[test]
fn a_single_odd_read_leaves_write_enable_unchanged() {
    let mut m = plus(); // powers on BANK2|WRITERAM
    rd(&mut m, 0xC081);
    assert_ne!(flags(&m) & mf::WRITERAM, 0);
    rd(&mut m, 0xC080);
    rd(&mut m, 0xC083);
    assert_eq!(flags(&m) & mf::WRITERAM, 0);
}

#[test]
fn all_sixteen_switches_match_the_utaiie_table() {
    // $C080+n: (bank2, ram read, write enabled) after the access is repeated (double read).
    let bank2 = [true, true, true, true, false, false, false, false];
    let ram_read = [true, false, false, true];
    let write = [false, true, false, true];
    for n in 0..16u16 {
        let mut m = plus();
        rd(&mut m, 0xC080);
        rd(&mut m, 0xC080 + n);
        rd(&mut m, 0xC080 + n);
        let f = flags(&m);
        let expect = |b: bool, flag| (f & flag != 0) == b;
        assert!(expect(bank2[(n >> 1) as usize & 7], mf::BANK2), "bank2 ${:04X}", 0xC080 + n);
        assert!(expect(ram_read[n as usize & 3], mf::HIGHRAM), "ram read ${:04X}", 0xC080 + n);
        assert!(expect(write[n as usize & 3], mf::WRITERAM), "write ${:04X}", 0xC080 + n);
    }
}

#[test]
fn bank_and_ram_rom_read_selection() {
    let mut m = plus();
    rd(&mut m, 0xC083);
    rd(&mut m, 0xC083); // bank 2, RAM read/write
    wr(&mut m, 0xD000, 2);
    wr(&mut m, 0xE000, 9);
    rd(&mut m, 0xC08B);
    rd(&mut m, 0xC08B); // bank 1
    assert_eq!(rd(&mut m, 0xD000), 0);
    wr(&mut m, 0xD000, 1);
    assert_eq!(rd(&mut m, 0xE000), 9, "$E000 is shared by both banks");
    rd(&mut m, 0xC082); // ROM read, write protect
    assert_eq!(rd(&mut m, 0xD000), 0xD0);
    wr(&mut m, 0xD000, 0x77);
    rd(&mut m, 0xC080); // bank 2, RAM read, protected
    assert_eq!(rd(&mut m, 0xD000), 2);
    wr(&mut m, 0xD000, 0x66);
    assert_eq!(rd(&mut m, 0xD000), 2, "write protected");
    rd(&mut m, 0xC088);
    assert_eq!(rd(&mut m, 0xD000), 1);
}

#[test]
fn rom_reads_while_writes_land_in_ram() {
    let mut m = plus();
    rd(&mut m, 0xC081);
    rd(&mut m, 0xC081); // ROM read, RAM write
    wr(&mut m, 0xD000, 0x55);
    assert_eq!(rd(&mut m, 0xD000), 0xD0);
    rd(&mut m, 0xC080);
    assert_eq!(rd(&mut m, 0xD000), 0x55);
}

#[test]
fn lc_switch_reads_the_floating_bus() {
    let mut m = plus();
    let fb = m.board().floating_bus();
    assert_eq!(rd(&mut m, 0xC083), fb);
}

#[test]
fn e_lc_ram_follows_altzp_into_aux() {
    let mut m = machine(Model::Apple2e, &[]);
    rd(&mut m, 0xC083);
    rd(&mut m, 0xC083);
    wr(&mut m, 0xD000, 0x11);
    wr(&mut m, 0xC009, 0); // ALTZP on
    assert_eq!(rd(&mut m, 0xD000), 0, "aux LC is a separate RAM");
    wr(&mut m, 0xD000, 0x22);
    assert_eq!(m.peek_aux(0xD000), 0x22);
    wr(&mut m, 0xC008, 0);
    assert_eq!(rd(&mut m, 0xD000), 0x11);
}

/// Runs `LDX #x; LDA $C080; <op> $C083; LDA #$55; STA $D000` (the opcode's own read is the first
/// of the double read, as on real hardware) and returns whether the final
/// store reached the Language Card RAM.
fn rmw_enables_write(model: Model, opcode: u8, x: u8) -> bool {
    let mut m = machine(model, &[]);
    let prog = [0xA2, x, 0xAD, 0x80, 0xC0, opcode, 0x83, 0xC0, 0xA9, 0x55, 0x8D, 0x00, 0xD0];
    for (i, b) in prog.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    for _ in 0..5 {
        m.step();
    }
    m.peek(0xD000) == 0x55
}

#[test]
fn gh700_inc_abs_x_with_x_zero_enables_on_both_cpus() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        assert!(rmw_enables_write(model, 0xFE, 0), "{model:?}");
        assert!(!rmw_enables_write(model, 0xFE, 1), "{model:?}: X != 0 touches another address");
    }
}

#[test]
fn gh700_rmw_abs_enables_on_65c02_only() {
    for op in [0xEE, 0xCE, 0x6E, 0x4E, 0x2E, 0x0E, 0x0C, 0x1C] {
        assert!(rmw_enables_write(Model::Apple2eEnhanced, op, 0), "65C02 opcode {op:02X}");
        assert!(!rmw_enables_write(Model::Apple2Plus, op, 0), "NMOS opcode {op:02X}");
    }
}

#[test]
fn plain_store_after_odd_read_does_not_enable() {
    // STA $C083 (8D 83 C0) after LDA $C083: not an RMW opcode.
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        assert!(!rmw_enables_write(model, 0x8D, 0));
    }
}

#[test]
fn saturn_selects_one_of_eight_banks() {
    let mut m = saturn_only(Model::Apple2Plus, 4);
    rd(&mut m, 0xC0C3); // slot 4: write-protected at power-on; two odd accesses enable
    rd(&mut m, 0xC0C3);
    for bank in 0..8u16 {
        // $C0n4-7 = banks 0-3, $C0nC-F = banks 4-7
        let sel = if bank < 4 { 0xC0C4 + bank } else { 0xC0C8 + bank };
        rd(&mut m, sel);
        wr(&mut m, 0xD000, 0x10 + bank as u8);
        wr(&mut m, 0xE000, 0x20 + bank as u8);
    }
    for bank in 0..8u16 {
        let sel = if bank < 4 { 0xC0C4 + bank } else { 0xC0C8 + bank };
        rd(&mut m, sel);
        assert_eq!(rd(&mut m, 0xD000), 0x10 + bank as u8);
        assert_eq!(rd(&mut m, 0xE000), 0x20 + bank as u8);
    }
    assert_eq!(m.board().lc.saturn_bank(4), Some(7));
}

#[test]
fn saturn_is_write_protected_until_two_odd_accesses_read_or_write() {
    let mut m = saturn_only(Model::Apple2Plus, 4);
    rd(&mut m, 0xC0C0); // RAM read bank 2, protected
    wr(&mut m, 0xD000, 1);
    assert_eq!(rd(&mut m, 0xD000), 0, "protected");
    rd(&mut m, 0xC0C3);
    assert_eq!(flags(&m) & mf::WRITERAM, 0, "one odd access is not enough");
    wr(&mut m, 0xC0C3, 0); // unlike the 16K card, a write counts as the second access
    assert_ne!(flags(&m) & mf::WRITERAM, 0);
    wr(&mut m, 0xD000, 2);
    assert_eq!(rd(&mut m, 0xD000), 2);
    rd(&mut m, 0xC0C2); // even clears
    assert_eq!(flags(&m) & mf::WRITERAM, 0);
}

#[test]
fn saturn_bank_select_does_not_disturb_write_enable() {
    let mut m = saturn_only(Model::Apple2Plus, 4);
    rd(&mut m, 0xC0C1);
    rd(&mut m, 0xC0C5); // bank select between the two odd accesses
    rd(&mut m, 0xC0C1);
    assert_ne!(flags(&m) & mf::WRITERAM, 0);
}

#[test]
fn saturn_in_slot_zero_replaces_the_16k_card_and_works_on_any_slot_number() {
    for slot in [0usize, 1, 7] {
        let mut m = saturn_only(Model::Apple2Plus, slot);
        let base = 0xC080 + 16 * slot as u16;
        rd(&mut m, base + 3);
        wr(&mut m, base + 3, 0);
        wr(&mut m, 0xD800, 0x42);
        assert_eq!(rd(&mut m, 0xD800), 0x42, "slot {slot}");
    }
}

#[test]
fn write_goes_to_the_last_accessed_writable_card() {
    let mut m = machine(Model::Apple2Plus, &[(4, CardKind::Saturn128K)]);
    rd(&mut m, 0xC080); // slot-0 card protected
    rd(&mut m, 0xC0C3);
    wr(&mut m, 0xC0C3, 0); // Saturn enabled
    wr(&mut m, 0xD000, 5);
    assert_eq!(m.board().memory.lc_region(), 1);
    rd(&mut m, 0xC080); // slot 0 again: Saturn is still the one that can write
    assert_eq!(rd(&mut m, 0xD000), 5);
    rd(&mut m, 0xC0C0); // protect the Saturn: nothing writable, slot 4's own state shows
    assert_eq!(flags(&m), mf::BANK2 | mf::HIGHRAM);
}

#[test]
fn reset_keeps_the_ii_plus_card_but_power_cycle_restores_it() {
    let mut m = plus();
    rd(&mut m, 0xC080);
    rd(&mut m, 0xC081);
    m.reset();
    assert_eq!(flags(&m), mf::BANK2);
    rd(&mut m, 0xC081); // the double-read latch survived the reset
    assert_ne!(flags(&m) & mf::WRITERAM, 0);
    rd(&mut m, 0xC080);
    m.power_cycle();
    assert_eq!(flags(&m), mf::BANK2 | mf::WRITERAM);
}

#[test]
fn reset_restores_the_e_card_and_clears_its_latch() {
    let mut m = machine(Model::Apple2e, &[]);
    rd(&mut m, 0xC080);
    rd(&mut m, 0xC081);
    m.reset();
    assert_eq!(flags(&m), mf::BANK2 | mf::WRITERAM);
    assert_eq!(m.board().lc.flags(0), Some(mf::BANK2 | mf::WRITERAM));
    rd(&mut m, 0xC080);
    rd(&mut m, 0xC081); // latch was cleared by the reset, but this is a fresh read pair
    rd(&mut m, 0xC081);
    assert_ne!(flags(&m) & mf::WRITERAM, 0);
}

#[test]
fn saturn_is_write_protected_after_power_cycle_and_on_e_reset_and_goes_to_bank_zero() {
    let mut m = saturn_only(Model::Apple2Plus, 4);
    rd(&mut m, 0xC0C3);
    rd(&mut m, 0xC0C3);
    rd(&mut m, 0xC0C7);
    m.reset(); // ][+: untouched
    assert_ne!(m.board().lc.flags(4).unwrap() & mf::WRITERAM, 0);
    assert_eq!(m.board().lc.saturn_bank(4), Some(3));
    m.power_cycle();
    assert_eq!(m.board().lc.flags(4).unwrap() & mf::WRITERAM, 0);
    assert_eq!(m.board().lc.saturn_bank(4), Some(0));

    let mut e = saturn_only(Model::Apple2e, 4);
    rd(&mut e, 0xC0C3);
    wr(&mut e, 0xC0C3, 0);
    assert_ne!(e.board().lc.flags(4).unwrap() & mf::WRITERAM, 0);
    e.reset();
    assert_eq!(e.board().lc.flags(4).unwrap() & mf::WRITERAM, 0);
}

fn f8(m: &Machine) -> Vec<u8> {
    (0xF800..=0xFFFFu16).map(|a| m.peek(a)).collect()
}

fn with_real_rom(model: Model, lc: bool, freeze: bool) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    cfg.slots[0] = lc.then_some(CardKind::LanguageCard16K);
    cfg.use_freeze_f8 = freeze;
    Machine::new(cfg)
}

#[test]
fn original_apple2_with_a_language_card_gets_the_plus_f8_rom() {
    let plain = apple2_roms::APPLE2;
    let plus = apple2_roms::APPLE2_PLUS;
    assert_ne!(plain[0x2800..], plus[0x2800..], "premise: the two F8 ROMs differ");
    assert_eq!(f8(&with_real_rom(Model::Apple2, false, false)), plain[0x2800..]);
    assert_eq!(f8(&with_real_rom(Model::Apple2, true, false)), plus[0x2800..]);
    // the rest of the ][ ROM is untouched
    let m = with_real_rom(Model::Apple2, true, false);
    assert_eq!(m.peek(0xD000), plain[0]);
    // a Saturn does not trigger the swap
    let mut cfg = MachineConfig::for_model(Model::Apple2);
    cfg.slots[0] = Some(CardKind::Saturn128K);
    assert_eq!(f8(&Machine::new(cfg)), plain[0x2800..]);
}

#[test]
fn freezes_f8_replaces_f8_on_the_ii_family_only() {
    let freeze = apple2_roms::FREEZES_NON_AUTOSTART_F8_ROM;
    assert_eq!(freeze.len(), 0x800);
    for (model, lc) in [(Model::Apple2, false), (Model::Apple2, true), (Model::Apple2Plus, true)] {
        assert_eq!(f8(&with_real_rom(model, lc, true)), freeze, "{model:?} lc={lc}");
    }
    let e = with_real_rom(Model::Apple2e, false, true);
    assert_ne!(f8(&e), freeze);
}
