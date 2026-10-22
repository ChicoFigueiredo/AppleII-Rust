//! HU-0401/0402: um `.aws.yaml` escrito pelo Rust (com ou sem o bloco `AppleII-Rust Extension`)
//! carrega no oráculo `--state` e o trace do oráculo bate com o do Rust. Prova que a extensão não
//! quebra a leitura pelo AppleWin e cobre, contra o oráculo executável, as regressões #1262
//! (página de pilha), #255 (Cycles This Frame), #591 (latch do teclado) e #413 (RamWorks 0x7F).
//!
//! Precisa do oráculo compilado (`tools/conformance/oracle/build-audetto-oracle.sh`; `ORACLE_BIN`
//! aponta um binário próprio). Sem ele o teste imprime BLOCKED e sai.

use apple2_conformance::case::RunSpec;
use apple2_conformance::runner::generate_trace_from;
use apple2_conformance::state::import_text;
use apple2_core::{AuxCardKind, Machine, MachineConfig, Model};
use apple2_cpu::Bus;
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
        assert_eq!(o, r, "linha {i}");
    }
    std::fs::remove_dir_all(dir).unwrap();
    true
}

fn roundtrip_boot(model: Model, extension: bool, tag: &str) {
    let mut m = Machine::new(MachineConfig::for_model(model));
    m.run_cycles(300_000);
    compare_with_oracle(&m, extension, tag, 50_000);
}

#[test]
fn rust_written_state_with_extension_loads_in_the_oracle_enhanced_iie() {
    roundtrip_boot(Model::Apple2eEnhanced, true, "iie-ext");
}

#[test]
fn rust_written_state_without_extension_loads_in_the_oracle_iiplus() {
    roundtrip_boot(Model::Apple2Plus, false, "iip");
}

#[test]
fn rust_written_state_with_extension_loads_in_the_oracle_iiplus() {
    roundtrip_boot(Model::Apple2Plus, true, "iip-ext");
}

/// Program at `$0300`: reads the whole stack page, the keyboard latch and a floating-bus switch,
/// pulls two bytes off the stack, and loops.
fn install_probe(m: &mut Machine) {
    let code: [u8; 24] = [
        0xA2, 0x00, // LDX #0
        0xBD, 0x00, 0x01, // L: LDA $0100,X
        0xAD, 0x00, 0xC0, // LDA $C000
        0xAD, 0x54, 0xC0, // LDA $C054
        0xE8, // INX
        0xD0, 0xF4, // BNE L
        0x68, 0x68, // PLA PLA
        0xAD, 0x10, 0xC0, // LDA $C010
        0x4C, 0x00, 0x03, // JMP $0300
        0xEA, 0xEA,
    ];
    for (i, b) in code.iter().enumerate() {
        m.poke_main(0x300 + i as u16, *b);
    }
    m.cpu_mut().pc = 0x300;
}

/// GH#1262: A2osX keeps its stack in the aux page 1 (ALTZP) while main page 1 holds other
/// data; both must survive.
#[test]
fn gh1262_stack_page_with_altzp_matches_the_oracle() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    for i in 0..256u16 {
        m.poke_main(0x100 + i, (i as u8).wrapping_mul(7).wrapping_add(3));
        m.poke_aux(0x100 + i, (i as u8).wrapping_mul(13).wrapping_add(0x55));
    }
    m.board_mut().write(0xC009, 0); // ALTZP on: stack and zero page in aux
    install_probe(&mut m);
    m.cpu_mut().sp = 0x40;
    for ext in [false, true] {
        compare_with_oracle(&m, ext, &format!("1262-{ext}"), 30_000);
    }
}

/// GH#255: Cycles This Frame saved mid-frame comes back and drives the video scanner (floating
/// bus) the same way.
#[test]
fn gh255_cycles_this_frame_mid_frame_matches_the_oracle() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    for a in 0..0xC000u32 {
        m.poke_main(a as u16, ((a.wrapping_mul(0x9E37)) >> 8) as u8 | 0x01);
    }
    install_probe(&mut m);
    // Run the probe so that the frame counter is in the middle of a frame.
    m.run_cycles(8_123);
    assert!(m.cycles_in_frame() > 0);
    for ext in [false, true] {
        compare_with_oracle(&m, ext, &format!("255-{ext}"), 30_000);
    }
}

/// GH#591: the keyboard latch (key and strobe) read after the load is the saved one.
#[test]
fn gh591_keyboard_latch_matches_the_oracle() {
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    install_probe(&mut m);
    assert!(m.key_press(b'Q'));
    assert_eq!(m.board_mut().read(0xC000) & 0x80, 0x80);
    for ext in [false, true] {
        compare_with_oracle(&m, ext, &format!("591-{ext}"), 20_000);
    }
}

