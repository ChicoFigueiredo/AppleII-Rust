//! //e paging through the public seam: the CPU bus (`Bus::read/write`) of a [`Machine`].

use apple2_core::memory::mf;
use apple2_core::{AuxCardKind, Machine, MachineConfig, Model};
use apple2_cpu::Bus;

/// //e ROM image: 4K internal Cx ROM (C1xx=$11, C3xx=$33, C8xx=$88, rest $CC) then 12K system ROM
/// ($D000=$D0, $E000=$E0, vectors RESET=$0300).
fn rom_e() -> Vec<u8> {
    let mut r = vec![0xCCu8; 0x4000];
    r[0x100] = 0x11;
    r[0x300] = 0x33;
    r[0x800] = 0x88;
    let sys = &mut r[0x1000..];
    sys.fill(0xEA);
    sys[0] = 0xD0;
    sys[0x1000] = 0xE0;
    sys[0x2FFC] = 0x00;
    sys[0x2FFD] = 0x03;
    r
}

fn machine_with(model: Model, aux: AuxCardKind, rom: Vec<u8>) -> Machine {
    let mut cfg = MachineConfig::for_model(model);
    cfg.aux_card = aux;
    cfg.custom_rom = Some(rom);
    Machine::new(cfg)
}

fn e(aux: AuxCardKind) -> Machine {
    machine_with(Model::Apple2e, aux, rom_e())
}

fn rd(m: &mut Machine, a: u16) -> u8 {
    m.board_mut().read(a)
}

fn wr(m: &mut Machine, a: u16, v: u8) {
    m.board_mut().write(a, v)
}

#[test]
fn flag_values_match_applewin() {
    assert_eq!(
        [
            mf::STORE80,
            mf::ALTZP,
            mf::AUXREAD,
            mf::AUXWRITE,
            mf::BANK2,
            mf::HIGHRAM,
            mf::HIRES,
            mf::PAGE2
        ],
        [1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80]
    );
    assert_eq!(
        [
            mf::SLOTC3ROM,
            mf::INTCXROM,
            mf::WRITERAM,
            mf::IOUDIS,
            mf::ALTROM0,
            mf::ALTROM1
        ],
        [0x100, 0x200, 0x400, 0x800, 0x1000, 0x2000]
    );
}

#[test]
fn initial_memmode_per_model() {
    assert_eq!(
        e(AuxCardKind::Extended80Col).memmode(),
        mf::BANK2 | mf::WRITERAM
    );
    assert_eq!(
        machine_with(Model::Apple2, AuxCardKind::Empty, vec![0; 0x3000]).memmode(),
        0
    );
    let p = machine_with(Model::Apple2Plus, AuxCardKind::Empty, vec![0; 0x3000]);
    assert_eq!(p.memmode(), mf::BANK2 | mf::WRITERAM);
}

#[test]
fn rom_is_read_only_and_writes_go_to_lc_ram_underneath() {
    let mut m = e(AuxCardKind::Extended80Col);
    assert_eq!(rd(&mut m, 0xD000), 0xD0);
    wr(&mut m, 0xD000, 0x55); // WRITERAM set: lands in LC RAM, ROM stays visible
    assert_eq!(rd(&mut m, 0xD000), 0xD0);
    m.board_mut()
        .memory
        .set_memmode(mf::BANK2 | mf::WRITERAM | mf::HIGHRAM);
    assert_eq!(rd(&mut m, 0xD000), 0x55);
    m.board_mut().memory.set_memmode(mf::BANK2 | mf::HIGHRAM); // write-protected
    wr(&mut m, 0xD000, 0x66);
    assert_eq!(rd(&mut m, 0xD000), 0x55);
}

#[test]
fn bank1_and_bank2_of_d000_are_distinct_but_e000_is_shared() {
    let mut m = e(AuxCardKind::Extended80Col);
    let rw = mf::HIGHRAM | mf::WRITERAM;
    m.board_mut().memory.set_memmode(rw | mf::BANK2);
    wr(&mut m, 0xD123, 2);
    wr(&mut m, 0xE123, 9);
    m.board_mut().memory.set_memmode(rw);
    assert_eq!(rd(&mut m, 0xD123), 0);
    wr(&mut m, 0xD123, 1);
    assert_eq!(rd(&mut m, 0xE123), 9);
    m.board_mut().memory.set_memmode(rw | mf::BANK2);
    assert_eq!(rd(&mut m, 0xD123), 2);
}

