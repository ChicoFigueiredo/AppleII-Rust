//! a2audit (github.com/zellyn/a2audit, MIT) no port: o `audit.dsk` e um programa de auditoria de
//! hardware do //e (cartao de linguagem, chaves de memoria, ROM de slot) e serve de oraculo
//! forte para a HU-0102 (upstream #392, #164, #423). Baixe com
//! `tools/conformance/fetch-a2audit.sh` (SHA fixo, cache fora do Git); sem o cache o teste
//! imprime BLOCKED e passa.

use apple2_core::text_screen::text_screen;
use apple2_core::{Machine, MachineConfig, Model};
use apple2_storage::disk2::Disk2Card;
use apple2_storage::image::DiskImage;
use std::path::PathBuf;

fn cache() -> PathBuf {
    std::env::var_os("A2AUDIT_CACHE").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/conformance-cache/a2audit"),
        PathBuf::from,
    )
}

fn screen(m: &Machine) -> String {
    text_screen(m)
        .iter()
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Runs until the screen shows `done` (or `max` frames); returns (frames, screen).
fn run_until_done(model: Model, max: u32) -> Option<(u32, String)> {
    let bytes = std::fs::read(cache().join("audit/audit.dsk")).ok()?;
    let mut m = Machine::new(MachineConfig::for_model(model));
    let mut card = Disk2Card::new();
    card.insert_disk(0, DiskImage::from_bytes("audit.dsk", bytes).unwrap());
    assert!(m.insert_card(6, Box::new(card)));
    m.power_cycle();
    for f in 0..max {
        m.run_frame();
        let s = screen(&m);
        if s.contains("SOFTSWITCH TESTS SUCCEEDED") || s.contains("ERROR") || s.contains("FAIL") {
            // let the line settle, then stop
            return Some((f, s));
        }
    }
    Some((max, screen(&m)))
}

fn check(model: Model) {
    let Some((frames, s)) = run_until_done(model, 3000) else {
        eprintln!("BLOCKED: a2audit ausente; rode tools/conformance/fetch-a2audit.sh");
        return;
    };
    eprintln!("{model:?}: a2audit terminou em {frames} quadros\n{s}");
    assert!(!s.contains("ERROR") && !s.contains("FAIL"), "{s}");
    assert_eq!(s.matches("LANGUAGE CARD TESTS SUCCEEDED").count(), 2, "{s}");
    assert!(s.contains("AUXMEM TESTS SUCCEEDED"), "{s}");
    assert!(s.contains("SOFTSWITCH TESTS SUCCEEDED"), "{s}");
}

/// #392 (LC e LC aux), #164 (chaves de ROM de slot sem cartoes nos slots 4/5), #423.
#[test]
fn a2audit_passes_on_apple2e() {
    check(Model::Apple2e);
}

#[test]
fn a2audit_passes_on_apple2e_enhanced() {
    check(Model::Apple2eEnhanced);
}
