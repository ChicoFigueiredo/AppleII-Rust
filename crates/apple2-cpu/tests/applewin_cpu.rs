//! Port of AppleWin `test/TestCPU6502/TestCPU6502.cpp` (GH#264, 271, 278, 282, 292, 1257 and
//! the cycle-timing table), plus per-opcode tests of the NMOS undocumented instructions and
//! the 65C02 specifics. Expected values of the ported tests are AppleWin's, verbatim. Memory is
//! one flat 64K array that, as in the C++ test, is *not* cleared between sub-tests.
//!
//! The C++ test runs everything twice (`g_isMemCacheValid` true/false: the "alt" read/write
//! macros); both paths share semantics, so the Rust core needs a single pass.
use apple2_cpu::*;

struct Ram {
    mem: Box<[u8; 65536]>,
    /// Reads of $C000 (the `IORead[0]` hook of GH#292).
    c000_reads: u32,
    reads: Vec<u16>,
    writes: Vec<(u16, u8)>,
}

impl Bus for Ram {
    fn read(&mut self, a: u16) -> u8 {
        self.reads.push(a);
        if a == 0xC000 {
            self.c000_reads += 1;
            return 42;
        }
        self.mem[a as usize]
    }
    fn write(&mut self, a: u16, v: u8) {
        self.writes.push((a, v));
        self.mem[a as usize] = v;
    }
}

struct T {
    ram: Ram,
    cpu: Cpu,
}

const N: Variant = Variant::Nmos6502;
const C: Variant = Variant::Cmos65C02;

impl T {
    fn new() -> T {
        let mut t = T {
            ram: Ram { mem: Box::new([0; 65536]), c000_reads: 0, reads: vec![], writes: vec![] },
            cpu: Cpu::new(N),
        };
        t.reset();
        t
    }

    /// TestCPU6502.cpp `reset()`.
    fn reset(&mut self) {
        let v = self.cpu.variant;
        self.cpu = Cpu::new(v);
        (self.cpu.a, self.cpu.x, self.cpu.y) = (0, 0, 0);
        self.cpu.pc = 0x300;
        self.cpu.sp = 0xFF; // regs.sp = 0x1FF
        self.cpu.p = 0;
    }

    /// `TestCpu6502(0)` / `TestCpu65C02(0)`: one instruction, returns its cycles.
    fn run(&mut self, v: Variant) -> u32 {
        self.cpu.variant = v;
        self.ram.reads.clear();
        self.ram.writes.clear();
        self.cpu.step(&mut self.ram)
    }
    fn n(&mut self) -> u32 {
        self.run(N)
    }
    fn c(&mut self) -> u32 {
        self.run(C)
    }
    fn m(&mut self, a: u16) -> &mut u8 {
        &mut self.ram.mem[a as usize]
    }
    fn put(&mut self, a: u16, bytes: &[u8]) {
        self.ram.mem[a as usize..a as usize + bytes.len()].copy_from_slice(bytes);
    }
}

// ---------------------------------------------------------------------------------------------
// GH#264: JMP (abs)
// ---------------------------------------------------------------------------------------------

#[test]
fn gh264_jmp_indirect() {
    let mut t = T::new();
    // No page-cross
    t.reset();
    let pc = t.cpu.pc;
    let abs = pc + 3;
    let dst = abs + 2;
    t.put(pc, &[0x6C, abs as u8, (abs >> 8) as u8, dst as u8, (dst >> 8) as u8]);
    assert_eq!(t.n(), 5);
    assert_eq!(t.cpu.pc, dst);
    t.reset();
    assert_eq!(t.c(), 6);
    assert_eq!(t.cpu.pc, dst);

    // Page-cross: 3FC: JMP (abs); 3FF: lo(dst), hi(dst)
    t.reset();
    t.cpu.pc = 0x3FC;
    let abs = t.cpu.pc + 3;
    let dst = abs + 2;
    t.put(0x3FC, &[0x6C, abs as u8, (abs >> 8) as u8, dst as u8, (dst >> 8) as u8]);
    *t.m(0x300) = (dst >> 8) as u8; // allow for the 6502 bug
    assert_eq!(t.n(), 5);
    assert_eq!(t.cpu.pc, dst);

    t.reset();
    t.cpu.pc = 0x3FC;
    *t.m(0x300) = 0; // the 65C02 fixes the bug
    assert_eq!(t.c(), 7);
    assert_eq!(t.cpu.pc, dst);
}

