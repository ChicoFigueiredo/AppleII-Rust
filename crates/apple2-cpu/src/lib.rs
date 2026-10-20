//! MOS 6502 / 65C02 CPU core.
//!
//! Behaviour follows AppleWin's `CPU/` sources: every bus access costs one cycle
//! and calls [`Bus::tick`]; "internal" cycles do not touch the bus (so they never
//! trigger Apple II soft switches); invalid 65C02 opcodes are NOPs with the same
//! sizes/cycle counts as AppleWin; the Rockwell/WDC opcodes (RMB/SMB/BBR/BBS, WAI, STP) are not
//! present, exactly as in AppleWin's `cpu65C02.h` ($CB and $DB are 1-cycle NOPs).
//!
//! Like AppleWin, the core performs *no* false reads: an indexed store never reads the
//! unindexed address, a read-modify-write is one read plus one write (no double write), and
//! the NMOS undocumented NOPs do not touch their operand address. (The only false read in
//! AppleWin is the Mockingboard 6522 hack of GH#52, which lives in the card.) The undocumented
//! NMOS opcodes are ported from `cpu_instructions.inl`/`cpu6502.h`, quirks included.

#![forbid(unsafe_code)]

/// Memory bus seen by the CPU.
pub trait Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
    /// Called once for every CPU cycle consumed (bus access or internal).
    fn tick(&mut self) {}
    /// Called right after the opcode byte is fetched (before operands), with the
    /// opcode's address. Lets the board know which instruction is executing
    /// (AppleWin GH#700: RMW abs on Language Card switches).
    fn opcode_fetched(&mut self, _pc: u16, _opcode: u8) {}
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Variant {
    /// NMOS 6502 (Apple II, II+, IIe).
    Nmos6502,
    /// CMOS 65C02 (Enhanced IIe, IIc).
    Cmos65C02,
}

pub const FLAG_C: u8 = 0x01;
pub const FLAG_Z: u8 = 0x02;
pub const FLAG_I: u8 = 0x04;
pub const FLAG_D: u8 = 0x08;
pub const FLAG_B: u8 = 0x10;
pub const FLAG_U: u8 = 0x20;
pub const FLAG_V: u8 = 0x40;
pub const FLAG_N: u8 = 0x80;

pub const VEC_NMI: u16 = 0xFFFA;
pub const VEC_RESET: u16 = 0xFFFC;
pub const VEC_IRQ: u16 = 0xFFFE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Imm,
    Zp,
    ZpX,
    ZpY,
    Abs,
    AbsX,
    AbsY,
    IndX,
    IndY,
    IndZ,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Adc, And, Asl, Bit, Cmp, Cpx, Cpy, Dec, Eor, Inc, Lda, Ldx, Ldy, Lsr, Ora, Rol, Ror, Sbc,
    Sta, Stx, Sty, Stz, Trb, Tsb,
    // NMOS undocumented (AppleWin names)
    Aso, Rla, Lse, Rra, Dcm, Ins, Axs, Lax, Anc, Alr, Arr, Xaa, Oal, Sax, Las, Nop,
    Axa, Say, Tas, Xas,
    // 65C02 undocumented "LoaD and Discard" (reads, result thrown away)
    Ldd,
}

#[derive(Clone, Debug)]
pub struct Cpu {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub p: u8,
    pub cycles: u64,
    pub variant: Variant,
    /// Level-triggered IRQ line.
    pub irq: bool,
    nmi_pending: bool,
    /// STP state (never entered: AppleWin's 65C02 has no STP). Kept for snapshots.
    pub stopped: bool,
    /// WAI state (never entered: AppleWin's 65C02 has no WAI). Kept for snapshots.
    pub waiting: bool,
    /// Opcode of the instruction currently (or last) executed.
    pub opcode: u8,
    /// An NMOS HLT/KIL opcode was executed (AppleWin `regs.bJammed`): the PC is left on the
    /// opcode, so it re-executes (2 cycles) until reset; interrupts are still serviced.
    pub jammed: bool,
}

impl Cpu {
    pub fn new(variant: Variant) -> Self {
        Cpu {
            a: 0, x: 0, y: 0, sp: 0xFF, pc: 0, p: FLAG_U | FLAG_I, cycles: 0, variant,
            irq: false, nmi_pending: false, stopped: false, waiting: false, opcode: 0, jammed: false,
        }
    }

    /// NMI edge latched but not yet taken (save-states).
    pub fn nmi_pending(&self) -> bool {
        self.nmi_pending
    }

    pub fn set_nmi_pending(&mut self, pending: bool) {
        self.nmi_pending = pending;
    }

    pub fn is_cmos(&self) -> bool {
        self.variant == Variant::Cmos65C02
    }

    /// Edge-triggered NMI request.
    pub fn nmi(&mut self) {
        self.nmi_pending = true;
    }

    pub fn reset<B: Bus>(&mut self, bus: &mut B) {
        self.sp = 0xFF;
        self.p = FLAG_U | FLAG_I;
        self.stopped = false;
        self.waiting = false;
        self.jammed = false;
        self.nmi_pending = false;
        let lo = bus.read(VEC_RESET) as u16;
        let hi = bus.read(VEC_RESET + 1) as u16;
        self.pc = lo | (hi << 8);
        self.cycles += 7;
    }

    #[inline]
    fn idle<B: Bus>(&mut self, bus: &mut B) {
        self.cycles += 1;
        bus.tick();
    }

    #[inline]
    fn rd<B: Bus>(&mut self, bus: &mut B, addr: u16) -> u8 {
        let v = bus.read(addr);
        self.cycles += 1;
        bus.tick();
        v
    }

    #[inline]
    fn wr<B: Bus>(&mut self, bus: &mut B, addr: u16, v: u8) {
        bus.write(addr, v);
        self.cycles += 1;
        bus.tick();
    }

