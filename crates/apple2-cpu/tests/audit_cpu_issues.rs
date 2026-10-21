//! HU-0101: one regression per CPU issue of `docs/conformance/wave1-candidates.md` ("CPU e
//! temporização") that can be decided on the CPU core alone. The expected values are the ones of
//! AppleWin's C++ at the frozen SHA (the oracle, ADR 0004), with the line cited in each test;
//! the cycle counts are the `CYC(n)` of `CPU/cpu6502.h` / `CPU/cpu65C02.h`.
//! Results are recorded in `docs/conformance/results/cpu.csv`.
use apple2_cpu::*;

struct Ram {
    mem: Box<[u8; 65536]>,
    reads: Vec<u16>,
    writes: Vec<(u16, u8)>,
    ticks: u32,
}

impl Bus for Ram {
    fn read(&mut self, a: u16) -> u8 {
        self.reads.push(a);
        self.mem[a as usize]
    }
    fn write(&mut self, a: u16, v: u8) {
        self.writes.push((a, v));
        self.mem[a as usize] = v;
    }
    fn tick(&mut self) {
        self.ticks += 1;
    }
}

const N: Variant = Variant::Nmos6502;
const C: Variant = Variant::Cmos65C02;

/// Runs one instruction placed at `at` after `setup`; returns (cpu, ram, cycles).
fn run_at(
    v: Variant,
    at: u16,
    code: &[u8],
    setup: impl FnOnce(&mut Cpu, &mut Ram),
) -> (Cpu, Ram, u32) {
    let mut r = Ram {
        mem: Box::new([0; 65536]),
        reads: vec![],
        writes: vec![],
        ticks: 0,
    };
    for (i, b) in code.iter().enumerate() {
        r.mem[at as usize + i] = *b;
    }
    let mut c = Cpu::new(v);
    (c.a, c.x, c.y) = (0, 0, 0);
    c.pc = at;
    c.p = 0;
    setup(&mut c, &mut r);
    r.reads.clear();
    r.writes.clear();
    r.ticks = 0;
    let n = c.step(&mut r);
    assert_eq!(r.ticks, n, "every cycle ticks the bus exactly once");
    (c, r, n)
}

fn run(v: Variant, code: &[u8], setup: impl FnOnce(&mut Cpu, &mut Ram)) -> (Cpu, Ram, u32) {
    run_at(v, 0x300, code, setup)
}

// ---- #264: JMP (ind) is 5 cycles on the NMOS 6502, 6 on the 65C02 ----------------------------
// cpu6502.h:228 `IABS_NMOS JMP CYC(5)` (GH#264); cpu65C02.h:228 `IABS_CMOS JMP CYC(6)`.

#[test]
fn issue_264_jmp_indirect_cycles_per_cpu() {
    let setup = |_: &mut Cpu, r: &mut Ram| {
        r.mem[0x1000] = 0x34;
        r.mem[0x1001] = 0x12;
    };
    let (c, _, n) = run(N, &[0x6C, 0x00, 0x10], setup);
    assert_eq!((n, c.pc), (5, 0x1234), "NMOS");
    let (c, _, n) = run(C, &[0x6C, 0x00, 0x10], setup);
    assert_eq!((n, c.pc), (6, 0x1234), "65C02");
    // Pointer on the last byte of a page: NMOS wraps inside the page (still 5 cycles); the
    // 65C02 reads the next page and AppleWin charges one more cycle (`_IABS_CMOS`,
    // cpu_general.inl:200-203).
    let setup = |_: &mut Cpu, r: &mut Ram| {
        r.mem[0x10FF] = 0x34;
        r.mem[0x1000] = 0x12;
        r.mem[0x1100] = 0x56;
    };
    let (c, _, n) = run(N, &[0x6C, 0xFF, 0x10], setup);
    assert_eq!((n, c.pc), (5, 0x1234), "NMOS page wrap");
    let (c, _, n) = run(C, &[0x6C, 0xFF, 0x10], setup);
    assert_eq!((n, c.pc), (7, 0x5634), "65C02 page straddle");
}

// ---- #271, #278, #801: abs,X read-modify-write timing ----------------------------------------
// NMOS: cpu6502.h `ABSX_CONST` ASL/ROL/LSR/ROR (:150,:182,:214,:246) CYC(7), DEC $DE (:342) and
// INC $FE (:374) CYC(7). 65C02: shifts/rotates `ABSX_OPT` CYC(6) (+1 on a page cross), DEC/INC
// still `ABSX_CONST` CYC(7). The page penalty is decided after the address is formed (#278).

