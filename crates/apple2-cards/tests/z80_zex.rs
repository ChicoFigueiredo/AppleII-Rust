//! ZEXDOC / ZEXALL (Frank Cringle's Z80 instruction exercisers) on the Z80 core, through a
//! minimal CP/M BDOS shim (functions 2 and 9). The binaries are fetched (not versioned) by
//! `tools/conformance/fetch-z80-tests.sh`; without them the test reports BLOCKED and returns.
//! Run: `cargo test -p apple2-cards --release --test z80_zex -- --ignored --nocapture`.
//! Writes `tests/conformance/reports/z80-zex.md`.

use apple2_cards::z80::{Z80Bus, Z80};
use std::path::PathBuf;

struct Ram(Vec<u8>);

impl Z80Bus for Ram {
    fn read(&mut self, a: u16) -> u8 {
        self.0[a as usize]
    }
    fn write(&mut self, a: u16, v: u8) {
        self.0[a as usize] = v;
    }
}

/// Runs a `.cim` image to the CP/M warm boot; returns the console output and the T-states.
fn run_cpm(image: &[u8]) -> (String, u64) {
    let mut ram = Ram(vec![0; 0x10000]);
    ram.0[0x100..0x100 + image.len()].copy_from_slice(image);
    ram.0[0] = 0x76; // warm boot: HALT
    ram.0[5] = 0xC9; // BDOS entry: RET (intercepted below)
    ram.0[6] = 0x00;
    ram.0[7] = 0xF0; // top of TPA
    let mut z = Z80 {
        pc: 0x100,
        sp: 0xEFFF,
        ..Default::default()
    };
    let mut out = String::new();
    let mut tstates = 0u64;
    loop {
        if z.pc == 5 {
            match z.c {
                2 => out.push(z.e as char),
                9 => {
                    let mut a = z.de();
                    while ram.0[a as usize] != b'$' {
                        out.push(ram.0[a as usize] as char);
                        a = a.wrapping_add(1);
                    }
                }
                _ => {}
            }
        }
        if z.pc == 0 {
            break;
        }
        tstates += u64::from(z.step(&mut ram));
    }
    (out, tstates)
}

#[test]
#[ignore = "needs tools/conformance/fetch-z80-tests.sh; takes minutes; writes tests/conformance/reports/z80-zex.md"]
fn zexdoc_and_zexall() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = std::env::var_os("Z80_TESTS_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/conformance-cache/z80-tests"))
        .join("zex");
    let mut md = String::from("# Z80: ZEXDOC / ZEXALL\n\n");
    let mut failed = false;
    for name in ["zexdoc.cim", "zexall.cim"] {
        let Ok(image) = std::fs::read(dir.join(name)) else {
            md += &format!(
                "## {name}\n\nBLOCKED: binário ausente em `{}`.\n\n",
                dir.display()
            );
            continue;
        };
        let (out, t) = run_cpm(&image);
        let errors = out.matches("ERROR").count();
        let ok = out.matches("OK").count();
        let tests = out.lines().filter(|l| l.contains("....")).count();
        failed |= errors > 0 || tests == 0;
        md += &format!(
            "## {name}\n\n- testes: {tests}, OK: {ok}, ERROR: {errors}\n- T-states: {t}\n\n```\n{out}\n```\n\n"
        );
    }
    std::fs::write(root.join("tests/conformance/reports/z80-zex.md"), &md).unwrap();
    println!("{md}");
    assert!(!failed, "ZEXDOC/ZEXALL com erros ou bloqueado");
}