    #[inline]
    fn fetch<B: Bus>(&mut self, bus: &mut B) -> u8 {
        let v = self.rd(bus, self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    #[inline]
    fn fetch16<B: Bus>(&mut self, bus: &mut B) -> u16 {
        let lo = self.fetch(bus) as u16;
        let hi = self.fetch(bus) as u16;
        lo | (hi << 8)
    }

    fn push<B: Bus>(&mut self, bus: &mut B, v: u8) {
        self.wr(bus, 0x0100 | self.sp as u16, v);
        self.sp = self.sp.wrapping_sub(1);
    }

    fn pull<B: Bus>(&mut self, bus: &mut B) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.rd(bus, 0x0100 | self.sp as u16)
    }

    #[inline]
    fn set_nz(&mut self, v: u8) {
        self.p = (self.p & !(FLAG_N | FLAG_Z)) | (v & FLAG_N) | if v == 0 { FLAG_Z } else { 0 };
    }

    #[inline]
    fn set_flag(&mut self, f: u8, on: bool) {
        if on { self.p |= f } else { self.p &= !f }
    }

    /// Effective address. `write` forces the indexed-mode extra cycle that reads
    /// always pay only on a page crossing.
    fn ea<B: Bus>(&mut self, bus: &mut B, mode: Mode, write: bool) -> u16 {
        match mode {
            Mode::Imm => {
                let a = self.pc;
                self.pc = self.pc.wrapping_add(1);
                a
            }
            Mode::Zp => self.fetch(bus) as u16,
            Mode::ZpX => {
                let z = self.fetch(bus);
                self.idle(bus);
                z.wrapping_add(self.x) as u16
            }
            Mode::ZpY => {
                let z = self.fetch(bus);
                self.idle(bus);
                z.wrapping_add(self.y) as u16
            }
            Mode::Abs => self.fetch16(bus),
            Mode::AbsX | Mode::AbsY => {
                let base = self.fetch16(bus);
                let idx = if mode == Mode::AbsX { self.x } else { self.y } as u16;
                let ea = base.wrapping_add(idx);
                if write || (base & 0xFF00) != (ea & 0xFF00) {
                    self.idle(bus);
                }
                ea
            }
            Mode::IndX => {
                let z = self.fetch(bus);
                self.idle(bus);
                let z = z.wrapping_add(self.x);
                let lo = self.rd(bus, z as u16) as u16;
                let hi = self.rd(bus, z.wrapping_add(1) as u16) as u16;
                lo | (hi << 8)
            }
            Mode::IndY => {
                let z = self.fetch(bus);
                let lo = self.rd(bus, z as u16) as u16;
                let hi = self.rd(bus, z.wrapping_add(1) as u16) as u16;
                let base = lo | (hi << 8);
                let ea = base.wrapping_add(self.y as u16);
                if write || (base & 0xFF00) != (ea & 0xFF00) {
                    self.idle(bus);
                }
                ea
            }
            Mode::IndZ => {
                let z = self.fetch(bus);
                let lo = self.rd(bus, z as u16) as u16;
                let hi = self.rd(bus, z.wrapping_add(1) as u16) as u16;
                lo | (hi << 8)
            }
        }
    }

    /// `ADC_NMOS` of cpu_instructions.inl. `temp` is a WORD in AppleWin (INS/RRA can pass 0x100).
    fn adc_nmos(&mut self, temp: u16) {
        let a = self.a as u16;
        let c = (self.p & FLAG_C) as u16;
        if self.p & FLAG_D != 0 {
            let mut val = (a & 0x0F) + (temp & 0x0F) + c;
            if val > 0x09 {
                val += 0x06;
            }
            if val <= 0x0F {
                val = (val & 0x0F) + (a & 0xF0) + (temp & 0xF0);
            } else {
                val = (val & 0x0F) + (a & 0xF0) + (temp & 0xF0) + 0x10;
            }
            let flagz = (a + temp + c) & 0xFF == 0;
            let flagn = val & 0x80 != 0;
            let flagv = ((a ^ val) & 0x80) != 0 && ((a ^ temp) & 0x80) == 0;
            if (val & 0x1F0) > 0x90 {
                val += 0x60;
            }
            self.set_flag(FLAG_C, (val & 0xFF0) > 0xF0);
            self.set_flag(FLAG_Z, flagz);
            self.set_flag(FLAG_N, flagn);
            self.set_flag(FLAG_V, flagv);
            self.a = val as u8;
        } else {
            let val = a + temp + c;
            self.set_flag(FLAG_C, val > 0xFF);
            self.set_flag(FLAG_V, (a & 0x80) == (temp & 0x80) && (a & 0x80) != (val & 0x80));
            self.a = val as u8;
            self.set_nz(self.a);
        }
    }

    /// `SBC_NMOS` of cpu_instructions.inl (WORD arithmetic, wrapping like the C++ casts).
    fn sbc_nmos(&mut self, temp: u16) {
        let a = self.a as u16;
        let nc = ((self.p & FLAG_C) ^ FLAG_C) as u16; // !flagc
        let temp2 = a.wrapping_sub(temp).wrapping_sub(nc);
        if self.p & FLAG_D != 0 {
            let mut val = (a & 0x0F).wrapping_sub(temp & 0x0F).wrapping_sub(nc);
            if val & 0x10 != 0 {
                val = (val.wrapping_sub(0x06) & 0x0F)
                    | (a & 0xF0).wrapping_sub(temp & 0xF0).wrapping_sub(0x10);
            } else {
                val = (val & 0x0F) | (a & 0xF0).wrapping_sub(temp & 0xF0);
            }
            if val & 0x100 != 0 {
                val = val.wrapping_sub(0x60);
            }
            self.set_flag(FLAG_C, temp2 < 0x100);
            self.set_nz(temp2 as u8);
            self.set_flag(FLAG_V, ((a ^ temp2) & 0x80) != 0 && ((a ^ temp) & 0x80) != 0);
            self.a = val as u8;
        } else {
            self.set_flag(FLAG_C, temp2 < 0x100);
            self.set_flag(FLAG_V, (a & 0x80) != (temp & 0x80) && (a & 0x80) != (temp2 & 0x80));
            self.a = temp2 as u8;
            self.set_nz(self.a);
        }
    }

    /// `ADC_CMOS` of cpu_instructions.inl.
    fn adc_cmos(&mut self, temp: u8) {
        let a = self.a as u32;
        let t = temp as u32;
        let c = (self.p & FLAG_C) as u32;
        let mut flagv = (a ^ t) & 0x80 == 0;
        let flagc;
        let mut val;
        if self.p & FLAG_D != 0 {
            val = (a & 0x0F) + (t & 0x0F) + c;
            if val >= 0x0A {
                val = 0x10 | ((val + 6) & 0x0F);
            }
            val += (a & 0xF0) + (t & 0xF0);
            if val >= 0xA0 {
                flagc = true;
                if val >= 0x180 {
                    flagv = false;
                }
                val += 0x60;
            } else {
                flagc = false;
                if val < 0x80 {
                    flagv = false;
                }
            }
        } else {
            val = a + t + c;
            if val >= 0x100 {
                flagc = true;
                if val >= 0x180 {
                    flagv = false;
                }
            } else {
                flagc = false;
                if val < 0x80 {
                    flagv = false;
                }
            }
        }
        self.set_flag(FLAG_C, flagc);
        self.set_flag(FLAG_V, flagv);
        self.a = val as u8;
        self.set_nz(self.a);
    }

    /// `SBC_CMOS` of cpu_instructions.inl.
    fn sbc_cmos(&mut self, temp: u8) {
        let a = self.a as i32;
        let t = temp as i32;
        let c = (self.p & FLAG_C) as i32;
        let mut flagv = (a ^ t) & 0x80 != 0;
        let flagc;
        let mut val: i32;
        if self.p & FLAG_D != 0 {
            let mut temp2 = 0x0F + (a & 0x0F) - (t & 0x0F) + c;
            if temp2 < 0x10 {
                val = 0;
                temp2 -= 0x06;
            } else {
                val = 0x10;
                temp2 -= 0x10;
            }
            val += 0xF0 + (a & 0xF0) - (t & 0xF0);
            if val < 0x100 {
                flagc = false;
                if val < 0x80 {
                    flagv = false;
                }
                val -= 0x60;
            } else {
                flagc = true;
                if val >= 0x180 {
                    flagv = false;
                }
            }
            val += temp2;
        } else {
            val = 0xFF + a - t + c;
            if val < 0x100 {
                flagc = false;
                if val < 0x80 {
                    flagv = false;
                }
            } else {
                flagc = true;
                if val >= 0x180 {
                    flagv = false;
                }
            }
        }
        self.set_flag(FLAG_C, flagc);
        self.set_flag(FLAG_V, flagv);
        self.a = val as u8;
        self.set_nz(self.a);
    }

    fn adc(&mut self, m: u8) {
        if self.is_cmos() { self.adc_cmos(m) } else { self.adc_nmos(m as u16) }
    }

    fn sbc(&mut self, m: u8) {
        if self.is_cmos() { self.sbc_cmos(m) } else { self.sbc_nmos(m as u16) }
    }

    /// `ARR` (immediate, NMOS undocumented), cpu_instructions.inl.
    fn arr(&mut self, m: u8) {
        let temp = (self.a & m) as u16;
        let c = self.p & FLAG_C != 0;
        let mut val = temp | if c { 0x100 } else { 0 };
        val >>= 1;
        if self.p & FLAG_D != 0 {
            self.set_flag(FLAG_N, c);
            self.set_flag(FLAG_Z, val & 0xFF == 0);
            self.set_flag(FLAG_V, (val ^ temp) & 0x40 != 0);
            if (val & 0x0F) + (val & 0x01) > 0x05 {
                val = (val & 0xF0) | ((val + 0x06) & 0x0F);
            }
            if (val & 0xF0) + (val & 0x10) > 0x50 {
                val = (val & 0x0F) | ((val + 0x60) & 0xF0);
                self.set_flag(FLAG_C, true);
            } else {
                self.set_flag(FLAG_C, false);
            }
            self.a = val as u8;
        } else {
            self.set_nz(val as u8);
            self.set_flag(FLAG_C, val & 0x40 != 0);
            self.set_flag(FLAG_V, ((val & 0x40) ^ ((val & 0x20) << 1)) != 0);
            self.a = val as u8;
        }
    }

    /// Read-modify-write undocumented NMOS ops (ASO/RLA/LSE/RRA/DCM/INS); returns the byte
    /// written back. The `u16` wrap-around of AppleWin's `WORD val` is kept (DCM of 0, INS of $FF).
    fn undoc_rmw(&mut self, op: Op, v: u8) -> u8 {
        let c = (self.p & FLAG_C) as u16;
        match op {
            Op::Aso => {
                let val = (v as u16) << 1;
                self.set_flag(FLAG_C, val > 0xFF);
                self.a |= val as u8;
                self.set_nz(self.a);
                val as u8
            }
            Op::Rla => {
                let val = ((v as u16) << 1) | c;
                self.set_flag(FLAG_C, val > 0xFF);
                self.a &= val as u8;
                self.set_nz(self.a);
                val as u8
            }
            Op::Lse => {
                self.set_flag(FLAG_C, v & 1 != 0);
                let val = v >> 1;
                self.a ^= val;
                self.set_nz(self.a);
                val
            }
            Op::Rra => {
                let val = (v >> 1) | if c != 0 { 0x80 } else { 0 };
                self.set_flag(FLAG_C, v & 1 != 0);
                self.adc_nmos(val as u16);
                val
            }
            Op::Dcm => {
                let val = (v as u16).wrapping_sub(1);
                let a = self.a as u16;
                self.set_flag(FLAG_C, a >= val);
                self.set_nz(a.wrapping_sub(val) as u8);
                val as u8
            }
            Op::Ins => {
                let val = v as u16 + 1;
                self.sbc_nmos(val);
                val as u8
            }
            _ => unreachable!(),
        }
    }

    fn cmp(&mut self, reg: u8, m: u8) {
        let r = reg.wrapping_sub(m);
        self.set_flag(FLAG_C, reg >= m);
        self.set_nz(r);
    }

    /// Pure read-modify ops on a value; returns the new value.
    fn rmw(&mut self, op: Op, v: u8) -> u8 {
        match op {
            Op::Asl => {
                self.set_flag(FLAG_C, v & 0x80 != 0);
                let r = v << 1;
                self.set_nz(r);
                r
            }
            Op::Lsr => {
                self.set_flag(FLAG_C, v & 1 != 0);
                let r = v >> 1;
                self.set_nz(r);
                r
            }
            Op::Rol => {
                let c = self.p & FLAG_C;
                self.set_flag(FLAG_C, v & 0x80 != 0);
                let r = (v << 1) | c;
                self.set_nz(r);
                r
            }
            Op::Ror => {
                let c = self.p & FLAG_C;
                self.set_flag(FLAG_C, v & 1 != 0);
                let r = (v >> 1) | (c << 7);
                self.set_nz(r);
                r
            }
            Op::Inc => {
                let r = v.wrapping_add(1);
                self.set_nz(r);
                r
            }
            Op::Dec => {
                let r = v.wrapping_sub(1);
                self.set_nz(r);
                r
            }
            Op::Trb => {
                self.set_flag(FLAG_Z, self.a & v == 0);
                v & !self.a
            }
            Op::Tsb => {
                self.set_flag(FLAG_Z, self.a & v == 0);
                v | self.a
            }
            _ => unreachable!(),
        }
    }

    fn read_op(&mut self, op: Op, m: u8, imm: bool) {
        match op {
            Op::Adc => self.adc(m),
            Op::Sbc => self.sbc(m),
            Op::And => { self.a &= m; self.set_nz(self.a) }
            Op::Ora => { self.a |= m; self.set_nz(self.a) }
            Op::Eor => { self.a ^= m; self.set_nz(self.a) }
            Op::Lda => { self.a = m; self.set_nz(m) }
            Op::Ldx => { self.x = m; self.set_nz(m) }
            Op::Ldy => { self.y = m; self.set_nz(m) }
            Op::Cmp => self.cmp(self.a, m),
            Op::Cpx => self.cmp(self.x, m),
            Op::Cpy => self.cmp(self.y, m),
            Op::Bit => {
                self.set_flag(FLAG_Z, self.a & m == 0);
                if !imm {
                    self.p = (self.p & !(FLAG_N | FLAG_V)) | (m & (FLAG_N | FLAG_V));
                }
            }
            Op::Lax => { self.a = m; self.x = m; self.set_nz(m) }
            Op::Anc => {
                self.a &= m;
                self.set_nz(self.a);
                self.set_flag(FLAG_C, self.a & 0x80 != 0);
            }
            Op::Alr => {
                self.a &= m;
                self.set_flag(FLAG_C, self.a & 1 != 0);
                self.a >>= 1;
                self.set_nz(self.a); // N is 0 after the shift
            }
            Op::Arr => self.arr(m),
            Op::Xaa => { self.a = self.x & m; self.set_nz(self.a) }
            Op::Oal => {
                self.a = (self.a | 0xEE) & m;
                self.x = self.a;
                self.set_nz(self.a);
            }
            Op::Sax => {
                let t = self.a & self.x;
                self.set_flag(FLAG_C, t >= m);
                self.x = t.wrapping_sub(m);
                self.set_nz(self.x);
            }
            Op::Las => {
                let v = m & self.sp;
                self.a = v;
                self.x = v;
                self.sp = v;
                self.set_nz(v);
            }
            _ => unreachable!(),
        }
    }

    fn branch<B: Bus>(&mut self, bus: &mut B, cond: bool) {
        let off = self.fetch(bus) as i8;
        if cond {
            self.idle(bus);
            let target = self.pc.wrapping_add(off as i16 as u16);
            if (target & 0xFF00) != (self.pc & 0xFF00) {
                self.idle(bus);
            }
            self.pc = target;
        }
    }

    fn interrupt<B: Bus>(&mut self, bus: &mut B, vector: u16, brk: bool) {
        self.push(bus, (self.pc >> 8) as u8);
        self.push(bus, self.pc as u8);
        let p = (self.p | FLAG_U) & !FLAG_B | if brk { FLAG_B } else { 0 };
        self.push(bus, p);
        self.p |= FLAG_I;
        if self.is_cmos() {
            self.p &= !FLAG_D;
        }
        let lo = self.rd(bus, vector) as u16;
        let hi = self.rd(bus, vector + 1) as u16;
        self.pc = lo | (hi << 8);
    }

    /// Executes one instruction (or services one interrupt); returns cycles used.
    pub fn step<B: Bus>(&mut self, bus: &mut B) -> u32 {
        let start = self.cycles;
        if self.stopped {
            self.idle(bus);
            return 1;
        }
        if self.nmi_pending {
            self.nmi_pending = false;
            self.waiting = false;
            self.idle(bus);
            self.idle(bus);
            self.interrupt(bus, VEC_NMI, false);
            return (self.cycles - start) as u32;
        }
        if self.irq {
            self.waiting = false;
            if self.p & FLAG_I == 0 {
                self.idle(bus);
                self.idle(bus);
                self.interrupt(bus, VEC_IRQ, false);
                return (self.cycles - start) as u32;
            }
        }
        if self.waiting {
            self.idle(bus);
            return 1;
        }
        let op_pc = self.pc;
        let opcode = self.fetch(bus);
        self.opcode = opcode;
        bus.opcode_fetched(op_pc, opcode);
        self.execute(bus, opcode);
        (self.cycles - start) as u32
    }

    fn execute<B: Bus>(&mut self, bus: &mut B, opcode: u8) {
        let cmos = self.is_cmos();
        use Mode::*;
        use Op::*;
        // (op, mode) for the regular memory instructions.
        let m: Option<(Op, Mode)> = match opcode {
            0x69 => Some((Adc, Imm)), 0x65 => Some((Adc, Zp)), 0x75 => Some((Adc, ZpX)),
            0x6D => Some((Adc, Abs)), 0x7D => Some((Adc, AbsX)), 0x79 => Some((Adc, AbsY)),
            0x61 => Some((Adc, IndX)), 0x71 => Some((Adc, IndY)),
            0x72 if cmos => Some((Adc, IndZ)),
            0xE9 => Some((Sbc, Imm)), 0xE5 => Some((Sbc, Zp)), 0xF5 => Some((Sbc, ZpX)),
            0xED => Some((Sbc, Abs)), 0xFD => Some((Sbc, AbsX)), 0xF9 => Some((Sbc, AbsY)),
            0xE1 => Some((Sbc, IndX)), 0xF1 => Some((Sbc, IndY)),
            0xF2 if cmos => Some((Sbc, IndZ)),
            0x29 => Some((And, Imm)), 0x25 => Some((And, Zp)), 0x35 => Some((And, ZpX)),
            0x2D => Some((And, Abs)), 0x3D => Some((And, AbsX)), 0x39 => Some((And, AbsY)),
            0x21 => Some((And, IndX)), 0x31 => Some((And, IndY)),
            0x32 if cmos => Some((And, IndZ)),
            0x09 => Some((Ora, Imm)), 0x05 => Some((Ora, Zp)), 0x15 => Some((Ora, ZpX)),
            0x0D => Some((Ora, Abs)), 0x1D => Some((Ora, AbsX)), 0x19 => Some((Ora, AbsY)),
            0x01 => Some((Ora, IndX)), 0x11 => Some((Ora, IndY)),
            0x12 if cmos => Some((Ora, IndZ)),
            0x49 => Some((Eor, Imm)), 0x45 => Some((Eor, Zp)), 0x55 => Some((Eor, ZpX)),
            0x4D => Some((Eor, Abs)), 0x5D => Some((Eor, AbsX)), 0x59 => Some((Eor, AbsY)),
            0x41 => Some((Eor, IndX)), 0x51 => Some((Eor, IndY)),
            0x52 if cmos => Some((Eor, IndZ)),
            0xA9 => Some((Lda, Imm)), 0xA5 => Some((Lda, Zp)), 0xB5 => Some((Lda, ZpX)),
            0xAD => Some((Lda, Abs)), 0xBD => Some((Lda, AbsX)), 0xB9 => Some((Lda, AbsY)),
            0xA1 => Some((Lda, IndX)), 0xB1 => Some((Lda, IndY)),
            0xB2 if cmos => Some((Lda, IndZ)),
            0xA2 => Some((Ldx, Imm)), 0xA6 => Some((Ldx, Zp)), 0xB6 => Some((Ldx, ZpY)),
            0xAE => Some((Ldx, Abs)), 0xBE => Some((Ldx, AbsY)),
            0xA0 => Some((Ldy, Imm)), 0xA4 => Some((Ldy, Zp)), 0xB4 => Some((Ldy, ZpX)),
            0xAC => Some((Ldy, Abs)), 0xBC => Some((Ldy, AbsX)),
            0xC9 => Some((Cmp, Imm)), 0xC5 => Some((Cmp, Zp)), 0xD5 => Some((Cmp, ZpX)),
            0xCD => Some((Cmp, Abs)), 0xDD => Some((Cmp, AbsX)), 0xD9 => Some((Cmp, AbsY)),
            0xC1 => Some((Cmp, IndX)), 0xD1 => Some((Cmp, IndY)),
            0xD2 if cmos => Some((Cmp, IndZ)),
            0xE0 => Some((Cpx, Imm)), 0xE4 => Some((Cpx, Zp)), 0xEC => Some((Cpx, Abs)),
            0xC0 => Some((Cpy, Imm)), 0xC4 => Some((Cpy, Zp)), 0xCC => Some((Cpy, Abs)),
            0x24 => Some((Bit, Zp)), 0x2C => Some((Bit, Abs)),
            0x89 if cmos => Some((Bit, Imm)),
            0x34 if cmos => Some((Bit, ZpX)),
            0x3C if cmos => Some((Bit, AbsX)),
            // stores
            0x85 => Some((Sta, Zp)), 0x95 => Some((Sta, ZpX)), 0x8D => Some((Sta, Abs)),
            0x9D => Some((Sta, AbsX)), 0x99 => Some((Sta, AbsY)), 0x81 => Some((Sta, IndX)),
            0x91 => Some((Sta, IndY)),
            0x92 if cmos => Some((Sta, IndZ)),
            0x86 => Some((Stx, Zp)), 0x96 => Some((Stx, ZpY)), 0x8E => Some((Stx, Abs)),
            0x84 => Some((Sty, Zp)), 0x94 => Some((Sty, ZpX)), 0x8C => Some((Sty, Abs)),
            0x64 if cmos => Some((Stz, Zp)), 0x74 if cmos => Some((Stz, ZpX)),
            0x9C if cmos => Some((Stz, Abs)), 0x9E if cmos => Some((Stz, AbsX)),
            // read-modify-write
            0x06 => Some((Asl, Zp)), 0x16 => Some((Asl, ZpX)), 0x0E => Some((Asl, Abs)), 0x1E => Some((Asl, AbsX)),
            0x46 => Some((Lsr, Zp)), 0x56 => Some((Lsr, ZpX)), 0x4E => Some((Lsr, Abs)), 0x5E => Some((Lsr, AbsX)),
            0x26 => Some((Rol, Zp)), 0x36 => Some((Rol, ZpX)), 0x2E => Some((Rol, Abs)), 0x3E => Some((Rol, AbsX)),
            0x66 => Some((Ror, Zp)), 0x76 => Some((Ror, ZpX)), 0x6E => Some((Ror, Abs)), 0x7E => Some((Ror, AbsX)),
            0xE6 => Some((Inc, Zp)), 0xF6 => Some((Inc, ZpX)), 0xEE => Some((Inc, Abs)), 0xFE => Some((Inc, AbsX)),
            0xC6 => Some((Dec, Zp)), 0xD6 => Some((Dec, ZpX)), 0xCE => Some((Dec, Abs)), 0xDE => Some((Dec, AbsX)),
            0x14 if cmos => Some((Trb, Zp)), 0x1C if cmos => Some((Trb, Abs)),
            0x04 if cmos => Some((Tsb, Zp)), 0x0C if cmos => Some((Tsb, Abs)),
            // 65C02 undocumented NOPs that do touch the bus / consume a data cycle
            0x02 | 0x22 | 0x42 | 0x62 | 0x82 | 0xC2 | 0xE2 if cmos => Some((Nop, Imm)),
            0x44 if cmos => Some((Nop, Zp)),
            0x54 | 0xD4 | 0xF4 if cmos => Some((Nop, ZpX)),
            0xDC | 0xFC if cmos => Some((Ldd, Abs)),
            _ if !cmos => nmos_undocumented(opcode),
            _ => None,
        };
        if let Some((op, mode)) = m {
            match op {
                Sta | Stx | Sty | Stz | Axs => {
                    let ea = self.ea(bus, mode, true);
                    let v = match op {
                        Sta => self.a,
                        Stx => self.x,
                        Sty => self.y,
                        Axs => self.a & self.x,
                        _ => 0,
                    };
                    self.wr(bus, ea, v);
                }
                Asl | Lsr | Rol | Ror | Inc | Dec | Trb | Tsb => {
                    // 65C02 shifts/rotates on abs,X take 6 cycles (+1 on page cross).
                    let force = !(cmos && mode == AbsX && matches!(op, Asl | Lsr | Rol | Ror));
                    let ea = self.ea(bus, mode, force);
                    let v = self.rd(bus, ea);
                    self.idle(bus);
                    let r = self.rmw(op, v);
                    self.wr(bus, ea, r);
                }
                Aso | Rla | Lse | Rra | Dcm | Ins => {
                    let ea = self.ea(bus, mode, true);
                    let v = self.rd(bus, ea);
                    self.idle(bus);
                    let r = self.undoc_rmw(op, v);
                    self.wr(bus, ea, r);
                }
                // NOPs consume the data cycle but, as in AppleWin, never access the operand address.
                Nop => {
                    self.ea(bus, mode, false);
                    self.idle(bus);
                }
                Ldd => {
                    let ea = self.ea(bus, mode, false);
                    self.rd(bus, ea);
                }
                Axa | Say | Tas | Xas => self.store_and_high(bus, op, mode),
                _ => {
                    let ea = self.ea(bus, mode, false);
                    let v = self.rd(bus, ea);
                    self.read_op(op, v, mode == Imm);
                    if cmos && matches!(op, Adc | Sbc) && self.p & FLAG_D != 0 {
                        self.idle(bus); // 65C02 decimal-mode ADC/SBC: +1 cycle
                    }
                }
            }
            return;
        }
        match opcode {
            0x0A | 0x2A | 0x4A | 0x6A => {
                self.idle(bus);
                let op = match opcode { 0x0A => Asl, 0x2A => Rol, 0x4A => Lsr, _ => Ror };
                self.a = self.rmw(op, self.a);
            }
            0x1A if cmos => { self.idle(bus); self.a = self.rmw(Inc, self.a); }
            0x3A if cmos => { self.idle(bus); self.a = self.rmw(Dec, self.a); }
            0xEA => self.idle(bus),
            0xAA => { self.idle(bus); self.x = self.a; self.set_nz(self.x) }
            0xA8 => { self.idle(bus); self.y = self.a; self.set_nz(self.y) }
            0x8A => { self.idle(bus); self.a = self.x; self.set_nz(self.a) }
            0x98 => { self.idle(bus); self.a = self.y; self.set_nz(self.a) }
            0xBA => { self.idle(bus); self.x = self.sp; self.set_nz(self.x) }
            0x9A => { self.idle(bus); self.sp = self.x }
            0xE8 => { self.idle(bus); self.x = self.x.wrapping_add(1); self.set_nz(self.x) }
            0xC8 => { self.idle(bus); self.y = self.y.wrapping_add(1); self.set_nz(self.y) }
            0xCA => { self.idle(bus); self.x = self.x.wrapping_sub(1); self.set_nz(self.x) }
            0x88 => { self.idle(bus); self.y = self.y.wrapping_sub(1); self.set_nz(self.y) }
            0x18 => { self.idle(bus); self.p &= !FLAG_C }
            0x38 => { self.idle(bus); self.p |= FLAG_C }
            0x58 => { self.idle(bus); self.p &= !FLAG_I }
            0x78 => { self.idle(bus); self.p |= FLAG_I }
            0xB8 => { self.idle(bus); self.p &= !FLAG_V }
            0xD8 => { self.idle(bus); self.p &= !FLAG_D }
            0xF8 => { self.idle(bus); self.p |= FLAG_D }
            0x48 => { self.idle(bus); self.push(bus, self.a) }
            0x08 => { self.idle(bus); self.push(bus, self.p | FLAG_B | FLAG_U) }
            0x68 => { self.idle(bus); self.idle(bus); self.a = self.pull(bus); self.set_nz(self.a) }
            0x28 => {
                self.idle(bus);
                self.idle(bus);
                self.p = (self.pull(bus) | FLAG_U) & !FLAG_B;
            }
            0xDA if cmos => { self.idle(bus); self.push(bus, self.x) }
            0x5A if cmos => { self.idle(bus); self.push(bus, self.y) }
            0xFA if cmos => { self.idle(bus); self.idle(bus); self.x = self.pull(bus); self.set_nz(self.x) }
            0x7A if cmos => { self.idle(bus); self.idle(bus); self.y = self.pull(bus); self.set_nz(self.y) }
            0x10 => self.branch(bus, self.p & FLAG_N == 0),
            0x30 => self.branch(bus, self.p & FLAG_N != 0),
            0x50 => self.branch(bus, self.p & FLAG_V == 0),
            0x70 => self.branch(bus, self.p & FLAG_V != 0),
            0x90 => self.branch(bus, self.p & FLAG_C == 0),
            0xB0 => self.branch(bus, self.p & FLAG_C != 0),
            0xD0 => self.branch(bus, self.p & FLAG_Z == 0),
            0xF0 => self.branch(bus, self.p & FLAG_Z != 0),
            0x80 if cmos => self.branch(bus, true),
            0x4C => self.pc = self.fetch16(bus),
            0x6C => {
                let ptr = self.fetch16(bus);
                let lo = self.rd(bus, ptr) as u16;
                let hi_addr = if cmos {
                    self.idle(bus);
                    if ptr & 0xFF == 0xFF {
                        self.idle(bus); // IABS_CMOS: +1 cycle when the pointer straddles a page
                    }
                    ptr.wrapping_add(1)
                } else {
                    (ptr & 0xFF00) | (ptr.wrapping_add(1) & 0x00FF)
                };
                let hi = self.rd(bus, hi_addr) as u16;
                self.pc = lo | (hi << 8);
            }
            0x7C if cmos => {
                let base = self.fetch16(bus);
                self.idle(bus);
                let ptr = base.wrapping_add(self.x as u16);
                let lo = self.rd(bus, ptr) as u16;
                let hi = self.rd(bus, ptr.wrapping_add(1)) as u16;
                self.pc = lo | (hi << 8);
            }
            0x20 => {
                let lo = self.fetch(bus) as u16;
                self.idle(bus);
                let ret = self.pc; // address of the high byte = return - 1
                self.push(bus, (ret >> 8) as u8);
                self.push(bus, ret as u8);
                let hi = self.fetch(bus) as u16;
                self.pc = lo | (hi << 8);
            }
            0x60 => {
                self.idle(bus);
                self.idle(bus);
                let lo = self.pull(bus) as u16;
                let hi = self.pull(bus) as u16;
                self.idle(bus);
                self.pc = (lo | (hi << 8)).wrapping_add(1);
            }
            0x40 => {
                self.idle(bus);
                self.idle(bus);
                self.p = (self.pull(bus) | FLAG_U) & !FLAG_B;
                let lo = self.pull(bus) as u16;
                let hi = self.pull(bus) as u16;
                self.pc = lo | (hi << 8);
            }
            0x00 => {
                self.idle(bus); // padding byte
                self.pc = self.pc.wrapping_add(1);
                self.interrupt(bus, VEC_IRQ, true);
            }
            // NMOS HLT/KIL (AppleWin: jam flag, PC stays on the opcode, 2 cycles)
            0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2
                if !cmos =>
            {
                self.idle(bus);
                self.jammed = true;
                self.pc = self.pc.wrapping_sub(1);
            }
            // NMOS 2-cycle implied NOPs
            0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA if !cmos => self.idle(bus),
            _ => self.invalid(bus, opcode),
        }
    }

    /// Remaining 65C02 undefined opcodes: $x3/$x7/$xB/$xF are 1-cycle 1-byte NOPs (AppleWin
    /// `NOP CYC(1)`, GH#292) and $5C is an 8-cycle 3-byte NOP; the rest was handled by the table.
    fn invalid<B: Bus>(&mut self, bus: &mut B, opcode: u8) {
        if opcode == 0x5C {
            self.fetch16(bus);
            for _ in 0..5 {
                self.idle(bus);
            }
        }
    }

    /// `AXA`/`SAY`/`TAS`/`XAS` (GH#282): store `reg & (high byte of base + 1)`; when the index
    /// crosses a page the value also replaces the high byte of the effective address.
    fn store_and_high<B: Bus>(&mut self, bus: &mut B, op: Op, mode: Mode) {
        let base = match mode {
            Mode::IndY => {
                let z = self.fetch(bus);
                let lo = self.rd(bus, z as u16) as u16;
                let hi = self.rd(bus, z.wrapping_add(1) as u16) as u16;
                lo | (hi << 8)
            }
            _ => self.fetch16(bus),
        };
        let idx = if mode == Mode::AbsX { self.x } else { self.y } as u16;
        let mut addr = base.wrapping_add(idx);
        self.idle(bus);
        let hi1 = ((base >> 8) as u8).wrapping_add(1);
        let val = match op {
            Op::Axa => self.a & self.x & hi1,
            Op::Say => self.y & hi1,
            Op::Tas => {
                self.sp = self.a & self.x;
                self.sp & hi1
            }
            _ => self.x & hi1, // Xas
        };
        if (base ^ addr) >> 8 != 0 {
            addr = ((val as u16) << 8) | (addr & 0xFF);
        }
        self.wr(bus, addr, val);
    }
}

/// NMOS undocumented opcodes implemented by AppleWin's `cpu6502.h` as (operation, mode).
/// Page-crossing "const" variants (fixed cycle count) vs "opt" ones is decided by the operation.
fn nmos_undocumented(opcode: u8) -> Option<(Op, Mode)> {
    use Mode::*;
    use Op::*;
    Some(match opcode {
        0x03 => (Aso, IndX), 0x07 => (Aso, Zp), 0x0F => (Aso, Abs), 0x13 => (Aso, IndY),
        0x17 => (Aso, ZpX), 0x1B => (Aso, AbsY), 0x1F => (Aso, AbsX),
        0x23 => (Rla, IndX), 0x27 => (Rla, Zp), 0x2F => (Rla, Abs), 0x33 => (Rla, IndY),
        0x37 => (Rla, ZpX), 0x3B => (Rla, AbsY), 0x3F => (Rla, AbsX),
        0x43 => (Lse, IndX), 0x47 => (Lse, Zp), 0x4F => (Lse, Abs), 0x53 => (Lse, IndY),
        0x57 => (Lse, ZpX), 0x5B => (Lse, AbsY), 0x5F => (Lse, AbsX),
        0x63 => (Rra, IndX), 0x67 => (Rra, Zp), 0x6F => (Rra, Abs), 0x73 => (Rra, IndY),
        0x77 => (Rra, ZpX), 0x7B => (Rra, AbsY), 0x7F => (Rra, AbsX),
        0xC3 => (Dcm, IndX), 0xC7 => (Dcm, Zp), 0xCF => (Dcm, Abs), 0xD3 => (Dcm, IndY),
        0xD7 => (Dcm, ZpX), 0xDB => (Dcm, AbsY), 0xDF => (Dcm, AbsX),
        0xE3 => (Ins, IndX), 0xE7 => (Ins, Zp), 0xEF => (Ins, Abs), 0xF3 => (Ins, IndY),
        0xF7 => (Ins, ZpX), 0xFB => (Ins, AbsY), 0xFF => (Ins, AbsX),
        0x83 => (Axs, IndX), 0x87 => (Axs, Zp), 0x8F => (Axs, Abs), 0x97 => (Axs, ZpY),
        0xA3 => (Lax, IndX), 0xA7 => (Lax, Zp), 0xAF => (Lax, Abs), 0xB3 => (Lax, IndY),
        0xB7 => (Lax, ZpY), 0xBF => (Lax, AbsY),
        0x0B | 0x2B => (Anc, Imm),
        0x4B => (Alr, Imm),
        0x6B => (Arr, Imm),
        0x8B => (Xaa, Imm),
        0xAB => (Oal, Imm),
        0xCB => (Sax, Imm),
        0xEB => (Sbc, Imm),
        0xBB => (Las, AbsY),
        0x93 => (Axa, IndY), 0x9F => (Axa, AbsY), 0x9C => (Say, AbsX),
        0x9B => (Tas, AbsY), 0x9E => (Xas, AbsY),
        // NOPs
        0x80 | 0x82 | 0x89 | 0xC2 | 0xE2 => (Nop, Imm),
        0x04 | 0x44 | 0x64 => (Nop, Zp),
        0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => (Nop, ZpX),
        0x0C => (Nop, Abs),
        0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => (Nop, AbsX),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Mem([u8; 65536]);
    impl Bus for Mem {
        fn read(&mut self, a: u16) -> u8 { self.0[a as usize] }
        fn write(&mut self, a: u16, v: u8) { self.0[a as usize] = v }
    }

    fn run1(variant: Variant, code: &[u8], setup: impl FnOnce(&mut Cpu)) -> (Cpu, u32) {
        let mut m = Mem([0; 65536]);
        m.0[0x300..0x300 + code.len()].copy_from_slice(code);
        let mut c = Cpu::new(variant);
        c.pc = 0x300;
        setup(&mut c);
        let n = c.step(&mut m);
        (c, n)
    }

    #[test]
    fn cycle_counts_match_apple_ii_timing() {
        // LDA #imm 2, LDA abs 4, LDA abs,X 4 (+1 crossing), STA abs,X 5, INC abs 6, JSR 6, RTS 6
        assert_eq!(run1(Variant::Nmos6502, &[0xA9, 1], |_| {}).1, 2);
        assert_eq!(run1(Variant::Nmos6502, &[0xAD, 0, 4], |_| {}).1, 4);
        assert_eq!(run1(Variant::Nmos6502, &[0xBD, 0, 4], |c| c.x = 1).1, 4);
        assert_eq!(run1(Variant::Nmos6502, &[0xBD, 0xFF, 4], |c| c.x = 1).1, 5);
        assert_eq!(run1(Variant::Nmos6502, &[0x9D, 0, 4], |_| {}).1, 5);
        assert_eq!(run1(Variant::Nmos6502, &[0xEE, 0, 4], |_| {}).1, 6);
        assert_eq!(run1(Variant::Nmos6502, &[0x20, 0, 4], |_| {}).1, 6);
        assert_eq!(run1(Variant::Nmos6502, &[0x60], |_| {}).1, 6);
        assert_eq!(run1(Variant::Nmos6502, &[0x00], |_| {}).1, 7);
        // Branch: not taken 2, taken 3, taken across page 4.
        assert_eq!(run1(Variant::Nmos6502, &[0xD0, 5], |c| c.p |= FLAG_Z).1, 2);
        assert_eq!(run1(Variant::Nmos6502, &[0xD0, 5], |_| {}).1, 3);
        let (c, n) = run1(Variant::Nmos6502, &[0xD0, 0x82], |_| {}); // -126: 0x302 -> 0x284
        assert_eq!((n, c.pc), (4, 0x284));
    }

    #[test]
    fn jmp_indirect_page_bug_only_on_nmos() {
        let mut m = Mem([0; 65536]);
        m.0[0x300..0x303].copy_from_slice(&[0x6C, 0xFF, 0x10]);
        m.0[0x10FF] = 0x34;
        m.0[0x1000] = 0x12; // NMOS wraps within the page
        m.0[0x1100] = 0x56;
        let mut n = Cpu::new(Variant::Nmos6502);
        n.pc = 0x300;
        n.step(&mut m);
        assert_eq!(n.pc, 0x1234);
        let mut c = Cpu::new(Variant::Cmos65C02);
        c.pc = 0x300;
        assert_eq!(c.step(&mut m), 7); // AppleWin IABS_CMOS: +1 when the pointer straddles a page
        assert_eq!(c.pc, 0x5634);
    }

    #[test]
    fn cmos_only_opcodes_are_nops_on_nmos() {
        // STZ $10 (0x64) is a 2-byte NOP-like on NMOS here, real STZ on CMOS.
        let (c, _) = run1(Variant::Cmos65C02, &[0x1A], |c| c.a = 4); // INC A
        assert_eq!(c.a, 5);
        let (c, _) = run1(Variant::Cmos65C02, &[0xDA], |c| c.x = 9); // PHX
        assert_eq!(c.sp, 0xFE);
    }

    #[test]
    fn decimal_mode_adc_sbc() {
        let (c, _) = run1(Variant::Nmos6502, &[0x69, 0x19], |c| { c.a = 0x28; c.p |= FLAG_D });
        assert_eq!(c.a, 0x47);
        let (c, _) = run1(Variant::Nmos6502, &[0xE9, 0x19], |c| { c.a = 0x46; c.p |= FLAG_D | FLAG_C });
        assert_eq!(c.a, 0x27);
        assert!(c.p & FLAG_C != 0);
    }

    #[test]
    fn opcode_hook_reports_fetched_opcode() {
        struct Spy { mem: [u8; 65536], seen: Vec<(u16, u8)> }
        impl Bus for Spy {
            fn read(&mut self, a: u16) -> u8 { self.mem[a as usize] }
            fn write(&mut self, a: u16, v: u8) { self.mem[a as usize] = v }
            fn opcode_fetched(&mut self, pc: u16, op: u8) { self.seen.push((pc, op)); }
        }
        let mut b = Spy { mem: [0; 65536], seen: vec![] };
        b.mem[0x300..0x305].copy_from_slice(&[0xA9, 0x01, 0xEE, 0x00, 0x04]);
        let mut c = Cpu::new(Variant::Nmos6502);
        c.pc = 0x300;
        c.step(&mut b);
        c.step(&mut b);
        assert_eq!(b.seen, vec![(0x300, 0xA9), (0x302, 0xEE)]);
        assert_eq!(c.opcode, 0xEE);
    }
}
