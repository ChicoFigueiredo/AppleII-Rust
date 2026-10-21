//! P2.3 (G0): cenarios de CPU/IRQ de issues do upstream executados contra o Oraculo. Cada teste
//! grava um programa na RAM do porte, salva o estado (`.aws.yaml`), carrega o mesmo arquivo no
//! oraculo (`--state`) e no porte e compara o trace linha a linha (ciclo, PC, opcode, A, X, Y, SP,
//! P): #282 (AXA (zp),Y / abs,Y), #728 (codigo executando no espaco dos soft switches) e #961
//! (HLT/KIL). Sem o oraculo compilado, imprime BLOCKED e sai (como oracle_roundtrip.rs).

use apple2_conformance::case::RunSpec;
use apple2_conformance::runner::generate_trace_from;
use apple2_conformance::state::import_text;
use apple2_core::{AuxCardKind, Machine, MachineConfig, Model};
use apple2_savestate::{save_aws_yaml, SaveOptions};
use std::path::PathBuf;
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle_available() -> bool {
    let script = root().join("tools/conformance/oracle/run-audetto-oracle.sh");
    if let Ok(bin) = std::env::var("ORACLE_BIN") {
        return PathBuf::from(bin).exists() && script.exists();
    }
    let out = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(root())
        .output();
    let Ok(out) = out else { return false };
    let common = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    common
        .parent()
        .map(|p| {
            p.join("target/conformance-cache/oracle/build/audetto-oracle")
                .exists()
        })
        .unwrap_or(false)
        && script.exists()
}

fn rows(csv: &str) -> Vec<String> {
    csv.lines()
        .filter(|l| !l.starts_with('#') && !l.starts_with("cycle,") && !l.is_empty())
        .map(str::to_string)
        .collect()
}

/// Saves `m`, loads the file in the oracle and in the Rust importer, runs `cycles` in both and
/// compares the traces row by row. Returns false (and prints BLOCKED) without an oracle.
fn compare_with_oracle(m: &Machine, extension: bool, tag: &str, cycles: u64) -> bool {
    if !oracle_available() {
        println!(
            "BLOCKED: oráculo não compilado (tools/conformance/oracle/build-audetto-oracle.sh)"
        );
        return false;
    }
    let text = save_aws_yaml(
        m,
        &SaveOptions {
            extension,
            ..Default::default()
        },
    )
    .unwrap()
    .text;
    compare_text(&text, tag, cycles)
}

fn compare_text(text: &str, tag: &str, cycles: u64) -> bool {
    compare_text_in(text, tag, cycles, vec![])
}