#[test]
fn no_lc_machine_discards_rom_writes() {
    let mut m = machine_with(Model::Apple2, AuxCardKind::Empty, vec![0xAB; 0x3000]);
    wr(&mut m, 0xD000, 1);
    assert_eq!(rd(&mut m, 0xD000), 0xAB);
}

#[test]
fn altzp_swaps_zero_page_and_stack_and_lc() {
    let mut m = e(AuxCardKind::Extended80Col);
    wr(&mut m, 0x10, 1);
    wr(&mut m, 0x1FF, 2);
    m.board_mut()
        .memory
        .set_memmode(mf::ALTZP | mf::HIGHRAM | mf::WRITERAM | mf::BANK2);
    assert_eq!((rd(&mut m, 0x10), rd(&mut m, 0x1FF)), (0, 0));
    wr(&mut m, 0x10, 3);
    wr(&mut m, 0xD000, 4);
    m.board_mut()
        .memory
        .set_memmode(mf::HIGHRAM | mf::WRITERAM | mf::BANK2);
    assert_eq!(
        (rd(&mut m, 0x10), rd(&mut m, 0x1FF), rd(&mut m, 0xD000)),
        (1, 2, 0)
    );
    assert_eq!((m.peek_aux(0x10), m.peek_aux(0xD000)), (3, 4));
}

#[test]
fn auxread_and_auxwrite_are_independent() {
    let mut m = e(AuxCardKind::Extended80Col);
    wr(&mut m, 0x2000, 0xA);
    m.board_mut()
        .memory
        .set_memmode(mf::BANK2 | mf::WRITERAM | mf::AUXWRITE);
    wr(&mut m, 0x2000, 0xB); // goes to aux
    assert_eq!(rd(&mut m, 0x2000), 0xA); // still reading main
    m.board_mut()
        .memory
        .set_memmode(mf::BANK2 | mf::WRITERAM | mf::AUXREAD);
    assert_eq!(rd(&mut m, 0x2000), 0xB);
    wr(&mut m, 0x2000, 0xC); // writes main
    assert_eq!((m.peek_main(0x2000), m.peek_aux(0x2000)), (0xC, 0xB));
}

#[test]
fn store80_page2_overrides_auxread_write_for_text_page1() {
    let mut m = e(AuxCardKind::Extended80Col);
    let base = mf::BANK2 | mf::WRITERAM | mf::STORE80;
    m.poke_main(0x400, 1);
    m.poke_aux(0x400, 2);
    m.poke_main(0x2000, 3);
    m.poke_aux(0x2000, 4);
    m.board_mut()
        .memory
        .set_memmode(base | mf::AUXREAD | mf::AUXWRITE); // PAGE2 off: main
    assert_eq!(rd(&mut m, 0x400), 1);
    wr(&mut m, 0x7FF, 7);
    assert_eq!(m.peek_main(0x7FF), 7);
    assert_eq!(rd(&mut m, 0x2000), 4); // HIRES off: not overridden
    m.board_mut().memory.set_memmode(base | mf::PAGE2);
    assert_eq!(rd(&mut m, 0x400), 2);
    assert_eq!(rd(&mut m, 0x2000), 3); // HIRES off: main
    m.board_mut()
        .memory
        .set_memmode(base | mf::PAGE2 | mf::HIRES);
    assert_eq!(rd(&mut m, 0x2000), 4);
    wr(&mut m, 0x3FFF, 9);
    assert_eq!(m.peek_aux(0x3FFF), 9);
}

#[test]
fn intcxrom_shows_internal_rom_in_cx_range() {
    let mut m = e(AuxCardKind::Extended80Col);
    // Slot ROM space is empty (slice 4 adds cards): floating bus placeholder = 0, except $C3xx
    // which is internal while SLOTC3ROM is off.
    assert_eq!(rd(&mut m, 0xC100), 0);
    assert_eq!(rd(&mut m, 0xC300), 0x33);
    m.board_mut()
        .memory
        .set_memmode(mf::BANK2 | mf::WRITERAM | mf::SLOTC3ROM);
    assert_eq!(rd(&mut m, 0xC300), 0);
    m.board_mut()
        .memory
        .set_memmode(mf::BANK2 | mf::WRITERAM | mf::INTCXROM);
    assert_eq!(
        (
            rd(&mut m, 0xC100),
            rd(&mut m, 0xC300),
            rd(&mut m, 0xC800),
            rd(&mut m, 0xCFFE)
        ),
        (0x11, 0x33, 0x88, 0xCC)
    );
    wr(&mut m, 0xC100, 0); // ROM: ignored
    assert_eq!(rd(&mut m, 0xC100), 0x11);
}

