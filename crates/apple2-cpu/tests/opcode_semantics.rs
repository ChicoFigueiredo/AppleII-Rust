//! Per-opcode behaviour of the NMOS undocumented instructions AppleWin implements (cpu6502.h /
//! cpu_instructions.inl), 65C02 specifics, interrupts and the bus-access policy (no false reads).
//! Expected values are worked out by hand from the instruction definitions, not recomputed.
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

fn ram() -> Ram {
    Ram { mem: Box::new([0; 65536]), reads: vec![], writes: vec![], ticks: 0 }
}

/// Runs one instruction at $0300 with `code`, after `setup`.
fn run(v: Variant, code: &[u8], setup: impl FnOnce(&mut Cpu, &mut Ram)) -> (Cpu, Ram, u32) {
    let mut r = ram();
    r.mem[0x300..0x300 + code.len()].copy_from_slice(code);
    let mut c = Cpu::new(v);
    c.pc = 0x300;
    c.p = 0;
    setup(&mut c, &mut r);
    r.reads.clear();
    r.ticks = 0;
    let n = c.step(&mut r);
    assert_eq!(r.ticks, n, "every cycle ticks the bus exactly once");
    (c, r, n)
}

fn flags(c: &Cpu) -> (bool, bool, bool, bool) {
    (c.p & FLAG_N != 0, c.p & FLAG_V != 0, c.p & FLAG_Z != 0, c.p & FLAG_C != 0)
}

// ---------------------------------------------------------------- NMOS undocumented: RMW + ALU

#[test]
fn aso_slo_shifts_memory_left_then_ors_into_a() {
    let (c, r, n) = run(N, &[0x07, 0x10], |c, r| {
        r.mem[0x10] = 0x81;
        c.a = 0x01;
    });
    assert_eq!((r.mem[0x10], c.a, n), (0x02, 0x03, 5));
    assert_eq!(flags(&c), (false, false, false, true));
}

#[test]
fn rla_rotates_memory_left_through_carry_then_ands_into_a() {
    let (c, r, n) = run(N, &[0x27, 0x10], |c, r| {
        r.mem[0x10] = 0x80;
        c.p = FLAG_C;
        c.a = 0xFF;
    });
    assert_eq!((r.mem[0x10], c.a, n), (0x01, 0x01, 5));
    assert!(c.p & FLAG_C != 0);
}

#[test]
fn lse_sre_shifts_memory_right_then_eors_into_a() {
    let (c, r, n) = run(N, &[0x47, 0x10], |c, r| {
        r.mem[0x10] = 0x03;
        c.a = 0x01;
    });
    assert_eq!((r.mem[0x10], c.a, n), (0x01, 0x00, 5));
    assert_eq!(flags(&c), (false, false, true, true));
}

#[test]
fn rra_rotates_memory_right_then_adds_with_the_new_carry() {
    let (c, r, n) = run(N, &[0x67, 0x10], |c, r| {
        r.mem[0x10] = 0x03;
        c.a = 0x10;
    });
    // 0x03 ror (C=0) -> 0x01, C=1; A = 0x10 + 0x01 + 1
    assert_eq!((r.mem[0x10], c.a, n), (0x01, 0x12, 5));
    assert_eq!(flags(&c), (false, false, false, false));
}

#[test]
fn dcm_dcp_decrements_memory_and_compares() {
    let (c, r, n) = run(N, &[0xC7, 0x10], |c, r| {
        r.mem[0x10] = 0x06;
        c.a = 0x05;
    });
    assert_eq!((r.mem[0x10], n), (0x05, 5));
    assert_eq!(flags(&c), (false, false, true, true));
    let (c, r, _) = run(N, &[0xC7, 0x10], |c, r| {
        r.mem[0x10] = 0x05;
        c.a = 0x05;
    });
    assert_eq!(r.mem[0x10], 0x04);
    assert_eq!(flags(&c), (false, false, false, true));
}

#[test]
fn dcm_of_zero_clears_carry_as_applewin_does() {
    // AppleWin keeps `val = READ-1` in a WORD (0xFFFF), so `A >= val` is false even for A=$FF.
    // (Real silicon sets C here.) AppleWin is the oracle.
    let (c, r, _) = run(N, &[0xC7, 0x10], |c, _r| c.a = 0xFF);
    assert_eq!(r.mem[0x10], 0xFF);
    assert!(c.p & FLAG_C == 0);
}