fn compare_text_in(text: &str, tag: &str, cycles: u64, image_dirs: Vec<PathBuf>) -> bool {
    let dir = std::env::temp_dir().join(format!("oracle-rt-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let state = dir.join("rust.aws.yaml");
    std::fs::write(&state, text).unwrap();

    let out = dir.join("oracle.csv");
    let st = Command::new(root().join("tools/conformance/oracle/run-audetto-oracle.sh"))
        .args(["--state"])
        .arg(&state)
        .args(["--cycles", &cycles.to_string(), "--out"])
        .arg(&out)
        .status()
        .unwrap();
    assert!(st.success(), "o oráculo recusou o YAML escrito pelo Rust");
    let oracle = rows(&std::fs::read_to_string(&out).unwrap());

    let imp = import_text(text, image_dirs).unwrap();
    assert!(
        imp.missing.is_empty(),
        "imagens ausentes: {:?}",
        imp.missing
    );
    let start = imp.machine.cycles();
    let run = RunSpec {
        frames: None,
        cycles: Some(start + cycles),
        repeat: 1,
    };
    let rust = generate_trace_from(imp.machine, &run, &[], &[]).unwrap();
    let rust = rows(&String::from_utf8(rust).unwrap());
    assert!(
        oracle.len() > 500,
        "trace do oráculo curto: {}",
        oracle.len()
    );
    assert_eq!(oracle.len(), rust.len(), "número de instruções");
    for (i, (o, r)) in oracle.iter().zip(&rust).enumerate() {
        if o != r {
            let lo = i.saturating_sub(2);
            let hi = (i + 6).min(oracle.len().min(rust.len()));
            let ctx = (lo..hi)
                .map(|k| format!("  {k}: oracle {} | rust {}", oracle[k], rust[k]))
                .collect::<Vec<_>>()
                .join("\n");
            panic!("linha {i} diverge:\n{ctx}");
        }
    }
    std::fs::remove_dir_all(dir).unwrap();
    true
}

fn load(m: &mut Machine, at: u16, code: &[u8]) {
    for (i, b) in code.iter().enumerate() {
        m.poke_main(at + i as u16, *b);
    }
}

fn machine(model: Model) -> Machine {
    Machine::new(MachineConfig::for_model(model))
}

/// Saves `m` (no extension) and compares `cycles` of trace with the oracle.
fn check(m: &Machine, tag: &str, cycles: u64) {
    compare_with_oracle(m, false, tag, cycles);
}

/// GH#282: timing of the undocumented AXA (zp),Y ($93) with and without page crossing, and the
/// abs,Y form ($9F). The issue's program: `LDA $C404 / AXA ($FE),Y / LDX $C404 / BRK`.
#[test]
fn gh282_axa_zp_y_and_abs_y_timing_matches_the_oracle() {
    for model in [Model::Apple2, Model::Apple2Plus, Model::Apple2e] {
        for y in [0u8, 1] {
            let mut m = machine(model);
            load(
                &mut m,
                0x300,
                &[
                    0xA0, 0xFF, 0x84, 0xFE, 0xA0, 0x20, 0x84, 0xFF, 0xA0, y, 0xAD, 0x04, 0xC4,
                    0x93, 0xFE, 0xAE, 0x04, 0xC4, 0x00,
                ],
            );
            m.cpu_mut().pc = 0x300;
            check(&m, &format!("282-zpy-{model:?}-{y}"), 3_000);
            let mut m = machine(model);
            load(
                &mut m,
                0x300,
                &[
                    0xA2, 0x7F, 0xA9, 0x3C, 0xA0, y, 0xAD, 0x04, 0xC4, 0x9F, 0xFF, 0x20, 0xAE,
                    0x04, 0xC4, 0x00,
                ],
            );
            m.cpu_mut().pc = 0x300;
            check(&m, &format!("282-absy-{model:?}-{y}"), 3_000);
        }
    }
}

/// GH#728 (1): code running inside the soft-switch space. `$800..$BFF` is filled with `E0 00`
/// (CPX #imm, as the issue's `802<801.BFEM`), the program spins on the floating bus until it reads
/// `$E0` and then does `JSR $C050`.
#[test]
fn gh728_code_running_in_the_soft_switch_space_matches_the_oracle() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model);
        for a in 0x800u16..0xC00 {
            m.poke_main(a, if a & 1 == 0 { 0xE0 } else { 0x00 });
        }
        load(
            &mut m,
            0x80B,
            &[0xA2, 0x33, 0xA0, 0x44, 0xE8, 0xC8, 0x60, 0x60],
        );
        load(
            &mut m,
            0x300,
            &[
                0xA2, 0x11, 0xA0, 0x22, 0xAD, 0x55, 0xC0, 0xAD, 0x55, 0xC0, 0xC9, 0xE0, 0xD0, 0xF9,
                0x20, 0x50, 0xC0, 0x2C, 0x54, 0xC0, 0x00,
            ],
        );
        m.cpu_mut().pc = 0x300;
        check(&m, &format!("728a-{model:?}"), 20_000);
    }
}

/// GH#728 (2): key `$60` latched, `JSR $C000` executes CPX #imm at `$C000` and RTS at `$C010`.
#[test]
fn gh728_jsr_into_the_keyboard_latch_matches_the_oracle() {
    let mut m = machine(Model::Apple2eEnhanced);
    load(
        &mut m,
        0x300,
        &[
            0xAD, 0x10, 0xC0, 0xAD, 0x00, 0xC0, 0x10, 0xFB, 0x20, 0x00, 0xC0, 0x00,
        ],
    );
    assert!(m.key_press(b'`'));
    m.cpu_mut().pc = 0x303; // after the strobe clear, so the pending key is read
    check(&m, "728b", 20_000);
}

/// GH#961: an NMOS HLT (`$02`) jams the CPU (AppleWin `regs.bJammed`); the trace after it must
/// be the same as the oracle's.
#[test]
fn gh961_hlt_matches_the_oracle() {
    for model in [Model::Apple2Plus, Model::Apple2eEnhanced] {
        let mut m = machine(model);
        load(&mut m, 0x300, &[0xA9, 0x11, 0x02, 0xA9, 0x22, 0xEA]);
        m.cpu_mut().pc = 0x300;
        check(&m, &format!("961-{model:?}"), 3_000);
    }
}

