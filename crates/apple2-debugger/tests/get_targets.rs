//! Port of AppleWin `test/TestDebugger/TestDebugger.cpp` (GH#445 and GH#451): the effective
//! addresses `_6502_GetTargets` reports for stack, absolute, JMP and branch opcodes, including
//! stack-pointer and 64K wrap-around. The expected values are the C++ test's, verbatim.
//!
//! The C++ test runs once with the 65C02 opcode table and once with the 6502 one
//! (`g_aOpcodes = ...`); here that is the `cmos` flag of the context.
use apple2_debugger::disasm::{get_targets, DataBlocks, DisasmConfig, DisasmCtx, NO_TARGET};
use apple2_debugger::mem::{DebugMem, Ram, Regs};
use apple2_debugger::opcodes::{OPCODE_BRK, OPCODE_JMP_A, OPCODE_JMP_IAX, OPCODE_JMP_NA, OPCODE_JSR, OPCODE_RTI, OPCODE_RTS};
use apple2_debugger::symbols::Symbols;

const OPCODE_LDA_A: u8 = 0xAD;

struct T {
    ram: Ram,
    regs: Regs,
    cmos: bool,
}

impl T {
    fn new(cmos: bool) -> T {
        let mut ram = Ram::default();
        ram.write(0x200, 0xDD); // bad data if SP wrap does not work
        ram.write(0x201, 0xDD);
        T { ram, regs: Regs { pc: 0x300, ..Regs::default() }, cmos }
    }

    /// `regs.sp = 0x1xx` as in the C++ test (only the low byte is kept by the CPU).
    fn sp(&mut self, sp: u16) {
        assert!((0x100..0x200).contains(&sp));
        self.regs.sp = sp as u8;
    }

    fn sp_full(&self) -> i32 {
        0x100 + self.regs.sp as i32
    }

    fn get(&self, ignore_branch: bool, include_next: bool) -> (i32, i32, i32, u8) {
        let syms = Symbols::default();
        let data = DataBlocks::default();
        let cfg = DisasmConfig::default();
        let ctx = DisasmCtx { mem: &self.ram, regs: self.regs, cmos: self.cmos, symbols: &syms, data: &data, cfg: &cfg };
        get_targets(&ctx, self.regs.pc, ignore_branch, include_next)
    }

    fn put_opcode(&mut self, op: u8) {
        self.ram.write(self.regs.pc, op);
    }

    fn put_operand16(&mut self, v: u16) {
        self.ram.write(self.regs.pc.wrapping_add(1), v as u8);
        self.ram.write(self.regs.pc.wrapping_add(2), (v >> 8) as u8);
    }
}

// ---------------------------------------------------------------------------- GH#445

fn gh445_ph(t: &mut T, op: u8) {
    t.put_opcode(op);
    for sp in [0x1FE, 0x1FF, 0x100] {
        t.sp(sp);
        let (_, _, ptr, _) = t.get(true, true);
        assert_eq!(ptr, t.sp_full(), "PHn {op:02X} sp={sp:03X}");
    }
}

fn gh445_pl(t: &mut T, op: u8) {
    t.put_opcode(op);
    t.sp(0x1FE);
    assert_eq!(t.get(true, true).2, t.sp_full() + 1, "PLn {op:02X}");
    t.sp(0x1FF);
    assert_eq!(t.get(true, true).2, 0x100, "PLn {op:02X} wraps to $100");
    t.sp(0x100);
    assert_eq!(t.get(true, true).2, t.sp_full() + 1, "PLn {op:02X}");
}

fn gh445_abs(t: &mut T, op: u8) {
    let target2 = 0x1234;
    t.put_opcode(op);
    t.put_operand16(target2);
    assert_eq!(t.get(true, true).2, target2 as i32, "pc={:04X}", t.regs.pc);
}

fn gh445_jsr(t: &mut T) {
    let target2 = 0x1234;
    t.put_opcode(OPCODE_JSR);
    t.put_operand16(target2);
    for (sp, second) in [(0x1FF, 0x1FE), (0x100, 0x1FF), (0x101, 0x100)] {
        t.sp(sp);
        let (a, b, c, _) = t.get(true, true);
        assert_eq!((a, b, c), (sp as i32, second, target2 as i32), "JSR sp={sp:03X}");
    }
}