#[test]
fn ins_isc_increments_memory_then_subtracts() {
    let (c, r, n) = run(N, &[0xE7, 0x10], |c, r| {
        r.mem[0x10] = 0x0F;
        c.a = 0x20;
        c.p = FLAG_C;
    });
    assert_eq!((r.mem[0x10], c.a, n), (0x10, 0x10, 5));
    assert_eq!(flags(&c), (false, false, false, true));
}

#[test]
fn rmw_undocumented_addressing_modes_have_fixed_cycle_counts() {
    // (op, code, cycles): (zp,X) 8, zp 5, abs 6, (zp),Y 8 (no PX variant), zp,X 6, abs,Y 7, abs,X 7
    for base in [0x00u8, 0x20, 0x40, 0x60, 0xC0, 0xE0] {
        for (low, code, cycles) in [
            (0x03u8, vec![0x10u8], 8u32),
            (0x07, vec![0x10], 5),
            (0x0F, vec![0x00, 0x20], 6),
            (0x13, vec![0x10], 8),
            (0x17, vec![0x10], 6),
            (0x1B, vec![0x00, 0x20], 7),
            (0x1F, vec![0x00, 0x20], 7),
        ] {
            let mut bytes = vec![base + low];
            bytes.extend(code);
            let (_, _, n) = run(N, &bytes, |c, r| {
                c.x = 1;
                c.y = 0xFF; // forces a page cross where indexed by Y
                r.mem[0x10] = 0x00;
                r.mem[0x11] = 0x20;
                r.mem[0x11 + 1] = 0;
            });
            assert_eq!(n, cycles, "opcode {:02X}", base + low);
        }
    }
}

#[test]
fn rmw_is_one_read_and_one_write_even_on_soft_switches() {
    // INC $C030 on both CPUs: AppleWin reads once and writes once (no double write, no dummy read).
    for v in [N, C] {
        let (_, r, n) = run(v, &[0xEE, 0x30, 0xC0], |_, _| {});
        assert_eq!(n, 6);
        assert_eq!(r.reads.iter().filter(|&&a| a == 0xC030).count(), 1, "{v:?}");
        assert_eq!(r.writes.iter().filter(|&&(a, _)| a == 0xC030).count(), 1, "{v:?}");
        assert_eq!(r.writes.len(), 1);
    }
}

#[test]
fn indexed_stores_do_not_read_the_target_page() {
    // STA $C0xx,X on the 6502: no false read of $C0xx (the Mockingboard hack of GH#52 lives in
    // the card, not in the CPU).
    for (v, code) in [
        (N, vec![0x9D, 0x00, 0xC0]),
        (N, vec![0x99, 0x00, 0xC0]),
        (C, vec![0x9D, 0x00, 0xC0]),
        (N, vec![0x91, 0x10]),
    ] {
        let (_, r, _) = run(v, &code, |c, r| {
            c.x = 0x30;
            c.y = 0x30;
            r.mem[0x10] = 0x00;
            r.mem[0x11] = 0xC0;
        });
        assert!(!r.reads.iter().any(|a| (0xC000..0xC100).contains(a)), "{v:?} {code:02X?}: {:04X?}", r.reads);
        assert_eq!(r.writes.len(), 1);
    }
}

// ---------------------------------------------------------------- NMOS undocumented: immediates

#[test]
fn anc_ands_and_copies_bit7_into_carry() {
    let (c, _, n) = run(N, &[0x0B, 0x80], |c, _| c.a = 0xFF);
    assert_eq!((c.a, n), (0x80, 2));
    assert_eq!(flags(&c), (true, false, false, true));
    let (c, _, _) = run(N, &[0x2B, 0x7F], |c, _| c.a = 0xFF);
    assert_eq!(flags(&c), (false, false, false, false));
}

#[test]
fn alr_ands_then_shifts_right() {
    let (c, _, n) = run(N, &[0x4B, 0x07], |c, _| c.a = 0xFF);
    assert_eq!((c.a, n), (0x03, 2));
    assert_eq!(flags(&c), (false, false, false, true));
}

