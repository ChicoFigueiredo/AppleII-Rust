//! Zilog Z80 core for the Microsoft SoftCard (replaces AppleWin's Z80VICE port, same behaviour:
//! no interrupts are delivered, as `z80_mainloop` ignores IRQ/NMI). Instruction timing is in
//! T-states; documented and the common undocumented opcodes (IXH/IXL, SLL, DDCB register copies,
//! flag bits 3 and 5) are implemented. MEMPTR (WZ) is modelled (beyond Z80VICE, which ignores
//! flags 3/5 of `BIT` altogether), so `BIT n,(HL)` takes flags 3/5 from WZ high.

use crate::state::{Dec, Enc};

pub const FS: u8 = 0x80;
pub const FZ: u8 = 0x40;
pub const FY: u8 = 0x20;
pub const FH: u8 = 0x10;
pub const FX: u8 = 0x08;
pub const FPV: u8 = 0x04;
pub const FN: u8 = 0x02;
pub const FC: u8 = 0x01;

/// What the Z80 sees: 64K of memory and 64K of I/O ports.
pub trait Z80Bus {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
    /// `IN`: AppleWin's Z80 I/O table is unconnected (reads 0, writes ignored).
    fn io_in(&mut self, _port: u16) -> u8 {
        0
    }
    fn io_out(&mut self, _port: u16, _value: u8) {}
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pfx {
    Hl,
    Ix,
    Iy,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Z80 {
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,
    pub a2: u8,
    pub f2: u8,
    pub b2: u8,
    pub c2: u8,
    pub d2: u8,
    pub e2: u8,
    pub h2: u8,
    pub l2: u8,
    pub ix: u16,
    pub iy: u16,
    pub sp: u16,
    pub pc: u16,
    pub i: u8,
    pub r: u8,
    pub iff1: bool,
    pub iff2: bool,
    pub im: u8,
    pub halted: bool,
    /// MEMPTR (WZ), the hidden register behind `BIT n,(HL)` flags 3/5.
    pub wz: u16,
}

impl Default for Z80 {
    fn default() -> Self {
        Z80 {
            a: 0xFF, f: 0xFF, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0, a2: 0, f2: 0, b2: 0, c2: 0, d2: 0, e2: 0, h2: 0, l2: 0,
            ix: 0, iy: 0, sp: 0xFFFF, pc: 0, i: 0, r: 0, iff1: false, iff2: false, im: 0, halted: false, wz: 0,
        }
    }
}

fn parity(v: u8) -> bool {
    v.count_ones().is_multiple_of(2)
}

fn sz(v: u8) -> u8 {
    (v & FS) | if v == 0 { FZ } else { 0 }
}

fn szp(v: u8) -> u8 {
    sz(v) | (v & (FY | FX)) | if parity(v) { FPV } else { 0 }
}

impl Z80 {
    /// `z80_reset` (z80.cpp:104): only PC and the interrupt state are cleared.
    pub fn reset(&mut self) {
        self.pc = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.im = 0;
        self.halted = false;
    }

    pub fn af(&self) -> u16 {
        u16::from(self.a) << 8 | u16::from(self.f)
    }
    pub fn bc(&self) -> u16 {
        u16::from(self.b) << 8 | u16::from(self.c)
    }
    pub fn de(&self) -> u16 {
        u16::from(self.d) << 8 | u16::from(self.e)
    }
    pub fn hl(&self) -> u16 {
        u16::from(self.h) << 8 | u16::from(self.l)
    }
    fn set_bc(&mut self, v: u16) {
        (self.b, self.c) = ((v >> 8) as u8, v as u8);
    }
    fn set_de(&mut self, v: u16) {
        (self.d, self.e) = ((v >> 8) as u8, v as u8);
    }
    fn set_hl(&mut self, v: u16) {
        (self.h, self.l) = ((v >> 8) as u8, v as u8);
    }
    fn set_af(&mut self, v: u16) {
        (self.a, self.f) = ((v >> 8) as u8, v as u8);
    }

    fn fetch(&mut self, bus: &mut dyn Z80Bus) -> u8 {
        let v = bus.read(self.pc);
        self.pc = self.pc.wrapping_add(1);
        v
    }

    fn fetch16(&mut self, bus: &mut dyn Z80Bus) -> u16 {
        let lo = self.fetch(bus);
        u16::from(self.fetch(bus)) << 8 | u16::from(lo)
    }

    /// Opcode fetch (M1): also bumps the low 7 bits of R.
    fn fetch_op(&mut self, bus: &mut dyn Z80Bus) -> u8 {
        self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
        self.fetch(bus)
    }

    fn rd16(bus: &mut dyn Z80Bus, a: u16) -> u16 {
        u16::from(bus.read(a)) | u16::from(bus.read(a.wrapping_add(1))) << 8
    }

    fn wr16(bus: &mut dyn Z80Bus, a: u16, v: u16) {
        bus.write(a, v as u8);
        bus.write(a.wrapping_add(1), (v >> 8) as u8);
    }

    fn push(&mut self, bus: &mut dyn Z80Bus, v: u16) {
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, (v >> 8) as u8);
        self.sp = self.sp.wrapping_sub(1);
        bus.write(self.sp, v as u8);
    }

    fn pop(&mut self, bus: &mut dyn Z80Bus) -> u16 {
        let v = Self::rd16(bus, self.sp);
        self.sp = self.sp.wrapping_add(2);
        v
    }

    fn idx(&self, p: Pfx) -> u16 {
        match p {
            Pfx::Hl => self.hl(),
            Pfx::Ix => self.ix,
            Pfx::Iy => self.iy,
        }
    }

    fn set_idx(&mut self, p: Pfx, v: u16) {
        match p {
            Pfx::Hl => self.set_hl(v),
            Pfx::Ix => self.ix = v,
            Pfx::Iy => self.iy = v,
        }
    }

    /// 8-bit register `r` (0-5, 7); H/L become IXH/IXL/IYH/IYL under a prefix.
    fn get8(&self, r: u8, p: Pfx) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 if p == Pfx::Hl => self.h,
            4 => (self.idx(p) >> 8) as u8,
            5 if p == Pfx::Hl => self.l,
            5 => self.idx(p) as u8,
            _ => self.a,
        }
    }

    fn set8(&mut self, r: u8, p: Pfx, v: u8) {
        match r {
            0 => self.b = v,
            1 => self.c = v,
            2 => self.d = v,
            3 => self.e = v,
            4 if p == Pfx::Hl => self.h = v,
            4 => self.set_idx(p, (self.idx(p) & 0xFF) | u16::from(v) << 8),
            5 if p == Pfx::Hl => self.l = v,
            5 => self.set_idx(p, (self.idx(p) & 0xFF00) | u16::from(v)),
            _ => self.a = v,
        }
    }

    /// Register pair `p` (0 BC, 1 DE, 2 HL/IX/IY, 3 SP).
    fn rp(&self, n: u8, p: Pfx) -> u16 {
        match n {
            0 => self.bc(),
            1 => self.de(),
            2 => self.idx(p),
            _ => self.sp,
        }
    }

    fn set_rp(&mut self, n: u8, p: Pfx, v: u16) {
        match n {
            0 => self.set_bc(v),
            1 => self.set_de(v),
            2 => self.set_idx(p, v),
            _ => self.sp = v,
        }
    }

    fn cond(&self, cc: u8) -> bool {
        let f = self.f;
        match cc {
            0 => f & FZ == 0,
            1 => f & FZ != 0,
            2 => f & FC == 0,
            3 => f & FC != 0,
            4 => f & FPV == 0,
            5 => f & FPV != 0,
            6 => f & FS == 0,
            _ => f & FS != 0,
        }
    }

    // ---- ALU ----

    fn add(&mut self, v: u8, carry: bool) {
        let a = self.a;
        let c = u16::from(carry);
        let sum = u16::from(a) + u16::from(v) + c;
        let r = sum as u8;
        let mut f = (r & (FS | FY | FX)) | if r == 0 { FZ } else { 0 };
        if (a & 0xF) + (v & 0xF) + carry as u8 > 0xF {
            f |= FH;
        }
        if !(a ^ v) & (a ^ r) & 0x80 != 0 {
            f |= FPV;
        }
        if sum > 0xFF {
            f |= FC;
        }
        self.a = r;
        self.f = f;
    }

    /// SUB/SBC/CP: returns the result; `cp` takes flags 3/5 from the operand and keeps A.
    fn sub(&mut self, v: u8, carry: bool, cp: bool) {
        let a = self.a;
        let c = u16::from(carry);
        let diff = u16::from(a).wrapping_sub(u16::from(v)).wrapping_sub(c);
        let r = diff as u8;
        let mut f = FN | (r & FS) | if r == 0 { FZ } else { 0 };
        f |= if cp { v & (FY | FX) } else { r & (FY | FX) };
        if (a & 0xF) < (v & 0xF) + carry as u8 {
            f |= FH;
        }
        if (a ^ v) & (a ^ r) & 0x80 != 0 {
            f |= FPV;
        }
        if diff > 0xFF {
            f |= FC;
        }
        if !cp {
            self.a = r;
        }
        self.f = f;
    }

    fn alu(&mut self, op: u8, v: u8) {
        match op {
            0 => self.add(v, false),
            1 => self.add(v, self.f & FC != 0),
            2 => self.sub(v, false, false),
            3 => self.sub(v, self.f & FC != 0, false),
            4 => {
                self.a &= v;
                self.f = szp(self.a) | FH;
            }
            5 => {
                self.a ^= v;
                self.f = szp(self.a);
            }
            6 => {
                self.a |= v;
                self.f = szp(self.a);
            }
            _ => self.sub(v, false, true),
        }
    }

    fn inc8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_add(1);
        let mut f = (self.f & FC) | (r & (FS | FY | FX)) | if r == 0 { FZ } else { 0 };
        if v & 0xF == 0xF {
            f |= FH;
        }
        if v == 0x7F {
            f |= FPV;
        }
        self.f = f;
        r
    }

    fn dec8(&mut self, v: u8) -> u8 {
        let r = v.wrapping_sub(1);
        let mut f = (self.f & FC) | FN | (r & (FS | FY | FX)) | if r == 0 { FZ } else { 0 };
        if v & 0xF == 0 {
            f |= FH;
        }
        if v == 0x80 {
            f |= FPV;
        }
        self.f = f;
        r
    }

    /// `LD (rr),A` / `LD (nn),A`: WZ = (addr+1)&FF | A<<8.
    fn wz_store_a(&mut self, addr: u16) {
        self.wz = u16::from(self.a) << 8 | (addr.wrapping_add(1) & 0xFF);
    }

    fn add16(&mut self, a: u16, b: u16) -> u16 {
        let sum = u32::from(a) + u32::from(b);
        let r = sum as u16;
        let mut f = (self.f & (FS | FZ | FPV)) | ((r >> 8) as u8 & (FY | FX));
        if (a & 0xFFF) + (b & 0xFFF) > 0xFFF {
            f |= FH;
        }
        if sum > 0xFFFF {
            f |= FC;
        }
        self.f = f;
        r
    }

    fn adc16(&mut self, b: u16) {
        let a = self.hl();
        self.wz = a.wrapping_add(1);
        let c = u32::from(self.f & FC);
        let sum = u32::from(a) + u32::from(b) + c;
        let r = sum as u16;
        let mut f = ((r >> 8) as u8 & (FS | FY | FX)) | if r == 0 { FZ } else { 0 };
        if (a & 0xFFF) + (b & 0xFFF) + c as u16 > 0xFFF {
            f |= FH;
        }
        if !(a ^ b) & (a ^ r) & 0x8000 != 0 {
            f |= FPV;
        }
        if sum > 0xFFFF {
            f |= FC;
        }
        self.f = f;
        self.set_hl(r);
    }

    fn sbc16(&mut self, b: u16) {
        let a = self.hl();
        self.wz = a.wrapping_add(1);
        let c = u32::from(self.f & FC);
        let diff = u32::from(a).wrapping_sub(u32::from(b)).wrapping_sub(c);
        let r = diff as u16;
        let mut f = FN | ((r >> 8) as u8 & (FS | FY | FX)) | if r == 0 { FZ } else { 0 };
        if (a & 0xFFF) < (b & 0xFFF) + c as u16 {
            f |= FH;
        }
        if (a ^ b) & (a ^ r) & 0x8000 != 0 {
            f |= FPV;
        }
        if diff > 0xFFFF {
            f |= FC;
        }
        self.f = f;
        self.set_hl(r);
    }

    /// CB-group rotate/shift `op` (0 RLC, 1 RRC, 2 RL, 3 RR, 4 SLA, 5 SRA, 6 SLL, 7 SRL).
    fn rot(&mut self, op: u8, v: u8) -> u8 {
        let cin = self.f & FC;
        let (r, c) = match op {
            0 => (v.rotate_left(1), v >> 7),
            1 => (v.rotate_right(1), v & 1),
            2 => (v << 1 | cin, v >> 7),
            3 => (v >> 1 | cin << 7, v & 1),
            4 => (v << 1, v >> 7),
            5 => (v >> 1 | v & 0x80, v & 1),
            6 => (v << 1 | 1, v >> 7),
            _ => (v >> 1, v & 1),
        };
        self.f = szp(r) | c;
        r
    }

    fn daa(&mut self) {
        let a = self.a;
        let (n, c, h) = (self.f & FN != 0, self.f & FC != 0, self.f & FH != 0);
        let mut corr = 0u8;
        let mut carry = c;
        if h || a & 0xF > 9 {
            corr |= 6;
        }
        if c || a > 0x99 {
            corr |= 0x60;
            carry = true;
        }
        let r = if n { a.wrapping_sub(corr) } else { a.wrapping_add(corr) };
        let hf = if n { h && a & 0xF < 6 } else { a & 0xF > 9 };
        self.f = szp(r) | (self.f & FN) | if hf { FH } else { 0 } | if carry { FC } else { 0 };
        self.a = r;
    }

    /// Executes one instruction; returns its T-states.
    pub fn step(&mut self, bus: &mut dyn Z80Bus) -> u32 {
        if self.halted {
            self.r = (self.r & 0x80) | (self.r.wrapping_add(1) & 0x7F);
            return 4; // keeps executing NOPs
        }
        let op = self.fetch_op(bus);
        match op {
            0xDD => self.prefixed(bus, Pfx::Ix),
            0xFD => self.prefixed(bus, Pfx::Iy),
            _ => self.main(bus, op, Pfx::Hl),
        }
    }

    fn prefixed(&mut self, bus: &mut dyn Z80Bus, p: Pfx) -> u32 {
        let op = self.fetch_op(bus);
        match op {
            0xDD | 0xFD | 0xED => {
                self.pc = self.pc.wrapping_sub(1); // the prefix alone acts as a 4-cycle NOP
                self.r = (self.r & 0x80) | (self.r.wrapping_sub(1) & 0x7F);
                4
            }
            0xCB => {
                let d = self.fetch(bus) as i8;
                let addr = self.idx(p).wrapping_add(d as u16);
                let op = self.fetch(bus);
                self.cb_indexed(bus, addr, op)
            }
            _ => 4 + self.main(bus, op, p),
        }
    }

    /// Operand address for the `(HL)` slot: HL, or IX/IY plus the displacement byte.
    fn mem_addr(&mut self, bus: &mut dyn Z80Bus, p: Pfx) -> u16 {
        match p {
            Pfx::Hl => self.hl(),
            _ => {
                let d = self.fetch(bus) as i8;
                let a = self.idx(p).wrapping_add(d as u16);
                self.wz = a;
                a
            }
        }
    }

    fn main(&mut self, bus: &mut dyn Z80Bus, op: u8, p: Pfx) -> u32 {
        let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
        let (pp, q) = (y >> 1, y & 1);
        let idxd = if p == Pfx::Hl { 0 } else { 8 }; // displacement cost for (IX+d) forms
        match x {
            0 => match z {
                0 => match y {
                    0 => 4,
                    1 => {
                        std::mem::swap(&mut self.a, &mut self.a2);
                        std::mem::swap(&mut self.f, &mut self.f2);
                        4
                    }
                    2 => {
                        let d = self.fetch(bus) as i8;
                        self.b = self.b.wrapping_sub(1);
                        if self.b != 0 {
                            self.pc = self.pc.wrapping_add(d as u16);
                            self.wz = self.pc;
                            13
                        } else {
                            8
                        }
                    }
                    3 => {
                        let d = self.fetch(bus) as i8;
                        self.pc = self.pc.wrapping_add(d as u16);
                        self.wz = self.pc;
                        12
                    }
                    _ => {
                        let d = self.fetch(bus) as i8;
                        if self.cond(y - 4) {
                            self.pc = self.pc.wrapping_add(d as u16);
                            self.wz = self.pc;
                            12
                        } else {
                            7
                        }
                    }
                },
                1 => {
                    if q == 0 {
                        let v = self.fetch16(bus);
                        self.set_rp(pp, p, v);
                        10
                    } else {
                        let (a, b) = (self.idx(p), self.rp(pp, p));
                        self.wz = a.wrapping_add(1);
                        let r = self.add16(a, b);
                        self.set_idx(p, r);
                        11
                    }
                }
                2 => match (q, pp) {
                    (0, 0) => {
                        bus.write(self.bc(), self.a);
                        self.wz_store_a(self.bc());
                        7
                    }
                    (0, 1) => {
                        bus.write(self.de(), self.a);
                        self.wz_store_a(self.de());
                        7
                    }
                    (0, 2) => {
                        let a = self.fetch16(bus);
                        Self::wr16(bus, a, self.idx(p));
                        self.wz = a.wrapping_add(1);
                        16
                    }
                    (0, _) => {
                        let a = self.fetch16(bus);
                        bus.write(a, self.a);
                        self.wz_store_a(a);
                        13
                    }
                    (_, 0) => {
                        self.a = bus.read(self.bc());
                        self.wz = self.bc().wrapping_add(1);
                        7
                    }
                    (_, 1) => {
                        self.a = bus.read(self.de());
                        self.wz = self.de().wrapping_add(1);
                        7
                    }
                    (_, 2) => {
                        let a = self.fetch16(bus);
                        let v = Self::rd16(bus, a);
                        self.set_idx(p, v);
                        self.wz = a.wrapping_add(1);
                        16
                    }
                    _ => {
                        let a = self.fetch16(bus);
                        self.a = bus.read(a);
                        self.wz = a.wrapping_add(1);
                        13
                    }
                },
                3 => {
                    let v = self.rp(pp, p);
                    self.set_rp(pp, p, if q == 0 { v.wrapping_add(1) } else { v.wrapping_sub(1) });
                    6
                }
                4 | 5 => {
                    let inc = z == 4;
                    if y == 6 {
                        let a = self.mem_addr(bus, p);
                        let v = bus.read(a);
                        let r = if inc { self.inc8(v) } else { self.dec8(v) };
                        bus.write(a, r);
                        11 + idxd
                    } else {
                        let v = self.get8(y, p);
                        let r = if inc { self.inc8(v) } else { self.dec8(v) };
                        self.set8(y, p, r);
                        4
                    }
                }
                6 => {
                    if y == 6 {
                        let a = self.mem_addr(bus, p);
                        let n = self.fetch(bus);
                        bus.write(a, n);
                        10 + if p == Pfx::Hl { 0 } else { 5 }
                    } else {
                        let n = self.fetch(bus);
                        self.set8(y, p, n);
                        7
                    }
                }
                _ => {
                    match y {
                        0 => {
                            let c = self.a >> 7;
                            self.a = self.a.rotate_left(1);
                            self.f = (self.f & (FS | FZ | FPV)) | (self.a & (FY | FX)) | c;
                        }
                        1 => {
                            let c = self.a & 1;
                            self.a = self.a.rotate_right(1);
                            self.f = (self.f & (FS | FZ | FPV)) | (self.a & (FY | FX)) | c;
                        }
                        2 => {
                            let c = self.a >> 7;
                            self.a = self.a << 1 | (self.f & FC);
                            self.f = (self.f & (FS | FZ | FPV)) | (self.a & (FY | FX)) | c;
                        }
                        3 => {
                            let c = self.a & 1;
                            self.a = self.a >> 1 | (self.f & FC) << 7;
                            self.f = (self.f & (FS | FZ | FPV)) | (self.a & (FY | FX)) | c;
                        }
                        4 => self.daa(),
                        5 => {
                            self.a = !self.a;
                            self.f = (self.f & (FS | FZ | FPV | FC)) | FH | FN | (self.a & (FY | FX));
                        }
                        6 => self.f = (self.f & (FS | FZ | FPV)) | FC | (self.a & (FY | FX)),
                        _ => {
                            let old_c = self.f & FC;
                            self.f = (self.f & (FS | FZ | FPV)) | if old_c != 0 { FH } else { FC } | (self.a & (FY | FX));
                        }
                    }
                    4
                }
            },
            1 => {
                if op == 0x76 {
                    self.halted = true;
                    self.pc = self.pc.wrapping_sub(1);
                    return 4;
                }
                if y == 6 {
                    let a = self.mem_addr(bus, p);
                    let v = self.get8(z, Pfx::Hl); // the register side is plain H/L here
                    bus.write(a, v);
                    7 + idxd
                } else if z == 6 {
                    let a = self.mem_addr(bus, p);
                    let v = bus.read(a);
                    self.set8(y, Pfx::Hl, v);
                    7 + idxd
                } else {
                    let v = self.get8(z, p);
                    self.set8(y, p, v);
                    4
                }
            }
            2 => {
                if z == 6 {
                    let a = self.mem_addr(bus, p);
                    let v = bus.read(a);
                    self.alu(y, v);
                    7 + idxd
                } else {
                    let v = self.get8(z, p);
                    self.alu(y, v);
                    4
                }
            }
            _ => self.group3(bus, op, p),
        }
    }

    fn group3(&mut self, bus: &mut dyn Z80Bus, op: u8, p: Pfx) -> u32 {
        let (y, z) = ((op >> 3) & 7, op & 7);
        let (pp, q) = (y >> 1, y & 1);
        match z {
            0 => {
                if self.cond(y) {
                    self.pc = self.pop(bus);
                    self.wz = self.pc;
                    11
                } else {
                    5
                }
            }
            1 => {
                if q == 0 {
                    let v = self.pop(bus);
                    match pp {
                        0 => self.set_bc(v),
                        1 => self.set_de(v),
                        2 => self.set_idx(p, v),
                        _ => self.set_af(v),
                    }
                    10
                } else {
                    match pp {
                        0 => {
                            self.pc = self.pop(bus);
                            self.wz = self.pc;
                            10
                        }
                        1 => {
                            std::mem::swap(&mut self.b, &mut self.b2);
                            std::mem::swap(&mut self.c, &mut self.c2);
                            std::mem::swap(&mut self.d, &mut self.d2);
                            std::mem::swap(&mut self.e, &mut self.e2);
                            std::mem::swap(&mut self.h, &mut self.h2);
                            std::mem::swap(&mut self.l, &mut self.l2);
                            4
                        }
                        2 => {
                            self.pc = self.idx(p);
                            4
                        }
                        _ => {
                            self.sp = self.idx(p);
                            6
                        }
                    }
                }
            }
            2 => {
                let a = self.fetch16(bus);
                self.wz = a;
                if self.cond(y) {
                    self.pc = a;
                }
                10
            }
            3 => match y {
                0 => {
                    self.pc = self.fetch16(bus);
                    self.wz = self.pc;
                    10
                }
                1 => {
                    let op = self.fetch_op(bus);
                    self.cb(bus, op)
                }
                2 => {
                    let n = self.fetch(bus);
                    bus.io_out(u16::from(self.a) << 8 | u16::from(n), self.a);
                    self.wz = u16::from(self.a) << 8 | u16::from(n.wrapping_add(1));
                    11
                }
                3 => {
                    let n = self.fetch(bus);
                    let port = u16::from(self.a) << 8 | u16::from(n);
                    self.a = bus.io_in(port);
                    self.wz = port.wrapping_add(1);
                    11
                }
                4 => {
                    let v = Self::rd16(bus, self.sp);
                    Self::wr16(bus, self.sp, self.idx(p));
                    self.set_idx(p, v);
                    self.wz = v;
                    19
                }
                5 => {
                    let (d, h) = (self.de(), self.hl());
                    self.set_de(h);
                    self.set_hl(d);
                    4
                }
                6 => {
                    self.iff1 = false;
                    self.iff2 = false;
                    4
                }
                _ => {
                    self.iff1 = true;
                    self.iff2 = true;
                    4
                }
            },
            4 => {
                let a = self.fetch16(bus);
                self.wz = a;
                if self.cond(y) {
                    self.push(bus, self.pc);
                    self.pc = a;
                    17
                } else {
                    10
                }
            }
            5 => {
                if q == 0 {
                    let v = match pp {
                        0 => self.bc(),
                        1 => self.de(),
                        2 => self.idx(p),
                        _ => self.af(),
                    };
                    self.push(bus, v);
                    11
                } else if pp == 0 {
                    let a = self.fetch16(bus);
                    self.wz = a;
                    self.push(bus, self.pc);
                    self.pc = a;
                    17
                } else if pp == 2 {
                    let op = self.fetch_op(bus);
                    self.ed(bus, op)
                } else {
                    4 // DD/FD are consumed in `step`
                }
            }
            6 => {
                let n = self.fetch(bus);
                self.alu(y, n);
                7
            }
            _ => {
                self.push(bus, self.pc);
                self.pc = u16::from(y) * 8;
                self.wz = self.pc;
                11
            }
        }
    }

    fn cb(&mut self, bus: &mut dyn Z80Bus, op: u8) -> u32 {
        let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
        let (v, hl) = if z == 6 { (bus.read(self.hl()), true) } else { (self.get8(z, Pfx::Hl), false) };
        match x {
            0 => {
                let r = self.rot(y, v);
                self.cb_store(bus, z, hl, r);
            }
            1 => {
                let xy = if hl { (self.wz >> 8) as u8 } else { v };
                self.bit(y, v, xy);
                return if hl { 12 } else { 8 };
            }
            2 => self.cb_store(bus, z, hl, v & !(1 << y)),
            _ => self.cb_store(bus, z, hl, v | 1 << y),
        }
        if hl {
            15
        } else {
            8
        }
    }

    fn cb_store(&mut self, bus: &mut dyn Z80Bus, z: u8, hl: bool, r: u8) {
        if hl {
            bus.write(self.hl(), r);
        } else {
            self.set8(z, Pfx::Hl, r);
        }
    }

    fn bit(&mut self, n: u8, v: u8, xy: u8) {
        let set = v & (1 << n) != 0;
        let mut f = (self.f & FC) | FH | (xy & (FY | FX));
        if !set {
            f |= FZ | FPV;
        }
        if n == 7 && set {
            f |= FS;
        }
        self.f = f;
    }

    /// `DD CB d op` / `FD CB d op`: always operates on (IX+d); other registers get a copy too.
    fn cb_indexed(&mut self, bus: &mut dyn Z80Bus, addr: u16, op: u8) -> u32 {
        let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
        self.wz = addr;
        let v = bus.read(addr);
        let r = match x {
            0 => self.rot(y, v),
            1 => {
                self.bit(y, v, (addr >> 8) as u8);
                return 20;
            }
            2 => v & !(1 << y),
            _ => v | 1 << y,
        };
        bus.write(addr, r);
        if z != 6 {
            self.set8(z, Pfx::Hl, r);
        }
        23
    }

    fn ed(&mut self, bus: &mut dyn Z80Bus, op: u8) -> u32 {
        let (x, y, z) = (op >> 6, (op >> 3) & 7, op & 7);
        let (pp, q) = (y >> 1, y & 1);
        match x {
            1 => match z {
                0 => {
                    let v = bus.io_in(self.bc());
                    self.wz = self.bc().wrapping_add(1);
                    self.f = (self.f & FC) | szp(v);
                    if y != 6 {
                        self.set8(y, Pfx::Hl, v);
                    }
                    12
                }
                1 => {
                    let v = if y == 6 { 0 } else { self.get8(y, Pfx::Hl) };
                    bus.io_out(self.bc(), v);
                    self.wz = self.bc().wrapping_add(1);
                    12
                }
                2 => {
                    let b = self.rp(pp, Pfx::Hl);
                    if q == 0 {
                        self.sbc16(b);
                    } else {
                        self.adc16(b);
                    }
                    15
                }
                3 => {
                    let a = self.fetch16(bus);
                    self.wz = a.wrapping_add(1);
                    if q == 0 {
                        Self::wr16(bus, a, self.rp(pp, Pfx::Hl));
                    } else {
                        let v = Self::rd16(bus, a);
                        self.set_rp(pp, Pfx::Hl, v);
                    }
                    20
                }
                4 => {
                    let a = self.a;
                    self.a = 0;
                    self.sub(a, false, false);
                    8
                }
                5 => {
                    self.iff1 = self.iff2;
                    self.pc = self.pop(bus);
                    self.wz = self.pc;
                    14
                }
                6 => {
                    self.im = [0, 0, 1, 2, 0, 0, 1, 2][y as usize];
                    8
                }
                _ => match y {
                    0 => {
                        self.i = self.a;
                        9
                    }
                    1 => {
                        self.r = self.a;
                        9
                    }
                    2 | 3 => {
                        let v = if y == 2 { self.i } else { self.r };
                        self.a = v;
                        self.f = (self.f & FC) | sz(v) | (v & (FY | FX)) | if self.iff2 { FPV } else { 0 };
                        9
                    }
                    4 | 5 => {
                        let t = bus.read(self.hl());
                        self.wz = self.hl().wrapping_add(1);
                        let (new_m, new_a) = if y == 5 {
                            (t << 4 | self.a & 0xF, self.a & 0xF0 | t >> 4)
                        } else {
                            (self.a << 4 | t >> 4, self.a & 0xF0 | t & 0xF)
                        };
                        bus.write(self.hl(), new_m);
                        self.a = new_a;
                        self.f = (self.f & FC) | szp(new_a);
                        18
                    }
                    _ => 8,
                },
            },
            2 if z < 4 && y >= 4 => self.block(bus, y, z),
            _ => 8,
        }
    }

    /// LDI/LDD/LDIR/LDDR, CPI.., INI.., OUTI.. (`y` 4-7 selects inc/dec and repeat, `z` the family).
    fn block(&mut self, bus: &mut dyn Z80Bus, y: u8, z: u8) -> u32 {
        let dec = y & 1 != 0;
        let repeat = y >= 6;
        let step = |v: u16| if dec { v.wrapping_sub(1) } else { v.wrapping_add(1) };
        let mut again;
        let start = self.pc.wrapping_sub(2);
        match z {
            0 => {
                let v = bus.read(self.hl());
                bus.write(self.de(), v);
                self.set_hl(step(self.hl()));
                self.set_de(step(self.de()));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                let n = self.a.wrapping_add(v);
                self.f = (self.f & (FS | FZ | FC)) | if bc != 0 { FPV } else { 0 } | (n & FX) | if n & 2 != 0 { FY } else { 0 };
                again = bc != 0;
            }
            1 => {
                let v = bus.read(self.hl());
                let r = self.a.wrapping_sub(v);
                let h = (self.a & 0xF) < (v & 0xF);
                self.set_hl(step(self.hl()));
                let bc = self.bc().wrapping_sub(1);
                self.set_bc(bc);
                let n = r.wrapping_sub(h as u8);
                self.f = (self.f & FC) | FN | sz(r) | if h { FH } else { 0 } | if bc != 0 { FPV } else { 0 } | (n & FX) | if n & 2 != 0 { FY } else { 0 };
                again = bc != 0 && r != 0;
                self.wz = step(self.wz);
            }
            2 => {
                let v = bus.io_in(self.bc());
                self.wz = step(self.bc());
                bus.write(self.hl(), v);
                self.b = self.b.wrapping_sub(1);
                let c = if dec { self.c.wrapping_sub(1) } else { self.c.wrapping_add(1) };
                self.set_hl(step(self.hl()));
                self.io_block_flags(v, u16::from(v) + u16::from(c));
                again = self.b != 0;
            }
            _ => {
                let v = bus.read(self.hl());
                self.b = self.b.wrapping_sub(1);
                bus.io_out(self.bc(), v);
                self.wz = step(self.bc());
                self.set_hl(step(self.hl()));
                self.io_block_flags(v, u16::from(v) + u16::from(self.l));
                again = self.b != 0;
            }
        }
        again &= repeat;
        if again {
            self.pc = start;
            self.wz = start.wrapping_add(1);
            21
        } else {
            16
        }
    }

    fn io_block_flags(&mut self, v: u8, k: u16) {
        let mut f = sz(self.b) | (self.b & (FY | FX));
        if v & 0x80 != 0 {
            f |= FN;
        }
        if k > 0xFF {
            f |= FH | FC;
        }
        if parity((k as u8 & 7) ^ self.b) {
            f |= FPV;
        }
        self.f = f;
    }

    pub fn save(&self, e: &mut Enc) {
        e.bytes(&[self.a, self.f, self.b, self.c, self.d, self.e, self.h, self.l]);
        e.bytes(&[self.a2, self.f2, self.b2, self.c2, self.d2, self.e2, self.h2, self.l2]);
        e.u16(self.ix).u16(self.iy).u16(self.sp).u16(self.pc);
        e.u8(self.i).u8(self.r).bool(self.iff1).bool(self.iff2).u8(self.im).bool(self.halted).u16(self.wz);
    }

    pub fn load(&mut self, d: &mut Dec) -> Option<()> {
        let mut m = [0u8; 16];
        d.bytes(&mut m)?;
        [self.a, self.f, self.b, self.c, self.d, self.e, self.h, self.l] = m[..8].try_into().ok()?;
        [self.a2, self.f2, self.b2, self.c2, self.d2, self.e2, self.h2, self.l2] = m[8..].try_into().ok()?;
        (self.ix, self.iy, self.sp, self.pc) = (d.u16()?, d.u16()?, d.u16()?, d.u16()?);
        (self.i, self.r) = (d.u8()?, d.u8()?);
        (self.iff1, self.iff2) = (d.bool()?, d.bool()?);
        self.im = d.u8()?.min(2);
        self.halted = d.bool()?;
        self.wz = d.u16()?;
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Ram(Vec<u8>, Vec<(u16, u8)>);

    impl Z80Bus for Ram {
        fn read(&mut self, a: u16) -> u8 {
            self.0[a as usize]
        }
        fn write(&mut self, a: u16, v: u8) {
            self.0[a as usize] = v;
        }
        fn io_out(&mut self, p: u16, v: u8) {
            self.1.push((p, v));
        }
        fn io_in(&mut self, p: u16) -> u8 {
            p as u8 ^ 0x5A
        }
    }

    /// Runs `code` from 0 until `HALT` (0x76); returns CPU, RAM and total T-states.
    fn run(code: &[u8], setup: impl FnOnce(&mut Z80)) -> (Z80, Ram, u32) {
        let mut ram = Ram(vec![0; 0x10000], vec![]);
        ram.0[..code.len()].copy_from_slice(code);
        let mut cpu = Z80 { sp: 0x8000, ..Default::default() };
        setup(&mut cpu);
        let mut t = 0;
        while !cpu.halted {
            t += cpu.step(&mut ram);
            assert!(t < 100_000);
        }
        (cpu, ram, t - 4) // minus the HALT itself
    }

    fn flags(f: u8) -> String {
        "SZYHXVNC".chars().enumerate().map(|(i, c)| if f & (0x80 >> i) != 0 { c } else { '-' }).collect()
    }

    #[test]
    fn ld_immediate_and_timing() {
        let (c, _, t) = run(&[0x3E, 0x12, 0x06, 0x34, 0x21, 0x78, 0x56, 0x76], |_| {});
        assert_eq!((c.a, c.b, c.hl()), (0x12, 0x34, 0x5678));
        assert_eq!(t, 7 + 7 + 10);
    }

    #[test]
    fn add_sets_half_carry_overflow_and_undocumented_bits() {
        // A=0x7F + 1 = 0x80: S H V, no carry
        let (c, _, _) = run(&[0xC6, 0x01, 0x76], |c| c.a = 0x7F);
        assert_eq!((c.a, flags(c.f)), (0x80, "S--H-V--".to_string()));
        // 0xFF + 1 = 0: Z H C
        let (c, _, _) = run(&[0xC6, 0x01, 0x76], |c| c.a = 0xFF);
        assert_eq!((c.a, flags(c.f)), (0, "-Z-H---C".to_string()));
        // 0x28 + 0x00: bits 3 and 5 copied from the result
        let (c, _, _) = run(&[0xC6, 0x00, 0x76], |c| c.a = 0x28);
        assert_eq!(c.f & (FY | FX), FY | FX);
    }

    #[test]
    fn sub_cp_and_neg_flags() {
        let (c, _, _) = run(&[0xD6, 0x01, 0x76], |c| c.a = 0x80);
        assert_eq!((c.a, flags(c.f)), (0x7F, "--YHXVN-".to_string()));
        // CP takes bits 3/5 from the operand and leaves A alone
        let (c, _, _) = run(&[0xFE, 0x28, 0x76], |c| c.a = 0x28);
        assert_eq!((c.a, c.f), (0x28, FZ | FN | FY | FX));
        let (c, _, _) = run(&[0xED, 0x44, 0x76], |c| c.a = 0x80);
        assert_eq!((c.a, flags(c.f)), (0x80, "S----VNC".to_string()));
    }

    #[test]
    fn daa_after_bcd_add_and_sub() {
        // 0x15 + 0x27 = 0x3C -> DAA -> 0x42
        let (c, _, _) = run(&[0xC6, 0x27, 0x27, 0x76], |c| c.a = 0x15);
        assert_eq!(c.a, 0x42);
        // 0x99 + 0x01 = 0x9A -> 0x00 with carry
        let (c, _, _) = run(&[0xC6, 0x01, 0x27, 0x76], |c| c.a = 0x99);
        assert_eq!((c.a, c.f & (FC | FZ)), (0x00, FC | FZ));
        // 0x42 - 0x15 = 0x2D -> DAA -> 0x27
        let (c, _, _) = run(&[0xD6, 0x15, 0x27, 0x76], |c| c.a = 0x42);
        assert_eq!(c.a, 0x27);
    }

    #[test]
    fn logic_and_parity() {
        let (c, _, _) = run(&[0xE6, 0x0F, 0x76], |c| c.a = 0xF3); // AND: H set, parity of 0x03
        assert_eq!((c.a, flags(c.f)), (0x03, "---H-V--".to_string()));
        let (c, _, _) = run(&[0xAF, 0x76], |_| {}); // XOR A
        assert_eq!((c.a, c.f), (0, FZ | FPV));
    }

    #[test]
    fn inc_dec_preserve_carry() {
        let (c, _, _) = run(&[0x3C, 0x76], |c| {
            c.a = 0x7F;
            c.f = FC;
        });
        assert_eq!((c.a, flags(c.f)), (0x80, "S--H-V-C".to_string()));
        let (c, _, _) = run(&[0x3D, 0x76], |c| {
            c.a = 0x80;
            c.f = 0;
        });
        assert_eq!((c.a, flags(c.f)), (0x7F, "--YHXVN-".to_string()));
    }

    #[test]
    fn sixteen_bit_arithmetic() {
        let (c, _, t) = run(&[0x09, 0x76], |c| {
            c.set_hl(0x0FFF);
            c.set_bc(0x0001);
        });
        assert_eq!((c.hl(), c.f & (FH | FC)), (0x1000, FH));
        assert_eq!(t, 11);
        // ADC HL,DE: 0x7FFF + 0 + C = 0x8000 -> S, V, H
        let (c, _, _) = run(&[0xED, 0x5A, 0x76], |c| {
            c.set_hl(0x7FFF);
            c.f = FC;
        });
        assert_eq!((c.hl(), flags(c.f)), (0x8000, "S--H-V--".to_string()));
        // SBC HL,DE: 0 - 0 - 1 = 0xFFFF
        let (c, _, _) = run(&[0xED, 0x52, 0x76], |c| c.f = FC);
        assert_eq!((c.hl(), flags(c.f)), (0xFFFF, "S-YHX-NC".to_string()));
    }

    #[test]
    fn jumps_calls_and_stack() {
        // CALL 0x0010; HALT ... 0x10: LD A,5; RET
        let mut code = vec![0xCD, 0x10, 0x00, 0x76];
        code.resize(0x10, 0);
        code.extend([0x3E, 0x05, 0xC9]);
        let (c, _, t) = run(&code, |_| {});
        assert_eq!((c.a, c.sp), (5, 0x8000));
        assert_eq!(t, 17 + 7 + 10);
        // DJNZ loop: B=3
        let (c, _, t) = run(&[0x10, 0xFE, 0x76], |c| c.b = 3);
        assert_eq!(c.b, 0);
        assert_eq!(t, 13 + 13 + 8);
    }

    #[test]
    fn conditional_branch_timing() {
        let (_, _, t) = run(&[0x28, 0x02, 0, 0, 0x76], |c| c.f = FZ);
        assert_eq!(t, 12);
        let (_, _, t) = run(&[0x28, 0x02, 0x76], |c| c.f = 0);
        assert_eq!(t, 7);
        let (_, _, t) = run(&[0xC0, 0x76], |c| c.f = FZ); // RET NZ not taken
        assert_eq!(t, 5);
    }

    #[test]
    fn ldir_copies_and_costs_21_per_repeat() {
        let mut code = vec![0xED, 0xB0, 0x76];
        code.resize(0x20, 0);
        code[0x10..0x13].copy_from_slice(&[1, 2, 3]);
        let (c, r, t) = run(&code, |c| {
            c.set_hl(0x10);
            c.set_de(0x100);
            c.set_bc(3);
        });
        assert_eq!(&r.0[0x100..0x103], &[1, 2, 3]);
        assert_eq!((c.bc(), c.hl(), c.de()), (0, 0x13, 0x103));
        assert_eq!(t, 21 + 21 + 16);
        assert_eq!(c.f & FPV, 0);
    }

    #[test]
    fn cpir_stops_on_match() {
        let mut code = vec![0xED, 0xB1, 0x76];
        code.resize(0x20, 0);
        code[0x10..0x14].copy_from_slice(&[9, 9, 7, 9]);
        let (c, _, _) = run(&code, |c| {
            c.a = 7;
            c.set_hl(0x10);
            c.set_bc(10);
        });
        assert_eq!((c.hl(), c.bc()), (0x13, 7));
        assert_eq!(c.f & (FZ | FPV | FN), FZ | FPV | FN);
    }

    #[test]
    fn cb_rotate_shift_and_bit() {
        let (c, _, t) = run(&[0xCB, 0x00, 0x76], |c| c.b = 0x81); // RLC B
        assert_eq!((c.b, c.f & FC, t), (0x03, FC, 8));
        let (c, _, _) = run(&[0xCB, 0x2F, 0x76], |c| c.a = 0x81); // SRA A
        assert_eq!((c.a, c.f & FC), (0xC0, FC));
        let (c, _, _) = run(&[0xCB, 0x37, 0x76], |c| c.a = 0x80); // SLL A
        assert_eq!(c.a, 0x01);
        let (c, _, t) = run(&[0xCB, 0x7E, 0x76], |c| {
            c.set_hl(0x100);
        }); // BIT 7,(HL) with 0
        assert_eq!((c.f & (FZ | FH | FPV), t), (FZ | FH | FPV, 12));
        let (c, r, t) = run(&[0xCB, 0xC6, 0x76], |c| c.set_hl(0x100)); // SET 0,(HL)
        assert_eq!((c.hl(), r.0[0x100], t), (0x100, 1, 15));
    }

    #[test]
    fn index_registers_and_displacements() {
        // LD IX,0x1000; LD (IX+5),0x77; LD A,(IX+5); INC (IX+5)
        let (c, r, t) = run(&[0xDD, 0x21, 0x00, 0x10, 0xDD, 0x36, 0x05, 0x77, 0xDD, 0x7E, 0x05, 0xDD, 0x34, 0x05, 0x76], |_| {});
        assert_eq!((c.a, r.0[0x1005]), (0x77, 0x78));
        assert_eq!(t, 14 + 19 + 19 + 23);
        // negative displacement
        let (c, _, _) = run(&[0xFD, 0x7E, 0xFF, 0x76], |c| c.iy = 0x1001);
        assert_eq!(c.a, 0);
        // undocumented IXH/IXL
        let (c, _, t) = run(&[0xDD, 0x26, 0x12, 0xDD, 0x2E, 0x34, 0xDD, 0x7C, 0x76], |_| {});
        assert_eq!((c.ix, c.a), (0x1234, 0x12));
        assert_eq!(t, 11 + 11 + 8);
        // ADD IX,BC and JP (IX)
        let (c, _, _) = run(&[0xDD, 0x09, 0x76], |c| {
            c.ix = 0x100;
            c.set_bc(0x23);
        });
        assert_eq!(c.ix, 0x123);
    }

    #[test]
    fn ddcb_operates_on_memory_and_copies_to_register() {
        // RLC (IX+1),B : memory and B both get the result
        let mut code = vec![0xDD, 0xCB, 0x01, 0x00, 0x76];
        code.resize(0x20, 0);
        let (c, r, t) = run(&code, |c| c.ix = 0x1FF);
        assert_eq!((c.b, r.0[0x200]), (0, 0));
        assert_eq!(t, 23);
        let mut ram = Ram(vec![0; 0x10000], vec![]);
        ram.0[0x200] = 0x81;
        ram.0[..5].copy_from_slice(&[0xDD, 0xCB, 0x01, 0x00, 0x76]);
        let mut cpu = Z80 { ix: 0x1FF, ..Default::default() };
        cpu.step(&mut ram);
        assert_eq!((cpu.b, ram.0[0x200]), (0x03, 0x03));
        // BIT 0,(IX+1): 20 T-states, flag bits 3/5 from the address high byte
        ram.0[..5].copy_from_slice(&[0xFD, 0xCB, 0x01, 0x46, 0x76]);
        cpu.pc = 0;
        cpu.iy = 0x2800 - 1;
        assert_eq!(cpu.step(&mut ram), 20);
        assert_eq!(cpu.f & (FY | FX), FY | FX);
    }

    #[test]
    fn exchange_and_stack_ops() {
        let (c, _, _) = run(&[0xD9, 0x08, 0x76], |c| {
            c.b = 1;
            c.b2 = 2;
            c.a = 3;
            c.a2 = 4;
        });
        assert_eq!((c.b, c.b2, c.a, c.a2), (2, 1, 4, 3));
        // PUSH BC; POP DE; EX (SP),HL
        let (c, _, t) = run(&[0xC5, 0xD1, 0x76], |c| c.set_bc(0xBEEF));
        assert_eq!((c.de(), t), (0xBEEF, 11 + 10));
        let (c, r, t) = run(&[0xE3, 0x76], |c| c.set_hl(0x1234));
        assert_eq!((c.hl(), r.0[0x8000], r.0[0x8001], t), (0, 0x34, 0x12, 19));
    }

    #[test]
    fn rld_rrd() {
        let (c, r, _) = run(&[0xED, 0x6F, 0x76], |c| {
            c.a = 0x7A;
            c.set_hl(0x100);
        });
        assert_eq!((c.a, r.0[0x100]), (0x70, 0x0A));
        let mut ram = Ram(vec![0; 0x10000], vec![]);
        ram.0[0x100] = 0x31;
        ram.0[..3].copy_from_slice(&[0xED, 0x67, 0x76]);
        let mut cpu = Z80 { a: 0x84, ..Default::default() };
        cpu.set_hl(0x100);
        cpu.step(&mut ram);
        assert_eq!((cpu.a, ram.0[0x100]), (0x81, 0x43));
    }

    #[test]
    fn io_instructions_use_the_bus() {
        let (c, r, _) = run(&[0xDB, 0x10, 0x32, 0x00, 0x01, 0x3E, 0x99, 0xD3, 0x20, 0x76], |c| c.a = 0xAB);
        assert_eq!(c.a, 0x99);
        assert_eq!(r.0[0x100], 0x10 ^ 0x5A);
        assert_eq!(r.1, vec![(0x9920, 0x99)]);
    }

    #[test]
    fn r_register_counts_fetches() {
        let (c, _, _) = run(&[0x00, 0x00, 0xDD, 0x23, 0x76], |_| {});
        assert_eq!(c.r & 0x7F, 5, "NOP NOP DD INC HALT: prefix counts too");
    }

    #[test]
    fn halt_spins_and_im_ei_di() {
        let (c, _, _) = run(&[0xFB, 0xED, 0x5E, 0x76], |_| {});
        assert!(c.iff1 && c.iff2);
        assert_eq!(c.im, 2);
        let mut cpu = c.clone();
        let mut ram = Ram(vec![0; 0x10000], vec![]);
        assert_eq!(cpu.step(&mut ram), 4);
        assert_eq!(cpu.pc, c.pc);
    }

    #[test]
    fn adc_sbc_with_carry_in() {
        let (c, _, _) = run(&[0xCE, 0xFF, 0x76], |c| {
            c.a = 0xFF;
            c.f = FC;
        }); // ADC A,0xFF
        assert_eq!((c.a, flags(c.f)), (0xFF, "S-YHX--C".to_string()));
        let (c, _, _) = run(&[0xDE, 0x00, 0x76], |c| {
            c.a = 0;
            c.f = FC;
        }); // SBC A,0
        assert_eq!((c.a, flags(c.f)), (0xFF, "S-YHX-NC".to_string()));
    }

    #[test]
    fn cpi_flags_and_ld_a_i() {
        let mut code = vec![0xED, 0xA1, 0x76];
        code.resize(0x20, 0);
        code[0x10] = 0x20;
        let (c, _, t) = run(&code, |c| {
            c.a = 0x10;
            c.f = FC;
            c.set_hl(0x10);
            c.set_bc(2);
        });
        assert_eq!((flags(c.f), c.bc(), c.hl(), t), ("S----VNC".to_string(), 1, 0x11, 16));
        let (c, _, _) = run(&[0xED, 0x57, 0x76], |c| {
            c.i = 0x80;
            c.iff2 = true;
        });
        assert_eq!((c.a, flags(c.f & !FC)), (0x80, "S----V--".to_string()));
    }

    #[test]
    fn otir_writes_a_block_to_the_port() {
        let mut code = vec![0xED, 0xB3, 0x76];
        code.resize(0x20, 0);
        code[0x10..0x12].copy_from_slice(&[0xAA, 0xBB]);
        let (c, r, t) = run(&code, |c| {
            c.set_hl(0x10);
            c.b = 2;
            c.c = 0x42;
        });
        assert_eq!((c.b, c.hl(), t), (0, 0x12, 21 + 16));
        assert_eq!(r.1, vec![(0x0142, 0xAA), (0x0042, 0xBB)], "B is decremented before the port address is formed");
        assert_eq!(c.f & FZ, FZ);
    }

    #[test]
    fn save_load_roundtrip() {
        let (c, _, _) = run(&[0x3E, 0x42, 0xD9, 0x76], |c| c.ix = 0xABCD);
        let mut e = Enc::default();
        c.save(&mut e);
        let mut c2 = Z80::default();
        let mut d = Dec::new(&e.0);
        assert!(c2.load(&mut d).is_some() && d.done());
        assert_eq!(c, c2);
        let (c, _, _) = run(&[0x3A, 0x00, 0x28, 0x76], |_| {});
        let mut e = Enc::default();
        c.save(&mut e);
        let mut c2 = Z80::default();
        assert!(c2.load(&mut Dec::new(&e.0)).is_some());
        assert_eq!(c2.wz, 0x2801);
    }

    // ---- MEMPTR (WZ): documented by "The Undocumented Z80 Documented" (Sean Young) §4 ----

    /// Runs `code`, then `BIT 0,(HL)` with HL=$1000 (bit set) and returns wz via flags 3/5.
    fn wz_after(code: &[u8], setup: impl FnOnce(&mut Z80)) -> u16 {
        let (c, _, _) = run(code, setup);
        c.wz
    }

    #[test]
    fn bit_n_hl_takes_flags_3_and_5_from_memptr_high() {
        // LD A,(0x2800) sets WZ=0x2801; BIT 0,(HL) copies bits 3/5 of WZ high ($28 = both).
        let mut ram = Ram(vec![0; 0x10000], vec![]);
        ram.0[..6].copy_from_slice(&[0x3A, 0x00, 0x28, 0xCB, 0x46, 0x76]);
        ram.0[0x1000] = 1;
        let mut c = Z80::default();
        c.set_hl(0x1000);
        while !c.halted {
            c.step(&mut ram);
        }
        assert_eq!(c.f & (FY | FX), FY | FX);
        assert_eq!(c.f & (FZ | FH), FH);
    }

    #[test]
    fn memptr_after_loads_and_stores() {
        assert_eq!(wz_after(&[0x3A, 0x00, 0x28, 0x76], |_| {}), 0x2801); // LD A,(nn)
        assert_eq!(wz_after(&[0x32, 0xFF, 0x28, 0x76], |c| c.a = 0x77), 0x7700); // LD (nn),A: (nn+1)&FF | A<<8
        assert_eq!(wz_after(&[0x0A, 0x76], |c| c.set_bc(0x1234)), 0x1235); // LD A,(BC)
        assert_eq!(wz_after(&[0x12, 0x76], |c| {
            c.set_de(0x12FF);
            c.a = 0x99;
        }), 0x9900); // LD (DE),A
        assert_eq!(wz_after(&[0x2A, 0x00, 0x30, 0x76], |_| {}), 0x3001); // LD HL,(nn)
        assert_eq!(wz_after(&[0x22, 0x00, 0x30, 0x76], |_| {}), 0x3001); // LD (nn),HL
        assert_eq!(wz_after(&[0xED, 0x4B, 0x00, 0x30, 0x76], |_| {}), 0x3001); // LD BC,(nn)
        assert_eq!(wz_after(&[0xED, 0x43, 0xFF, 0xFF, 0x76], |_| {}), 0x0000); // LD (nn),BC wraps
    }

    #[test]
    fn memptr_after_jumps_calls_and_returns() {
        assert_eq!(wz_after(&[0xC3, 0x04, 0x00, 0x76, 0x76], |_| {}), 0x0004); // JP nn
        let mut ram_prog = vec![0xC2, 0x00, 0x20, 0x76]; // JP NZ,nn with Z set: not taken, WZ=nn anyway
        ram_prog.resize(8, 0);
        assert_eq!(wz_after(&ram_prog, |c| c.f = FZ), 0x2000);
        assert_eq!(wz_after(&[0xCD, 0x04, 0x00, 0x76, 0x76], |_| {}), 0x0004); // CALL nn
        assert_eq!(wz_after(&[0xC4, 0x00, 0x20, 0x76], |c| c.f = FZ), 0x2000); // CALL NZ,nn not taken
        let mut rst = vec![0xCF, 0, 0, 0, 0, 0, 0, 0, 0x76];
        rst.resize(16, 0);
        assert_eq!(wz_after(&rst, |_| {}), 0x0008); // RST 8
    }

    #[test]
    fn memptr_after_ret_jr_and_djnz() {
        // CALL 4 ; HALT ; (4:) RET -> WZ = return address 3
        assert_eq!(wz_after(&[0xCD, 0x05, 0x00, 0x76, 0x00, 0xC9], |_| {}), 0x0003);
        // RET NZ not taken leaves WZ alone
        assert_eq!(wz_after(&[0xC0, 0x76], |c| {
            c.f = FZ;
            c.wz = 0x4321;
        }), 0x4321);
        // JR +2 taken: WZ = target
        assert_eq!(wz_after(&[0x18, 0x02, 0x00, 0x00, 0x76], |_| {}), 0x0004);
        // JR NZ not taken: unchanged
        assert_eq!(wz_after(&[0x20, 0x02, 0x76], |c| {
            c.f = FZ;
            c.wz = 0x1111;
        }), 0x1111);
        // DJNZ taken (B=2 -> 1): WZ = target; ends with B=0 not taken
        assert_eq!(wz_after(&[0x10, 0xFE, 0x76], |c| c.b = 2), 0x0000);
    }

    #[test]
    fn memptr_after_16_bit_arithmetic_and_exchange() {
        assert_eq!(wz_after(&[0x09, 0x76], |c| c.set_hl(0x4000)), 0x4001); // ADD HL,BC
        assert_eq!(wz_after(&[0xED, 0x4A, 0x76], |c| c.set_hl(0x4000)), 0x4001); // ADC HL,BC
        assert_eq!(wz_after(&[0xED, 0x42, 0x76], |c| c.set_hl(0x4000)), 0x4001); // SBC HL,BC
        assert_eq!(wz_after(&[0xDD, 0x09, 0x76], |c| c.ix = 0x5000), 0x5001); // ADD IX,BC
        // EX (SP),HL: WZ = new HL (the word from the stack)
        let mut ram = Ram(vec![0; 0x10000], vec![]);
        ram.0[0] = 0xE3;
        ram.0[1] = 0x76;
        ram.0[0x8000] = 0x34;
        ram.0[0x8001] = 0x12;
        let mut c = Z80 { sp: 0x8000, ..Default::default() };
        c.step(&mut ram);
        assert_eq!(c.wz, 0x1234);
    }

    #[test]
    fn memptr_after_indexed_access_rld_and_io() {
        assert_eq!(wz_after(&[0xDD, 0x7E, 0x05, 0x76], |c| c.ix = 0x1000), 0x1005); // LD A,(IX+5)
        assert_eq!(wz_after(&[0xFD, 0x36, 0xFF, 0x01, 0x76], |c| c.iy = 0x1000), 0x0FFF); // LD (IY-1),n
        assert_eq!(wz_after(&[0xDD, 0xCB, 0x02, 0x46, 0x76], |c| c.ix = 0x1000), 0x1002); // BIT 0,(IX+2)
        assert_eq!(wz_after(&[0xED, 0x6F, 0x76], |c| c.set_hl(0x1000)), 0x1001); // RLD
        assert_eq!(wz_after(&[0xDB, 0x10, 0x76], |c| c.a = 0x20), 0x2011); // IN A,(n)
        assert_eq!(wz_after(&[0xD3, 0xFF, 0x76], |c| c.a = 0x20), 0x2000); // OUT (n),A: (n+1)&FF | A<<8
        assert_eq!(wz_after(&[0xED, 0x78, 0x76], |c| c.set_bc(0x1234)), 0x1235); // IN A,(C)
        assert_eq!(wz_after(&[0xED, 0x79, 0x76], |c| c.set_bc(0x1234)), 0x1235); // OUT (C),A
    }

    #[test]
    fn memptr_after_block_instructions() {
        // CPI / CPD adjust WZ by +-1
        assert_eq!(wz_after(&[0xED, 0xA1, 0x76], |c| {
            c.wz = 0x1000;
            c.set_bc(5);
        }), 0x1001);
        assert_eq!(wz_after(&[0xED, 0xA9, 0x76], |c| {
            c.wz = 0x1000;
            c.set_bc(5);
        }), 0x0FFF);
        // LDIR repeating: WZ = PC+1 where PC is the instruction address (here 0 -> 1)
        let mut ram = Ram(vec![0; 0x10000], vec![]);
        ram.0[..3].copy_from_slice(&[0xED, 0xB0, 0x76]);
        let mut c = Z80::default();
        c.set_bc(2);
        c.set_hl(0x100);
        c.set_de(0x200);
        c.step(&mut ram);
        assert_eq!((c.pc, c.wz), (0, 0x0001));
        // INI: WZ = BC_before + 1 ; OUTI: WZ = BC_after + 1
        assert_eq!(wz_after(&[0xED, 0xA2, 0x76], |c| c.set_bc(0x0510)), 0x0511);
        assert_eq!(wz_after(&[0xED, 0xA3, 0x76], |c| c.set_bc(0x0510)), 0x0411);
    }
}
