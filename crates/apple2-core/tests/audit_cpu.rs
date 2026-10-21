//! HU-0101: regressions for the CPU/timing issues of `docs/conformance/wave1-candidates.md` that
//! need the whole Machine (models, interrupts, I/O timing, video scanner). Expected values are
//! AppleWin's C++ at the frozen SHA (ADR 0004); each test cites the line. Results are recorded in
//! `docs/conformance/results/cpu.csv`.
use apple2_core::card::{Card, CardCtx};
use apple2_core::machine::EventAction;
use apple2_core::{EventId, IrqSource, Machine, MachineConfig, Model, VideoStandard};
use apple2_cpu::Variant;

const SRC: IrqSource = IrqSource::Mouse;

/// Machine with a 12K custom ROM: RESET=$0300, IRQ=$0400, NMI=$0500; $0300 = CLI then NOPs; the
/// IRQ handler is `INC $10 : JMP $0402`.
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

// ---- #307: every Pravets clone is a 6502 -------------------------------------------------------
// CPU.cpp:162-166 `IsCpu65C02`: only the Enhanced //e, the TK3000 //e and the //c (GH#307).

#[test]
fn issue_307_pravets_models_are_nmos_6502() {
    for model in [
        Model::Pravets82,
        Model::Pravets8M,
        Model::Pravets8A,
        Model::BadPravets82,
        Model::BadPravets8M,
        Model::Base64A,
    ] {
        assert_eq!(model.cpu_variant(), Variant::Nmos6502, "{model:?}");
    }
    for (model, v) in [
        (Model::Apple2, Variant::Nmos6502),
        (Model::Apple2Plus, Variant::Nmos6502),
        (Model::Apple2e, Variant::Nmos6502),
        (Model::Apple2eEnhanced, Variant::Cmos65C02),
        (Model::Tk3000e, Variant::Cmos65C02),
        (Model::Apple2c, Variant::Cmos65C02),
    ] {
        assert_eq!(model.cpu_variant(), v, "{model:?}");
    }
    // The machine really runs NMOS opcodes: STZ zp ($64) is a NOP and INC A ($1A) a 2-cycle NOP.
    for model in [Model::Pravets82, Model::Pravets8M, Model::Pravets8A] {
        let mut m = Machine::new(MachineConfig::for_model(model));
        assert_eq!(m.cpu().variant, Variant::Nmos6502, "{model:?}");
        m.poke_main(0x10, 0x55);
        for (i, b) in [0x64u8, 0x10, 0x1A].into_iter().enumerate() {
            m.poke_main(0x6000 + i as u16, b);
        }
        m.cpu_mut().pc = 0x6000;
        m.cpu_mut().a = 7;
        assert_eq!(m.step(), 3, "{model:?} NOP zp");
        assert_eq!(m.peek_main(0x10), 0x55, "{model:?}: no STZ");
        assert_eq!(m.step(), 2, "{model:?} $1A");
        assert_eq!(m.cpu().a, 7, "{model:?}: no INC A");
    }
}

// ---- #321: the I/O handler of a 4-cycle instruction sees the opcode's first cycle -------------
// cpu_general.inl:70-78 `_READ`: `IORead[...](regs.pc, addr, 0, 0, uExecutedCycles)` runs before
// `CYC(4)` adds the opcode's cycles. AppleWin still behaves this way (the issue is open upstream),
// so the oracle says: the handler gets the cycle count at the start of the instruction.

struct Probe {
    seen: Vec<(u64, u64)>,
}

impl Card for Probe {
    fn name(&self) -> &'static str {
        "Probe"
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
    fn io_read(&mut self, _lo: u8, ctx: &mut CardCtx) -> u8 {
        self.seen.push((ctx.access_cycle, ctx.cycles));
        0
    }
}

#[test]
fn issue_321_io_access_is_stamped_with_the_start_of_the_opcode_as_applewin() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = Machine::new(MachineConfig::for_model(model));
        assert!(m.insert_card(4, Box::new(Probe { seen: vec![] })));
        // LDA $C0C0 (4 cycles) twice, with a NOP between
        for (i, b) in [0xADu8, 0xC0, 0xC0, 0xEA, 0xAD, 0xC0, 0xC0]
            .into_iter()
            .enumerate()
        {
            m.poke_main(0x6000 + i as u16, b);
        }
        m.cpu_mut().pc = 0x6000;
        let t0 = m.cycles();
        assert_eq!(m.step(), 4);
        m.step();
        m.step();
        let seen = &m.card::<Probe>(4).unwrap().seen;
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0, t0, "{model:?}: opcode start, not the 4th cycle");
        assert_eq!(seen[1].0, t0 + 4 + 2, "{model:?}");
        assert!(
            seen[0].1 >= seen[0].0,
            "{model:?}: the absolute counter never goes back"
        );
    }
}

// ---- #366: the extra cycles of the previous opcode are not charged again to an IRQ -----------
// CPU.cpp:448-505 `IRQ()`: `UINT uExtraCycles = 0; // Needed for CYC(a) macro` then `CYC(7)`.

#[test]
fn issue_366_irq_after_a_page_crossing_opcode_costs_exactly_seven_cycles() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model);
        m.step(); // CLI
        m.poke(0x301, 0xBD); // LDA $20F0,X with X = $20: crosses (5 cycles)
        m.poke(0x302, 0xF0);
        m.poke(0x303, 0x20);
        m.cpu_mut().x = 0x20;
        assert_eq!(m.step(), 5, "{model:?} LDA abs,X crossing");
        m.board_mut().interrupts.set_irq(SRC, true);
        assert_eq!(m.step(), 7, "{model:?} IRQ entry");
        assert_eq!(m.cpu().pc, 0x400);
        // and another crossing opcode after the handler returns does not inherit anything either
        m.board_mut().interrupts.set_irq(SRC, false);
        assert_eq!(m.step(), 5, "{model:?} INC $10");
    }
}