#[test]
fn issue_271_278_801_abs_x_rmw_cycles() {
    // (opcode, NMOS cycles, 65C02 cycles without cross, 65C02 cycles with cross)
    let table = [
        (0x1E, 7, 6, 7), // ASL
        (0x3E, 7, 6, 7), // ROL
        (0x5E, 7, 6, 7), // LSR
        (0x7E, 7, 6, 7), // ROR
        (0xDE, 7, 7, 7), // DEC
        (0xFE, 7, 7, 7), // INC
    ];
    for (op, nmos, cmos, cmos_cross) in table {
        for (x, cross) in [(0x01u8, false), (0xFF, true)] {
            let setup = |c: &mut Cpu, r: &mut Ram| {
                c.x = x;
                r.mem[0x2010 + x as usize] = 0x40;
            };
            let (_, r, n) = run(N, &[op, 0x10, 0x20], setup);
            assert_eq!(n, nmos, "NMOS {op:02X} cross={cross}");
            assert_eq!(r.writes.len(), 1, "one write, no dummy write");
            let (_, _, n) = run(C, &[op, 0x10, 0x20], setup);
            assert_eq!(
                n,
                if cross { cmos_cross } else { cmos },
                "65C02 {op:02X} cross={cross}"
            );
        }
    }
}

// ---- #288: DEC abs is 6 cycles on both CPUs ----------------------------------------------------
// cpu6502.h:326 / cpu65C02.h:326 `ABS DEC CYC(6)`.

#[test]
fn issue_288_dec_abs_is_six_cycles() {
    for v in [N, C] {
        for addr in [0x2000u16, 0x20FF, 0x2100] {
            let (_, r, n) = run(v, &[0xCE, addr as u8, (addr >> 8) as u8], |_, r| {
                r.mem[addr as usize] = 0x10
            });
            assert_eq!(n, 6, "{v:?} DEC {addr:#06x}");
            assert_eq!(r.writes, [(addr, 0x0F)]);
        }
    }
}

// ---- #292: AND abs is 4 cycles; the 65C02 undefined opcodes ------------------------------------
// cpu65C02.h:165 `ABS AND CYC(4)`; the undefined opcodes are the `// invalid` lines of
// cpu65C02.h (length from the addressing mode, cycles from CYC(n)); GH#292.