fn gh445_brk(t: &mut T) {
    t.put_opcode(OPCODE_BRK);
    for (sp, second) in [(0x1FF, 0x1FE), (0x100, 0x1FF), (0x101, 0x100)] {
        t.sp(sp);
        let (a, b, _, _) = t.get(true, true);
        assert_eq!((a, b), (sp as i32, second), "BRK sp={sp:03X}");
    }
}

/// Returns the stack addresses of the low/high return bytes and the expected return address.
fn rti_rts_setup(t: &mut T, sp: u16, is_rti: bool) -> (i32, i32, i32) {
    t.put_opcode(if is_rti { OPCODE_RTI } else { OPCODE_RTS });
    t.sp(sp);
    let s = t.regs.sp as u16;
    let (lo_addr, hi_addr);
    if is_rti {
        let p = 0x100 + ((s + 1) & 0xFF);
        lo_addr = 0x100 + ((s + 2) & 0xFF);
        hi_addr = 0x100 + ((s + 3) & 0xFF);
        t.ram.write(p, 0xEA);
    } else {
        lo_addr = 0x100 + ((s + 1) & 0xFF);
        hi_addr = 0x100 + ((s + 2) & 0xFF);
    }
    let mut ret = 0x1234u16;
    t.ram.write(lo_addr, ret as u8);
    t.ram.write(hi_addr, (ret >> 8) as u8);
    if !is_rti {
        ret += 1; // the return address from the stack is incremented before it goes to PC
    }
    (lo_addr as i32, hi_addr as i32, ret as i32)
}

fn gh445_rti_rts(t: &mut T, sp: u16, is_rti: bool) {
    let (lo, hi, ret) = rti_rts_setup(t, sp, is_rti);
    let got = t.get(true, true);
    assert_eq!((got.0, got.1, got.2), (lo, hi, ret), "{} sp={sp:03X}", if is_rti { "RTI" } else { "RTS" });
}

fn gh445_jmp(t: &mut T, op: u8) {
    let target16 = 0x1234u16;
    let (t0, t1, t2);
    if op == OPCODE_JMP_A {
        (t0, t1, t2) = (NO_TARGET, NO_TARGET, target16 as i32);
    } else if op == OPCODE_JMP_NA {
        t0 = target16 as i32;
        t1 = (target16 as i32 + 1) & 0xFFFF;
        t2 = 0x5678;
        t.ram.write(t0 as u16, t2 as u8);
        t.ram.write(t1 as u16, (t2 >> 8) as u8);
    } else {
        assert_eq!(op, OPCODE_JMP_IAX);
        t0 = (target16 as i32 + t.regs.x as i32) & 0xFFFF;
        t1 = (target16 as i32 + t.regs.x as i32 + 1) & 0xFFFF;
        t2 = 0xABCD;
        t.ram.write(t0 as u16, t2 as u8);
        t.ram.write(t1 as u16, (t2 >> 8) as u8);
    }
    t.put_opcode(op);
    t.put_operand16(target16);
    let got = t.get(true, true);
    assert_eq!((got.0, got.1, got.2), (t0, t1, t2), "JMP {op:02X}");
}

fn gh445_bcc(t: &mut T) {
    for off in [0x00u8, 0xFE] {
        t.put_opcode(0x10); // BPL next-op / this-op
        t.ram.write(t.regs.pc.wrapping_add(1), off);
        let got = t.get(true, true);
        assert_eq!((got.0, got.1, got.2), (NO_TARGET, NO_TARGET, NO_TARGET));
    }
}

fn gh445_sub(cmos: bool) {
    let mut t = T::new(cmos);
    t.regs.pc = 0x300;

    // PHn/PLn
    gh445_ph(&mut t, 0x08); // PHP
    gh445_ph(&mut t, 0x48); // PHA
    if cmos {
        gh445_ph(&mut t, 0x5A); // PHY
        gh445_ph(&mut t, 0xDA); // PHX
    }
    gh445_pl(&mut t, 0x28); // PLP
    gh445_pl(&mut t, 0x68); // PLA
    if cmos {
        gh445_pl(&mut t, 0x7A); // PLY
        gh445_pl(&mut t, 0xFA); // PLX
    }

    // LDA abs, with the operand wrapping around the 64K boundary
    for pc in [0xFFFD, 0xFFFE, 0xFFFF] {
        t.regs.pc = pc;
        gh445_abs(&mut t, OPCODE_LDA_A);
    }

    // JSR abs
    gh445_jsr(&mut t);

    // BRK (vector $FA40)
    t.ram.write(0xFFFE, 0x40);
    t.ram.write(0xFFFF, 0xFA);
    t.regs.pc = 0x300;
    gh445_brk(&mut t);

    // RTI / RTS
    for sp in [0x1FE, 0x1FF, 0x100] {
        gh445_rti_rts(&mut t, sp, true);
    }
    for sp in [0x1FE, 0x1FF, 0x100] {
        gh445_rti_rts(&mut t, sp, false);
    }

    // JMP
    gh445_jmp(&mut t, OPCODE_JMP_A);
    gh445_jmp(&mut t, OPCODE_JMP_NA);
    if cmos {
        t.regs.x = 0xFF;
        gh445_jmp(&mut t, OPCODE_JMP_IAX);
    }

    gh445_bcc(&mut t);
}