// ---- Soft switches and status reads ----

/// (clear address, set address, flag read through $C011-$C01F, MF flag when it is a memmode one).
const PAIRS: [(u16, u16, u16, u32); 8] = [
    (0xC000, 0xC001, 0xC018, mf::STORE80),
    (0xC002, 0xC003, 0xC013, mf::AUXREAD),
    (0xC004, 0xC005, 0xC014, mf::AUXWRITE),
    (0xC006, 0xC007, 0xC015, mf::INTCXROM),
    (0xC008, 0xC009, 0xC016, mf::ALTZP),
    (0xC00A, 0xC00B, 0xC017, mf::SLOTC3ROM),
    (0xC054, 0xC055, 0xC01C, mf::PAGE2),
    (0xC056, 0xC057, 0xC01D, mf::HIRES),
];

#[test]
fn each_switch_turns_on_off_and_shows_in_status() {
    for (off, on, status, flag) in PAIRS {
        let mut m = e(AuxCardKind::Extended80Col);
        assert_eq!(rd(&mut m, status) & 0x80, 0, "{status:#x} initially off");
        wr(&mut m, on, 0);
        assert_ne!(m.memmode() & flag, 0, "{on:#x}");
        assert_eq!(rd(&mut m, status) & 0x80, 0x80, "{status:#x}");
        wr(&mut m, off, 0);
        assert_eq!(m.memmode() & flag, 0, "{off:#x}");
        assert_eq!(rd(&mut m, status) & 0x80, 0);
        // Reading $C054-$C057 toggles too; the MMU switches C000-C00B only act on writes.
        if off >= 0xC054 {
            rd(&mut m, on);
            assert_ne!(m.memmode() & flag, 0);
            rd(&mut m, off);
            assert_eq!(m.memmode() & flag, 0);
        } else {
            rd(&mut m, on);
            assert_eq!(m.memmode() & flag, 0, "read of {on:#x} must not set");
        }
    }
}

#[test]
fn lc_status_reads_follow_flags() {
    let mut m = e(AuxCardKind::Extended80Col);
    assert_eq!((rd(&mut m, 0xC011), rd(&mut m, 0xC012)), (0x80, 0));
    m.board_mut().memory.set_memmode(mf::HIGHRAM);
    assert_eq!((rd(&mut m, 0xC011), rd(&mut m, 0xC012)), (0, 0x80));
}

#[test]
fn video_flag_switches_and_status() {
    let mut m = e(AuxCardKind::Extended80Col);
    assert_eq!(rd(&mut m, 0xC01A), 0x80, "TEXT is on after power-on");
    for (addr, status, expect) in [
        (0xC050, 0xC01A, 0),
        (0xC052, 0xC01B, 0),
        (0xC053, 0xC01B, 0x80),
        (0xC00D, 0xC01F, 0x80),
        (0xC00F, 0xC01E, 0x80),
    ] {
        wr(&mut m, addr, 0);
        assert_eq!(rd(&mut m, status), expect, "{addr:#x}");
    }
    rd(&mut m, 0xC051);
    assert_eq!(rd(&mut m, 0xC01A), 0x80);
    wr(&mut m, 0xC00C, 0);
    wr(&mut m, 0xC00E, 0);
    assert_eq!((rd(&mut m, 0xC01F), rd(&mut m, 0xC01E)), (0, 0));
    wr(&mut m, 0xC05E, 0); // DHIRES on
    assert!(m.board().video.dhires);
    rd(&mut m, 0xC05F);
    assert!(!m.board().video.dhires);
}

#[test]
fn vbl_status_bit_is_set_on_the_visible_lines_and_key_bits_are_zero() {
    let mut m = e(AuxCardKind::Extended80Col);
    assert_eq!(rd(&mut m, 0xC019), 0x80);
}

#[test]
fn soft_switches_drive_real_paging() {
    let mut m = e(AuxCardKind::Extended80Col);
    wr(&mut m, 0x300, 1);
    wr(&mut m, 0xC005, 0); // AUXWRITE
    wr(&mut m, 0x300, 2);
    wr(&mut m, 0xC004, 0);
    wr(&mut m, 0xC003, 0); // AUXREAD
    assert_eq!(rd(&mut m, 0x300), 2);
    wr(&mut m, 0xC002, 0);
    assert_eq!(rd(&mut m, 0x300), 1);
    wr(&mut m, 0xC007, 0); // INTCXROM
    assert_eq!(rd(&mut m, 0xC100), 0x11);
    wr(&mut m, 0xC006, 0);
    assert_eq!(rd(&mut m, 0xC100), 0);
}