#[test]
fn arr_binary_mode_flags() {
    let (c, _, _) = run(N, &[0x6B, 0xFF], |c, _| {
        c.a = 0xFF;
        c.p = FLAG_C;
    });
    assert_eq!(c.a, 0xFF);
    assert_eq!(flags(&c), (true, false, false, true));
    let (c, _, _) = run(N, &[0x6B, 0x80], |c, _| c.a = 0x80);
    assert_eq!(c.a, 0x40);
    assert_eq!(flags(&c), (false, true, false, true));
}

#[test]
fn xaa_oal_sax_las_eb() {
    let (c, _, n) = run(N, &[0x8B, 0x3C], |c, _| {
        c.x = 0x0F;
        c.a = 0xFF;
    });
    assert_eq!((c.a, n), (0x0C, 2)); // XAA: A = X & imm

    let (c, _, _) = run(N, &[0xAB, 0x13], |c, _| c.a = 0x01);
    assert_eq!((c.a, c.x), (0x03, 0x03)); // OAL: A = (A | $EE) & imm; X = A

    let (c, _, _) = run(N, &[0xCB, 0x10], |c, _| {
        c.a = 0xF0;
        c.x = 0x3C;
    });
    assert_eq!(c.x, 0x20); // SAX: X = (A & X) - imm
    assert!(c.p & FLAG_C != 0);

    let (c, _, n) = run(N, &[0xBB, 0x00, 0x20], |c, r| {
        r.mem[0x2000] = 0xF0;
        c.sp = 0x3C;
    });
    assert_eq!((c.a, c.x, c.sp, n), (0x30, 0x30, 0x30, 4)); // LAS

    let (c, _, n) = run(N, &[0xEB, 0x01], |c, _| {
        c.a = 0x05;
        c.p = FLAG_C;
    });
    assert_eq!((c.a, n), (0x04, 2)); // $EB behaves as SBC #imm
}

#[test]
fn lax_loads_a_and_x() {
    let (c, _, n) = run(N, &[0xA7, 0x10], |_, r| r.mem[0x10] = 0x80);
    assert_eq!((c.a, c.x, n), (0x80, 0x80, 3));
    assert_eq!(flags(&c), (true, false, false, false));
    let (c, _, n) = run(N, &[0xB3, 0x10], |c, r| {
        c.y = 0x10;
        r.mem[0x10] = 0xF8;
        r.mem[0x11] = 0x20; // $20F8 + $10 crosses a page
        r.mem[0x2108] = 0x00;
    });
    assert_eq!((c.a, c.x, n), (0, 0, 6));
    assert!(c.p & FLAG_Z != 0);
}

#[test]
fn axs_stores_a_and_x_without_touching_flags() {
    let (c, r, n) = run(N, &[0x87, 0x10], |c, _| {
        c.a = 0xF0;
        c.x = 0x3C;
        c.p = FLAG_Z | FLAG_C;
    });
    assert_eq!((r.mem[0x10], n, c.p), (0x30, 3, FLAG_Z | FLAG_C));
}

// ---------------------------------------------------------------- NMOS HLT / NOPs

#[test]
fn hlt_jams_with_pc_left_on_the_opcode() {
    for op in [0x02u8, 0x12, 0x22, 0x32, 0x42, 0x52, 0x62, 0x72, 0x92, 0xB2, 0xD2, 0xF2] {
        let mut r = ram();
        r.mem[0x300] = op;
        let mut c = Cpu::new(N);
        c.pc = 0x300;
        assert_eq!(c.step(&mut r), 2);
        assert_eq!(c.pc, 0x300, "opcode {op:02X}");
        assert!(c.jammed);
        assert_eq!(c.step(&mut r), 2);
        assert_eq!(c.pc, 0x300);
    }
}

