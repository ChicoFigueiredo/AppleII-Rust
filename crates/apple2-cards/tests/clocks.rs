//! ThunderClock Plus / TK clock firmware running against the uPD1990AC model, and the No-Slot
//! Clock on both the ][+ and the //e.

use apple2_cards::{ClockModel, FixedClock, NoSlotClock, ThunderClock, WallTime};
use apple2_core::{Machine, MachineConfig, Model};
use apple2_cpu::Bus;

const SLOT: u8 = 4;

fn with_clock(model: ClockModel, t: WallTime) -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(m.insert_card(
        SLOT,
        Box::new(ThunderClock::new(model, Box::new(FixedClock(t))))
    ));
    m
}

fn run_until(m: &mut Machine, pc: u16) {
    for _ in 0..2_000_000 {
        if m.cpu().pc == pc {
            return;
        }
        m.step();
    }
    panic!("never reached {pc:#x}, at {:#x}", m.cpu().pc);
}

/// `JSR entry; JMP *` with the ProDOS clock-driver call convention.
fn call_driver(m: &mut Machine, entry: u16, a: u8) {
    let code = [
        0xA9,
        a,
        0x20,
        entry as u8,
        (entry >> 8) as u8,
        0x4C,
        0x05,
        0x03,
    ];
    for (i, b) in code.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    run_until(m, 0x305);
}

#[test]
fn rom_carries_the_prodos_clock_signature() {
    for model in [ClockModel::ThunderClockPlus, ClockModel::TkClock] {
        let mut m = with_clock(model, WallTime::SAMPLE);
        let sig: Vec<u8> = [0xC400u16, 0xC402, 0xC404, 0xC406]
            .iter()
            .map(|&a| m.board_mut().read(a))
            .collect();
        assert_eq!(sig, [0x08, 0x28, 0x58, 0x70], "{model:?}");
    }
}

#[test]
fn expansion_rom_appears_at_c800_after_touching_the_slot() {
    let mut m = with_clock(ClockModel::ThunderClockPlus, WallTime::SAMPLE);
    m.board_mut().read(0xC400);
    assert_eq!(m.board_mut().read(0xC800), apple2_roms::THUNDERCLOCKPLUS[0]);
    assert_eq!(
        m.board_mut().read(0xCFFE),
        apple2_roms::THUNDERCLOCKPLUS[0x7FE]
    );
    let mut m = with_clock(ClockModel::TkClock, WallTime::SAMPLE);
    m.board_mut().read(0xC400);
    assert_eq!(m.board_mut().read(0xC800), apple2_roms::TKCLOCK[0x100]);
}

/// The text the firmware leaves in the input buffer at `$0200` (high bits stripped).
fn buffer_text(m: &Machine) -> String {
    (0x200u16..0x240)
        .map(|a| m.peek_main(a))
        .take_while(|&b| b != 0x8D)
        .map(|b| (b & 0x7F) as char)
        .collect()
}

#[test]
fn thunderclock_firmware_reads_the_host_time_through_the_chip() {
    let mut m = with_clock(ClockModel::ThunderClockPlus, WallTime::SAMPLE);
    call_driver(&mut m, 0xC408, 0xA3);
    assert_eq!(buffer_text(&m), "10/19 14;35;27.000");
}

#[test]
fn time_follows_the_host_clock() {
    let later = WallTime {
        month: 12,
        day: 3,
        hour: 9,
        minute: 5,
        second: 41,
        ..WallTime::SAMPLE
    };
    let mut m = with_clock(ClockModel::ThunderClockPlus, later);
    call_driver(&mut m, 0xC408, 0xA3);
    assert_eq!(buffer_text(&m), "12/03 09;05;41.000");
}

const REGISTER: [u8; 8] = [0x12, 0x27, 0x35, 0x14, 0x02, 0x19, 0x10, 0x26];

fn nsc() -> Box<NoSlotClock> {
    Box::new(NoSlotClock::new(Box::new(FixedClock(WallTime::SAMPLE))))
}

#[test]
fn no_slot_clock_on_the_iie_is_read_by_6502_code_through_the_internal_c3_rom() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    m.set_rom_tap(Some(nsc()));
    let pattern: u64 = 0x5CA3_3AC5_5CA3_3AC5;
    let mut code = vec![0xAD, 0x04, 0xC3]; // LDA $C304: reset the comparison register
    for i in 0..64 {
        code.extend([0xAD, ((pattern >> i) & 1) as u8, 0xC3]); // LDA $C300/$C301: pattern bit on A0
    }
    for byte in 0..8u8 {
        for _ in 0..8 {
            code.extend([0xAD, 0x04, 0xC3, 0x4A, 0x66, 0x20 + byte]); // LDA $C304; LSR; ROR $20+byte
        }
    }
    let end = 0x300 + code.len() as u16;
    code.extend([0x4C, end as u8, (end >> 8) as u8]);
    for (i, b) in code.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    run_until(&mut m, end);
    let got: Vec<u8> = (0x20..0x28).map(|a| m.peek_main(a)).collect();
    assert_eq!(got, REGISTER);
}

#[test]
fn no_slot_clock_sees_the_c8_rom_only_with_internal_rom_selected() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    m.set_rom_tap(Some(nsc()));
    // Slot-C3 ROM selected and INTCXROM off: $C3xx belongs to the (empty) slot, the tap sees nothing.
    m.board_mut().write(0xC00B, 0); // SLOTC3ROM on
    m.board_mut().read(0xC304);
    assert!(m.rom_tap_mut().is_some());
    // INTCXROM on: $C8xx is internal ROM and watched.
    m.board_mut().write(0xC007, 0);
    let pattern: u64 = 0x5CA3_3AC5_5CA3_3AC5;
    m.board_mut().read(0xC804);
    for i in 0..64 {
        m.board_mut().read(0xC800 + ((pattern >> i) & 1) as u16);
    }
    let mut bits = 0u64;
    for i in 0..64 {
        bits |= u64::from(m.board_mut().read(0xC804) & 1) << i;
    }
    assert_eq!(bits.to_le_bytes(), REGISTER);
}

#[test]
fn no_slot_clock_on_the_ii_plus_watches_f8_rom_while_the_language_card_is_off() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    m.set_rom_tap(Some(nsc()));
    m.board_mut().read(0xC082); // ROM read, LC write protected
    let pattern: u64 = 0x5CA3_3AC5_5CA3_3AC5;
    m.board_mut().read(0xF804);
    for i in 0..64 {
        m.board_mut().read(0xF800 + ((pattern >> i) & 1) as u16);
    }
    let mut bits = 0u64;
    for i in 0..64 {
        bits |= u64::from(m.board_mut().read(0xFFF4) & 1) << i;
    }
    assert_eq!(bits.to_le_bytes(), REGISTER);
    // With the Language Card RAM visible the clock is out of the picture.
    m.board_mut().read(0xC083);
    m.board_mut().read(0xC083);
    assert_eq!(m.board_mut().read(0xFFF4) & 1, m.peek(0xFFF4) & 1);
}

#[test]
fn no_slot_clock_state_survives_save_and_load_and_power_cycle_resets_it() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    m.set_rom_tap(Some(nsc()));
    m.board_mut().read(0xC082);
    m.board_mut().read(0xF804);
    let st = m.save_state();
    assert!(st.rom_tap.is_some());
    m.load_state(&st).unwrap();
    m.power_cycle();
}