#[test]
fn ii_family_ignores_e_switches_but_toggles_page2_hires() {
    let mut m = machine_with(Model::Apple2Plus, AuxCardKind::Empty, vec![0xEA; 0x3000]);
    let before = m.memmode();
    for a in [
        0xC001, 0xC003, 0xC005, 0xC007, 0xC009, 0xC00B, 0xC071, 0xC073,
    ] {
        wr(&mut m, a, 1);
    }
    assert_eq!(m.memmode(), before);
    wr(&mut m, 0x2000, 5);
    assert_eq!(rd(&mut m, 0x2000), 5);
    rd(&mut m, 0xC055);
    rd(&mut m, 0xC057);
    assert_eq!(m.memmode() & (mf::PAGE2 | mf::HIRES), mf::PAGE2 | mf::HIRES);
    // C01x: strobe clear + floating bus, no status.
    m.board_mut().memory.set_memmode(mf::HIGHRAM);
    assert_eq!(rd(&mut m, 0xC012), 0);
    // //e-only video switches are ignored too.
    wr(&mut m, 0xC00D, 0);
    wr(&mut m, 0xC05E, 0);
    assert!(!m.board().video.col80 && !m.board().video.dhires);
    wr(&mut m, 0xC051, 0);
    assert!(m.board().video.text);
}

// ---- Aux cards ----

#[test]
fn empty_aux_reads_floating_bus_and_discards_writes() {
    let mut m = e(AuxCardKind::Empty);
    m.board_mut()
        .memory
        .set_memmode(mf::BANK2 | mf::WRITERAM | mf::AUXREAD | mf::AUXWRITE | mf::ALTZP);
    m.board_mut().write(0x2000, 7);
    m.board_mut().write(0x10, 7);
    assert_eq!((rd(&mut m, 0x2000), rd(&mut m, 0x10)), (0, 0));
    m.poke_main(0x2000, 9);
    assert_eq!(rd(&mut m, 0x2000), 0, "aux read does not see main");
    m.board_mut().memory.set_memmode(mf::BANK2 | mf::WRITERAM);
    assert_eq!(rd(&mut m, 0x2000), 9);
}

#[test]
fn col80_1k_mirrors_all_aux_into_1k() {
    let mut m = e(AuxCardKind::Col80_1K);
    m.board_mut()
        .memory
        .set_memmode(mf::BANK2 | mf::WRITERAM | mf::AUXREAD | mf::AUXWRITE);
    wr(&mut m, 0x2000, 0x5A);
    assert_eq!(rd(&mut m, 0x2000), 0x5A);
    assert_eq!(rd(&mut m, 0x0400), 0x5A, "$2000 and $0400 share a 1K cell");
    assert_eq!(rd(&mut m, 0x8800), 0x5A);
    wr(&mut m, 0x2101, 0x6B);
    assert_eq!(rd(&mut m, 0x7D01), 0x6B);
    assert_eq!(m.peek_main(0x2000), 0, "main untouched");
    assert_eq!(m.peek_aux(0x2000), 0x5A);
}

#[test]
fn extended_80col_has_64k_aux_and_ignores_bank_select() {
    let mut m = e(AuxCardKind::Extended80Col);
    m.board_mut()
        .memory
        .set_memmode(mf::BANK2 | mf::WRITERAM | mf::AUXWRITE);
    wr(&mut m, 0xBFFF, 3);
    wr(&mut m, 0xC073, 1);
    assert_eq!((m.board().memory.aux_bank(), m.peek_aux(0xBFFF)), (0, 3));
}

#[test]
fn ramworks_selects_banks_with_c071_and_c073_writes() {
    let mut m = e(AuxCardKind::RamWorksIII { banks: 4 });
    wr(&mut m, 0xC005, 0); // AUXWRITE
    wr(&mut m, 0x1234, 0xA0);
    wr(&mut m, 0xC073, 2);
    wr(&mut m, 0x1234, 0xA2);
    wr(&mut m, 0xC071, 3);
    wr(&mut m, 0x1234, 0xA3);
    wr(&mut m, 0xC073, 9); // out of range: ignored
    assert_eq!(m.board().memory.aux_bank(), 3);
    wr(&mut m, 0xC073, 0);
    assert_eq!(m.peek_aux(0x1234), 0xA0);
    wr(&mut m, 0xC073, 2);
    assert_eq!(m.peek_aux(0x1234), 0xA2);
    rd(&mut m, 0xC073); // reads do not select
    wr(&mut m, 0xC073, 0);
    assert_eq!(m.board().memory.aux_bank(), 0);
}

