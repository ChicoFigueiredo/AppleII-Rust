//! Z80 SoftCard in slot 4 of a ][+: the 6502 hands the CPU to the Z80 by writing to `$C400`,
//! the Z80 works on Apple memory through the SoftCard address map and hands it back.

use apple2_cards::softcard::SoftCard;
use apple2_core::{Machine, MachineConfig, Model};

const SLOT: u8 = 4;

fn machine() -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(m.insert_card(SLOT, Box::new(SoftCard::new())));
    m
}

fn load(m: &mut Machine, at: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke(at + i as u16, *b);
    }
}

fn start_6502(m: &mut Machine, code: &[u8]) {
    load(m, 0x300, code);
    m.cpu_mut().pc = 0x300;
}

#[test]
fn z80_runs_a_program_writes_apple_memory_and_hands_back() {
    let mut m = machine();
    // Z80 starts at its PC 0 = Apple $1000. It stores into Apple $0010 (Z80 $F010), I/O-writes
    // $C400 (Z80 $E400) to return, and would loop if it kept running.
    load(&mut m, 0x1000, &[0x3E, 0x42, 0x32, 0x10, 0xF0, 0x32, 0x00, 0xE4, 0x18, 0xFE]);
    // 6502: LDA #1; STA $C400 (-> Z80); STA $11 (runs after the Z80 returns); JMP *
    start_6502(&mut m, &[0xA9, 0x01, 0x8D, 0x00, 0xC4, 0x85, 0x11, 0x4C, 0x07, 0x03]);
    m.run_cycles(200);
    assert_eq!(m.peek(0x10), 0x42, "Z80 wrote Apple RAM");
    assert_eq!(m.peek(0x11), 1, "6502 resumed after the Z80 handed back");
    assert_eq!(m.cpu_owner(), None);
}

#[test]
fn cpu_is_z80_while_active_and_clock_is_half_speed() {
    let mut m = machine();
    load(&mut m, 0x1000, &[0, 0, 0, 0, 0, 0, 0, 0, 0x18, 0xFE]); // NOPs, then JR $
    start_6502(&mut m, &[0x8D, 0x00, 0xC4, 0x4C, 0x03, 0x03]);
    m.step(); // STA $C400
    assert_eq!(m.cpu_owner(), Some(SLOT));
    let pc6502 = m.cpu().pc;
    let c0 = m.cycles();
    assert_eq!(m.step(), 2, "NOP = 4 T-states = 2 cycles");
    assert_eq!(m.cycles() - c0, 2);
    m.run_cycles(100);
    assert_eq!(m.cpu().pc, pc6502, "6502 registers untouched while the Z80 runs");
    let pc = m
        .with_card(SLOT, |c, _| c.as_any_mut().unwrap().downcast_mut::<SoftCard>().unwrap().z80().unwrap().pc)
        .unwrap();
    assert_eq!(pc, 8, "Z80 spins on its JR $");
}

#[test]
fn z80_memory_map_reaches_zero_page_stack_and_rom() {
    let mut m = machine();
    m.poke(0x0020, 0x77);
    // LD A,(0xF020); LD (0x0000),A (Z80 $0000 = Apple $1000); LD A,(0xB000) = Apple $D000 ROM; HALT-ish loop
    load(&mut m, 0x1000, &[0x3A, 0x20, 0xF0, 0x32, 0x00, 0x00, 0x3A, 0x00, 0xB0, 0x32, 0x30, 0xF0, 0x18, 0xFE]);
    start_6502(&mut m, &[0x8D, 0x00, 0xC4]);
    m.run_cycles(300);
    assert_eq!(m.peek(0x1000), 0x77, "Z80 $F020 is Apple $0020; Z80 $0000 is Apple RAM $1000");
    assert_eq!(m.peek(0x30), m.peek(0xD000), "Z80 $B000 reads Apple $D000");
}

#[test]
fn reset_returns_the_cpu_to_the_6502() {
    let mut m = machine();
    start_6502(&mut m, &[0x8D, 0x00, 0xC4]);
    m.step();
    assert_eq!(m.cpu_owner(), Some(SLOT));
    m.reset();
    assert_eq!(m.cpu_owner(), None);
}

#[test]
fn state_roundtrip_keeps_z80_registers_and_owner() {
    let mut m = machine();
    load(&mut m, 0x1000, &[0x3E, 0x99, 0x18, 0xFE]);
    start_6502(&mut m, &[0x8D, 0x00, 0xC4]);
    m.run_cycles(50);
    let st = m.save_state();
    m.run_cycles(50);
    m.load_state(&st).unwrap();
    assert_eq!(m.cpu_owner(), Some(SLOT));
    let a = m
        .with_card(SLOT, |c, _| c.as_any_mut().unwrap().downcast_mut::<SoftCard>().unwrap().z80().unwrap().a)
        .unwrap();
    assert_eq!(a, 0x99);
}
