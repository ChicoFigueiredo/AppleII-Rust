//! Klaus Dormann's 6502/65C02 functional tests.
use apple2_cpu::{Bus, Cpu, Variant};

struct Ram(Box<[u8; 65536]>);

impl Bus for Ram {
    fn read(&mut self, a: u16) -> u8 { self.0[a as usize] }
    fn write(&mut self, a: u16, v: u8) { self.0[a as usize] = v }
}

fn run(path: &str, variant: Variant, success: u16) {
    run_with(path, variant, success, false)
}

/// With `rockwell_shim` the harness itself executes the Rockwell bit opcodes
/// (RMBn $n7, SMBn $n7+$80, BBRn $nF, BBSn $nF+$80), which AppleWin's 65C02 lacks
/// (its table has them as 1-cycle NOPs): everything else of the test then runs on the real core.
fn run_with(path: &str, variant: Variant, success: u16, rockwell_shim: bool) {
    let data = std::fs::read(format!("{}/tests/data/{path}", env!("CARGO_MANIFEST_DIR"))).unwrap();
    let mut ram = Ram(Box::new([0; 65536]));
    ram.0.copy_from_slice(&data);
    let mut cpu = Cpu::new(variant);
    cpu.pc = 0x0400;
    loop {
        let before = cpu.pc;
        let op = ram.0[before as usize];
        if rockwell_shim && matches!(op & 0x0F, 0x07 | 0x0F) {
            let zp = ram.0[before.wrapping_add(1) as usize] as usize;
            let bit = 1u8 << ((op >> 4) & 7);
            if op & 0x0F == 0x07 {
                if op & 0x80 == 0 { ram.0[zp] &= !bit } else { ram.0[zp] |= bit }
                cpu.pc = before.wrapping_add(2);
            } else {
                let rel = ram.0[before.wrapping_add(2) as usize] as i8;
                let set = ram.0[zp] & bit != 0;
                let next = before.wrapping_add(3);
                cpu.pc = if set == (op & 0x80 != 0) { next.wrapping_add(rel as u16) } else { next };
            }
            continue;
        }
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

/// The stock binary is built with `rkwl_wdc_op = 1`: it traps at $0733 (`BBR0 $0C`, case 00)
/// because AppleWin's 65C02 table (cpu65C02.h) has no Rockwell opcodes ($07/$0F/... are
/// `NOP CYC(1)`; TestCPU6502 GH#292 asserts it). Klaus Dormann ships no build without them, so
/// the shim test below runs the rest of the suite on the real core.
#[test]
#[ignore = "binary uses Rockwell bit opcodes (BBR/BBS/RMB/SMB), which AppleWin's 65C02 does not implement"]
fn extended_cmos() { run("65C02_extended_opcodes_test.bin", Variant::Cmos65C02, 0x24F1); }

#[test]
fn extended_cmos_with_rockwell_ops_emulated_by_the_harness() {
    run_with("65C02_extended_opcodes_test.bin", Variant::Cmos65C02, 0x24F1, true);
}
