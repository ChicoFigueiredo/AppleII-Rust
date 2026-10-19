//! Klaus Dormann's 6502/65C02 functional tests.
use apple2_cpu::{Bus, Cpu, Variant};

struct Ram(Box<[u8; 65536]>);

impl Bus for Ram {
    fn read(&mut self, a: u16) -> u8 { self.0[a as usize] }
    fn write(&mut self, a: u16, v: u8) { self.0[a as usize] = v }
}

fn run(path: &str, variant: Variant, success: u16) {
    let data = std::fs::read(format!("{}/tests/data/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut ram = Ram(Box::new([0; 65536]));
    ram.0.copy_from_slice(&data);
    let mut cpu = Cpu::new(variant);
    cpu.pc = 0x0400;
    loop {
        let before = cpu.pc;
        cpu.step(&mut ram);
        if cpu.pc == before {
            assert_eq!(cpu.pc, success, "trapped at {:04X} (test case {:02X})", cpu.pc, ram.0[0x0200]);
            return;
        }
        assert!(cpu.cycles < 200_000_000, "did not finish; pc={:04X}", cpu.pc);
    }
}

#[test]
fn functional_nmos() { run("6502_functional_test.bin", Variant::Nmos6502, 0x3469); }

#[test]
fn functional_cmos() { run("6502_functional_test.bin", Variant::Cmos65C02, 0x3469); }

#[test]
#[ignore = "binary uses Rockwell/WDC bit opcodes (BBR/BBS/RMB/SMB), which AppleWin's 65C02 does not implement"]
fn extended_cmos() { run("65C02_extended_opcodes_test.bin", Variant::Cmos65C02, 0x24F1); }