/// GH#532 (fix: "Support floating-bus in full-speed mode", #508/#519/#532): a wait on the
/// floating bus (`LDA $C050 / BNE`) over non-zero video memory ends at the same cycle as in the
/// oracle on the //e family, where Oo-Topos' slot-3 probing depends on it.
#[test]
fn gh532_floating_bus_wait_matches_the_oracle_on_the_iie() {
    for model in [Model::Apple2e, Model::Apple2eEnhanced] {
        let mut m = machine(model);
        for a in 0x400u16..0x800 {
            m.poke_main(a, 0xA0);
        }
        for a in 0x2000u16..0x4000 {
            m.poke_main(a, 0xA0);
        }
        load(
            &mut m,
            0x6000,
            &[0xAD, 0x50, 0xC0, 0xD0, 0xFB, 0xA2, 0x77, 0x4C, 0x07, 0x60],
        );
        m.cpu_mut().pc = 0x6000;
        check(&m, &format!("532-{model:?}"), 60_000);
    }
}

/// GH#404: the NMOS 6502 page-crossing indexed load does a false read at the uncorrected address
/// (`LDA $C0FF,X` with X=$82 reads `$C081` first), which the Language Card counts: two of them
/// write-enable the card. Compared with the oracle on the 6502 models and the 65C02.
#[test]
fn gh404_6502_false_read_enables_the_language_card_like_the_oracle() {
    for model in [Model::Apple2Plus, Model::Apple2e, Model::Apple2eEnhanced] {
        let mut m = machine(model);
        load(
            &mut m,
            0x300,
            &[
                0xA2, 0x82, // LDX #$82
                0xBD, 0xFF, 0xC0, // LDA $C0FF,X  (false read $C081, real read $C181)
                0xBD, 0xFF, 0xC0, // LDA $C0FF,X
                0xA9, 0x5A, // LDA #$5A
                0x8D, 0x00, 0xD0, // STA $D000
                0xAD, 0x80, 0xC0, // LDA $C080 (RAM read, bank 2)
                0xAD, 0x00, 0xD0, // LDA $D000
                0x4C, 0x13, 0x03, // JMP *
            ],
        );
        m.cpu_mut().pc = 0x300;
        check(&m, &format!("404-{model:?}"), 4_000);
    }
}

/// GH#478: a RamWorks with 256 banks (16 MB, `kMaxExMemoryBanks`) is written as `Num Aux Banks:
/// 0x100`; the oracle loads it and runs the same trace after the program selects the last bank
/// ($FF), writes there with RAMWRT and goes back to bank 0. (No reads from a bank other than 0:
/// the oracle build aborts with a heap corruption, "double free or corruption", when RAMRD reads
/// a RamWorks bank other than 0; the read-back is covered by `issue_478_1349_ramworks_with_256_
/// banks_is_addressable` in apple2-core.)
#[test]
fn gh478_ramworks_with_256_banks_matches_the_oracle() {
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.aux_card = AuxCardKind::RamWorksIII { banks: 256 };
    let mut m = Machine::new(cfg);
    load(
        &mut m,
        0x300,
        &[
            0xA9, 0xFF, 0x8D, 0x73, 0xC0, // LDA #$FF / STA $C073
            0x8D, 0x05, 0xC0, // RAMWRT on
            0xA9, 0xA7, 0x8D, 0x00, 0x20, // LDA #$A7 / STA $2000 (bank $FF)
            0xA9, 0x00, 0x8D, 0x73, 0xC0, // bank 0
            0x8D, 0x04, 0xC0, // RAMWRT off
            0x4C, 0x15, 0x03,
        ],
    );
    m.cpu_mut().pc = 0x300;
    let text = save_aws_yaml(&m, &SaveOptions::default()).unwrap().text;
    assert!(text.contains("Num Aux Banks: 0x100"), "campo Num Aux Banks");
    let back = import_text(&text, vec![]).unwrap().machine;
    assert!(matches!(
        back.config().aux_card,
        AuxCardKind::RamWorksIII { banks: 256 }
    ));
    compare_with_oracle(&m, false, "478", 5_000);
}
