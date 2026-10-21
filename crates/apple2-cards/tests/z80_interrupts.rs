//! HU-0601: interrupt semantics of the Z80 core (IM 0/1/2, NMI, HALT, EI delay, RETN/RETI) and
//! the report of undefined `ED` opcodes. The SoftCard does not wire the lines (as AppleWin).

use apple2_cards::z80::{Z80Bus, Z80};
use apple2_cards::SoftCard;

struct Ram(Vec<u8>);

impl Z80Bus for Ram {
    fn read(&mut self, a: u16) -> u8 {
        self.0[a as usize]
    }
    fn write(&mut self, a: u16, v: u8) {
        self.0[a as usize] = v;
    }
}

fn ram(code: &[(u16, &[u8])]) -> Ram {
    let mut r = Ram(vec![0; 0x10000]);
    for (at, c) in code {
        r.0[*at as usize..*at as usize + c.len()].copy_from_slice(c);
    }
    r
}

fn cpu(pc: u16) -> Z80 {
    Z80 {
        pc,
        sp: 0x8000,
        ..Default::default()
    }
}

#[test]
fn irq_is_ignored_while_interrupts_are_disabled() {
    let mut r = ram(&[(0, &[0, 0])]);
    let mut z = cpu(0);
    z.irq(true);
    assert_eq!(z.step(&mut r), 4);
    assert_eq!(z.pc, 1);
}

#[test]
fn im1_irq_pushes_pc_and_jumps_to_0038() {
    let mut r = ram(&[(0x100, &[0])]);
    let mut z = cpu(0x100);
    z.iff1 = true;
    z.iff2 = true;
    z.im = 1;
    z.irq(true);
    assert_eq!(z.step(&mut r), 13);
    assert_eq!((z.pc, z.sp, z.iff1, z.iff2), (0x38, 0x7FFE, false, false));
    assert_eq!((r.0[0x7FFE], r.0[0x7FFF]), (0x00, 0x01));
}

#[test]
fn im2_uses_the_vector_table() {
    let mut r = ram(&[(0x2010, &[0x34, 0x12])]);
    let mut z = cpu(0x100);
    z.iff1 = true;
    z.im = 2;
    z.i = 0x20;
    z.irq_data = 0x10;
    z.irq(true);
    assert_eq!(z.step(&mut r), 19);
    assert_eq!(z.pc, 0x1234);
}

#[test]
fn im0_executes_rst_from_the_data_bus() {
    let mut r = ram(&[]);
    let mut z = cpu(0x100);
    z.iff1 = true;
    z.im = 0;
    z.irq_data = 0xD7; // RST 10h
    z.irq(true);
    assert_eq!(z.step(&mut r), 13);
    assert_eq!(z.pc, 0x10);
}

#[test]
fn ei_delays_the_interrupt_by_one_instruction() {
    // EI; NOP; NOP with the line already asserted: taken only after the NOP.
    let mut r = ram(&[(0, &[0xFB, 0x00, 0x00])]);
    let mut z = cpu(0);
    z.im = 1;
    z.irq(true);
    z.step(&mut r); // EI
    assert_eq!(z.pc, 1);
    z.step(&mut r); // NOP runs, no interrupt yet
    assert_eq!(z.pc, 2);
    assert_eq!(z.step(&mut r), 13);
    assert_eq!(z.pc, 0x38);
}

#[test]
fn di_blocks_the_irq_but_not_the_nmi() {
    let mut r = ram(&[(0x100, &[0xF3, 0x00])]);
    let mut z = cpu(0x100);
    z.iff1 = true;
    z.iff2 = true;
    z.step(&mut r); // DI
    z.irq(true);
    z.step(&mut r);
    assert_eq!(z.pc, 0x102);
    z.nmi();
    assert_eq!(z.step(&mut r), 11);
    assert_eq!(z.pc, 0x66);
}

#[test]
fn nmi_keeps_iff2_and_retn_restores_iff1() {
    // NMI: IFF1 cleared, IFF2 keeps the old IFF1; RETN copies IFF2 back into IFF1.
    let mut r = ram(&[(0x66, &[0xED, 0x45])]);
    let mut z = cpu(0x300);
    z.iff1 = true;
    z.iff2 = true;
    z.nmi();
    z.step(&mut r);
    assert_eq!((z.pc, z.iff1, z.iff2), (0x66, false, true));
    z.step(&mut r); // RETN
    assert_eq!((z.pc, z.iff1), (0x300, true));
}

#[test]
fn nmi_is_an_edge_and_served_once() {
    let mut r = ram(&[(0x66, &[0xED, 0x45])]);
    let mut z = cpu(0x300);
    z.nmi();
    z.step(&mut r);
    z.step(&mut r);
    assert_eq!(z.pc, 0x300);
}

#[test]
fn halt_waits_for_the_interrupt_counting_nop_cycles() {
    let mut r = ram(&[(0x100, &[0x76, 0x00])]);
    let mut z = cpu(0x100);
    z.iff1 = true;
    z.im = 1;
    assert_eq!(z.step(&mut r), 4);
    assert!(z.halted);
    for _ in 0..5 {
        assert_eq!(z.step(&mut r), 4);
        assert!(z.halted);
    }
    z.irq(true);
    assert_eq!(z.step(&mut r), 13);
    assert!(!z.halted);
    assert_eq!(z.pc, 0x38);
    // The pushed return address is the instruction after HALT.
    assert_eq!((r.0[0x7FFE], r.0[0x7FFF]), (0x01, 0x01));
}

#[test]
fn nmi_wakes_a_halted_cpu_even_with_interrupts_disabled() {
    let mut r = ram(&[(0x100, &[0x76])]);
    let mut z = cpu(0x100);
    z.step(&mut r);
    z.nmi();
    assert_eq!(z.step(&mut r), 11);
    assert_eq!(z.pc, 0x66);
}

#[test]
fn reti_returns_from_an_interrupt_handler() {
    let mut r = ram(&[(0x38, &[0xFB, 0xED, 0x4D])]);
    let mut z = cpu(0x500);
    z.iff1 = true;
    z.im = 1;
    z.irq(true);
    z.step(&mut r);
    z.irq(false);
    z.step(&mut r); // EI
    z.step(&mut r); // RETI
    assert_eq!(z.pc, 0x500);
}

#[test]
fn undefined_ed_opcodes_are_counted_not_silent() {
    let mut r = ram(&[(0, &[0xED, 0x00, 0xED, 0x00, 0xED, 0xFF, 0xED, 0x77])]);
    let mut z = cpu(0);
    for _ in 0..4 {
        assert_eq!(z.step(&mut r), 8);
    }
    assert_eq!(z.undefined_ed_total(), 4);
    assert_eq!(
        z.undefined_ed_report(),
        vec![(0x00, 2), (0x77, 1), (0xFF, 1)]
    );
}

#[test]
fn defined_ed_opcodes_are_not_reported() {
    let mut r = ram(&[(0, &[0xED, 0x44, 0xED, 0xA0])]);
    let mut z = cpu(0);
    z.step(&mut r);
    z.step(&mut r);
    assert_eq!(z.undefined_ed_total(), 0);
}

/// The SoftCard exposes no IRQ/NMI line (AppleWin parity): nothing is delivered unless a caller
/// drives the core directly.
#[test]
fn softcard_does_not_wire_the_interrupt_lines() {
    let card = SoftCard::new();
    let z = card.z80().expect("z80");
    assert!(!z.irq_line && !z.nmi_pending);
}