const INVALID_NMOS: &[(u8, u16, u32)] = &[
    (0x02, 1, 2), // HLT
    (0x03, 2, 8), // ASO
    (0x04, 2, 3), // NOP
    (0x07, 2, 5), // ASO
    (0x0B, 2, 2), // ANC
    (0x0C, 3, 4), // NOP
    (0x0F, 3, 6), // ASO
    (0x12, 1, 2), // HLT
    (0x13, 2, 8), // ASO
    (0x14, 2, 4), // NOP
    (0x17, 2, 6), // ASO
    (0x1A, 1, 2), // NOP
    (0x1B, 3, 7), // ASO
    (0x1C, 3, 4), // NOP
    (0x1F, 3, 7), // ASO
    (0x22, 1, 2), // HLT
    (0x23, 2, 8), // RLA
    (0x27, 2, 5), // RLA
    (0x2B, 2, 2), // ANC
    (0x2F, 3, 6), // RLA
    (0x32, 1, 2), // HLT
    (0x33, 2, 8), // RLA
    (0x34, 2, 4), // NOP
    (0x37, 2, 6), // RLA
    (0x3A, 1, 2), // NOP
    (0x3B, 3, 7), // RLA
    (0x3C, 3, 4), // NOP
    (0x3F, 3, 7), // RLA
    (0x42, 1, 2), // HLT
    (0x43, 2, 8), // LSE
    (0x44, 2, 3), // NOP
    (0x47, 2, 5), // LSE
    (0x4B, 2, 2), // ALR
    (0x4F, 3, 6), // LSE
    (0x52, 1, 2), // HLT
    (0x53, 2, 8), // LSE
    (0x54, 2, 4), // NOP
    (0x57, 2, 6), // LSE
    (0x5A, 1, 2), // NOP
    (0x5B, 3, 7), // LSE
    (0x5C, 3, 4), // NOP
    (0x5F, 3, 7), // LSE
    (0x62, 1, 2), // HLT
    (0x63, 2, 8), // RRA
    (0x64, 2, 3), // NOP
    (0x67, 2, 5), // RRA
    (0x6B, 2, 2), // ARR
    (0x6F, 3, 6), // RRA
    (0x72, 1, 2), // HLT
    (0x73, 2, 8), // RRA
    (0x74, 2, 4), // NOP
    (0x77, 2, 6), // RRA
    (0x7A, 1, 2), // NOP
    (0x7B, 3, 7), // RRA
    (0x7C, 3, 4), // NOP
    (0x7F, 3, 7), // RRA
    (0x80, 2, 2), // NOP
    (0x82, 2, 2), // NOP
    (0x83, 2, 6), // AXS
    (0x87, 2, 3), // AXS
    (0x89, 2, 2), // NOP
    (0x8B, 2, 2), // XAA
    (0x8F, 3, 4), // AXS
    (0x92, 1, 2), // HLT
    (0x93, 2, 6), // AXA
    (0x97, 2, 4), // AXS
    (0x9B, 3, 5), // TAS
    (0x9C, 3, 5), // SAY
    (0x9E, 3, 5), // XAS
    (0x9F, 3, 5), // AXA
    (0xA3, 2, 6), // LAX
    (0xA7, 2, 3), // LAX
    (0xAB, 2, 2), // OAL
    (0xAF, 3, 4), // LAX
    (0xB2, 1, 2), // HLT
    (0xB3, 2, 5), // LAX
    (0xB7, 2, 4), // LAX
    (0xBB, 3, 4), // LAS
    (0xBF, 3, 4), // LAX
    (0xC2, 2, 2), // NOP
    (0xC3, 2, 8), // DCM
    (0xC7, 2, 5), // DCM
    (0xCB, 2, 2), // SAX
    (0xCF, 3, 6), // DCM
    (0xD2, 1, 2), // HLT
    (0xD3, 2, 8), // DCM
    (0xD4, 2, 4), // NOP
    (0xD7, 2, 6), // DCM
    (0xDA, 1, 2), // NOP
    (0xDB, 3, 7), // DCM
    (0xDC, 3, 4), // NOP
    (0xDF, 3, 7), // DCM
    (0xE2, 2, 2), // NOP
    (0xE3, 2, 8), // INS
    (0xE7, 2, 5), // INS
    (0xEB, 2, 2), // SBCn
    (0xEF, 3, 6), // INS
    (0xF2, 1, 2), // HLT
    (0xF3, 2, 8), // INS
    (0xF4, 2, 4), // NOP
    (0xF7, 2, 6), // INS
    (0xFA, 1, 2), // NOP
    (0xFB, 3, 7), // INS
    (0xFC, 3, 4), // NOP
    (0xFF, 3, 7), // INS
];
const INVALID_CMOS: &[(u8, u16, u32)] = &[
    (0x02, 2, 2), // NOP
    (0x03, 1, 1), // NOP
    (0x07, 1, 1), // NOP
    (0x0B, 1, 1), // NOP
    (0x0F, 1, 1), // NOP
    (0x13, 1, 1), // NOP
    (0x17, 1, 1), // NOP
    (0x1B, 1, 1), // NOP
    (0x1F, 1, 1), // NOP
    (0x22, 2, 2), // NOP
    (0x23, 1, 1), // NOP
    (0x27, 1, 1), // NOP
    (0x2B, 1, 1), // NOP
    (0x2F, 1, 1), // NOP
    (0x33, 1, 1), // NOP
    (0x37, 1, 1), // NOP
    (0x3B, 1, 1), // NOP
    (0x3F, 1, 1), // NOP
    (0x42, 2, 2), // NOP
    (0x43, 1, 1), // NOP
    (0x44, 2, 3), // NOP
    (0x47, 1, 1), // NOP
    (0x4B, 1, 1), // NOP
    (0x4F, 1, 1), // NOP
    (0x53, 1, 1), // NOP
    (0x54, 2, 4), // NOP
    (0x57, 1, 1), // NOP
    (0x5B, 1, 1), // NOP
    (0x5C, 3, 8), // NOP
    (0x5F, 1, 1), // NOP
    (0x62, 2, 2), // NOP
    (0x63, 1, 1), // NOP
    (0x67, 1, 1), // NOP
    (0x6B, 1, 1), // NOP
    (0x6F, 1, 1), // NOP
    (0x73, 1, 1), // NOP
    (0x77, 1, 1), // NOP
    (0x7B, 1, 1), // NOP
    (0x7F, 1, 1), // NOP
    (0x82, 2, 2), // NOP
    (0x83, 1, 1), // NOP
    (0x87, 1, 1), // NOP
    (0x8B, 1, 1), // NOP
    (0x8F, 1, 1), // NOP
    (0x93, 1, 1), // NOP
    (0x97, 1, 1), // NOP
    (0x9B, 1, 1), // NOP
    (0x9F, 1, 1), // NOP
    (0xA3, 1, 1), // NOP
    (0xA7, 1, 1), // NOP
    (0xAB, 1, 1), // NOP
    (0xAF, 1, 1), // NOP
    (0xB3, 1, 1), // NOP
    (0xB7, 1, 1), // NOP
    (0xBB, 1, 1), // NOP
    (0xBF, 1, 1), // NOP
    (0xC2, 2, 2), // NOP
    (0xC3, 1, 1), // NOP
    (0xC7, 1, 1), // NOP
    (0xCB, 1, 1), // NOP
    (0xCF, 1, 1), // NOP
    (0xD3, 1, 1), // NOP
    (0xD4, 2, 4), // NOP
    (0xD7, 1, 1), // NOP
    (0xDB, 1, 1), // NOP
    (0xDC, 3, 4), // LDD
    (0xDF, 1, 1), // NOP
    (0xE2, 2, 2), // NOP
    (0xE3, 1, 1), // NOP
    (0xE7, 1, 1), // NOP
    (0xEB, 1, 1), // NOP
    (0xEF, 1, 1), // NOP
    (0xF3, 1, 1), // NOP
    (0xF4, 2, 4), // NOP
    (0xF7, 1, 1), // NOP
    (0xFB, 1, 1), // NOP
    (0xFC, 3, 4), // LDD
    (0xFF, 1, 1), // NOP
];

