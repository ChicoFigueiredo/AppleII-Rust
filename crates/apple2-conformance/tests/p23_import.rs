//! P2.3 (G0): importacao do corpus real do AppleWin-Test para issues de save-state.
//!
//! Precisa do corpus em `target/conformance-cache/applewin-test` (tools/conformance/
//! fetch-applewin-test.sh); sem ele o teste imprime BLOCKED e sai.

use apple2_conformance::state::import_text;
use std::path::PathBuf;
use std::process::Command;

fn corpus(rel: &str) -> Option<String> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(&root)
        .output()
        .ok()?;
    let common = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let path = common
        .parent()?
        .join("target/conformance-cache/applewin-test")
        .join(rel);
    std::fs::read_to_string(path).ok()
}

/// GH#609: carregar um save-state sem Phasor (Ultima-V: slot 4 vazio) substitui a configuracao
/// inteira; o Phasor que estava no slot 4 antes da carga nao sobrevive. O importador monta a
/// maquina so a partir do arquivo, entao o slot 4 sai vazio (e o trace bate com o oraculo no caso
/// `savestate-corpus-Ultima-V`).
#[test]
fn gh609_state_without_phasor_leaves_slot_4_empty() {
    let Some(text) = corpus("v2/Ultima-V.aws.yaml") else {
        println!("BLOCKED: corpus do AppleWin-Test ausente");
        return;
    };
    let imp = import_text(&text, vec![]).unwrap();
    let cfg = imp.machine.config();
    assert!(cfg.slots[4].is_none(), "slot 4: {:?}", cfg.slots[4]);
    assert!(cfg.slots[3].is_none(), "slot 3: {:?}", cfg.slots[3]);
}