#[test]
fn ramworks_bank_survives_reset_but_not_power_cycle() {
    let mut m = e(AuxCardKind::RamWorksIII { banks: 4 });
    wr(&mut m, 0xC073, 2);
    wr(&mut m, 0xC009, 0);
    m.reset();
    assert_eq!(m.board().memory.aux_bank(), 2);
    assert_eq!(m.memmode() & mf::ALTZP, 0, "reset clears the MMU switches");
    m.power_cycle();
    assert_eq!(m.board().memory.aux_bank(), 0);
}

// ---- Reset ----

#[test]
fn reset_clears_e_switches_and_video_but_ii_keeps_them() {
    let mut m = e(AuxCardKind::Extended80Col);
    for a in [
        0xC001, 0xC003, 0xC005, 0xC007, 0xC009, 0xC00B, 0xC055, 0xC057, 0xC00D, 0xC051,
    ] {
        wr(&mut m, a, 0);
    }
    let mode = m.memmode();
    m.board_mut().memory.set_memmode(mode | mf::HIGHRAM);
    m.reset();
    assert_eq!(m.memmode(), mf::BANK2 | mf::WRITERAM);
    assert!(m.board().video.text && !m.board().video.col80);

    let mut p = machine_with(Model::Apple2Plus, AuxCardKind::Empty, vec![0xEA; 0x3000]);
    rd(&mut p, 0xC055);
    let mode = p.memmode();
    p.board_mut().memory.set_memmode(mode | mf::HIGHRAM);
    let before = p.memmode();
    p.reset();
    assert_eq!(p.memmode(), before);
    p.power_cycle();
    assert_eq!(p.memmode(), mf::BANK2 | mf::WRITERAM);
}

#[test]
fn reset_vector_boots_from_rom_after_reset_with_lc_on() {
    let mut m = e(AuxCardKind::Extended80Col);
    m.board_mut()
        .memory
        .set_memmode(mf::HIGHRAM | mf::BANK2 | mf::WRITERAM);
    m.reset();
    assert_eq!(m.cpu().pc, 0x0300);
}

// ---- Base64A ALTROM ----

#[test]
fn base64a_c058_c05b_select_rom_page() {
    let mut rom = vec![0u8; 0xC000];
    for (i, page) in rom.chunks_mut(0x3000).enumerate() {
        page.fill(0x10 + i as u8);
    }
    let mut m = machine_with(Model::Base64A, AuxCardKind::Empty, rom);
    assert_eq!(rd(&mut m, 0xD000), 0x10);
    rd(&mut m, 0xC059); // ALTROM0
    assert_eq!(rd(&mut m, 0xD000), 0x11);
    rd(&mut m, 0xC05B); // ALTROM1
    assert_eq!((rd(&mut m, 0xD000), rd(&mut m, 0xFFFF)), (0x13, 0x13));
    rd(&mut m, 0xC058);
    assert_eq!(rd(&mut m, 0xD000), 0x12);
    rd(&mut m, 0xC05A);
    assert_eq!(rd(&mut m, 0xD000), 0x10);
    rd(&mut m, 0xC05B);
    m.reset();
    assert_eq!(
        m.memmode() & (mf::ALTROM0 | mf::ALTROM1),
        0,
        "RESET goes back to page 0"
    );
}

#[test]
fn annunciators_follow_c058_c05f() {
    let mut m = e(AuxCardKind::Extended80Col);
    rd(&mut m, 0xC059); // AN0 on
    rd(&mut m, 0xC05D); // AN2 on
    assert_eq!(m.board().annunciators, [true, false, true, false]);
    rd(&mut m, 0xC05C); // AN2 off
    rd(&mut m, 0xC05F); // AN3 on (also DHIRES off)
    assert_eq!(m.board().annunciators, [true, false, false, true]);
    m.reset();
    assert_eq!(m.board().annunciators, [false; 4]);
}

#[test]
fn memory_state_roundtrips() {
    let mut m = e(AuxCardKind::RamWorksIII { banks: 2 });
    wr(&mut m, 0xC073, 1);
    m.poke_aux(0x100, 7);
    m.poke_main(0x100, 8);
    let s = m.board().memory.state();
    m.power_cycle();
    assert!(m.board_mut().memory.restore(&s));
    assert_eq!(
        (
            m.peek_aux(0x100),
            m.peek_main(0x100),
            m.board().memory.aux_bank()
        ),
        (7, 8, 1)
    );
}
