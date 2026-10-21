//! AppleMouse II card driven through its real firmware entry points on a ][+ (slot 4).

use apple2_cards::{mouse_button, mouse_move, MouseCard};
use apple2_core::{Card, Machine, MachineConfig, Model};
use apple2_cpu::Bus;

const SLOT: u8 = 4;

fn machine() -> Machine {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2Plus));
    assert!(m.insert_card(SLOT, Box::new(MouseCard::new())));
    m
}

/// Calls firmware entry `entry` (the offset byte index, $12 SETMOUSE ... $19 INITMOUSE) with A=`a`.
fn call(m: &mut Machine, entry: u8, a: u8) {
    let code = [
        0xA2, 0xC4, 0xA0, 0x40, 0xA9, a, 0x20, 0x20, 0x03, 0x4C, 0x09, 0x03,
    ];
    for (i, b) in code.iter().enumerate() {
        m.poke(0x300 + i as u16, *b);
    }
    // PHA; LDA $C4xx; STA $0329; PLA; JMP $C400 (low byte patched)
    let sub = [
        0x48, 0xAD, entry, 0xC4, 0x8D, 0x29, 0x03, 0x68, 0x4C, 0x00, 0xC4,
    ];
    for (i, b) in sub.iter().enumerate() {
        m.poke(0x320 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
    for _ in 0..200_000 {
        if m.cpu().pc == 0x309 {
            break;
        }
        m.step();
    }
    assert_eq!(m.cpu().pc, 0x309, "firmware call returned");
}

fn hole(m: &Machine, base: u16) -> u8 {
    m.peek(base + SLOT as u16)
}

fn card(m: &mut Machine) -> (i32, i32, u8, u8) {
    m.with_card(SLOT, |c, _| {
        let mc = c.as_any_mut().unwrap().downcast_mut::<MouseCard>().unwrap();
        (mc.position().0, mc.position().1, mc.mode(), mc.status())
    })
    .unwrap()
}

#[test]
fn firmware_identifies_as_applemouse() {
    let mut m = machine();
    for (a, v) in [
        (0xC405, 0x38),
        (0xC407, 0x18),
        (0xC40B, 0x01),
        (0xC40C, 0x20),
        (0xC4FB, 0xD6),
    ] {
        assert_eq!(m.board_mut().read(a), v, "{a:#x}");
    }
}

#[test]
fn init_set_move_read_reports_position_in_screen_holes() {
    let mut m = machine();
    call(&mut m, 0x19, 0); // INITMOUSE
    call(&mut m, 0x12, 0x01); // SETMOUSE: on, no interrupts
    assert_eq!(card(&mut m).2, 1);
    assert!(mouse_move(&mut m, SLOT, 300, 5));
    call(&mut m, 0x14, 0); // READMOUSE
    assert_eq!(
        (hole(&m, 0x478), hole(&m, 0x578)),
        (300u16 as u8, 1),
        "X lo/hi"
    );
    assert_eq!((hole(&m, 0x4F8), hole(&m, 0x5F8)), (5, 0), "Y lo/hi");
    assert_eq!(hole(&m, 0x778) & 0x20, 0x20, "moved since last read");
    call(&mut m, 0x14, 0);
    assert_eq!(hole(&m, 0x778) & 0x20, 0, "no move since");
}

#[test]
fn clamp_limits_host_movement() {
    let mut m = machine();
    call(&mut m, 0x19, 0);
    call(&mut m, 0x12, 0x01);
    // CLAMPMOUSE X: A=0; limits in the fixed screen holes ($0478 min lo, $04F8 max lo, $0578 min hi, $05F8 max hi).
    for (a, v) in [(0x478, 10), (0x4F8, 100), (0x578, 0), (0x5F8, 0)] {
        m.poke_main(a, v);
    }
    call(&mut m, 0x17, 0);
    assert!(mouse_move(&mut m, SLOT, 500, 0));
    assert_eq!(card(&mut m).0, 100);
    mouse_move(&mut m, SLOT, -500, 0);
    assert_eq!(card(&mut m).0, 10);
}

#[test]
fn buttons_show_in_status_and_previous_state() {
    let mut m = machine();
    call(&mut m, 0x19, 0);
    call(&mut m, 0x12, 0x01);
    mouse_button(&mut m, SLOT, 0, true);
    call(&mut m, 0x14, 0);
    assert_eq!(hole(&m, 0x778) & 0xC0, 0x80, "down now, up before");
    call(&mut m, 0x14, 0);
    assert_eq!(hole(&m, 0x778) & 0xC0, 0xC0, "down now and before");
    mouse_button(&mut m, SLOT, 0, false);
    call(&mut m, 0x14, 0);
    assert_eq!(hole(&m, 0x778) & 0xC0, 0x40, "up now, down before");
}

#[test]
fn vbl_interrupt_asserts_irq_until_servemouse() {
    let mut m = machine();
    call(&mut m, 0x19, 0);
    call(&mut m, 0x12, 0x09); // on + VBL interrupt
    assert!(!m.board().interrupts.irq_line());
    m.run_frame();
    m.run_frame();
    assert!(m.board().interrupts.irq_line(), "VBL asserted IRQ");
    assert_ne!(card(&mut m).3 & 0x08, 0, "VBL status bit");
    call(&mut m, 0x13, 0); // SERVEMOUSE
    assert!(!m.board().interrupts.irq_line(), "SERVEMOUSE released it");
}

#[test]
fn movement_interrupt_only_when_enabled_in_mode() {
    let mut m = machine();
    call(&mut m, 0x19, 0);
    call(&mut m, 0x12, 0x01);
    mouse_move(&mut m, SLOT, 1, 0);
    assert!(!m.board().interrupts.irq_line());
    call(&mut m, 0x12, 0x03); // movement interrupts on
    mouse_move(&mut m, SLOT, 1, 0);
    assert!(m.board().interrupts.irq_line());
}

#[test]
fn save_and_load_roundtrip() {
    let mut m = machine();
    call(&mut m, 0x19, 0);
    call(&mut m, 0x12, 0x01);
    mouse_move(&mut m, SLOT, 42, 7);
    let st = m.save_state();
    mouse_move(&mut m, SLOT, 100, 100);
    m.load_state(&st).unwrap();
    assert_eq!(card(&mut m).0, 42);
    let mut c = MouseCard::new();
    assert!(!c.load(&[1, 2, 3]));
}