#[test]
fn nmos_nops_never_touch_their_operand_address() {
    // (opcode, operand bytes, cycles)
    let nops: [(u8, &[u8], u32); 16] = [
        (0x04, &[0x30], 3),
        (0x0C, &[0x30, 0xC0], 4),
        (0x14, &[0x30], 4),
        (0x1A, &[], 2),
        (0x1C, &[0x30, 0xC0], 4),
        (0x34, &[0x30], 4),
        (0x3A, &[], 2),
        (0x3C, &[0x30, 0xC0], 4),
        (0x44, &[0x30], 3),
        (0x5C, &[0x30, 0xC0], 4),
        (0x64, &[0x30], 3),
        (0x80, &[0x30], 2),
        (0x82, &[0x30], 2),
        (0x89, &[0x30], 2),
        (0xDC, &[0x30, 0xC0], 4),
        (0xFA, &[], 2),
    ];
    for (op, operand, cycles) in nops {
        let mut code = vec![op];
        code.extend_from_slice(operand);
        let (c, r, n) = run(N, &code, |_, _| {});
        assert_eq!(n, cycles, "opcode {op:02X}");
        assert_eq!(c.pc, 0x300 + code.len() as u16, "opcode {op:02X}");
        assert!(r.reads.iter().all(|&a| (0x300..0x303).contains(&a)), "opcode {op:02X}: {:04X?}", r.reads);
        assert!(r.writes.is_empty());
    }
}

// ---------------------------------------------------------------- decimal mode

#[test]
fn decimal_adc_99_plus_1_differs_between_nmos_and_cmos() {
    let setup = |c: &mut Cpu, _: &mut Ram| {
        c.a = 0x99;
        c.p = FLAG_D;
    };
    let (n, _, cn) = run(N, &[0x69, 0x01], setup);
    assert_eq!((n.a, cn), (0x00, 2));
    // NMOS: Z reflects the binary sum ($9A), N the pre-correction sum.
    assert_eq!(flags(&n), (true, false, false, true));
    let (c, _, cc) = run(C, &[0x69, 0x01], setup);
    assert_eq!((c.a, cc), (0x00, 3)); // +1 cycle
    assert_eq!(flags(&c), (false, false, true, true));
}

#[test]
fn decimal_sbc_borrow() {
    let setup = |c: &mut Cpu, _: &mut Ram| {
        c.a = 0x00;
        c.p = FLAG_D | FLAG_C;
    };
    let (c, _, n) = run(C, &[0xE9, 0x01], setup);
    assert_eq!((c.a, n), (0x99, 3));
    assert_eq!(flags(&c), (true, false, false, false));
    let (c, _, n) = run(N, &[0xE9, 0x01], setup);
    assert_eq!((c.a, n), (0x99, 2));
    assert!(c.p & FLAG_C == 0);
}

// ---------------------------------------------------------------- 65C02 specifics

#[test]
fn cmos_new_instructions() {
    // BIT #imm only affects Z
    let (c, _, _) = run(C, &[0x89, 0xC0], |c, _| {
        c.a = 0x01;
        c.p = 0;
    });
    assert_eq!(flags(&c), (false, false, true, false));
    // STZ, TSB, TRB
    let (_, r, n) = run(C, &[0x64, 0x10], |_, r| r.mem[0x10] = 0xAA);
    assert_eq!((r.mem[0x10], n), (0, 3));
    let (c, r, n) = run(C, &[0x04, 0x10], |c, r| {
        r.mem[0x10] = 0x0F;
        c.a = 0xF0;
    });
    assert_eq!((r.mem[0x10], n), (0xFF, 5));
    assert!(c.p & FLAG_Z != 0);
    let (_, r, _) = run(C, &[0x14, 0x10], |c, r| {
        r.mem[0x10] = 0xFF;
        c.a = 0x0F;
    });
    assert_eq!(r.mem[0x10], 0xF0);
    // PHX/PLY, INC A, (zp)
    let (c, r, _) = run(C, &[0xDA], |c, _| c.x = 7);
    assert_eq!((r.mem[0x1FF], c.sp), (7, 0xFE));
    let (c, _, n) = run(C, &[0x1A], |c, _| c.a = 0xFF);
    assert_eq!((c.a, n), (0, 2));
    let (c, _, n) = run(C, &[0xB2, 0x10], |_, r| {
        r.mem[0x10] = 0x00;
        r.mem[0x11] = 0x20;
        r.mem[0x2000] = 0x5A;
    });
    assert_eq!((c.a, n), (0x5A, 5));
}