#[test]
fn gh445_targets_65c02() {
    gh445_sub(true);
}

#[test]
fn gh445_targets_6502() {
    gh445_sub(false);
}

// ---------------------------------------------------------------------------- GH#451
// bIncludeNextOpcodeAddress == false: TargetAddr[2] is set for data opcodes (LDA abs) but is
// NO_6502_TARGET for the control-flow ones (BRK, RTI, RTS, JSR, JMP, Bcc).

fn gh451_sub(cmos: bool) {
    let mut t = T::new(cmos);
    t.regs.pc = 0x300;

    // LDA abs
    gh445_abs_no_next(&mut t);

    // JSR abs
    t.put_opcode(OPCODE_JSR);
    for (sp, second) in [(0x1FF, 0x1FE), (0x100, 0x1FF), (0x101, 0x100)] {
        t.sp(sp);
        let (a, b, c, _) = t.get(true, false);
        assert_eq!((a, b, c), (sp as i32, second, NO_TARGET), "JSR sp={sp:03X}");
    }

    // BRK
    t.ram.write(0xFFFE, 0x40);
    t.ram.write(0xFFFF, 0xFA);
    t.put_opcode(OPCODE_BRK);
    for (sp, second) in [(0x1FF, 0x1FE), (0x100, 0x1FF), (0x101, 0x100)] {
        t.sp(sp);
        let (a, b, c, _) = t.get(true, false);
        assert_eq!((a, b, c), (sp as i32, second, NO_TARGET), "BRK sp={sp:03X}");
    }

    // RTI / RTS
    for is_rti in [true, false] {
        for sp in [0x1FE, 0x1FF, 0x100] {
            let (lo, hi, _) = rti_rts_setup(&mut t, sp, is_rti);
            let got = t.get(true, false);
            assert_eq!((got.0, got.1, got.2), (lo, hi, NO_TARGET), "rti={is_rti} sp={sp:03X}");
        }
    }

    // JMP
    let target16 = 0x1234u16;
    for op in [OPCODE_JMP_A, OPCODE_JMP_NA, OPCODE_JMP_IAX] {
        if op == OPCODE_JMP_IAX {
            if !cmos {
                continue;
            }
            t.regs.x = 0xFF;
        }
        let (t0, t1) = match op {
            OPCODE_JMP_A => (NO_TARGET, NO_TARGET),
            OPCODE_JMP_NA => (target16 as i32, target16 as i32 + 1),
            _ => ((target16 as i32 + t.regs.x as i32) & 0xFFFF, (target16 as i32 + t.regs.x as i32 + 1) & 0xFFFF),
        };
        t.put_opcode(op);
        t.put_operand16(target16);
        let got = t.get(true, false);
        assert_eq!((got.0, got.1, got.2), (t0, t1, NO_TARGET), "JMP {op:02X}");
    }

    // Bcc
    for off in [0x00u8, 0xFE] {
        t.put_opcode(0x10);
        t.ram.write(t.regs.pc.wrapping_add(1), off);
        let got = t.get(true, false);
        assert_eq!((got.0, got.1, got.2), (NO_TARGET, NO_TARGET, NO_TARGET));
    }
}

fn gh445_abs_no_next(t: &mut T) {
    let target2 = 0x1234;
    t.put_opcode(OPCODE_LDA_A);
    t.put_operand16(target2);
    assert_eq!(t.get(true, false).2, target2 as i32);
}

#[test]
fn gh451_targets_65c02() {
    gh451_sub(true);
}

#[test]
fn gh451_targets_6502() {
    gh451_sub(false);
}