#[test]
fn issue_292_and_abs_is_four_cycles() {
    for v in [N, C] {
        let (c, _, n) = run(v, &[0x2D, 0x00, 0x20], |c, r| {
            c.a = 0xF0;
            r.mem[0x2000] = 0x3C;
        });
        assert_eq!((n, c.a), (4, 0x30), "{v:?}");
    }
}

#[test]
fn issue_292_cmos_undefined_opcodes_length_and_cycles_match_applewin() {
    for &(op, len, cycles) in INVALID_CMOS {
        // zero page pointer 0 -> $0000; X = Y = 0: no page cross, no side effect on the code.
        let (c, _, n) = run(C, &[op, 0x00, 0x00], |_, _| {});
        assert_eq!((c.pc, n), (0x300 + len, cycles), "65C02 opcode {op:02X}");
    }
}

#[test]
fn issue_292_nmos_undefined_opcodes_length_and_cycles_match_applewin() {
    for &(op, len, cycles) in INVALID_NMOS {
        if matches!(
            op,
            0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2
        ) {
            continue; // HLT: PC stays on the opcode (cpu_instructions.inl:379-380)
        }
        let (c, _, n) = run(N, &[op, 0x00, 0x00], |_, _| {});
        assert_eq!((c.pc, n), (0x300 + len, cycles), "NMOS opcode {op:02X}");
    }
}

// ---- #503: taken branch at the end of a page ---------------------------------------------------
// cpu_general.inl:140-148 BRANCH_TAKEN: base = pc after the operand; +1 cycle taken, +2 when
// (base ^ target) crosses a page.

#[test]
fn issue_503_branch_taken_page_crossing_matrix() {
    // (address of the branch, offset, expected cycles, expected pc)
    let cases: [(u16, i8, u32, u16); 6] = [
        (0x02FD, 1, 4, 0x0300),  // next = $02FF, target $0300: crosses
        (0x02FD, 0, 3, 0x02FF),  // target = next: no cross
        (0x02FE, 2, 3, 0x0302),  // next = $0300 already in the new page
        (0x0300, -3, 4, 0x02FF), // backwards across the boundary
        (0x02FC, 3, 4, 0x0301),  // forward, lands in the next page
        (0x0301, 0x70, 3, 0x0373),
    ];
    for v in [N, C] {
        for (at, off, cycles, pc) in cases {
            let (c, _, n) = run_at(v, at, &[0xD0, off as u8], |_, _| {});
            assert_eq!((n, c.pc), (cycles, pc), "{v:?} BNE at {at:#06x} off {off}");
        }
        let (c, _, n) = run_at(v, 0x02FD, &[0xD0, 0x01], |c, _| c.p = FLAG_Z);
        assert_eq!((n, c.pc), (2, 0x02FF), "{v:?} not taken");
    }
    // BRA ($80, 65C02 only) uses the same BRANCH_TAKEN
    let (c, _, n) = run_at(C, 0x02FD, &[0x80, 0x01], |_, _| {});
    assert_eq!((n, c.pc), (4, 0x0300));
}

// ---- #967: indexed load crossing a page: no spurious read ---------------------------------------
// The issue asks for the NMOS dummy read at the uncorrected address (visible on $C0xx). AppleWin
// does not do it (`_READ` is a single access, cpu_general.inl:70-78; page cross only adds
// uExtraCycles, :153-155), the issue is still open upstream, and the port follows the oracle.