// ---- #651: the IRQ is taken at instruction boundaries, deferred when it appears on the last
// cycle -------------------------------------------------------------------------------------------
// AppleWin is not cycle-exact inside an instruction: CPU.cpp:448-505 and :208-212. An IRQ source
// that goes active during cycle k of an n-cycle opcode is taken before the next opcode, except
// k == n (the last cycle), which defers it by exactly one opcode.

#[test]
fn issue_651_irq_inside_a_six_cycle_instruction_per_cycle() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        for k in 0..=6i32 {
            let mut m = machine(model);
            m.poke(0x301, 0xEE); // INC $0010 (6 cycles)
            m.poke(0x302, 0x10);
            m.poke(0x303, 0x00);
            m.step(); // CLI
            if k == 0 {
                m.board_mut().interrupts.set_irq(SRC, true);
            } else {
                m.schedule(EventId(1), k, EventAction::RaiseIrq(SRC));
            }
            let first = m.step();
            if k == 0 {
                // already asserted before the opcode: taken instead of it
                assert_eq!((first, m.cpu().pc), (7, 0x400), "{model:?} k=0");
                continue;
            }
            assert_eq!(
                (first, m.cpu().pc),
                (6, 0x304),
                "{model:?} k={k}: INC completes"
            );
            if k == 6 {
                assert_eq!(
                    m.step(),
                    2,
                    "{model:?} k=6: deferred by one opcode (a NOP runs)"
                );
                assert_eq!(m.cpu().pc, 0x305);
            }
            assert_eq!(m.step(), 7, "{model:?} k={k}: IRQ entry");
            assert_eq!(m.cpu().pc, 0x400, "{model:?} k={k}");
            assert_eq!(m.peek(0x10), 1, "{model:?} k={k}: INC ran exactly once");
        }
    }
}

// ---- #1303: an IRQ source that is never cleared does not disturb code running under SEI --------
// CPU.cpp:452 `if (g_bmIRQ && !(regs.ps & AF_INTERRUPT))`: with I set the line is ignored, however
// long it stays asserted, on the 6502 and on the 65C02.

#[test]
fn issue_1303_masked_irq_never_taken_under_sei() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model);
        for (i, b) in [
            0x78u8, // SEI
            0xA9, 0x00, // LDA #0
            0xEA, 0xEA, 0xEA, 0xEA, 0xEA, 0xEA, // NOP x6
            0xA9, 0x01, // LDA #1
            0x4C, 0x0B, 0x03, // JMP *
        ]
        .into_iter()
        .enumerate()
        {
            m.poke(0x300 + i as u16, b);
        }
        m.schedule(EventId(1), 6, EventAction::RaiseIrq(SRC)); // "T1 expires", never acknowledged
        m.run_cycles(200);
        assert_eq!(m.cpu().a, 1, "{model:?}: the snippet ran to its end");
        assert_eq!(m.peek(0x10), 0, "{model:?}: the IRQ handler never ran");
        assert!(m.board().interrupts.irq_line());
        // CLI at the end: the pending IRQ is now taken (once the deferral rule allows)
        m.poke(0x30B, 0x58); // CLI
        m.poke(0x30C, 0xEA);
        m.poke(0x30D, 0xEA);
        m.cpu_mut().pc = 0x30B;
        m.run_cycles(40);
        assert!(m.peek(0x10) > 0, "{model:?}: handler runs after CLI");
    }
}

// ---- #508, #519: waiting for the II+ video sync through the floating bus ends ------------------
// Canyon Climber (`LDA $C050 / BNE`) waits for the floating bus to read 0, which only happens
// while the scanner is outside the text/graphics page (vertical blank). Memory.cpp
// `MemReadFloatingBus` + the video clock advanced per opcode (cpu6502.h:382). There is no separate
// "full speed" CPU path in the port: the loop must end within two frames, whatever the speed.

#[test]
fn issue_508_519_floating_bus_vbl_wait_terminates_on_the_ii_plus() {
    let mut cfg = MachineConfig::for_model(Model::Apple2Plus);
    cfg.video_standard = VideoStandard::Ntsc;
    let mut m = Machine::new(cfg);
    // Non-zero everywhere the scanner reads while it draws, zero elsewhere.
    for a in 0x0400..0x0800u16 {
        m.poke_main(a, 0xA0);
    }
    for a in 0x2000..0x4000u16 {
        m.poke_main(a, 0xA0);
    }
    // $6000: LDA $C050 ; BNE $6000 ; LDX #$77 ; JMP *
    for (i, b) in [0xADu8, 0x50, 0xC0, 0xD0, 0xFB, 0xA2, 0x77, 0x4C, 0x07, 0x60]
        .into_iter()
        .enumerate()
    {
        m.poke_main(0x6000 + i as u16, b);
    }
    m.cpu_mut().pc = 0x6000;
    m.cpu_mut().x = 0;
    let mut spent = 0u64;
    let mut last_lda_frame_cycle = 0;
    while m.cpu().x != 0x77 && spent < 2 * 17030 {
        if m.cpu().pc == 0x6000 {
            last_lda_frame_cycle = m.cycles_in_frame();
        }
        spent += m.step() as u64;
    }
    assert_eq!(
        m.cpu().x,
        0x77,
        "the wait ended within two frames ({spent} cycles)"
    );
    assert!(
        last_lda_frame_cycle >= 192 * 65 || last_lda_frame_cycle <= 2 * 65,
        "the last read started in the vertical blank (frame cycle {last_lda_frame_cycle})"
    );
}
