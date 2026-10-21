//! What the debugger needs from the machine: side-effect-free memory access and the registers.

use apple2_core::Machine;

/// CPU registers as the debugger shows them (`regs`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Regs {
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub sp: u8,
    pub pc: u16,
    pub p: u8,
}

/// `ReadByteFromMemory` / `WriteByteToMemory`: no soft-switch side effects.
pub trait DebugMem {
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, value: u8);
    fn read_word(&self, addr: u16) -> u16 {
        self.read(addr) as u16 | (self.read(addr.wrapping_add(1)) as u16) << 8
    }
}

impl DebugMem for Machine {
    fn read(&self, addr: u16) -> u8 {
        self.peek(addr)
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.poke(addr, value)
    }
}

/// Plain 64K, for tests and offline disassembly.
pub struct Ram(pub Vec<u8>);

impl Default for Ram {
    fn default() -> Self {
        Ram(vec![0; 0x10000])
    }
}

impl Ram {
    pub fn load(&mut self, at: u16, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.0[at as usize + i] = *b;
        }
    }
}

impl DebugMem for Ram {
    fn read(&self, addr: u16) -> u8 {
        self.0[addr as usize]
    }

    fn write(&mut self, addr: u16, value: u8) {
        self.0[addr as usize] = value;
    }
}

pub fn regs_of(m: &Machine) -> Regs {
    let c = m.cpu();
    Regs {
        a: c.a,
        x: c.x,
        y: c.y,
        sp: c.sp,
        pc: c.pc,
        p: c.p,
    }
}
