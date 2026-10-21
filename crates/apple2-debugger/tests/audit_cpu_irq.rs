//! #1356 (HU-0101): with a breakpoint set the debugger's stepping loop must deliver the IRQ exactly
//! as the plain CPU loop does (AppleWin 1.30.20 hung on the Total Replay boot with `bp 106` and an
//! active Mockingboard IRQ). The reference is the plain `Machine::step` loop, which follows
//! CPU.cpp `IRQ()`.
use apple2_core::{IrqSource, Machine, MachineConfig, Model};
use apple2_debugger::Debugger;

/// Custom ROM of NOPs: RESET=$0300, IRQ=$0400; $0300 = CLI + NOP sled + JMP $0301, handler
/// `INC $10 : RTI`.
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
    for (i, b) in [0x58u8, 0xEA, 0xEA, 0xEA, 0x4C, 0x01, 0x03]
        .into_iter()
        .enumerate()
    {
        m.poke(0x300 + i as u16, b);
    }
    for (i, b) in [0xE6u8, 0x10, 0x40].into_iter().enumerate() {
        m.poke(0x400 + i as u16, b);
    }
    m.board_mut().interrupts.set_irq(IrqSource::Mouse, true); // never acknowledged
    m
}

#[test]
fn issue_1356_breakpoint_does_not_disturb_irq_delivery() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut plain = machine(model);
        let mut done = 0;
        while done < 30_000 {
            done += plain.step() as u64;
        }

        let mut m = machine(model);
        let mut d = Debugger::new();
        d.initialize(&mut m);
        d.enter(&mut m);
        d.execute(&mut m, "bp 106"); // never executed
        d.execute(&mut m, "g");
        assert!(d.is_stepping());
        let mut spent = 0;
        while spent < 30_000 {
            let n = d.run(&mut m, 30_000 - spent);
            assert!(n > 0, "{model:?}: the debugger made no progress (hang)");
            spent += n;
        }
        assert_eq!(
            m.peek(0x10),
            plain.peek(0x10),
            "{model:?}: same number of IRQs taken"
        );
        assert!(
            plain.peek(0x10) > 100,
            "{model:?}: the IRQ really fires repeatedly"
        );
        assert_eq!(m.cpu().pc, plain.cpu().pc, "{model:?}");
        assert_eq!(m.cpu().cycles, plain.cpu().cycles, "{model:?}");
    }
}