/// GH#413: a RamWorks with 0x7F banks is written as `0x07F` and read back as 127 banks; the
/// oracle accepts the file and the banks hold the same bytes.
#[test]
fn gh413_ramworks_with_0x7f_banks_matches_the_oracle() {
    let mut cfg = MachineConfig::for_model(Model::Apple2eEnhanced);
    cfg.aux_card = AuxCardKind::RamWorksIII { banks: 0x7F };
    let mut m = Machine::new(cfg);
    install_probe(&mut m);
    let text = save_aws_yaml(&m, &SaveOptions::default()).unwrap().text;
    assert!(text.contains("Num Aux Banks: 0x07F"), "campo Num Aux Banks");
    let back = import_text(&text, vec![]).unwrap().machine;
    assert!(matches!(
        back.config().aux_card,
        AuxCardKind::RamWorksIII { banks: 0x7F }
    ));
    compare_with_oracle(&m, false, "413", 20_000);
}

/// GH#838: a file with a UTF-8 BOM loads in the oracle and in the port with the same trace; a
/// file with bytes that are not UTF-8 is refused by the oracle (libyaml) and by the port.
#[test]
fn gh838_bom_and_invalid_utf8_agree_with_the_oracle() {
    if !oracle_available() {
        println!("BLOCKED: oráculo não compilado");
        return;
    }
    let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
    install_probe(&mut m);
    m.run_cycles(5_000);
    let text = save_aws_yaml(
        &m,
        &SaveOptions {
            extension: false,
            ..Default::default()
        },
    )
    .unwrap()
    .text;
    if !compare_text(&format!("\u{FEFF}{text}"), "838-bom", 20_000) {
        return;
    }
    // Invalid UTF-8 in a comment line: libyaml reports "invalid trailing UTF-8 octet".
    let mut bytes = text.into_bytes();
    let at = bytes.iter().position(|&b| b == b'#').unwrap();
    bytes.insert(at + 2, 0xC3);
    let dir = std::env::temp_dir().join(format!("oracle-838-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let state = dir.join("bad.aws.yaml");
    std::fs::write(&state, &bytes).unwrap();
    let out = Command::new(root().join("tools/conformance/oracle/run-audetto-oracle.sh"))
        .args(["--state"])
        .arg(&state)
        .args(["--cycles", "1000", "--out"])
        .arg(dir.join("o.csv"))
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("MessageBox") && stderr.contains("Load State"),
        "o oráculo deveria recusar o arquivo: {stderr}"
    );
    assert!(
        String::from_utf8(bytes).is_err(),
        "o porte lê o arquivo como UTF-8 e o recusa"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

/// GH#1499 / GH#1510 / AUD-C-020: a disk image whose real path has spaces, `#`, Unicode and a
/// literal backslash is found by the oracle through the `Absolute Path` the port wrote (the
/// state file lives elsewhere, so only that path can resolve). The booting machine then gives
/// the same trace in the oracle and in the port. The oracle ignores a missing image silently, so
/// equal traces (the DOS boot reads the disk) are the proof that the text decoded identically.
#[test]
fn gh1499_image_paths_written_by_the_port_decode_in_the_oracle() {
    use apple2_storage::disk2::Disk2Card;
    use apple2_storage::image::DiskImage;
    if !oracle_available() {
        println!("BLOCKED: oráculo não compilado");
        return;
    }
    let bytes =
        std::fs::read(root().join("assets/disks/DOS 3.3 System Master - 680-0210-A.dsk")).unwrap();
    for (i, sub) in ["Zoë Müller #1", "back\\slash dir", "dois  espaços 日本"]
        .iter()
        .enumerate()
    {
        let imgdir = std::env::temp_dir()
            .join(format!("oracle-1499-img-{i}-{}", std::process::id()))
            .join(sub);
        std::fs::create_dir_all(&imgdir).unwrap();
        let img = imgdir.join("día #1.dsk");
        std::fs::write(&img, &bytes).unwrap();
        let mut m = Machine::new(MachineConfig::for_model(Model::Apple2eEnhanced));
        let mut card = Disk2Card::new();
        let image =
            DiskImage::open_path(&img, apple2_storage::image::Class::Floppy, false).unwrap();
        card.insert_disk(0, image);
        assert!(m.insert_card(6, Box::new(card)));
        m.power_cycle();
        m.run_cycles(150_000);
        let text = save_aws_yaml(
            &m,
            &SaveOptions {
                extension: false,
                ..Default::default()
            },
        )
        .unwrap()
        .text;
        assert!(text.contains("Absolute Path"), "sem Absolute Path");
        compare_text_in(&text, &format!("1499-{i}"), 40_000, vec![imgdir.clone()]);
        std::fs::remove_dir_all(imgdir.parent().unwrap()).unwrap();
    }
}