// ---------------------------------------------------------------------------------------------
// GH#271: ASL/DEC/INC abs,X
// ---------------------------------------------------------------------------------------------

fn rmw_absx(t: &mut T, op: u8, x: u8, base: u16, d: u8) {
    *t.m(base.wrapping_add(x as u16)) = d;
    t.reset();
    t.cpu.x = x;
    t.put(0x300, &[op, base as u8, (base >> 8) as u8]);
}

#[test]
fn gh271_rmw_abs_x_cycles() {
    let base = 0x20FF;
    let d = 0x40u8;
    let mut t = T::new();
    // (opcode, expected result, [6502, 65C02] for no page-cross then page-cross)
    let cases: [(u8, u8, [[u32; 2]; 2]); 3] = [
        (0x1E, d << 1, [[7, 6], [7, 7]]), // ASL: non-PX optimised on the 65C02
        (0xDE, d - 1, [[7, 7], [7, 7]]),  // DEC: not optimised
        (0xFE, d + 1, [[7, 7], [7, 7]]),  // INC: not optimised
    ];
    for (op, res, cyc) in cases {
        for (px, x) in [(0usize, 0u8), (1, 1)] {
            rmw_absx(&mut t, op, x, base, d);
            assert_eq!(t.n(), cyc[px][0], "6502 op {op:02X} x={x}");
            assert_eq!(*t.m(base + x as u16), res);
            rmw_absx(&mut t, op, x, base, d);
            assert_eq!(t.c(), cyc[px][1], "65C02 op {op:02X} x={x}");
            assert_eq!(*t.m(base + x as u16), res);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// GH#278: opcode timing table, branches, JMP (abs,X), CMOS decimal +1 cycle
// ---------------------------------------------------------------------------------------------

/// 6502 (no page-cross), 6502 (page-cross), 65C02 (no page-cross), 65C02 (page-cross)
const TIMINGS: [[u8; 4]; 256] = [
    [7, 7, 7, 7], // 00
    [6, 6, 6, 6], // 01
    [2, 2, 2, 2], // 02
    [8, 8, 1, 1], // 03
    [3, 3, 5, 5], // 04
    [3, 3, 3, 3], // 05
    [5, 5, 5, 5], // 06
    [5, 5, 1, 1], // 07
    [3, 3, 3, 3], // 08
    [2, 2, 2, 2], // 09
    [2, 2, 2, 2], // 0A
    [2, 2, 1, 1], // 0B
    [4, 4, 6, 6], // 0C (GH#1360: NMOS 6502: ABS, not ABS,X... so no 6502 page-cross)
    [4, 4, 4, 4], // 0D
    [6, 6, 6, 6], // 0E
    [6, 6, 1, 1], // 0F
    [3, 3, 3, 3], // 10
    [5, 6, 5, 6], // 11
    [2, 2, 5, 5], // 12
    [8, 8, 1, 1], // 13
    [4, 4, 5, 5], // 14
    [4, 4, 4, 4], // 15
    [6, 6, 6, 6], // 16
    [6, 6, 1, 1], // 17
    [2, 2, 2, 2], // 18
    [4, 5, 4, 5], // 19
    [2, 2, 2, 2], // 1A
    [7, 7, 1, 1], // 1B
    [4, 5, 6, 6], // 1C
    [4, 5, 4, 5], // 1D
    [7, 7, 6, 7], // 1E
    [7, 7, 1, 1], // 1F
    [6, 6, 6, 6], // 20
    [6, 6, 6, 6], // 21
    [2, 2, 2, 2], // 22
    [8, 8, 1, 1], // 23
    [3, 3, 3, 3], // 24
    [3, 3, 3, 3], // 25
    [5, 5, 5, 5], // 26
    [5, 5, 1, 1], // 27
    [4, 4, 4, 4], // 28
    [2, 2, 2, 2], // 29
    [2, 2, 2, 2], // 2A
    [2, 2, 1, 1], // 2B
    [4, 4, 4, 4], // 2C
    [4, 4, 4, 4], // 2D
    [6, 6, 6, 6], // 2E
    [6, 6, 1, 1], // 2F
    [2, 2, 2, 2], // 30
    [5, 6, 5, 6], // 31
    [2, 2, 5, 5], // 32
    [8, 8, 1, 1], // 33
    [4, 4, 4, 4], // 34
    [4, 4, 4, 4], // 35
    [6, 6, 6, 6], // 36
    [6, 6, 1, 1], // 37
    [2, 2, 2, 2], // 38
    [4, 5, 4, 5], // 39
    [2, 2, 2, 2], // 3A
    [7, 7, 1, 1], // 3B
    [4, 5, 4, 5], // 3C
    [4, 5, 4, 5], // 3D
    [7, 7, 6, 7], // 3E
    [7, 7, 1, 1], // 3F
    [6, 6, 6, 6], // 40
    [6, 6, 6, 6], // 41
    [2, 2, 2, 2], // 42
    [8, 8, 1, 1], // 43
    [3, 3, 3, 3], // 44
    [3, 3, 3, 3], // 45
    [5, 5, 5, 5], // 46
    [5, 5, 1, 1], // 47
    [3, 3, 3, 3], // 48
    [2, 2, 2, 2], // 49
    [2, 2, 2, 2], // 4A
    [2, 2, 1, 1], // 4B
    [3, 3, 3, 3], // 4C
    [4, 4, 4, 4], // 4D
    [6, 6, 6, 6], // 4E
    [6, 6, 1, 1], // 4F
    [3, 3, 3, 3], // 50
    [5, 6, 5, 6], // 51
    [2, 2, 5, 5], // 52
    [8, 8, 1, 1], // 53
    [4, 4, 4, 4], // 54
    [4, 4, 4, 4], // 55
    [6, 6, 6, 6], // 56
    [6, 6, 1, 1], // 57
    [2, 2, 2, 2], // 58
    [4, 5, 4, 5], // 59
    [2, 2, 3, 3], // 5A
    [7, 7, 1, 1], // 5B
    [4, 5, 8, 8], // 5C
    [4, 5, 4, 5], // 5D
    [7, 7, 6, 7], // 5E
    [7, 7, 1, 1], // 5F
    [6, 6, 6, 6], // 60
    [6, 6, 6, 6], // 61
    [2, 2, 2, 2], // 62
    [8, 8, 1, 1], // 63
    [3, 3, 3, 3], // 64
    [3, 3, 3, 3], // 65
    [5, 5, 5, 5], // 66
    [5, 5, 1, 1], // 67
    [4, 4, 4, 4], // 68
    [2, 2, 2, 2], // 69
    [2, 2, 2, 2], // 6A
    [2, 2, 1, 1], // 6B
    [5, 5, 7, 7], // 6C
    [4, 4, 4, 4], // 6D
    [6, 6, 6, 6], // 6E
    [6, 6, 1, 1], // 6F
    [2, 2, 2, 2], // 70
    [5, 6, 5, 6], // 71
    [2, 2, 5, 5], // 72
    [8, 8, 1, 1], // 73
    [4, 4, 4, 4], // 74
    [4, 4, 4, 4], // 75
    [6, 6, 6, 6], // 76
    [6, 6, 1, 1], // 77
    [2, 2, 2, 2], // 78
    [4, 5, 4, 5], // 79
    [2, 2, 4, 4], // 7A
    [7, 7, 1, 1], // 7B
    [4, 5, 6, 6], // 7C
    [4, 5, 4, 5], // 7D
    [7, 7, 6, 7], // 7E
    [7, 7, 1, 1], // 7F
    [2, 2, 3, 3], // 80
    [6, 6, 6, 6], // 81
    [2, 2, 2, 2], // 82
    [6, 6, 1, 1], // 83
    [3, 3, 3, 3], // 84
    [3, 3, 3, 3], // 85
    [3, 3, 3, 3], // 86
    [3, 3, 1, 1], // 87
    [2, 2, 2, 2], // 88
    [2, 2, 2, 2], // 89
    [2, 2, 2, 2], // 8A
    [2, 2, 1, 1], // 8B
    [4, 4, 4, 4], // 8C
    [4, 4, 4, 4], // 8D
    [4, 4, 4, 4], // 8E
    [4, 4, 1, 1], // 8F
    [3, 3, 3, 3], // 90
    [6, 6, 6, 6], // 91
    [2, 2, 5, 5], // 92
    [6, 6, 1, 1], // 93
    [4, 4, 4, 4], // 94
    [4, 4, 4, 4], // 95
    [4, 4, 4, 4], // 96
    [4, 4, 1, 1], // 97
    [2, 2, 2, 2], // 98
    [5, 5, 5, 5], // 99
    [2, 2, 2, 2], // 9A
    [5, 5, 1, 1], // 9B
    [5, 5, 4, 4], // 9C
    [5, 5, 5, 5], // 9D
    [5, 5, 5, 5], // 9E
    [5, 5, 1, 1], // 9F
    [2, 2, 2, 2], // A0
    [6, 6, 6, 6], // A1
    [2, 2, 2, 2], // A2
    [6, 6, 1, 1], // A3
    [3, 3, 3, 3], // A4
    [3, 3, 3, 3], // A5
    [3, 3, 3, 3], // A6
    [3, 3, 1, 1], // A7
    [2, 2, 2, 2], // A8
    [2, 2, 2, 2], // A9
    [2, 2, 2, 2], // AA
    [2, 2, 1, 1], // AB
    [4, 4, 4, 4], // AC
    [4, 4, 4, 4], // AD
    [4, 4, 4, 4], // AE
    [4, 4, 1, 1], // AF
    [2, 2, 2, 2], // B0
    [5, 6, 5, 6], // B1
    [2, 2, 5, 5], // B2
    [5, 6, 1, 1], // B3
    [4, 4, 4, 4], // B4
    [4, 4, 4, 4], // B5
    [4, 4, 4, 4], // B6
    [4, 4, 1, 1], // B7
    [2, 2, 2, 2], // B8
    [4, 5, 4, 5], // B9
    [2, 2, 2, 2], // BA
    [4, 5, 1, 1], // BB
    [4, 5, 4, 5], // BC
    [4, 5, 4, 5], // BD
    [4, 5, 4, 5], // BE
    [4, 5, 1, 1], // BF
    [2, 2, 2, 2], // C0
    [6, 6, 6, 6], // C1
    [2, 2, 2, 2], // C2
    [8, 8, 1, 1], // C3
    [3, 3, 3, 3], // C4
    [3, 3, 3, 3], // C5
    [5, 5, 5, 5], // C6
    [5, 5, 1, 1], // C7
    [2, 2, 2, 2], // C8
    [2, 2, 2, 2], // C9
    [2, 2, 2, 2], // CA
    [2, 2, 1, 1], // CB
    [4, 4, 4, 4], // CC
    [4, 4, 4, 4], // CD
    [6, 6, 6, 6], // CE
    [6, 6, 1, 1], // CF
    [3, 3, 3, 3], // D0
    [5, 6, 5, 6], // D1
    [2, 2, 5, 5], // D2
    [8, 8, 1, 1], // D3
    [4, 4, 4, 4], // D4
    [4, 4, 4, 4], // D5
    [6, 6, 6, 6], // D6
    [6, 6, 1, 1], // D7
    [2, 2, 2, 2], // D8
    [4, 5, 4, 5], // D9
    [2, 2, 3, 3], // DA
    [7, 7, 1, 1], // DB
    [4, 5, 4, 4], // DC
    [4, 5, 4, 5], // DD
    [7, 7, 7, 7], // DE
    [7, 7, 1, 1], // DF
    [2, 2, 2, 2], // E0
    [6, 6, 6, 6], // E1
    [2, 2, 2, 2], // E2
    [8, 8, 1, 1], // E3
    [3, 3, 3, 3], // E4
    [3, 3, 3, 3], // E5
    [5, 5, 5, 5], // E6
    [5, 5, 1, 1], // E7
    [2, 2, 2, 2], // E8
    [2, 2, 2, 2], // E9
    [2, 2, 2, 2], // EA
    [2, 2, 1, 1], // EB
    [4, 4, 4, 4], // EC
    [4, 4, 4, 4], // ED
    [6, 6, 6, 6], // EE
    [6, 6, 1, 1], // EF
    [2, 2, 2, 2], // F0
    [5, 6, 5, 6], // F1
    [2, 2, 5, 5], // F2
    [8, 8, 1, 1], // F3
    [4, 4, 4, 4], // F4
    [4, 4, 4, 4], // F5
    [6, 6, 6, 6], // F6
    [6, 6, 1, 1], // F7
    [2, 2, 2, 2], // F8
    [4, 5, 4, 5], // F9
    [2, 2, 4, 4], // FA
    [7, 7, 1, 1], // FB
    [4, 5, 4, 4], // FC
    [4, 5, 4, 5], // FD
    [7, 7, 7, 7], // FE
    [7, 7, 1, 1], // FF
];

const AF_CARRY: u8 = 0x01;
const AF_ZERO: u8 = 0x02;
const AF_DECIMAL: u8 = 0x08;
const AF_OVERFLOW: u8 = 0x40;
const AF_SIGN: u8 = 0x80;

fn setup_op(t: &mut T, op: u8, px: bool) {
    t.reset();
    if px {
        t.cpu.x = 1;
        t.cpu.y = 1;
    }
    let base = 0x20FFu16;
    t.put(0x300, &[op, base as u8, (base >> 8) as u8]);
    if px {
        *t.m(0xFF) = 0xFF;
        *t.m(0x00) = 0x00; // for: OPCODE (zp),Y
    }
}

#[test]
fn gh278_opcode_timings_6502_no_page_cross() {
    let mut t = T::new();
    for op in 0..=255u8 {
        setup_op(&mut t, op, false);
        assert_eq!(t.n(), TIMINGS[op as usize][0] as u32, "NMOS opcode {op:02X}");
    }
}

#[test]
fn gh278_opcode_timings_6502_page_cross() {
    let mut t = T::new();
    for op in 0..=255u8 {
        setup_op(&mut t, op, true);
        assert_eq!(t.n(), TIMINGS[op as usize][1] as u32, "NMOS opcode {op:02X} (page-cross)");
    }
}

#[test]
fn gh278_opcode_timings_65c02_no_page_cross() {
    let mut t = T::new();
    for op in 0..=255u8 {
        setup_op(&mut t, op, false);
        assert_eq!(t.c(), TIMINGS[op as usize][2] as u32, "CMOS opcode {op:02X}");
    }
}

#[test]
fn gh278_opcode_timings_65c02_page_cross() {
    let mut t = T::new();
    for op in 0..=255u8 {
        setup_op(&mut t, op, true);
        assert_eq!(t.c(), TIMINGS[op as usize][3] as u32, "CMOS opcode {op:02X} (page-cross)");
    }
}

fn bcc_sub(t: &mut T, op: u8, ps_not_taken: u8, ps_taken: u8, pc: u16) {
    t.put(pc, &[op, 0x01]);
    let dst_not_taken = pc + 2;
    let dst_taken = pc + 2 + 1;
    let pagecross = (((pc + 2) ^ dst_taken) >> 8) & 1;
    for v in [N, C] {
        t.reset();
        t.cpu.pc = pc;
        t.cpu.p = ps_not_taken;
        assert_eq!(t.run(v), 2, "{v:?} op {op:02X} not taken");
        assert_eq!(t.cpu.pc, dst_not_taken);
        t.reset();
        t.cpu.pc = pc;
        t.cpu.p = ps_taken;
        assert_eq!(t.run(v), 3 + pagecross as u32, "{v:?} op {op:02X} taken @{pc:04X}");
        assert_eq!(t.cpu.pc, dst_taken);
    }
}

#[test]
fn gh278_conditional_branches() {
    let mut t = T::new();
    let cases = [
        (0x10, AF_SIGN, 0),
        (0x30, 0, AF_SIGN),
        (0x50, AF_OVERFLOW, 0),
        (0x70, 0, AF_OVERFLOW),
        (0x90, AF_CARRY, 0),
        (0xB0, 0, AF_CARRY),
        (0xD0, AF_ZERO, 0),
        (0xF0, 0, AF_ZERO),
    ];
    for (op, nt, tk) in cases {
        bcc_sub(&mut t, op, nt, tk, 0x300); // no page cross
        bcc_sub(&mut t, op, nt, tk, 0x3FD); // page cross
    }
}

#[test]
fn gh278_bra() {
    let mut t = T::new();
    for (pc, cycles) in [(0x300u16, 3), (0x3FD, 4)] {
        t.put(pc, &[0x80, 0x01]);
        t.reset();
        t.cpu.pc = pc;
        assert_eq!(t.c(), cycles);
        assert_eq!(t.cpu.pc, pc + 3);
    }
}

#[test]
fn gh278_jmp_indirect_x() {
    let mut t = T::new();
    // No page-cross
    t.reset();
    let abs = t.cpu.pc + 3;
    let dst = abs + 2;
    t.put(0x300, &[0x7C, abs as u8, (abs >> 8) as u8, dst as u8, (dst >> 8) as u8]);
    assert_eq!(t.c(), 6);
    assert_eq!(t.cpu.pc, dst);

    // Page-cross (case 1)
    t.reset();
    t.cpu.pc = 0x3FC;
    let abs = t.cpu.pc + 3;
    let dst = abs + 2;
    t.put(0x3FC, &[0x7C, abs as u8, (abs >> 8) as u8, dst as u8, (dst >> 8) as u8]);
    assert_eq!(t.c(), 6);
    assert_eq!(t.cpu.pc, dst);

    // Page-cross (case 2)
    t.reset();
    t.cpu.x = 1;
    t.cpu.pc = 0x3FA;
    let abs = t.cpu.pc + 3;
    let dst = abs + 2 + t.cpu.x as u16;
    t.put(0x3FA, &[0x7C, abs as u8, (abs >> 8) as u8, 0xCC, dst as u8, (dst >> 8) as u8]);
    assert_eq!(t.c(), 6);
    assert_eq!(t.cpu.pc, dst);
}

fn adc_sbc_decimal(t: &mut T, op: u8) {
    let t6 = |px: bool| TIMINGS[op as usize][px as usize] as u32;
    let tc = |px: bool| TIMINGS[op as usize][2 + px as usize] as u32;
    for px in [false, true] {
        setup_op(t, op, px);
        t.cpu.p = AF_DECIMAL;
        assert_eq!(t.n(), t6(px), "NMOS {op:02X} px={px}");
        setup_op(t, op, px);
        t.cpu.p = AF_DECIMAL;
        assert_eq!(t.c(), tc(px) + 1, "CMOS {op:02X} px={px}: +1 cycle in decimal mode");
    }
}

#[test]
fn gh278_adc_sbc_cmos_decimal_is_one_cycle_slower() {
    let mut t = T::new();
    for op in [0x61, 0x65, 0x69, 0x6D, 0x71, 0x72, 0x75, 0x79, 0x7D] {
        adc_sbc_decimal(&mut t, op);
    }
    for op in [0xE1, 0xE5, 0xE9, 0xED, 0xF1, 0xF2, 0xF5, 0xF9, 0xFD] {
        adc_sbc_decimal(&mut t, op);
    }
}

// ---------------------------------------------------------------------------------------------
// GH#282: AXA / SAY / TAS / XAS (page-cross replaces the high address byte)
// ---------------------------------------------------------------------------------------------

#[test]
fn gh282_undocumented_stores_with_high_byte_and() {
    let mut t = T::new();
    let hi1 = |base: u16| ((base >> 8) as u8).wrapping_add(1);
    let base = 0x20FFu16;
    const A: u8 = 0xEA;
    const XF: u8 = 0xFF;
    for (y, addr) in [(0u8, 0x20FFu16), (1, 0x2000)] {
        // axa (zp),y
        *t.m(addr) = 0xCC;
        t.reset();
        *t.m(0xFE) = base as u8;
        *t.m(0xFF) = (base >> 8) as u8;
        (t.cpu.a, t.cpu.x, t.cpu.y) = (0xEA, 0xFF, y);
        t.put(0x300, &[0x93, 0xFE]);
        assert_eq!(t.n(), 6);
        assert_eq!(*t.m(addr), A & XF & hi1(base), "axa (zp),y y={y}");

        // axa abs,y
        *t.m(addr) = 0xCC;
        t.reset();
        (t.cpu.a, t.cpu.x, t.cpu.y) = (0xEA, 0xFF, y);
        t.put(0x300, &[0x9F, base as u8, (base >> 8) as u8]);
        assert_eq!(t.n(), 5);
        assert_eq!(*t.m(addr), A & XF & hi1(base), "axa abs,y y={y}");

        // say abs,x
        *t.m(addr) = 0xCC;
        t.reset();
        (t.cpu.a, t.cpu.x, t.cpu.y) = (0xEA, y, 0x20);
        t.put(0x300, &[0x9C, base as u8, (base >> 8) as u8]);
        assert_eq!(t.n(), 5);
        assert_eq!(*t.m(addr), 0x20 & hi1(base), "say abs,x x={y}");

        // tas abs,y
        *t.m(addr) = 0xCC;
        t.reset();
        (t.cpu.a, t.cpu.x, t.cpu.y) = (0xEA, 0xFF, y);
        t.put(0x300, &[0x9B, base as u8, (base >> 8) as u8]);
        assert_eq!(t.n(), 5);
        assert_eq!(*t.m(addr), A & XF & hi1(base), "tas abs,y y={y}");
        assert_eq!(t.cpu.sp, A & XF);

        // xas abs,y
        *t.m(addr) = 0xCC;
        t.reset();
        (t.cpu.a, t.cpu.x, t.cpu.y) = (0xEA, 0x20, y);
        t.put(0x300, &[0x9E, base as u8, (base >> 8) as u8]);
        assert_eq!(t.n(), 5);
        assert_eq!(*t.m(addr), 0x20 & hi1(base), "xas abs,y y={y}");
    }
}

// ---------------------------------------------------------------------------------------------
// GH#292: undocumented 65C02 NOPs
// ---------------------------------------------------------------------------------------------

#[test]
fn gh292_undocumented_65c02_nops_are_one_cycle_one_to_four_bytes() {
    let mut t = T::new();
    for op in (0..256u16).step_by(0x10) {
        let op = op as u8;
        t.reset();
        let base = t.cpu.pc;
        for (i, low) in [0x03u8, 0x07, 0x0B, 0x0F].into_iter().enumerate() {
            // the C++ test deliberately executes these as consecutive 1-byte instructions
            let pc = t.cpu.pc;
            *t.m(pc) = op + low;
            assert_eq!(t.c(), 1, "opcode {:02X}", op + low);
            assert_eq!(t.cpu.pc, base + 1 + i as u16, "opcode {:02X}", op + low);
        }
    }
}

#[test]
fn gh292_ldd_loads_and_discards() {
    let mut t = T::new();
    for (op, count) in [(0xDCu8, 1u32), (0xFC, 2)] {
        t.reset();
        let base = t.cpu.pc;
        t.put(base, &[op, 0x00, 0xC0]);
        assert_eq!(t.c(), 4);
        assert_eq!(t.cpu.pc, base + 3);
        assert_eq!(t.ram.c000_reads, count);
        assert_eq!(t.cpu.a, 0);
    }
}

// ---------------------------------------------------------------------------------------------
// GH#1257: JSR reads its high byte *after* pushing the return address
// ---------------------------------------------------------------------------------------------

#[test]
fn gh1257_jsr_reads_high_byte_after_the_push() {
    let code: [[u8; 6]; 3] = [
        [0xA2, 0x7C, 0x9A, 0x20, 0x55, 0x13], // LDX #$7C; TXS; JSR $1355
        [0xA2, 0x7D, 0x9A, 0x20, 0x55, 0x13], // ... actually JSRs to $0155
        [0xA2, 0x7E, 0x9A, 0x20, 0x55, 0x13], // ... actually JSRs to $7D55
    ];
    let res_pc = [0x1355u16, 0x0155, 0x7D55];
    for v in [N, C] {
        let mut t = T::new();
        for i in 0..3 {
            t.put(0x178, &code[i]);
            *t.m(res_pc[i]) = 0x00; // BRK
            t.reset();
            t.cpu.pc = 0x178;
            for _ in 0..3 {
                t.run(v);
            }
            assert_eq!(t.cpu.pc, res_pc[i], "{v:?} case {i}");
        }
    }
}