#[test]
fn issue_967_indexed_load_does_one_read_of_the_effective_address_as_applewin() {
    for v in [N, C] {
        let (c, r, n) = run(v, &[0xBD, 0xF0, 0xC0], |c, r| {
            c.x = 0x20; // $C0F0 + $20 = $C110: crosses
            r.mem[0xC110] = 0x77;
        });
        assert_eq!((c.a, n), (0x77, 5), "{v:?}");
        assert_eq!(
            r.reads,
            [0x300, 0x301, 0x302, 0xC110],
            "{v:?}: no read at $C010"
        );
    }
}

// ---- #1099: D flag on BRK / IRQ / NMI / RESET ----------------------------------------------------
// cpu_instructions.inl:311-338 (`_BRK_NMOS` keeps D, `_BRK_CMOS` clears it), CPU.cpp:419,430
// (IRQ and NMI, GH#1099), CPU.cpp:804-805 (CpuReset).

#[test]
fn issue_1099_decimal_flag_on_brk_irq_nmi_and_reset() {
    for v in [N, C] {
        let keeps_d = v == N;
        let (c, _, _) = run(v, &[0x00, 0xEA], |c, _| c.p = FLAG_D);
        assert_eq!(c.p & FLAG_D != 0, keeps_d, "{v:?} BRK");
        let (c, _, _) = run(v, &[0xEA], |c, _| {
            c.p = FLAG_D;
            c.irq = true;
        });
        assert_eq!(c.p & FLAG_D != 0, keeps_d, "{v:?} IRQ");
        let (mut c, mut r, _) = run(v, &[0xEA], |c, _| c.p = FLAG_D);
        c.set_nmi_pending(true);
        c.step(&mut r);
        assert_eq!(c.p & FLAG_D != 0, keeps_d, "{v:?} NMI");
        let mut c = Cpu::new(v);
        c.p = FLAG_D;
        let mut r = Ram {
            mem: Box::new([0; 65536]),
            reads: vec![],
            writes: vec![],
            ticks: 0,
        };
        c.reset(&mut r);
        assert_eq!(c.p & FLAG_D != 0, keeps_d, "{v:?} RESET");
        assert!(c.p & FLAG_I != 0);
    }
}

// ---- #1257: JSR with the stack overwriting the operand -------------------------------------------
// cpu_instructions.inl:418-422 `_JSR`: the low byte is read, PC incremented, PCH and PCL pushed,
// and only then the high byte read from mem[pc] (GH#1257). A JSR at $0178 with S = $7A pushes
// PCH to $017A, the address of its own high operand byte.

#[test]
fn issue_1257_jsr_reads_the_high_byte_after_the_pushes() {
    for v in [N, C] {
        let (c, r, n) = run_at(v, 0x0178, &[0x20, 0x55, 0x13], |c, _| c.sp = 0x7A);
        assert_eq!(n, 6);
        // PCH = $01 was pushed to $017A over the operand, so the target is $0155, not $1355
        assert_eq!(c.pc, 0x0155, "{v:?}");
        assert_eq!(r.writes, [(0x017A, 0x01), (0x0179, 0x7A)]);
        assert_eq!(c.sp, 0x78);
        // Ordinary JSR still reads the real operand
        let (c, _, _) = run_at(v, 0x0300, &[0x20, 0x55, 0x13], |c, _| c.sp = 0xFF);
        assert_eq!(c.pc, 0x1355);
    }
}

// ---- #1360: NMOS $0C is a 3-byte, 4-cycle NOP abs ------------------------------------------------
// cpu6502.h:132 `ABS NOP CYC(4)` (GH#1360: ABS, not ABS,X); cpu65C02.h:132 is TSB abs, 6 cycles.

#[test]
fn issue_1360_skw_0c_is_abs_nop_on_nmos() {
    let (c, r, n) = run(N, &[0x0C, 0xFF, 0x20], |c, _| c.x = 0xFF);
    assert_eq!(
        (c.pc, n),
        (0x303, 4),
        "no +1 for a page cross: it is not ABS,X"
    );
    assert!(
        r.reads.iter().all(|&a| a < 0x400),
        "never reads the operand address {:x?}",
        r.reads
    );
    assert!(r.writes.is_empty());
    let (c, r, n) = run(C, &[0x0C, 0x00, 0x20], |c, r| {
        c.a = 0x0F;
        r.mem[0x2000] = 0xF0;
    });
    assert_eq!((c.pc, n, r.mem[0x2000]), (0x303, 6, 0xFF), "65C02 TSB abs");
}
