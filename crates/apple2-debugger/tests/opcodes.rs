//! The disassembler tables against an independent source: the instruction lengths and memory
//! effects of the `apple2-cpu` core that actually executes the code.

use apple2_cpu::{Bus, Cpu, Variant};
use apple2_debugger::opcodes::{table, Mode, OPCODES_65C02};

struct Flat {
    mem: Vec<u8>,
    reads: Vec<u16>,
    writes: Vec<u16>,
}

impl Bus for Flat {
    fn read(&mut self, a: u16) -> u8 {
        self.reads.push(a);
        self.mem[a as usize]
    }

    fn write(&mut self, a: u16, v: u8) {
        self.writes.push(a);
        self.mem[a as usize] = v;
    }
}

/// Opcodes whose PC after the instruction is not simply "next instruction".
fn changes_flow(op: u8, cmos: bool) -> bool {
    let info = table(cmos)[op as usize];
    info.mode == Mode::Rel
        || matches!(op, 0x00 | 0x20 | 0x40 | 0x4C | 0x60 | 0x6C | 0x7C)
        // NMOS KIL/halt opcodes jam the CPU
        || (!cmos && info.mnemonic == "hlt")
}

#[test]
fn instruction_lengths_match_what_the_cpu_consumes() {
    for cmos in [false, true] {
        for op in 0..=255u8 {
            if changes_flow(op, cmos) {
                continue;
            }
            let mut bus = Flat { mem: vec![0; 0x10000], reads: vec![], writes: vec![] };
            bus.mem[0x1000] = op;
            bus.mem[0x1001] = 0x12;
            bus.mem[0x1002] = 0x34;
            let mut cpu = Cpu::new(if cmos { Variant::Cmos65C02 } else { Variant::Nmos6502 });
            cpu.pc = 0x1000;
            cpu.step(&mut bus);
            let len = table(cmos)[op as usize].mode.bytes() as u16;
            assert_eq!(cpu.pc, 0x1000 + len, "{} opcode {op:02X} ({}): CPU consumed {} bytes", if cmos { "65C02" } else { "6502" }, table(cmos)[op as usize].mnemonic, cpu.pc - 0x1000);
        }
    }
}

#[test]
fn memory_access_flags_match_the_cpu_for_documented_opcodes() {
    // For absolute-mode data opcodes ($xxxx) the table's R/W flags say whether the CPU touches the
    // operand address as a read and/or a write.
    use apple2_debugger::opcodes::{MEM_R, MEM_W};
    for (op, info) in OPCODES_65C02.iter().enumerate() {
        if info.mode != Mode::Abs || info.is_extension() || matches!(op, 0x20 | 0x4C) {
            continue;
        }
        let mut bus = Flat { mem: vec![0; 0x10000], reads: vec![], writes: vec![] };
        bus.mem[0x1000] = op as u8;
        bus.mem[0x1001] = 0x00;
        bus.mem[0x1002] = 0x20; // $2000
        let mut cpu = Cpu::new(Variant::Cmos65C02);
        cpu.pc = 0x1000;
        cpu.step(&mut bus);
        let read = bus.reads.iter().skip(3).any(|a| *a == 0x2000);
        let wrote = bus.writes.contains(&0x2000);
        assert_eq!(wrote, info.mem & MEM_W != 0, "{} ${op:02X} write", info.mnemonic);
        // AppleWin lists several read-modify-write opcodes (TSB, TRB, LSR abs, ...) as write-only in the
        // 65C02 table; so a table flag must be backed by the CPU, not the other way round.
        if info.mem & MEM_R != 0 {
            assert!(read, "{} ${op:02X} reads its operand", info.mnemonic);
        } else if !wrote {
            assert!(!read, "{} ${op:02X} does not read its operand", info.mnemonic);
        }
    }
}