#[test]
fn cmos_wai_stp_and_rockwell_ops_are_one_cycle_nops() {
    // AppleWin's cpu65C02.h has no WAI/STP/RMBn/SMBn/BBRn/BBSn: $CB, $DB and $x7/$xF are NOP CYC(1).
    for op in [0xCBu8, 0xDB, 0x07, 0x17, 0x87, 0x97, 0x0F, 0x1F, 0x8F, 0x9F] {
        let (c, _, n) = run(C, &[op, 0xEA], |_, _| {});
        assert_eq!((n, c.pc), (1, 0x301), "opcode {op:02X}");
        assert!(!c.waiting && !c.stopped);
    }
}

#[test]
fn cmos_undefined_nops_sizes() {
    for (op, len, cycles) in [(0x02u8, 2u16, 2u32), (0x44, 2, 3), (0x54, 2, 4), (0x5C, 3, 8), (0xDC, 3, 4)] {
        let (c, _, n) = run(C, &[op, 0, 0], |_, _| {});
        assert_eq!((c.pc, n), (0x300 + len, cycles), "opcode {op:02X}");
    }
}

// ---------------------------------------------------------------- interrupts

#[test]
fn brk_pushes_pc_plus_two_and_status_with_b() {
    for v in [N, C] {
        let (c, r, n) = run(v, &[0x00, 0xEA], |c, r| {
            r.mem[0xFFFE] = 0x34;
            r.mem[0xFFFF] = 0x12;
            c.p = FLAG_D;
        });
        assert_eq!((c.pc, n, c.sp), (0x1234, 7, 0xFC));
        assert_eq!((r.mem[0x1FF], r.mem[0x1FE]), (0x03, 0x02));
        assert_eq!(r.mem[0x1FD], FLAG_D | FLAG_B | FLAG_U);
        assert!(c.p & FLAG_I != 0);
        assert_eq!(c.p & FLAG_D != 0, v == N, "65C02 clears D on BRK, NMOS keeps it");
    }
}

#[test]
fn irq_takes_seven_cycles_pushes_status_without_b_and_respects_i() {
    for v in [N, C] {
        let (c, r, n) = run(v, &[0xEA], |c, r| {
            r.mem[0xFFFE] = 0x00;
            r.mem[0xFFFF] = 0x40;
            c.irq = true;
            c.p = FLAG_D;
        });
        assert_eq!((c.pc, n), (0x4000, 7));
        assert_eq!((r.mem[0x1FF], r.mem[0x1FE]), (0x03, 0x00)); // the interrupted PC, unmodified
        assert_eq!(r.mem[0x1FD], FLAG_D | FLAG_U);
        assert!(c.p & FLAG_I != 0);
        assert_eq!(c.p & FLAG_D != 0, v == N);
        // masked by I: the NOP runs
        let (c, _, n) = run(v, &[0xEA], |c, _| {
            c.irq = true;
            c.p = FLAG_I;
        });
        assert_eq!((c.pc, n), (0x301, 2));
    }
}

#[test]
fn nmi_is_edge_triggered_and_not_maskable() {
    let (mut c, mut r) = {
        let mut r = ram();
        r.mem[0xFFFA] = 0x00;
        r.mem[0xFFFB] = 0x50;
        r.mem[0x300] = 0xEA;
        r.mem[0x301] = 0xEA;
        let mut c = Cpu::new(N);
        c.pc = 0x300;
        c.p = FLAG_I;
        (c, r)
    };
    c.nmi();
    assert_eq!(c.step(&mut r), 7);
    assert_eq!(c.pc, 0x5000);
    assert_eq!(r.mem[0x1FD], FLAG_I | FLAG_U);
    r.mem[0x5000] = 0xEA;
    assert_eq!(c.step(&mut r), 2, "serviced once");
}

#[test]
fn rti_restores_status_and_pc() {
    let (c, _, n) = run(N, &[0x40], |c, r| {
        c.sp = 0xFC;
        r.mem[0x1FD] = FLAG_C | FLAG_B;
        r.mem[0x1FE] = 0x34;
        r.mem[0x1FF] = 0x12;
    });
    assert_eq!((c.pc, n, c.sp), (0x1234, 6, 0xFF));
    assert!(c.p & FLAG_C != 0 && c.p & FLAG_B == 0);
}

#[test]
fn php_pushes_break_and_unused_bits() {
    let (_, r, n) = run(N, &[0x08], |c, _| c.p = FLAG_C);
    assert_eq!((r.mem[0x1FF], n), (FLAG_C | FLAG_B | FLAG_U, 3));
}
